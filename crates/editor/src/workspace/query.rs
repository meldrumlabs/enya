//! Query execution coordination for the workspace.
//!
//! This module handles polling for query results, executing queries for panes
//! that need refresh, and coordinating with the query executor and diagnostics.

use super::{Workspace, WorkspaceAction};
use crate::components::{Diagnostic, DiagnosticSource, ExecuteParams, QueryPane, QueryPollResult};

impl Workspace {
    /// Process query execution: poll for pending results and execute queries for panes that need refresh
    /// Returns a notification action if a connection status changed.
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
            // Convert MetricLabels to FxHashMap<String, FxHashSet<String>> for the finder
            if let Some(labels) = self.query_executor.get_metric_labels(&metric_name) {
                let tags: rustc_hash::FxHashMap<String, rustc_hash::FxHashSet<String>> = labels
                    .labels
                    .iter()
                    .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
                    .collect();
                self.metrics_finder.update_metric_tags(&metric_name, tags);

                // Also update buffer editor completions if editing this metric
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

        // 0c. If metrics finder is open and connected, fetch labels for selected metric
        if self.metrics_finder.is_open() && self.query_executor.is_connected() {
            if let Some(metric_name) = self.metrics_finder.selected_metric_name() {
                // Only fetch if not already cached and not currently fetching this metric
                if !self.query_executor.has_metric_labels(metric_name)
                    && self.query_executor.fetching_metric() != Some(metric_name)
                {
                    self.query_executor.fetch_metric_labels(metric_name, ctx);
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

        // 1. Poll for query results if there's a pending query
        // We use the pane's component ID (not TileId) because TileIds can change when
        // egui_tiles restructures the tree during ui() calls
        if let Some(pending_pane_id) = self.pending_query_pane_id {
            let tile_count = self.viewport_tree.tiles.len();
            log::debug!(
                "Polling pending query for pane ID {pending_pane_id}. Total tiles: {tile_count}"
            );

            // Find the pane by its component ID
            let mut pane_found = false;
            for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
                if let egui_tiles::Tile::Pane(component) = tile {
                    if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                        if query_pane.id() == pending_pane_id {
                            pane_found = true;
                            let pane_id = query_pane.id();
                            let pane_name = query_pane.name().to_string();

                            match self.query_executor.poll(query_pane.visualization_mut()) {
                                QueryPollResult::Complete {
                                    series_count,
                                    point_count,
                                    suggested_viz,
                                } => {
                                    // Query completed
                                    self.pending_query_pane_id = None;
                                    query_pane.set_loading(false);
                                    // Clear any previous errors for this pane
                                    self.diagnostics_pane.clear_for_pane(pane_id);

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
                                        log::info!(
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
                                    self.pending_query_pane_id = None;
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
                                    // Still waiting for results
                                }
                            }
                            break;
                        }
                    }
                }
            }

            // If we couldn't find the pane (it was removed), clean up
            if !pane_found {
                log::warn!(
                    "Pending query pane ID {pending_pane_id} no longer exists, clearing pending state"
                );
                self.pending_query_pane_id = None;
                self.query_executor.cancel_query();
                // Clear loading state on all panes to prevent stuck loading animations
                for (_id, tile) in self.viewport_tree.tiles.iter_mut() {
                    if let egui_tiles::Tile::Pane(component) = tile {
                        if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>()
                        {
                            if query_pane.is_loading() {
                                log::debug!("Clearing orphaned loading state on pane");
                                query_pane.set_loading(false);
                            }
                        }
                    }
                }
            }
        }

        // 2. If no query in flight, check for panes that need refresh and execute
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
                        }
                    }
                }
            }
        }

        let can_execute = !self.query_executor.is_connected() || self.query_executor.is_online();

        if self.pending_query_pane_id.is_none() && can_execute {
            let (start_ns, end_ns) = self.time_range_toolbar.get_range_ns();

            // Find the first pane that needs refresh and get its component ID
            let mut pane_to_execute: Option<(usize, String, String, u64)> = None;
            for (_tile_id, tile) in self.viewport_tree.tiles.iter() {
                if let egui_tiles::Tile::Pane(component) = tile {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        if query_pane.needs_refresh() {
                            log::debug!(
                                "Found pane {} that needs refresh: {}",
                                query_pane.id(),
                                query_pane.name()
                            );
                            pane_to_execute = Some((
                                query_pane.id(),
                                query_pane.name().to_string(),
                                query_pane.saved_query().to_string(),
                                query_pane.query_state().granularity.seconds(),
                            ));
                            break;
                        }
                    }
                }
            }

            // Execute the query for the pane we found
            if let Some((pane_id, metric, query, step_secs)) = pane_to_execute {
                let tile_count = self.viewport_tree.tiles.len();
                // Find the pane again to modify it (needed because we can't hold a reference across the iter)
                for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
                    if let egui_tiles::Tile::Pane(component) = tile {
                        if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>()
                        {
                            if query_pane.id() == pane_id {
                                // Clear the refresh flag
                                query_pane.clear_refresh();

                                // Execute the query
                                let params = ExecuteParams {
                                    metric: &metric,
                                    query: &query,
                                    step_secs,
                                    start_ns: Some(start_ns),
                                    end_ns: Some(end_ns),
                                };
                                self.query_executor.execute(
                                    &params,
                                    query_pane.visualization_mut(),
                                    ctx,
                                );
                                self.pending_query_pane_id = Some(pane_id);
                                query_pane.set_loading(true);

                                log::debug!(
                                    "Executing query for pane {pane_id}: {query}. Total tiles: {tile_count}"
                                );
                                break;
                            }
                        }
                    }
                }
            }
        }

        notification_action
    }
}
