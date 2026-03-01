//! Query execution coordination for the workspace.
//!
//! This module handles polling for query results, executing queries for panes
//! that need refresh, and coordinating with the query executor and diagnostics.
//!
//! Queries are executed in parallel (Grafana-style) - all panes that need refresh
//! are queried simultaneously, and results are processed as they arrive.

use super::{Workspace, WorkspaceAction};
use crate::components::util::query_executor::{populate_from_response, response_to_series};
use crate::components::{Diagnostic, DiagnosticSource, ExecuteParams, QueryPane, QueryPollResult};

impl Workspace {
    /// Process query execution: poll for pending results and execute queries for panes that need refresh
    /// Returns a notification action if a connection status changed.
    ///
    /// This uses parallel query execution - all panes needing refresh are queried simultaneously,
    /// similar to how Grafana refreshes all panels at once.
    #[profiling::function]
    pub(super) fn process_query_execution(&mut self, ctx: &egui::Context) -> WorkspaceAction {
        // 0. Poll for health check completion
        let mut notification_action = WorkspaceAction::None;
        if let Some(success) = self.query_executor.poll_health_check() {
            if success {
                if let crate::components::util::query_executor::ConnectionHealth::Online {
                    ref version,
                } = self.query_executor.connection_health().clone()
                {
                    log::info!("Connected to Prometheus v{version}");
                    // Add success diagnostic (no toast - status bar shows connection state)
                    let diagnostic = crate::components::overlay::diagnostics::Diagnostic::info(
                        format!("Connected to Prometheus v{version}"),
                    )
                    .with_source(
                        crate::components::overlay::diagnostics::DiagnosticSource::DataConnection,
                    );
                    self.diagnostics_pane.add(diagnostic);
                }
            } else if let crate::components::util::query_executor::ConnectionHealth::Failed {
                ref error,
            } = self.query_executor.connection_health().clone()
            {
                log::error!("Connection failed: {error}");
                // Add error diagnostic
                let diagnostic = crate::components::overlay::diagnostics::Diagnostic::error(
                    format!("Connection failed: {error}"),
                )
                .with_source(
                    crate::components::overlay::diagnostics::DiagnosticSource::DataConnection,
                );
                self.diagnostics_pane.add(diagnostic);
                // Show error notification
                notification_action = WorkspaceAction::Notify {
                    level: "error".to_string(),
                    message: format!("Connection failed: {error}"),
                };
            }
        }

        // 0a. Poll for metric names and label names fetch completion
        if self.query_executor.poll_metric_names() {
            // Update buffer editor if it's open
            if self.buffer_editor.is_open() {
                let metric_names = self.query_executor.metric_names().to_vec();
                log::debug!(
                    "Updating buffer editor with {} newly fetched metric names",
                    metric_names.len()
                );
                self.buffer_editor.set_metric_names(metric_names);
            }
        }
        self.query_executor.poll_label_names();

        // 0b. Poll for per-metric labels and update the finder/buffer editor if labels were received
        if let Some(metric_name) = self.query_executor.poll_metric_labels() {
            // Update buffer editor completions if editing this metric
            if let Some(labels) = self.query_executor.get_metric_labels(&metric_name) {
                if self.buffer_editor.editing_metric_name() == Some(metric_name.as_str()) {
                    self.buffer_editor
                        .set_completions_from_labels(&labels.labels);
                    log::debug!(
                        "Updated buffer editor completions from {} labels for '{}'",
                        labels.labels.len(),
                        metric_name
                    );
                }
            }
        }

        // 0d. If buffer editor is open and connected, fetch labels for the metric being edited
        if self.buffer_editor.is_open() && self.query_executor.is_connected() {
            if let Some(metric_name) = self.buffer_editor.editing_metric_name() {
                // Only fetch if not already cached and not currently fetching this metric
                if !self.query_executor.has_metric_labels(metric_name)
                    && self.query_executor.fetching_metric() != Some(metric_name)
                {
                    self.query_executor.fetch_metric_labels(metric_name, ctx);
                }
            }
        }

        // 1. Poll for ALL completed query results (parallel execution)
        let completed_results = self.query_executor.poll_all();
        for (pane_id, poll_result) in completed_results {
            // Check if this is a pending inline chart query
            if let Some(idx) = self
                .pending_inline_charts
                .iter()
                .position(|p| p.query_id == pane_id)
            {
                let pending = self.pending_inline_charts.remove(idx);
                match poll_result {
                    QueryPollResult::Complete { response, .. } => {
                        use crate::components::pane::inline_content::InlineContent;
                        let series = response_to_series(&response);
                        let chart = crate::components::pane::inline_content::InlineChart {
                            title: pending.title,
                            series,
                            height: pending.height,
                        };
                        self.inject_inline_content_to_agent_pane(InlineContent::Chart(chart));
                        log::debug!("Inline chart completed with real data");
                    }
                    QueryPollResult::Error(error) => {
                        log::warn!("Inline chart query failed: {error}");
                    }
                    QueryPollResult::Pending => {}
                }
                continue;
            }

            // Find the pane by its component ID
            let mut pane_found = false;
            for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
                if let egui_tiles::Tile::Pane(component) = tile {
                    if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                        if query_pane.id() == pane_id {
                            pane_found = true;
                            let pane_name = query_pane.name().to_string();

                            match poll_result {
                                QueryPollResult::Complete {
                                    series_count,
                                    point_count,
                                    suggested_viz,
                                    response,
                                } => {
                                    // Query completed - update visualization
                                    query_pane.set_loading(false);
                                    // Clear any previous errors for this pane
                                    self.diagnostics_pane.clear_for_pane(pane_id);

                                    // Populate visualization from response
                                    let viz = query_pane.visualization_mut();
                                    viz.clear();
                                    viz.set_metric_name(&response.metric);
                                    populate_from_response(viz, &response);

                                    // Apply suggested visualization if user hasn't manually overridden
                                    if !query_pane.has_user_override() {
                                        query_pane.set_visualization_type_auto(suggested_viz);
                                        log::debug!(
                                            "Auto-selected visualization {suggested_viz:?} for pane '{pane_name}'"
                                        );
                                    }

                                    if series_count == 0 || point_count == 0 {
                                        // Query succeeded but returned no data - add info diagnostic
                                        let diagnostic = Diagnostic::info(
                                            "Query returned no data. Check the metric name and time range.",
                                        )
                                        .with_source(DiagnosticSource::DataConnection)
                                        .with_pane(pane_id, &pane_name);
                                        self.diagnostics_pane.add(diagnostic);
                                        log::debug!(
                                            "Query for pane {pane_id} returned no data (0 series, 0 points)"
                                        );
                                    } else {
                                        log::debug!(
                                            "Query completed for pane {pane_id}: {series_count} series, {point_count} points"
                                        );
                                    }
                                }
                                QueryPollResult::Error(error) => {
                                    // Query failed - add diagnostic
                                    query_pane.set_loading(false);
                                    // Clear previous diagnostics for this pane and add the new error
                                    self.diagnostics_pane.clear_for_pane(pane_id);
                                    let diagnostic = Diagnostic::error(&error)
                                        .with_source(DiagnosticSource::DataConnection)
                                        .with_pane(pane_id, &pane_name);
                                    self.diagnostics_pane.add(diagnostic);
                                    log::error!("Query failed for pane {pane_id}: {error}");
                                }
                                QueryPollResult::Pending => {
                                    // This shouldn't happen in poll_all results
                                }
                            }
                            break;
                        }
                    }
                }
            }

            // If we couldn't find the pane (it was removed), cancel its query
            if !pane_found {
                log::warn!(
                    "Completed query for pane ID {pane_id} but pane no longer exists, ignoring result"
                );
                self.query_executor.cancel_query(pane_id);
            }
        }

        // 2. Execute queries for ALL panes that need refresh (parallel execution)
        // Only execute queries if:
        // - We're in demo mode (always works), OR
        // - We're connected to Prometheus AND the connection is online
        //
        // If the connection failed, clear the refresh flags on all panes to prevent
        // them from staying in a "needs refresh" state indefinitely.
        if self.query_executor.is_connection_failed() {
            for (_id, tile) in self.viewport_tree.tiles.iter_mut() {
                if let egui_tiles::Tile::Pane(component) = tile {
                    if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                        if query_pane.needs_refresh() {
                            query_pane.clear_refresh();
                            query_pane.set_loading(false);
                        }
                    }
                }
            }
        }

        let can_execute = !self.query_executor.is_connected() || self.query_executor.is_online();

        if can_execute {
            let (start_ns, end_ns) = self.time_range_toolbar.get_range_ns();

            // Collect all panes that need refresh
            let mut panes_to_execute: Vec<(usize, String, String, u64)> = Vec::new();
            for (_tile_id, tile) in self.viewport_tree.tiles.iter() {
                if let egui_tiles::Tile::Pane(component) = tile {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        if query_pane.needs_refresh()
                            && !self.query_executor.is_querying_pane(query_pane.id())
                        {
                            panes_to_execute.push((
                                query_pane.id(),
                                query_pane.name().to_string(),
                                query_pane.saved_query().to_string(),
                                query_pane.query_state().granularity.seconds(),
                            ));
                        }
                    }
                }
            }

            // Execute queries for ALL panes that need refresh (in parallel)
            if !panes_to_execute.is_empty() {
                log::debug!("Executing {} queries in parallel", panes_to_execute.len());

                for (pane_id, metric, query, step_secs) in panes_to_execute {
                    // Find the pane again to modify it
                    for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
                        if let egui_tiles::Tile::Pane(component) = tile {
                            if let Some(query_pane) =
                                component.as_any_mut().downcast_mut::<QueryPane>()
                            {
                                if query_pane.id() == pane_id {
                                    // Clear the refresh flag and set loading
                                    query_pane.clear_refresh();
                                    query_pane.set_loading(true);

                                    // Execute the query
                                    let params = ExecuteParams {
                                        metric: &metric,
                                        query: &query,
                                        step_secs,
                                        start_ns: Some(start_ns),
                                        end_ns: Some(end_ns),
                                    };
                                    self.query_executor.execute_for_pane(pane_id, &params, ctx);

                                    log::debug!("Fired query for pane {pane_id}: {query}");
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Poll tracing panes that need refresh (trace loading)
        if self.tracing_client.is_some() {
            use crate::components::TracingPane;

            // Check for tracing panes that need a trace loaded
            let mut trace_to_fetch: Option<String> = None;
            for tile_id in self.get_pane_tile_ids() {
                if let Some(egui_tiles::Tile::Pane(component)) =
                    self.viewport_tree.tiles.get_mut(tile_id)
                {
                    if let Some(tracing_pane) = component.as_any_mut().downcast_mut::<TracingPane>()
                    {
                        if let Some(trace_id) = tracing_pane.trace_id_to_load() {
                            trace_to_fetch = Some(trace_id.to_string());
                            tracing_pane.set_loading(true);
                            tracing_pane.clear_refresh();
                            break;
                        }
                    }
                }
            }

            // Issue the fetch if needed
            if let (Some(trace_id), Some(client)) = (trace_to_fetch, &self.tracing_client) {
                self.trace_manager
                    .fetch_trace(client.as_ref(), &trace_id, ctx);
            }

            // Deliver completed trace results
            if let Some(result) = self.trace_manager.poll_trace() {
                for tile_id in self.get_pane_tile_ids() {
                    if let Some(egui_tiles::Tile::Pane(component)) =
                        self.viewport_tree.tiles.get_mut(tile_id)
                    {
                        if let Some(tracing_pane) =
                            component.as_any_mut().downcast_mut::<TracingPane>()
                        {
                            if tracing_pane.is_loading() {
                                match &result {
                                    Ok(trace) => tracing_pane.set_trace(trace.clone()),
                                    Err(e) => tracing_pane.set_error(e.to_string()),
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        notification_action
    }

    /// Refresh all query panes (triggered by time range change or manual refresh).
    ///
    /// This marks all query panes as needing refresh, which will cause them to
    /// be re-queried in parallel on the next frame.
    pub fn refresh_all_panes(&mut self) {
        let mut count = 0;
        for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
            if let egui_tiles::Tile::Pane(component) = tile {
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    query_pane.mark_needs_refresh();
                    count += 1;
                }
            }
        }
        if count > 0 {
            log::debug!("Marked {count} panes for refresh");
        }
    }

    // =========================================================================
    // Auto-refresh
    // =========================================================================

    /// Set the auto-refresh interval
    pub fn set_refresh_interval(&mut self, interval: super::RefreshInterval) {
        if interval == super::RefreshInterval::Off {
            self.refresh_interval = None;
            self.last_refresh = None;
            log::debug!("Auto-refresh disabled");
        } else {
            self.refresh_interval = Some(interval);
            // Reset the timer when interval changes
            self.last_refresh = Some(crate::util::Instant::now());
            log::debug!("Auto-refresh set to {}", interval.label());
        }
    }

    /// Get the current refresh interval
    pub fn refresh_interval(&self) -> Option<super::RefreshInterval> {
        self.refresh_interval
    }

    /// Check if auto-refresh is due and trigger it if so.
    /// Returns true if a refresh was triggered.
    pub(super) fn check_auto_refresh(&mut self) -> bool {
        let Some(interval) = self.refresh_interval else {
            return false;
        };

        let Some(interval_secs) = interval.to_secs() else {
            return false;
        };

        let now = crate::util::Instant::now();

        // Check if enough time has passed
        let should_refresh = match self.last_refresh {
            Some(last) => now.duration_since(last).as_secs() >= interval_secs,
            None => true, // First refresh
        };

        if should_refresh {
            log::debug!("Auto-refresh triggered (interval: {})", interval.label());
            self.last_refresh = Some(now);
            self.refresh_all_panes();
            true
        } else {
            false
        }
    }

    /// Get the time remaining until the next auto-refresh in seconds.
    /// Returns None if auto-refresh is disabled.
    pub fn time_until_refresh(&self) -> Option<u64> {
        let interval = self.refresh_interval?;
        let interval_secs = interval.to_secs()?;
        let last = self.last_refresh?;

        let elapsed = crate::util::Instant::now().duration_since(last).as_secs();
        Some(interval_secs.saturating_sub(elapsed))
    }
}
