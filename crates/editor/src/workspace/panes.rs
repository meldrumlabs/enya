//! Pane management methods for the workspace.
//!
//! This module handles adding, removing, splitting, and navigating panes
//! in the tile tree. It includes methods for managing the viewport layout
//! and tracking open charts.

use egui_tiles::{Tile, TileId};

/// Maximum recursion depth for tree traversal operations.
/// Prevents stack overflow on pathological tree structures.
const MAX_TREE_DEPTH: usize = 100;

use super::{AgentCommand, NavDirection, Workspace, WorkspaceAction};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::InlineSource;
#[cfg(not(target_arch = "wasm32"))]
use crate::components::pane::inline_content::{
    InlineDiff, InlineDiffFile, InlineDiffLine, InlineDiffLineKind, InlineSearchResults,
    SearchResultItem,
};
use crate::components::pane::logs_pane::LogsBackend;
use crate::components::pane::query_pane::QueryPaneAction;
use crate::components::pane::time_series_chart::{DataPoint, Series};
use crate::components::util::query_executor::ExecuteParams;
use crate::components::util::{ActivityItem, ActivityType};
use crate::components::{Buffer, Component, InlineChart, InlineContent, LogsPane, QueryPane};

/// Metadata for a pending inline chart query.
pub(super) struct PendingInlineChart {
    /// Query ID used with QueryExecutor.
    pub(super) query_id: usize,
    /// Chart title.
    pub(super) title: String,
    /// Optional height override.
    pub(super) height: Option<f32>,
}

/// Parse a unified diff string into `InlineDiffFile` structs for inline display.
#[cfg(not(target_arch = "wasm32"))]
fn parse_diff_to_inline_files(diff: &str) -> Vec<InlineDiffFile> {
    let mut files: Vec<InlineDiffFile> = Vec::new();
    let mut current_file: Option<InlineDiffFile> = None;
    let mut old_line_num: usize = 0;
    let mut new_line_num: usize = 0;

    for line in diff.lines() {
        // New file header: diff --git a/path b/path
        if line.starts_with("diff --git") {
            // Save previous file
            if let Some(file) = current_file.take() {
                files.push(file);
            }

            // Extract path from "diff --git a/path b/path"
            let path = line
                .strip_prefix("diff --git a/")
                .and_then(|s| s.split(" b/").next())
                .unwrap_or("")
                .to_string();

            current_file = Some(InlineDiffFile {
                path,
                lines: Vec::new(),
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        // Skip other metadata lines
        if line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
        {
            continue;
        }

        // Parse hunk header for line numbers
        if line.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header_for_inline(line) {
                old_line_num = old_start;
                new_line_num = new_start;
            }

            if let Some(ref mut file) = current_file {
                file.lines.push(InlineDiffLine {
                    content: line.to_string(),
                    kind: InlineDiffLineKind::Hunk,
                    old_line: None,
                    new_line: None,
                });
            }
            continue;
        }

        // Process diff content lines
        if let Some(ref mut file) = current_file {
            if let Some(added) = line.strip_prefix('+') {
                file.lines.push(InlineDiffLine {
                    content: added.to_string(),
                    kind: InlineDiffLineKind::Addition,
                    old_line: None,
                    new_line: Some(new_line_num),
                });
                new_line_num += 1;
                file.additions += 1;
            } else if let Some(removed) = line.strip_prefix('-') {
                file.lines.push(InlineDiffLine {
                    content: removed.to_string(),
                    kind: InlineDiffLineKind::Deletion,
                    old_line: Some(old_line_num),
                    new_line: None,
                });
                old_line_num += 1;
                file.deletions += 1;
            } else if let Some(context) = line.strip_prefix(' ') {
                file.lines.push(InlineDiffLine {
                    content: context.to_string(),
                    kind: InlineDiffLineKind::Context,
                    old_line: Some(old_line_num),
                    new_line: Some(new_line_num),
                });
                old_line_num += 1;
                new_line_num += 1;
            }
        }
    }

    // Don't forget the last file
    if let Some(file) = current_file {
        files.push(file);
    }

    files
}

/// Parse a hunk header to extract starting line numbers.
/// Format: @@ -old_start,old_count +new_start,new_count @@ optional_context
#[cfg(not(target_arch = "wasm32"))]
fn parse_hunk_header_for_inline(line: &str) -> Option<(usize, usize)> {
    let content = line.strip_prefix("@@")?.trim_start();
    let content = content.split("@@").next()?.trim();

    let mut parts = content.split_whitespace();

    // Parse old: -start,count or -start
    let old_part = parts.next()?.strip_prefix('-')?;
    let old_start: usize = old_part.split(',').next()?.parse().ok()?;

    // Parse new: +start,count or +start
    let new_part = parts.next()?.strip_prefix('+')?;
    let new_start: usize = new_part.split(',').next()?.parse().ok()?;

    Some((old_start, new_start))
}

impl Workspace {
    /// Base ID for inline chart queries, well above normal pane IDs.
    const INLINE_CHART_ID_BASE: usize = usize::MAX - 10_000;

    // ==================== Pane Adding ====================

    /// Add a chart for a metric and return a tracking action
    pub(super) fn add_chart_for_metric_with_tracking(
        &mut self,
        metric_name: &str,
    ) -> WorkspaceAction {
        // Don't add duplicate charts
        if self.open_charts.contains(metric_name) {
            log::debug!("Chart for {metric_name} already open");
            return WorkspaceAction::None;
        }

        // Create a QueryPane (buffer + chart) for the metric
        // Use real query pane when connected to a backend, demo pane otherwise
        let query_number = self.next_query_number;
        self.next_query_number += 1;
        let pane: Box<dyn Component> = if self.query_executor.is_connected() {
            Box::new(QueryPane::for_metric_with_number(metric_name, query_number))
        } else {
            Box::new(QueryPane::with_demo_metric_numbered(
                metric_name,
                query_number,
            ))
        };
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(metric_name.to_string());
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::debug!("Added query pane for {metric_name}");

            // Return action to track this in recent queries
            // Use "Query N" as the display name, metric_name for lookup
            return WorkspaceAction::TrackRecentPlot {
                name: format!("Query {query_number}"),
                metric_name: metric_name.to_string(),
                is_query: false,
            };
        }

        WorkspaceAction::None
    }

    /// Add a query pane with a PromQL query and optional title.
    ///
    /// This is used by the agent and plugins to create panes programmatically.
    pub fn add_query_pane(&mut self, query: &str, title: Option<&str>) {
        let query_number = self.next_query_number;
        self.next_query_number += 1;

        // Create the pane with the given query
        let name = title.unwrap_or(query);
        let pane: Box<dyn Component> = if self.query_executor.is_connected() {
            Box::new(QueryPane::with_query_named(query, name, query_number))
        } else {
            Box::new(QueryPane::with_demo_query_named(query, name, query_number))
        };
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(query.to_string());
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::info!("Agent created query pane: {}", title.unwrap_or(query));
        }
    }

    /// Add a floating (detached) query pane with a PromQL query.
    ///
    /// This is used by the agent to create floating panes for investigation.
    pub fn add_floating_query_pane(
        &mut self,
        query: &str,
        title: Option<&str>,
        position: Option<[f32; 2]>,
    ) {
        let query_number = self.next_query_number;
        self.next_query_number += 1;

        let name = title.unwrap_or(query);
        let pane: Box<dyn Component> = if self.query_executor.is_connected() {
            Box::new(QueryPane::with_query_named(query, name, query_number))
        } else {
            Box::new(QueryPane::with_demo_query_named(query, name, query_number))
        };

        let offset = (self.floating_panes.count() as f32) * 30.0;
        let pos = if let Some([x, y]) = position {
            egui::pos2(x, y)
        } else {
            egui::pos2(100.0 + offset, 100.0 + offset)
        };

        self.floating_panes.add_pane(pane, pos);
        log::info!("Agent created floating pane: {}", title.unwrap_or(query));
    }

    /// Add a terminal pane to the viewport.
    ///
    /// Creates a new terminal pane backed by ghostty-vt for running shell commands.
    /// Requires the "terminal" feature to be enabled.
    #[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
    pub fn add_terminal_pane(&mut self) -> Option<TileId> {
        use crate::components::TerminalPane;
        use crate::ui::theme::AppTheme;

        // Use the default theme - it will be updated via set_theme() later
        match TerminalPane::new(AppTheme::default()) {
            Ok(terminal_pane) => {
                let pane: Box<dyn Component> = Box::new(terminal_pane);
                let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

                if self.add_tile_to_viewport(pane_tile) {
                    self.behavior.set_focused_tile(Some(pane_tile));
                    self.show_landing = false;
                    log::info!("Added terminal pane");
                    Some(pane_tile)
                } else {
                    None
                }
            }
            Err(e) => {
                log::error!("Failed to create terminal pane: {e}");
                None
            }
        }
    }

    /// Add a terminal pane (stub - terminal feature not enabled).
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "terminal")))]
    pub fn add_terminal_pane(&mut self) -> Option<TileId> {
        log::warn!("Terminal panes require the 'terminal' feature (needs zig toolchain)");
        None
    }

    /// Add a terminal pane (WASM stub - terminals not supported in browser).
    #[cfg(target_arch = "wasm32")]
    pub fn add_terminal_pane(&mut self) -> Option<TileId> {
        log::warn!("Terminal panes are not available in the browser");
        None
    }

    /// Add a tracing pane to the viewport.
    ///
    /// Creates a new tracing pane for visualizing distributed traces.
    /// Optionally pre-fills a trace ID to load.
    pub fn add_tracing_pane(&mut self, trace_id: Option<&str>) -> Option<TileId> {
        use crate::components::TracingPane;

        let tracing_pane = if let Some(id) = trace_id {
            TracingPane::with_trace_id(id)
        } else {
            TracingPane::new()
        };

        let pane: Box<dyn Component> = Box::new(tracing_pane);
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::info!("Added tracing pane");
            Some(pane_tile)
        } else {
            None
        }
    }

    /// Add a SQL pane to the viewport.
    ///
    /// Creates a new SQL pane for running DataFusion queries on local files.
    /// Requires the `sql` feature to be enabled.
    #[cfg(all(not(target_arch = "wasm32"), feature = "sql"))]
    pub fn add_sql_pane(&mut self) -> Option<TileId> {
        use crate::components::SqlPane;
        use crate::ui::theme::AppTheme;

        let runtime_handle = self.query_executor.runtime_handle();
        let mut sql_pane = SqlPane::new(AppTheme::default(), runtime_handle);
        log::info!(
            "add_sql_pane: cached {} connections, syncing to new pane",
            self.cached_flight_sql_connections.len()
        );
        sql_pane.sync_connections(&self.cached_flight_sql_connections);
        let pane: Box<dyn Component> = Box::new(sql_pane);
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::info!("Added SQL pane");
            Some(pane_tile)
        } else {
            None
        }
    }

    /// Add a SQL pane (stub version for WASM or when sql feature is disabled).
    #[cfg(any(target_arch = "wasm32", not(feature = "sql")))]
    pub fn add_sql_pane(&mut self) -> Option<TileId> {
        use crate::components::SqlPane;
        use crate::ui::theme::AppTheme;

        let mut sql_pane = SqlPane::new(AppTheme::default());
        sql_pane.sync_connections(&self.cached_flight_sql_connections);
        let pane: Box<dyn Component> = Box::new(sql_pane);
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::info!("Added SQL pane stub (sql feature disabled)");
            Some(pane_tile)
        } else {
            None
        }
    }

    /// Enable or disable keyboard input for all terminal panes.
    ///
    /// Call this when modals open/close to prevent terminals from capturing
    /// keyboard input meant for overlays like the style picker.
    #[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
    pub(super) fn set_terminal_keyboard_enabled(&mut self, enabled: bool) {
        use crate::components::TerminalPane;

        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(terminal) = component.as_any_mut().downcast_mut::<TerminalPane>() {
                    terminal.set_keyboard_enabled(enabled);
                }
            }
        }
    }

    /// Enable or disable keyboard input for terminal panes (no-op without terminal feature).
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "terminal")))]
    pub(super) fn set_terminal_keyboard_enabled(&mut self, _enabled: bool) {
        // No-op: terminal feature not enabled
    }

    /// Add a logs pane to the viewport with the demo backend.
    ///
    /// Creates a new logs pane that displays log entries for metric→log correlation.
    ///
    /// # Arguments
    ///
    /// * `start_ns` - Start of time range in nanoseconds since Unix epoch
    /// * `end_ns` - End of time range in nanoseconds since Unix epoch
    pub(super) fn add_logs_pane(&mut self, start_ns: i64, end_ns: i64) -> Option<TileId> {
        self.add_logs_pane_with_backend(start_ns, end_ns, LogsBackend::Demo)
    }

    /// Add a logs pane connected to a Loki server.
    ///
    /// # Arguments
    ///
    /// * `start_ns` - Start of time range in nanoseconds since Unix epoch
    /// * `end_ns` - End of time range in nanoseconds since Unix epoch
    /// * `loki_url` - The Loki server URL (e.g., "http://localhost:3100")
    pub(super) fn add_loki_pane(
        &mut self,
        start_ns: i64,
        end_ns: i64,
        loki_url: impl Into<String>,
    ) -> Option<TileId> {
        self.add_logs_pane_with_backend(start_ns, end_ns, LogsBackend::Loki(loki_url.into()))
    }

    /// Add a logs pane with a specific backend.
    fn add_logs_pane_with_backend(
        &mut self,
        start_ns: i64,
        end_ns: i64,
        backend: LogsBackend,
    ) -> Option<TileId> {
        let backend_name = match &backend {
            LogsBackend::Demo => "demo".to_string(),
            LogsBackend::Loki(url) => format!("loki@{url}"),
        };

        let pane: Box<dyn Component> = Box::new(LogsPane::with_backend(start_ns, end_ns, backend));
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::info!("Added logs pane with backend: {backend_name}");
            Some(pane_tile)
        } else {
            None
        }
    }

    /// Handle commands from the AI agent.
    ///
    /// These commands are parsed from the agent's response and executed
    /// to manipulate the workspace (create panes, change time range, etc.)
    /// Handle agent commands and return activity items for UI feedback.
    ///
    /// Each command execution generates an `ActivityItem` showing what action
    /// was taken and whether it succeeded. These can be displayed in the agent
    /// panel to provide visual feedback during command execution.
    ///
    /// When commands are executed, the caller should typically exit agent mode.
    pub(super) fn handle_agent_commands(
        &mut self,
        commands: Vec<AgentCommand>,
        ctx: &egui::Context,
    ) -> Vec<ActivityItem> {
        let mut activities = Vec::new();

        for command in commands {
            // Get the description before executing (for activity display)
            let description = command.description();
            let mut success = false;

            match command {
                AgentCommand::CreatePane {
                    query,
                    title,
                    floating,
                    position,
                } => {
                    if floating.unwrap_or(false) {
                        self.add_floating_query_pane(&query, title.as_deref(), position);
                    } else {
                        self.add_query_pane(&query, title.as_deref());
                    }
                    success = true;
                }
                AgentCommand::SetTimeRange { preset } => {
                    // Parse preset string into a TimeRangePreset
                    if let Some(preset_enum) = Self::parse_time_preset(&preset) {
                        self.time_range_toolbar.set_preset(preset_enum);
                        // Trigger global refresh of all panes (Grafana-style)
                        self.refresh_all_panes();
                        log::info!("Agent set time range to: {preset}, refreshing all panes");
                        success = true;
                    } else {
                        log::warn!("Agent requested unknown time preset: {preset}");
                    }
                }
                AgentCommand::SearchMetrics { pattern } => {
                    // Open the unified finder in metrics mode with the pattern
                    self.unified_finder
                        .open_with_mode(crate::components::overlay::FinderMode::Metrics);
                    self.unified_finder.set_query(&pattern);
                    log::info!("Agent opened metrics search: {pattern}");
                    success = true;
                }
                AgentCommand::ShowMetricSource { metric } => {
                    // Open the source preview for the metric definition
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.open_metric_definition(&metric);
                        log::info!("Agent opened metric source: {metric}");
                        success = true;
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        log::warn!("ShowMetricSource not available: {metric}");
                    }
                }
                AgentCommand::ShowAlertSource { alert } => {
                    // Open the source preview for the alert rule
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.open_alert_definition(&alert);
                        log::info!("Agent opened alert source: {alert}");
                        success = true;
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        log::warn!("ShowAlertSource not available: {alert}");
                    }
                }
                AgentCommand::ShowInlineChart {
                    query,
                    title,
                    time_range: _,
                    height,
                } => {
                    let chart_title = title.unwrap_or_else(|| query.clone());

                    if self.query_executor.is_online() {
                        // Fire real query and track as pending inline chart
                        self.request_inline_chart(&query, &chart_title, height, ctx);
                        log::info!("Requested inline chart for query: {query}");
                    } else {
                        // Demo/offline mode: use generated data
                        let chart = self.generate_demo_inline_chart(&query, &chart_title, height);
                        self.inject_inline_content_to_agent_pane(InlineContent::Chart(chart));
                        log::info!("Injected demo inline chart for query: {query}");
                    }
                    success = true;
                }
                AgentCommand::ShowInlineSource {
                    metric,
                    context_lines,
                } => {
                    // Look up metric source and generate inline source preview
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let lines = context_lines.unwrap_or(5);
                        if let Some(source) = self.generate_inline_source(&metric, lines) {
                            self.inject_inline_content_to_agent_pane(InlineContent::Source(source));
                            log::info!("Injected inline source for metric: {metric}");
                            success = true;
                        } else {
                            log::warn!("Could not find source for metric: {metric}");
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = (metric, context_lines); // Silence unused warnings
                        log::warn!("ShowInlineSource not available without codebase feature");
                    }
                }
                AgentCommand::SearchCodebase {
                    query,
                    filter,
                    limit,
                } => {
                    // Search the Tantivy index and return results
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let filter_str = filter.as_deref().unwrap_or("all");
                        let results = self.search_codebase(&query, Some(filter_str), limit);

                        // Log results with details
                        if results.is_empty() {
                            log::info!(
                                "Agent searched codebase for '{query}' (filter: {filter_str}): no results"
                            );
                        } else {
                            let count = results.len();
                            log::info!(
                                "Agent searched codebase for '{query}' (filter: {filter_str}): {count} results"
                            );
                            for (i, r) in results.iter().take(5).enumerate() {
                                let idx = i + 1;
                                let kind = &r.kind;
                                let name = &r.name;
                                let score = r.score;
                                log::info!("  [{idx}] {kind:?}: {name} (score: {score:.2})");
                            }
                            if count > 5 {
                                let remaining = count - 5;
                                log::info!("  ... and {remaining} more");
                            }
                        }

                        // Convert to inline search results and inject into agent pane
                        let inline_results =
                            self.convert_to_inline_search_results(&query, filter_str, results);
                        self.inject_inline_content_to_agent_pane(InlineContent::SearchResults(
                            inline_results,
                        ));
                        success = true;
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = (query, filter, limit);
                        log::warn!("SearchCodebase not available in WASM");
                    }
                }
                AgentCommand::AddLogsPane {
                    query,
                    loki_url,
                    title: _,
                } => {
                    // Use current time range for logs (in nanoseconds)
                    let (start_ns, end_ns) = self.time_range_toolbar.get_range_ns();
                    let start_ns = start_ns as i64;
                    let end_ns = end_ns as i64;

                    // Create logs pane with appropriate backend
                    let tile_id = if let Some(url) = loki_url {
                        self.add_loki_pane(start_ns, end_ns, url)
                    } else {
                        self.add_logs_pane(start_ns, end_ns)
                    };

                    // Set query if provided
                    if let (Some(tile_id), Some(query)) = (tile_id, query) {
                        if let Some(egui_tiles::Tile::Pane(component)) =
                            self.viewport_tree.tiles.get_mut(tile_id)
                        {
                            if let Some(logs_pane) =
                                component.as_any_mut().downcast_mut::<LogsPane>()
                            {
                                logs_pane.set_query(&query);
                            }
                        }
                    }

                    log::info!("Agent created logs pane");
                    success = true;
                }
                AgentCommand::AddTracingPane { trace_id, title: _ } => {
                    self.add_tracing_pane(trace_id.as_deref());
                    log::info!(
                        "Agent created tracing pane{}",
                        trace_id
                            .as_ref()
                            .map(|id| format!(" with trace_id: {id}"))
                            .unwrap_or_default()
                    );
                    success = true;
                }
                AgentCommand::AddTerminalPane { title: _ } => {
                    if self.add_terminal_pane().is_some() {
                        log::info!("Agent created terminal pane");
                        success = true;
                    } else {
                        log::warn!(
                            "Agent failed to create terminal pane (not available on this platform)"
                        );
                    }
                }
                AgentCommand::SetVisualization { pane, viz_type } => {
                    // Parse the visualization type
                    if let Some(viz) = Self::parse_visualization_type(&viz_type) {
                        // Find the target pane (default to focused if not specified)
                        let target_tile = pane
                            .as_deref()
                            .map(|p| self.resolve_pane_target(p))
                            .unwrap_or_else(|| self.behavior.focused_tile());

                        if let Some(tile_id) = target_tile {
                            if let Some(egui_tiles::Tile::Pane(component)) =
                                self.viewport_tree.tiles.get_mut(tile_id)
                            {
                                if let Some(query_pane) =
                                    component.as_any_mut().downcast_mut::<QueryPane>()
                                {
                                    query_pane.set_visualization_type(viz);
                                    log::info!(
                                        "Agent set visualization to {viz_type} for pane: {}",
                                        pane.as_deref().unwrap_or("focused")
                                    );
                                    success = true;
                                }
                            }
                        } else {
                            log::warn!("Agent could not find pane to set visualization: {pane:?}");
                        }
                    } else {
                        log::warn!("Agent requested unknown visualization type: {viz_type}");
                    }
                }
                AgentCommand::SetAbsoluteTimeRange { start, end } => {
                    // Set the custom time range
                    self.time_range_toolbar.set_custom_range(start, end);
                    // Refresh all panes to use the new time range
                    self.refresh_all_panes();
                    log::info!("Agent set absolute time range: {start} to {end}");
                    success = true;
                }
                AgentCommand::RefreshPane { pane } => {
                    if let Some(ref pane_name) = pane {
                        // Refresh a specific pane
                        if let Some(tile_id) = self.find_pane_by_name(pane_name) {
                            if let Some(egui_tiles::Tile::Pane(component)) =
                                self.viewport_tree.tiles.get_mut(tile_id)
                            {
                                if let Some(query_pane) =
                                    component.as_any_mut().downcast_mut::<QueryPane>()
                                {
                                    query_pane.mark_needs_refresh();
                                    log::info!("Agent refreshed pane: {pane_name}");
                                    success = true;
                                }
                            }
                        } else {
                            log::warn!("Agent could not find pane to refresh: {pane_name}");
                        }
                    } else {
                        // Refresh all panes
                        self.refresh_all_panes();
                        log::info!("Agent refreshed all panes");
                        success = true;
                    }
                }
                AgentCommand::ClosePane { pane } => {
                    if let Some(tile_id) = self.resolve_pane_target(&pane) {
                        self.close_tile(tile_id);
                        log::info!("Agent closed pane: {pane}");
                        success = true;
                    } else {
                        log::warn!("Agent could not find pane to close: {pane}");
                    }
                }
                AgentCommand::CreateSection { name, collapsed } => {
                    // Create a new section config
                    use crate::workspace::config::SectionConfig;
                    use crate::workspace::input::SectionState;

                    let section_config = SectionConfig::new(&name);
                    let section_state = SectionState::new(collapsed.unwrap_or(false));

                    self.section_configs.push(section_config);
                    self.section_states.push(section_state);

                    log::info!(
                        "Agent created section: {} (collapsed: {})",
                        name,
                        collapsed.unwrap_or(false)
                    );
                    success = true;
                }
                AgentCommand::CreateFloatingPane {
                    query,
                    title,
                    position,
                } => {
                    self.add_floating_query_pane(&query, title.as_deref(), position);
                    success = true;
                }
                AgentCommand::MaximizePane { pane } => {
                    if let Some(tile_id) = self.resolve_pane_target(&pane) {
                        // Verify it's a pane (not a container)
                        if matches!(
                            self.viewport_tree.tiles.get(tile_id),
                            Some(egui_tiles::Tile::Pane(_))
                        ) {
                            self.fullscreen_tile = Some(tile_id);
                            log::info!("Agent maximized pane: {pane}");
                            success = true;
                        }
                    } else {
                        log::warn!("Agent could not find pane to maximize: {pane}");
                    }
                }
                AgentCommand::RenamePane { pane, new_name } => {
                    if let Some(tile_id) = self.resolve_pane_target(&pane) {
                        if let Some(egui_tiles::Tile::Pane(component)) =
                            self.viewport_tree.tiles.get_mut(tile_id)
                        {
                            if let Some(query_pane) =
                                component.as_any_mut().downcast_mut::<QueryPane>()
                            {
                                query_pane.set_name(&new_name);
                                log::info!("Agent renamed pane '{pane}' to '{new_name}'");
                                success = true;
                            }
                        }
                    } else {
                        log::warn!("Agent could not find pane to rename: {pane}");
                    }
                }
                AgentCommand::DuplicatePane { pane, new_name } => {
                    // Find the source pane and get its query
                    let source_query = if let Some(tile_id) = self.resolve_pane_target(&pane) {
                        if let Some(egui_tiles::Tile::Pane(component)) =
                            self.viewport_tree.tiles.get(tile_id)
                        {
                            component
                                .as_any()
                                .downcast_ref::<QueryPane>()
                                .map(|qp| (qp.query().to_string(), qp.name().to_string()))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some((query, original_name)) = source_query {
                        // Create a duplicate pane
                        let query_number = self.next_query_number;
                        self.next_query_number += 1;

                        let default_name = format!("{original_name} (copy)");
                        let name = new_name.as_deref().unwrap_or(&default_name);
                        let new_pane: Box<dyn Component> = if self.query_executor.is_connected() {
                            Box::new(QueryPane::with_query_named(&query, name, query_number))
                        } else {
                            Box::new(QueryPane::with_demo_query_named(&query, name, query_number))
                        };

                        let tile_id = self.viewport_tree.tiles.insert_pane(new_pane);
                        if self.add_tile_to_viewport(tile_id) {
                            self.show_landing = false;
                            log::info!("Agent duplicated pane '{pane}' as '{name}'");
                            success = true;
                        }
                    } else {
                        log::warn!("Agent could not find pane to duplicate: {pane}");
                    }
                }
                AgentCommand::FocusPane { pane } => {
                    if let Some(tile_id) = self.find_pane_by_name(&pane) {
                        self.behavior.set_focused_tile(Some(tile_id));
                        self.activate_tile(tile_id);
                        log::info!("Agent focused pane: {pane}");
                        success = true;
                    } else {
                        log::warn!("Agent could not find pane to focus: {pane}");
                    }
                }
                AgentCommand::ToggleZenMode => {
                    self.zen_mode = !self.zen_mode;
                    log::info!("Agent toggled zen mode: {}", self.zen_mode);
                    success = true;
                }
                AgentCommand::ExitFullscreen => {
                    if self.fullscreen_tile.is_some() {
                        self.fullscreen_tile = None;
                        log::info!("Agent exited fullscreen mode");
                        success = true;
                    }
                }
                AgentCommand::Sync => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.codebase_manager.fetch_updates(ctx);
                        log::info!("Agent triggered repository sync and re-indexing");
                        success = true;
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        log::warn!("Sync command not supported in WASM");
                    }
                }
                AgentCommand::ShowInlineDiff { commit, file } => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        // Get the diff and inject it as inline content
                        if let Some(diff_content) =
                            self.get_git_diff_for_inline(commit.as_deref(), file.as_deref())
                        {
                            self.inject_inline_content_to_agent_pane(InlineContent::Diff(
                                diff_content,
                            ));
                            log::info!("Agent showed inline diff");
                            success = true;
                        } else {
                            log::warn!("Failed to get git diff");
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = (commit, file);
                        log::warn!("ShowInlineDiff command not supported in WASM");
                    }
                }
                AgentCommand::ShowSource {
                    name,
                    source_type,
                    context_lines,
                } => {
                    let is_alert = source_type.as_deref() == Some("alert");
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if is_alert {
                            self.open_alert_definition(&name);
                            log::info!("Agent showed alert source: {name}");
                            success = true;
                        } else {
                            // Try inline source first, fall back to modal
                            let lines = context_lines.unwrap_or(5);
                            if let Some(source) = self.generate_inline_source(&name, lines) {
                                self.inject_inline_content_to_agent_pane(InlineContent::Source(
                                    source,
                                ));
                                log::info!("Agent showed inline source for: {name}");
                                success = true;
                            } else {
                                self.open_metric_definition(&name);
                                log::info!("Agent showed metric source (modal): {name}");
                                success = true;
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = (name, is_alert, context_lines);
                        log::warn!("ShowSource not available in WASM");
                    }
                }
                AgentCommand::ShowInlineTable { query, title } => {
                    if let Some(mut table) = self.get_sql_result_as_inline_table(query.as_deref()) {
                        if let Some(t) = title {
                            table.title = t;
                        }
                        self.inject_inline_content_to_agent_pane(InlineContent::Table(table));
                        log::info!("Agent showed inline table");
                        success = true;
                    } else {
                        log::warn!("No SQL results available for inline table");
                    }
                }
                AgentCommand::LoadWorkspace { workspace } => {
                    self.pending_load_workspace = Some(workspace.clone());
                    log::info!("Agent requested workspace load: {workspace}");
                    success = true;
                }
            }

            // Create activity item for this command
            activities.push(ActivityItem {
                activity_type: ActivityType::EditorAction {
                    description,
                    success,
                },
                in_progress: false,
            });
        }

        // Request repaint to ensure query execution runs on next frame
        if !activities.is_empty() {
            ctx.request_repaint();
        }

        activities
    }

    /// Poll all agent panes for pending commands and execute them.
    ///
    /// Poll for commands from the agent panel.
    ///
    /// This should be called during the workspace's show() method to ensure
    /// commands from the agent panel are processed.
    ///
    /// Note: This was previously used for polling AgentPane instances as well,
    /// but AgentPane has been removed in favor of the AgentPanel overlay.
    pub(super) fn poll_agent_pane_commands(&mut self, _ctx: &egui::Context) {
        // Agent panel commands are handled directly in show() via AgentPanelResult::Commands
        // This function is kept for potential future use or can be removed
    }

    /// Parse a time range preset string into the enum.
    fn parse_time_preset(
        preset: &str,
    ) -> Option<crate::components::widget::time_range::TimeRangePreset> {
        use crate::components::widget::time_range::TimeRangePreset;
        match preset.to_lowercase().as_str() {
            "5m" | "5min" | "5 minutes" => Some(TimeRangePreset::Last5Minutes),
            "15m" | "15min" | "15 minutes" => Some(TimeRangePreset::Last15Minutes),
            "30m" | "30min" | "30 minutes" => Some(TimeRangePreset::Last30Minutes),
            "1h" | "1hour" | "1 hour" => Some(TimeRangePreset::Last1Hour),
            "6h" | "6hour" | "6 hours" => Some(TimeRangePreset::Last6Hours),
            "24h" | "1d" | "1day" | "1 day" => Some(TimeRangePreset::Last24Hours),
            "7d" | "7day" | "7 days" | "1 week" | "1w" => Some(TimeRangePreset::Last7Days),
            _ => None,
        }
    }

    /// Parse a visualization type string into the enum.
    fn parse_visualization_type(
        viz_type: &str,
    ) -> Option<crate::components::pane::visualization::VisualizationType> {
        use crate::components::pane::visualization::VisualizationType;
        match viz_type.to_lowercase().replace(['-', '_'], "").as_str() {
            "timeseries" | "line" | "chart" => Some(VisualizationType::TimeSeries),
            "stat" | "bignumber" | "single" => Some(VisualizationType::Stat),
            "gauge" | "dial" | "meter" => Some(VisualizationType::Gauge),
            "barchart" | "bar" | "bars" | "horizontal" => Some(VisualizationType::BarChart),
            "sparkline" | "spark" | "mini" => Some(VisualizationType::Sparkline),
            "heatmap" | "heat" | "matrix" => Some(VisualizationType::Heatmap),
            _ => None,
        }
    }

    /// Resolve a pane target string to a TileId.
    ///
    /// Handles "focused" keyword or looks up by name.
    fn resolve_pane_target(&self, pane: &str) -> Option<TileId> {
        if pane.to_lowercase() == "focused" {
            self.behavior.focused_tile()
        } else {
            self.find_pane_by_name(pane)
        }
    }

    /// Find a pane by its name/title.
    ///
    /// Matching priority:
    /// 1. Exact match on label or pane name (case-insensitive)
    /// 2. Substring match (name contains search term or vice versa)
    fn find_pane_by_name(&self, name: &str) -> Option<TileId> {
        let name_lower = name.to_lowercase();
        let mut substring_match: Option<TileId> = None;

        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                // Check the label
                let label = component.label().text().to_lowercase();

                // Exact match - return immediately
                if label == name_lower {
                    return Some(tile_id);
                }

                // For QueryPane, also check the name
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    let pane_name = query_pane.name().to_lowercase();

                    // Exact match on pane name - return immediately
                    if pane_name == name_lower {
                        return Some(tile_id);
                    }

                    // Substring match - save for fallback (prefer first match)
                    if substring_match.is_none()
                        && (pane_name.contains(&name_lower) || name_lower.contains(&pane_name))
                    {
                        substring_match = Some(tile_id);
                    }
                }

                // Substring match on label - save for fallback
                if substring_match.is_none()
                    && (label.contains(&name_lower) || name_lower.contains(&label))
                {
                    substring_match = Some(tile_id);
                }
            }
        }

        // Return substring match if no exact match found
        substring_match
    }

    /// Add a tile to the viewport, handling different container types
    /// Returns true if the tile was successfully added
    ///
    /// If sections mode is active, this clears sections mode so the new pane
    /// is visible. Sections mode renders panes defined in the TOML config,
    /// but dynamically added panes aren't in any section.
    pub(super) fn add_tile_to_viewport(&mut self, tile_id: TileId) -> bool {
        // Clear sections mode if active - dynamically added panes aren't in any section
        // and need tile-based rendering to be visible
        if !self.section_configs.is_empty() {
            log::info!(
                "Clearing {} sections to show dynamically added pane in tile mode",
                self.section_configs.len()
            );
            self.section_configs.clear();
            self.section_states.clear();
        }

        let tiles_before = self.viewport_tree.tiles.len();
        let Some(root_id) = self.viewport_tree.root() else {
            // No root exists (all panes were closed), create a new tabs container
            log::warn!(
                "add_tile_to_viewport: No root exists! Creating new tabs container. tiles_before={tiles_before}"
            );
            let new_root = self.viewport_tree.tiles.insert_tab_tile(vec![tile_id]);
            self.viewport_tree.root = Some(new_root);
            return true;
        };
        log::debug!(
            "add_tile_to_viewport: Adding tile {tile_id:?} to root {root_id:?}. tiles_before={tiles_before}"
        );

        match self.viewport_tree.tiles.get_mut(root_id) {
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) => {
                tabs.add_child(tile_id);
                tabs.set_active(tile_id);
                true
            }
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) => {
                linear.add_child(tile_id);
                true
            }
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Grid(grid))) => {
                grid.add_child(tile_id);
                true
            }
            _ => false,
        }
    }

    // ==================== Pane Closing ====================

    /// Close a tile and remove it from the viewport.
    ///
    /// This captures the pane's position information before removal and pushes
    /// an undo action so the pane can be restored with 'u'.
    pub(super) fn close_tile(&mut self, tile_id: TileId) {
        // Check if this tile was focused (for undo restoration)
        let was_focused = self.behavior.focused_tile() == Some(tile_id);

        // Get the pane's label before removing it (for open_charts tracking)
        let label = if let Some(egui_tiles::Tile::Pane(component)) =
            self.viewport_tree.tiles.get(tile_id)
        {
            Some(component.label().text().to_string())
        } else {
            None
        };

        // Capture parent container info BEFORE removal for undo
        let parent_info = self.find_parent_container_info(tile_id);

        // Get the container kind if parent exists
        let container_kind = parent_info.and_then(|(parent_id, _)| {
            if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(parent_id) {
                Some(container.kind())
            } else {
                None
            }
        });

        // Find the next tile to focus before removing
        let pane_ids = self.get_pane_tile_ids();
        let next_focus = if pane_ids.len() > 1 {
            // Try to find a sibling to focus
            self.find_sibling_in_direction(tile_id, NavDirection::Right)
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Left))
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Down))
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Up))
                .or_else(|| pane_ids.iter().find(|&&id| id != tile_id).copied())
        } else {
            None
        };

        // Remove the tile from the tree and capture the component for undo
        if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.remove(tile_id) {
            // Push undo action with captured info
            let closed_pane_info = super::ClosedPaneInfo {
                component,
                parent_id: parent_info.map(|(id, _)| id),
                child_index: parent_info.map(|(_, idx)| idx).unwrap_or(0),
                container_kind: container_kind.unwrap_or(egui_tiles::ContainerKind::Tabs),
                was_focused,
            };
            self.undo_stack
                .push(super::UndoAction::RestorePane(closed_pane_info));
            log::debug!("Pushed close pane to undo stack");
        }

        // Remove from open_charts tracking
        if let Some(label) = label {
            self.open_charts.remove(&label);
            // Also try removing with query: prefix
            self.open_charts.remove(&format!("query:{label}"));
            log::debug!("Closed tile: {label}");
        }

        // Update focus to next tile, validating it still exists after removal
        // (tree structure may have changed, e.g., collapsed containers)
        let validated_focus = next_focus.filter(|&id| self.viewport_tree.tiles.get(id).is_some());
        if validated_focus.is_none() && next_focus.is_some() {
            // Original focus target was removed, find a fallback
            let fresh_pane_ids = self.get_pane_tile_ids();
            self.behavior
                .set_focused_tile(fresh_pane_ids.first().copied());
        } else {
            self.behavior.set_focused_tile(validated_focus);
        }
    }

    /// Execute the most recent undo action.
    ///
    /// Returns true if an action was undone, false if the undo stack was empty.
    pub(super) fn execute_undo(&mut self) -> bool {
        let Some(action) = self.undo_stack.pop() else {
            log::debug!("Undo: stack is empty");
            return false;
        };

        match action {
            super::UndoAction::RestorePane(info) => {
                self.restore_closed_pane(info);
                true
            }
            super::UndoAction::UnfloatPane(info) => {
                self.unfloat_pane(info);
                true
            }
            super::UndoAction::UndockPane(info) => {
                self.undock_pane(info);
                true
            }
        }
    }

    /// Restore a closed pane to its previous position.
    fn restore_closed_pane(&mut self, info: super::ClosedPaneInfo) {
        // Re-insert the component to get a new TileId
        let new_tile_id = self.viewport_tree.tiles.insert_pane(info.component);

        // Try to restore to the original position
        let restored = if let Some(parent_id) = info.parent_id {
            // Check if the parent container still exists
            if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get_mut(parent_id) {
                match container {
                    egui_tiles::Container::Tabs(tabs) => {
                        // Insert at the original index if possible, otherwise at the end
                        let insert_idx = info.child_index.min(tabs.children.len());
                        tabs.children.insert(insert_idx, new_tile_id);
                        tabs.set_active(new_tile_id);
                        true
                    }
                    egui_tiles::Container::Linear(linear) => {
                        // Insert at the original index if possible, otherwise at the end
                        let insert_idx = info.child_index.min(linear.children.len());
                        linear.children.insert(insert_idx, new_tile_id);
                        true
                    }
                    egui_tiles::Container::Grid(grid) => {
                        // Grid doesn't support positional insertion well, just add
                        grid.add_child(new_tile_id);
                        true
                    }
                }
            } else {
                // Parent no longer exists, fall back to adding to viewport root
                false
            }
        } else {
            // No parent info, fall back to adding to viewport root
            false
        };

        // If we couldn't restore to the original position, add to viewport root
        if !restored {
            self.add_tile_to_viewport(new_tile_id);
        }

        // Restore focus if the pane was focused when closed
        if info.was_focused {
            self.behavior.set_focused_tile(Some(new_tile_id));
            self.activate_tile(new_tile_id);
        }

        self.show_landing = false;
        log::debug!("Restored closed pane, new tile_id={new_tile_id:?}");
    }

    /// Undo a float operation: remove from floating panes and restore to tile tree.
    fn unfloat_pane(&mut self, info: super::FloatedPaneInfo) {
        // Remove the component from floating panes
        let Some(component) = self.floating_panes.remove_pane(info.floating_pane_id) else {
            log::warn!(
                "Cannot undo float: floating pane {:?} no longer exists",
                info.floating_pane_id
            );
            return;
        };

        // Re-insert the component to get a new TileId
        let new_tile_id = self.viewport_tree.tiles.insert_pane(component);

        // Try to restore to the original position in the tile tree
        let restored = if let Some(parent_id) = info.parent_id {
            // Check if the parent container still exists
            if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get_mut(parent_id) {
                match container {
                    egui_tiles::Container::Tabs(tabs) => {
                        let insert_idx = info.child_index.min(tabs.children.len());
                        tabs.children.insert(insert_idx, new_tile_id);
                        tabs.set_active(new_tile_id);
                        true
                    }
                    egui_tiles::Container::Linear(linear) => {
                        let insert_idx = info.child_index.min(linear.children.len());
                        linear.children.insert(insert_idx, new_tile_id);
                        true
                    }
                    egui_tiles::Container::Grid(grid) => {
                        grid.add_child(new_tile_id);
                        true
                    }
                }
            } else {
                false
            }
        } else {
            false
        };

        // If we couldn't restore to the original position, add to viewport root
        if !restored {
            self.add_tile_to_viewport(new_tile_id);
        }

        // Restore focus if it was focused before floating
        if info.was_tile_focused {
            self.behavior.set_focused_tile(Some(new_tile_id));
            self.activate_tile(new_tile_id);
        }

        self.show_landing = false;
        log::debug!("Undid float: restored pane to tile tree, new tile_id={new_tile_id:?}");
    }

    /// Undo a dock operation: remove from tile tree and restore to floating.
    fn undock_pane(&mut self, info: super::DockedPaneInfo) {
        // Find the pane by name (TileIds can change due to tree restructuring)
        let tile_id = self.get_pane_tile_ids().into_iter().find(|&tile_id| {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                component.name() == info.component_name
            } else {
                false
            }
        });

        let Some(tile_id) = tile_id else {
            log::warn!(
                "Cannot undo dock: pane '{}' not found in tile tree",
                info.component_name
            );
            return;
        };

        // Remove the component from the tile tree
        let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.remove(tile_id)
        else {
            log::warn!("Cannot undo dock: failed to remove tile {tile_id:?}");
            return;
        };

        // Add back to floating panes with the original position and size
        let floating_pane_id =
            self.floating_panes
                .add_pane_with_size(component, info.position, info.size);

        // Configure the restored pane: skip animation and restore pinned state
        if let Some(pane) = self
            .floating_panes
            .panes
            .iter_mut()
            .find(|p| p.id == floating_pane_id)
        {
            // Skip appearing animation - pane should be immediately visible for undo
            pane.skip_animation();

            // Restore pinned state
            if info.pinned {
                pane.pinned = true;
            }
        }

        // Set focus to the floating pane
        self.floating_panes.set_focus(Some(floating_pane_id));
        self.behavior.set_focused_tile(None);

        // Ensure landing page doesn't take over
        self.show_landing = false;

        log::debug!("Undid dock: restored pane to floating");
    }

    /// Close all charts and reset the viewport to show landing page
    pub(super) fn close_all_charts(&mut self) {
        // Get all pane tile IDs and close them
        let pane_ids = self.get_pane_tile_ids();
        for tile_id in pane_ids {
            self.viewport_tree.tiles.remove(tile_id);
        }

        // Clear tracking
        self.open_charts.clear();
        self.behavior.set_focused_tile(None);
        self.fullscreen_tile = None;
        self.zen_mode = false;

        log::debug!("Closed all charts, showing landing page");
    }

    // ==================== Pane Splitting ====================

    /// Split panes horizontally (`:split` - panes stacked vertically, one above another)
    pub(super) fn split_panes_horizontal(&mut self) {
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.len() < 2 {
            log::debug!("Need at least 2 panes to split");
            return;
        }

        // Preserve focus on the currently focused pane, or first pane
        let focus_pane = self
            .behavior
            .focused_tile()
            .filter(|id| pane_ids.contains(id))
            .or_else(|| pane_ids.first().copied());

        // Create a new vertical container (panes stacked on top of each other)
        let new_root = self.viewport_tree.tiles.insert_vertical_tile(pane_ids);
        self.viewport_tree.root = Some(new_root);

        // Restore focus
        self.behavior.set_focused_tile(focus_pane);
        log::debug!("Split panes horizontally (vertical layout)");
    }

    /// Split panes vertically (`:vsplit` - panes side by side)
    pub(super) fn split_panes_vertical(&mut self) {
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.len() < 2 {
            log::debug!("Need at least 2 panes to split");
            return;
        }

        // Preserve focus on the currently focused pane, or first pane
        let focus_pane = self
            .behavior
            .focused_tile()
            .filter(|id| pane_ids.contains(id))
            .or_else(|| pane_ids.first().copied());

        // Create a new horizontal container (panes side by side)
        let new_root = self.viewport_tree.tiles.insert_horizontal_tile(pane_ids);
        self.viewport_tree.root = Some(new_root);

        // Restore focus
        self.behavior.set_focused_tile(focus_pane);
        log::debug!("Split panes vertically (horizontal layout)");
    }

    /// Setup the tutorial layout with two panes stacked vertically:
    /// - "HTTP Requests" on top, "Memory Used" on bottom
    pub(super) fn setup_tutorial_layout(&mut self) {
        use crate::components::pane::QueryPane;

        let demo_queries = [
            (
                "http_requests_total{method=\"GET\", path=\"/api/users\"}",
                "HTTP Requests",
                "",
            ),
            ("node_memory_Active_bytes", "Memory Used", "MB"),
        ];

        let mut pane_ids = Vec::new();
        for (query, name, unit) in demo_queries {
            let pane: Box<dyn Component> =
                Box::new(QueryPane::with_demo_query_named_unit(query, name, unit));
            let pane_tile = self.viewport_tree.tiles.insert_pane(pane);
            self.open_charts.insert(query.to_string());
            pane_ids.push(pane_tile);
        }

        let root = self
            .viewport_tree
            .tiles
            .insert_vertical_tile(vec![pane_ids[0], pane_ids[1]]);

        // Set as the tree root
        self.viewport_tree.root = Some(root);

        // Focus the first pane
        self.behavior.set_focused_tile(Some(pane_ids[0]));

        log::debug!("Setup tutorial layout with 2 panes side by side");
    }

    // ==================== Pane Movement (Ctrl+W H/J/K/L) ====================

    /// Move the focused pane to the far left (becomes leftmost vertical split).
    /// This is vim's Ctrl+W H behavior.
    pub(super) fn move_pane_to_far_left(&mut self) {
        self.move_pane_to_edge(super::NavDirection::Left);
    }

    /// Move the focused pane to the far right (becomes rightmost vertical split).
    /// This is vim's Ctrl+W L behavior.
    pub(super) fn move_pane_to_far_right(&mut self) {
        self.move_pane_to_edge(super::NavDirection::Right);
    }

    /// Move the focused pane to the very top (becomes top horizontal split).
    /// This is vim's Ctrl+W K behavior.
    pub(super) fn move_pane_to_top(&mut self) {
        self.move_pane_to_edge(super::NavDirection::Up);
    }

    /// Move the focused pane to the very bottom (becomes bottom horizontal split).
    /// This is vim's Ctrl+W J behavior.
    pub(super) fn move_pane_to_bottom(&mut self) {
        self.move_pane_to_edge(super::NavDirection::Down);
    }

    /// Move the focused pane to the edge of the viewport in the given direction.
    ///
    /// This extracts the pane from its current position and creates a new split
    /// at the edge of the viewport. For Left/Right, it creates a horizontal layout
    /// with the pane on the specified side. For Up/Down, it creates a vertical layout.
    fn move_pane_to_edge(&mut self, direction: super::NavDirection) {
        let Some(focused_id) = self.behavior.focused_tile() else {
            log::debug!("No focused pane to move");
            return;
        };

        // Verify it's actually a pane before removing
        if !matches!(
            self.viewport_tree.tiles.get(focused_id),
            Some(Tile::Pane(_))
        ) {
            log::debug!("Focused tile is not a pane");
            return;
        }

        // Extract the pane
        let Some(Tile::Pane(pane)) = self.viewport_tree.tiles.remove(focused_id) else {
            log::debug!("Focused tile not found");
            return;
        };

        // Re-insert the pane to get a fresh TileId
        let new_pane_id = self.viewport_tree.tiles.insert_pane(pane);

        // Get current root after removal (tree may have auto-simplified)
        let Some(current_root) = self.viewport_tree.root() else {
            // Tree is empty, just set the pane as root
            self.viewport_tree.root = Some(new_pane_id);
            self.behavior.set_focused_tile(Some(new_pane_id));
            log::debug!("Tree was empty, pane is now root");
            return;
        };

        // If the root is now just the pane we're moving (only pane case),
        // nothing more to do
        if current_root == new_pane_id {
            self.behavior.set_focused_tile(Some(new_pane_id));
            log::debug!("Only one pane, nothing to move");
            return;
        }

        // Create new container with the pane at the edge
        let new_root = match direction {
            super::NavDirection::Left => {
                // Pane on left, rest on right (horizontal split)
                self.viewport_tree
                    .tiles
                    .insert_horizontal_tile(vec![new_pane_id, current_root])
            }
            super::NavDirection::Right => {
                // Rest on left, pane on right (horizontal split)
                self.viewport_tree
                    .tiles
                    .insert_horizontal_tile(vec![current_root, new_pane_id])
            }
            super::NavDirection::Up => {
                // Pane on top, rest on bottom (vertical split)
                self.viewport_tree
                    .tiles
                    .insert_vertical_tile(vec![new_pane_id, current_root])
            }
            super::NavDirection::Down => {
                // Rest on top, pane on bottom (vertical split)
                self.viewport_tree
                    .tiles
                    .insert_vertical_tile(vec![current_root, new_pane_id])
            }
        };

        self.viewport_tree.root = Some(new_root);

        // Start layout animation for smooth transition
        self.layout_animator
            .animate_split(new_root, new_pane_id, current_root, 1.0);

        // Maintain focus on the moved pane
        self.behavior.set_focused_tile(Some(new_pane_id));

        log::debug!("Moved pane to {direction:?} edge, new id {new_pane_id:?}");
    }

    // ==================== Pane Tabbing (Ctrl+W t) ====================

    /// Move the focused pane into a tab container with the pane in the given direction.
    /// If the target is already in a tab container, add to that container.
    /// Otherwise, create a new tab container with both panes.
    pub(super) fn move_pane_to_tab_with(&mut self, direction: super::NavDirection) {
        let Some(focused_id) = self.behavior.focused_tile() else {
            log::debug!("No focused pane to move to tab");
            return;
        };

        // Find the target pane in the given direction
        let Some(target_id) = self.find_sibling_in_direction(focused_id, direction) else {
            log::debug!("No sibling pane found in direction {direction:?}");
            return;
        };

        // Don't tab with ourselves
        if target_id == focused_id {
            log::debug!("Cannot tab pane with itself");
            return;
        }

        // Verify both are panes
        if !matches!(
            self.viewport_tree.tiles.get(focused_id),
            Some(Tile::Pane(_))
        ) {
            log::debug!("Focused tile is not a pane");
            return;
        }

        // Check if target is already in a tab container
        if let Some(parent_tab_id) = self.find_parent_tab_container(target_id) {
            // Add focused pane to the existing tab container
            self.add_pane_to_tab_container(focused_id, parent_tab_id);
        } else {
            // Create a new tab container with both panes
            self.create_tab_container_with_panes(focused_id, target_id);
        }
    }

    /// Find the parent tab container of a tile, if any.
    fn find_parent_tab_container(&self, target_id: TileId) -> Option<TileId> {
        let root_id = self.viewport_tree.root()?;
        self.find_parent_tab_recursive(root_id, target_id, 0)
    }

    fn find_parent_tab_recursive(
        &self,
        container_id: TileId,
        target_id: TileId,
        depth: usize,
    ) -> Option<TileId> {
        // Guard against pathological tree structures
        if depth > MAX_TREE_DEPTH {
            log::warn!("find_parent_tab_recursive exceeded max depth {MAX_TREE_DEPTH}");
            return None;
        }

        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            let children: Vec<TileId> = container.children().copied().collect();

            // Check if target is a direct child of this container
            if children.contains(&target_id) {
                // Only return if this is a tabs container
                if matches!(container.kind(), egui_tiles::ContainerKind::Tabs) {
                    return Some(container_id);
                }
                // Not a tabs container, target is a direct child but not in tabs
                return None;
            }

            // Recursively search nested containers
            for child_id in children {
                if let Some(parent) = self.find_parent_tab_recursive(child_id, target_id, depth + 1)
                {
                    return Some(parent);
                }
            }
        }
        None
    }

    /// Add a pane to an existing tab container.
    fn add_pane_to_tab_container(&mut self, pane_id: TileId, tab_container_id: TileId) {
        // Extract the pane first
        let Some(Tile::Pane(pane)) = self.viewport_tree.tiles.remove(pane_id) else {
            log::debug!("Could not extract pane {pane_id:?}");
            return;
        };

        // Re-insert to get a fresh ID
        let new_pane_id = self.viewport_tree.tiles.insert_pane(pane);

        // Add to the tab container
        if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) =
            self.viewport_tree.tiles.get_mut(tab_container_id)
        {
            tabs.add_child(new_pane_id);
            tabs.set_active(new_pane_id);
            self.behavior.set_focused_tile(Some(new_pane_id));
            log::debug!("Added pane to existing tab container {tab_container_id:?}");
        } else {
            log::warn!("Tab container {tab_container_id:?} not found or not a tabs container");
        }
    }

    /// Create a new tab container with both panes, replacing the target's position.
    fn create_tab_container_with_panes(&mut self, pane_id: TileId, target_id: TileId) {
        // Find the parent container of the target to know where to insert the new tabs
        let parent_info = self.find_parent_container_info(target_id);

        // Extract the focused pane
        let Some(Tile::Pane(pane)) = self.viewport_tree.tiles.remove(pane_id) else {
            log::debug!("Could not extract focused pane {pane_id:?}");
            return;
        };

        // Re-insert to get a fresh ID
        let new_pane_id = self.viewport_tree.tiles.insert_pane(pane);

        // Create a new tab container with both the target and the moved pane
        // Target goes first (it was there first), moved pane second (and becomes active)
        let tab_container_id = self
            .viewport_tree
            .tiles
            .insert_tab_tile(vec![target_id, new_pane_id]);

        // Replace the target in its parent with the new tab container
        if let Some((parent_id, child_index)) = parent_info {
            // Replace the target in the parent container with the tab container
            if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get_mut(parent_id) {
                match container {
                    egui_tiles::Container::Linear(linear) => {
                        // Remove target and insert tab container at the same position
                        let children: Vec<TileId> = linear.children.to_vec();
                        linear.children.clear();
                        for (i, child) in children.into_iter().enumerate() {
                            if i == child_index {
                                linear.children.push(tab_container_id);
                            } else if child != target_id {
                                linear.children.push(child);
                            } else {
                                // Skip the target, it's now inside the tab container
                            }
                        }
                        // If target was at the position, we already inserted tab_container
                        // If not found at index, just push
                        if !linear.children.contains(&tab_container_id) {
                            linear.children.push(tab_container_id);
                        }
                    }
                    egui_tiles::Container::Tabs(tabs) => {
                        // Replace target with tab container in the tabs
                        // This creates nested tabs, which might be unusual but valid
                        let children: Vec<TileId> = tabs.children.to_vec();
                        tabs.children.clear();
                        for child in children {
                            if child == target_id {
                                tabs.children.push(tab_container_id);
                            } else {
                                tabs.children.push(child);
                            }
                        }
                        tabs.set_active(tab_container_id);
                    }
                    egui_tiles::Container::Grid(_) => {
                        // Grid containers are not commonly used in this editor.
                        // For now, log a warning - this case is rare.
                        log::warn!(
                            "Cannot replace child in grid container - grid not supported for tab merging"
                        );
                    }
                }
            }
        } else {
            // Target was the root, or no parent found - make tab container the new root
            self.viewport_tree.root = Some(tab_container_id);
        }

        // Set the moved pane as active in the new tab container
        if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) =
            self.viewport_tree.tiles.get_mut(tab_container_id)
        {
            tabs.set_active(new_pane_id);
        }

        self.behavior.set_focused_tile(Some(new_pane_id));
        log::debug!(
            "Created new tab container with target {target_id:?} and moved pane {new_pane_id:?}"
        );
    }

    /// Find the parent container and the index of a child within it.
    fn find_parent_container_info(&self, target_id: TileId) -> Option<(TileId, usize)> {
        let root_id = self.viewport_tree.root()?;
        self.find_parent_info_recursive(root_id, target_id, 0)
    }

    fn find_parent_info_recursive(
        &self,
        container_id: TileId,
        target_id: TileId,
        depth: usize,
    ) -> Option<(TileId, usize)> {
        // Guard against pathological tree structures
        if depth > MAX_TREE_DEPTH {
            log::warn!("find_parent_info_recursive exceeded max depth {MAX_TREE_DEPTH}");
            return None;
        }

        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            let children: Vec<TileId> = container.children().copied().collect();

            // Check if target is a direct child
            for (index, &child) in children.iter().enumerate() {
                if child == target_id {
                    return Some((container_id, index));
                }
            }

            // Recursively search nested containers
            for child_id in children {
                if let Some(info) = self.find_parent_info_recursive(child_id, target_id, depth + 1)
                {
                    return Some(info);
                }
            }
        }
        None
    }

    // ==================== Pane Queries ====================

    /// Get all pane tile IDs in the viewport (for navigation)
    #[profiling::function]
    pub(super) fn get_pane_tile_ids(&self) -> Vec<TileId> {
        let mut pane_ids = Vec::new();

        if let Some(root_id) = self.viewport_tree.root() {
            self.collect_pane_ids(root_id, &mut pane_ids, 0);
        }

        pane_ids
    }

    /// Collect PromQL queries from all open QueryPane components.
    ///
    /// Used by AI context builders to provide agents with awareness of
    /// currently active queries in the dashboard.
    pub(super) fn collect_pane_queries(&self) -> Vec<String> {
        self.get_pane_tile_ids()
            .iter()
            .filter_map(|&tile_id| {
                if let Some(egui_tiles::Tile::Pane(component)) =
                    self.viewport_tree.tiles.get(tile_id)
                {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        return Some(query_pane.saved_query().to_string());
                    }
                }
                None
            })
            .collect()
    }

    /// Collect pane info for a single tile, returning the info and query string.
    pub(super) fn collect_pane_info_for_tile(
        &self,
        tile_id: egui_tiles::TileId,
    ) -> Option<(crate::components::pane::PaneInfo, String)> {
        use crate::components::pane::PaneVisualization;
        use crate::components::pane::visualization::VisualizationType;

        let component = match self.viewport_tree.tiles.get(tile_id) {
            Some(egui_tiles::Tile::Pane(c)) => c,
            _ => return None,
        };
        let query_pane = component.as_any().downcast_ref::<QueryPane>()?;
        let query_text = query_pane.saved_query().to_string();
        let viz = query_pane.visualization();
        let viz_type = viz.viz_type();
        let name = query_pane.name().to_string();

        let pane_viz = match viz_type {
            VisualizationType::TimeSeries => {
                let ts_chart = viz.as_time_series()?;
                PaneVisualization::TimeSeries {
                    series: ts_chart.series().to_vec(),
                }
            }
            VisualizationType::Stat => {
                let stat = viz.as_stat()?;
                PaneVisualization::Stat {
                    value: stat.value(),
                    unit: stat.unit().to_string(),
                    sparkline: stat.sparkline_data().to_vec(),
                }
            }
            VisualizationType::Gauge => {
                let gauge = viz.as_gauge()?;
                PaneVisualization::Gauge {
                    value: gauge.value(),
                    min: gauge.min(),
                    max: gauge.max(),
                    unit: gauge.unit().to_string(),
                }
            }
            VisualizationType::BarChart => {
                let bar = viz.as_bar_chart()?;
                PaneVisualization::BarChart {
                    bars: bar
                        .bars()
                        .iter()
                        .map(|b| (b.label.clone(), b.value))
                        .collect(),
                }
            }
            VisualizationType::Sparkline => {
                let spark = viz.as_sparkline()?;
                PaneVisualization::Sparkline {
                    data: spark.data().to_vec(),
                }
            }
            VisualizationType::Heatmap => PaneVisualization::Heatmap,
        };

        Some((
            crate::components::pane::PaneInfo {
                name,
                viz_type,
                visualization: pane_viz,
            },
            query_text,
        ))
    }

    /// Open the diff viewer with specific content.
    pub fn open_diff_viewer_with_content(&mut self, hash: &str, message: &str, diff: &str) {
        log::info!("Opening diff viewer for commit from chat: {hash}");
        self.diff_viewer.open(hash, message, 0, diff);
    }

    /// Open the diff viewer for a specific commit hash (fetches diff content automatically).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_diff_viewer_for_commit(&mut self, hash: &str, message: &str) {
        use std::path::PathBuf;
        use std::process::Command;

        // Get repo path: prefer codebase manager index, fall back to current directory
        let repo_path = self
            .codebase_manager
            .index()
            .map(|idx| idx.repo_path.clone())
            .unwrap_or_else(|| PathBuf::from("."));

        // Fetch the full diff for this commit
        let diff_output = Command::new("git")
            .args(["show", hash, "--format=", "--unified=3", "-p"])
            .current_dir(&repo_path)
            .output();

        match diff_output {
            Ok(output) if output.status.success() => {
                let diff_content = String::from_utf8_lossy(&output.stdout).to_string();
                if !diff_content.is_empty() {
                    log::info!("Opening diff viewer for commit: {hash}");
                    self.diff_viewer.open(hash, message, 0, &diff_content);
                } else {
                    log::warn!("No diff content for commit: {hash}");
                }
            }
            Ok(output) => {
                log::warn!(
                    "git show failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => {
                log::warn!("Failed to run git show: {e}");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn open_diff_viewer_for_commit(&mut self, _hash: &str, _message: &str) {
        // Diff viewer not available on WASM
    }

    /// Recursively collect all pane tile IDs
    fn collect_pane_ids(&self, tile_id: TileId, pane_ids: &mut Vec<TileId>, depth: usize) {
        // Guard against pathological tree structures
        if depth > MAX_TREE_DEPTH {
            log::warn!("collect_pane_ids exceeded max depth {MAX_TREE_DEPTH}");
            return;
        }

        if let Some(tile) = self.viewport_tree.tiles.get(tile_id) {
            match tile {
                Tile::Pane(_) => {
                    pane_ids.push(tile_id);
                }
                Tile::Container(container) => {
                    for child_id in container.children() {
                        self.collect_pane_ids(*child_id, pane_ids, depth + 1);
                    }
                }
            }
        }
    }

    /// Count how many panes match the current filter and total panes
    #[profiling::function]
    pub(super) fn count_filtered_panes(&self) -> (usize, usize) {
        let pane_ids = self.get_pane_tile_ids();
        let total = pane_ids.len();

        if !self.viewport_filter.is_active() {
            return (total, total);
        }

        let matching = pane_ids
            .iter()
            .filter(|&&tile_id| {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    // Check QueryPane - match on query content OR tag
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        return self.viewport_filter.matches(query_pane.saved_query())
                            || self.viewport_filter.matches(query_pane.tag());
                    }
                    // Check Buffer
                    if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                        return self.viewport_filter.matches(buffer.saved_content());
                    }
                }
                true // Unknown component types are always shown
            })
            .count();

        (matching, total)
    }

    // ==================== Tile Activation ====================

    /// Activate a tile (make it the active tab in its parent container)
    pub(super) fn activate_tile(&mut self, tile_id: TileId) {
        // Find the parent tabs container and set this tile as active
        if let Some(root_id) = self.viewport_tree.root() {
            self.activate_tile_in_container(root_id, tile_id);
        }
    }

    /// Recursively find and activate a tile in its parent tabs container
    fn activate_tile_in_container(&mut self, container_id: TileId, target_id: TileId) -> bool {
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            let children: Vec<TileId> = container.children().copied().collect();

            // Check if target is a direct child
            if children.contains(&target_id) {
                // Set this tile as active in the tabs container
                if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                    self.viewport_tree.tiles.get_mut(container_id)
                {
                    tabs.set_active(target_id);
                    return true;
                }
            }

            // Recursively search children
            for child_id in children {
                if self.activate_tile_in_container(child_id, target_id) {
                    return true;
                }
            }
        }
        false
    }

    // ==================== Inline Content Generation ====================

    /// Request an inline chart by firing a real PromQL query.
    ///
    /// The query is executed asynchronously through the QueryExecutor. Results
    /// are picked up by `poll_inline_charts()` and injected into the agent panel.
    fn request_inline_chart(
        &mut self,
        query: &str,
        title: &str,
        height: Option<f32>,
        ctx: &egui::Context,
    ) {
        let query_id = Self::INLINE_CHART_ID_BASE + self.next_inline_chart_id;
        self.next_inline_chart_id += 1;

        let time_range = self.time_range_toolbar.time_range();
        let start_ns = (time_range.start * 1_000_000_000.0) as u128;
        let end_ns = (time_range.end * 1_000_000_000.0) as u128;
        let duration_secs = (time_range.end - time_range.start) as u64;
        let step_secs = (duration_secs / 60).max(1);

        let metric = Self::extract_metric_from_query(query);
        let params = ExecuteParams {
            metric: &metric,
            query,
            step_secs,
            start_ns: Some(start_ns),
            end_ns: Some(end_ns),
        };
        self.query_executor.execute_for_pane(query_id, &params, ctx);

        self.pending_inline_charts.push(PendingInlineChart {
            query_id,
            title: title.to_string(),
            height,
        });
    }

    /// Generate an inline chart with sample data for demo/offline mode.
    fn generate_demo_inline_chart(
        &self,
        query: &str,
        title: &str,
        height: Option<f32>,
    ) -> InlineChart {
        let time_range = self.time_range_toolbar.time_range();
        let now = time_range.end;
        let start = time_range.start;
        let duration_secs = now - start;

        let num_points = 60;
        let step = duration_secs / num_points as f64;
        let mut points = Vec::with_capacity(num_points);

        let base_value = 50.0;
        let amplitude = 20.0;

        for i in 0..num_points {
            let t = start + (i as f64 * step);
            let phase = (i as f64 / num_points as f64) * std::f64::consts::PI * 4.0;
            let noise = ((t as i64 % 17) as f64 - 8.0) / 8.0 * 5.0;
            let value = base_value + amplitude * phase.sin() + noise;

            points.push(DataPoint {
                timestamp: t,
                value: value.max(0.0),
            });
        }

        let series_name = Self::extract_metric_from_query(query);
        let series = Series::new(&series_name).with_points(points);

        InlineChart {
            title: title.to_string(),
            series: vec![series],
            height,
        }
    }

    /// Extract the metric name from a PromQL query.
    fn extract_metric_from_query(query: &str) -> String {
        // Try to find metric name - look for word before { or (
        let query = query.trim();

        // Handle rate(metric_name[...]) pattern
        if let Some(paren_idx) = query.find('(') {
            let after = &query[paren_idx + 1..];
            if let Some(end) = after.find(|c: char| !c.is_alphanumeric() && c != '_') {
                let metric = &after[..end];
                if !metric.is_empty() {
                    return metric.to_string();
                }
            }
        }

        // Handle metric_name{...} pattern
        if let Some(brace_idx) = query.find('{') {
            return query[..brace_idx].trim().to_string();
        }

        // Just return the query as-is (it might be just a metric name)
        query.to_string()
    }

    /// Generate inline source preview for a metric.
    ///
    /// Looks up the metric in the codebase index and returns source lines
    /// with pre-computed tree-sitter syntax highlighting.
    #[cfg(not(target_arch = "wasm32"))]
    fn generate_inline_source(&self, metric: &str, context_lines: usize) -> Option<InlineSource> {
        use crate::components::util::SyntaxHighlightData;

        // Check if codebase is ready
        if !self.codebase_manager.status().is_ready() {
            return None;
        }

        // Search for the metric - take exact match or first partial match
        let metrics = self.codebase_manager.search_metrics(metric);
        let metric_info = metrics
            .iter()
            .find(|m| m.name == metric)
            .or_else(|| metrics.first())
            .copied()?;

        // Get repo path from index
        let index = self.codebase_manager.index()?;
        let file_path = index.repo_path.join(&metric_info.file);

        // Read the source file
        let content = std::fs::read_to_string(&file_path).ok()?;
        let all_lines: Vec<&str> = content.lines().collect();

        // Calculate line range (0-indexed internally, 1-indexed for display)
        let target_line = metric_info.line;
        let start_line = target_line.saturating_sub(context_lines);
        let end_line = (target_line + context_lines).min(all_lines.len());

        // Extract the lines
        let lines: Vec<String> = all_lines
            .get(start_line..end_line)?
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        // Determine language from file extension
        let language = metric_info
            .file
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| match ext {
                "rs" => "rust",
                "go" => "go",
                "py" => "python",
                "js" | "ts" => "javascript",
                "java" => "java",
                "rb" => "ruby",
                _ => ext,
            })
            .unwrap_or("")
            .to_string();

        // Pre-compute tree-sitter syntax highlighting for the full file content
        // This allows efficient per-line highlighting during rendering
        let highlight_data = SyntaxHighlightData::new(&content, &language);

        Some(InlineSource {
            file_path: metric_info.file.display().to_string(),
            line: target_line,
            lines,
            start_line: start_line + 1, // Convert to 1-indexed
            language,
            highlight_data,
        })
    }

    /// Inject inline content into the active agent conversation.
    ///
    /// If the agent panel is open, adds to the last assistant message there.
    /// If in agent input bar mode, stores for handoff to the panel later.
    pub(super) fn inject_inline_content_to_agent_pane(&mut self, content: InlineContent) {
        if self.agent_panel.is_open() {
            self.agent_panel.add_inline_content(content);
            log::debug!("Injected inline content into agent panel");
        } else if self.agent_mode_active {
            // Store in input bar for handoff when user opens the panel
            self.agent_input_bar.add_inline_content(content);
            log::debug!("Stored inline content in agent input bar for handoff");
        } else {
            log::warn!("No active agent conversation for inline content");
        }
    }

    /// Get SQL query results from the SQL pane as an InlineTable.
    ///
    /// Searches tile tree for a SQL pane and retrieves results matching
    /// the given query, or the latest result if no query is specified.
    fn get_sql_result_as_inline_table(
        &self,
        query: Option<&str>,
    ) -> Option<crate::components::pane::inline_content::InlineTable> {
        use crate::components::SqlPane;
        // Find the first SQL pane in the tile tree
        for (_tile_id, tile) in self.viewport_tree.tiles.iter() {
            if let egui_tiles::Tile::Pane(component) = tile {
                if let Some(sql_pane) = component.as_any().downcast_ref::<SqlPane>() {
                    return sql_pane.get_inline_table(query);
                }
            }
        }
        None
    }

    /// Get a git diff and convert it to `InlineDiff` for display in the agent panel.
    ///
    /// If `commit` is provided, shows the diff for that commit.
    /// If `commit` is None, shows working directory changes (unstaged).
    /// If `file` is provided, filters to only show that file's diff.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_git_diff_for_inline(
        &self,
        commit: Option<&str>,
        file: Option<&str>,
    ) -> Option<InlineDiff> {
        use std::path::PathBuf;
        use std::process::Command;

        // Get repo path: prefer codebase manager index, fall back to current directory
        let repo_path = self
            .codebase_manager
            .index()
            .map(|idx| idx.repo_path.clone())
            .unwrap_or_else(|| PathBuf::from("."));

        log::info!(
            "Getting git diff: commit={:?}, file={:?}, repo_path={}",
            commit,
            file,
            repo_path.display()
        );

        // Default to HEAD if no commit specified (more useful than working directory for cloned repos)
        let effective_commit = commit.or(Some("HEAD"));

        // Build the git command based on parameters
        let diff_output = if let Some(commit_ref) = effective_commit {
            // Show diff for a specific commit
            let mut cmd = Command::new("git");
            cmd.args(["show", commit_ref, "--format=", "--unified=3", "-p"]);
            if let Some(f) = file {
                cmd.arg("--").arg(f);
            }
            cmd.current_dir(&repo_path).output().ok()?
        } else {
            // Show working directory changes (unstaged) - this branch won't be hit now
            // but kept for potential future use with explicit "working" parameter
            let mut cmd = Command::new("git");
            cmd.args(["diff", "--unified=3"]);
            if let Some(f) = file {
                cmd.arg("--").arg(f);
            }
            cmd.current_dir(&repo_path).output().ok()?
        };

        if !diff_output.status.success() {
            log::warn!(
                "git diff failed: {}",
                String::from_utf8_lossy(&diff_output.stderr)
            );
            return None;
        }

        let diff_content = String::from_utf8_lossy(&diff_output.stdout);
        if diff_content.trim().is_empty() {
            log::info!("No diff content found for commit {effective_commit:?}");
            return None;
        }

        // Get commit info for the header
        let (commit_hash, commit_message) = if let Some(commit_ref) = effective_commit {
            // Get the actual commit info
            let hash_output = Command::new("git")
                .args(["rev-parse", "--short", commit_ref])
                .current_dir(&repo_path)
                .output()
                .ok()?;
            let hash = String::from_utf8_lossy(&hash_output.stdout)
                .trim()
                .to_string();

            let msg_output = Command::new("git")
                .args(["log", "-1", "--format=%s", commit_ref])
                .current_dir(&repo_path)
                .output()
                .ok()?;
            let msg = String::from_utf8_lossy(&msg_output.stdout)
                .trim()
                .to_string();

            (hash, msg)
        } else {
            ("working".to_string(), "Uncommitted changes".to_string())
        };

        // Parse the diff into file diffs
        let file_diffs = parse_diff_to_inline_files(&diff_content);

        // Calculate totals
        let additions: usize = file_diffs.iter().map(|f| f.additions).sum();
        let deletions: usize = file_diffs.iter().map(|f| f.deletions).sum();

        Some(InlineDiff {
            commit_hash,
            commit_message,
            file_diffs,
            additions,
            deletions,
        })
    }

    /// Search the codebase using Tantivy full-text search.
    ///
    /// Returns ranked search results for metrics, alerts, and commits.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn search_codebase(
        &self,
        query: &str,
        filter: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<crate::codebase::SearchResult> {
        use crate::codebase::SearchFilter;

        // Parse filter string
        let filter_enum = filter
            .map(|s| match s.to_lowercase().as_str() {
                "metrics" => SearchFilter::Metrics,
                "alerts" => SearchFilter::Alerts,
                "commits" => SearchFilter::Commits,
                _ => SearchFilter::All,
            })
            .unwrap_or(SearchFilter::All);

        let limit = limit.unwrap_or(10).min(50);

        self.codebase_manager
            .search_ranked(query, filter_enum, limit)
    }

    /// Convert search results to inline display format.
    #[cfg(not(target_arch = "wasm32"))]
    fn convert_to_inline_search_results(
        &self,
        query: &str,
        filter: &str,
        results: Vec<crate::codebase::SearchResult>,
    ) -> InlineSearchResults {
        use crate::codebase::SearchResultKind;

        let items = results
            .into_iter()
            .map(|r| {
                let kind = match &r.kind {
                    SearchResultKind::Metric(_) => "metric".to_string(),
                    SearchResultKind::Alert { .. } => "alert".to_string(),
                    SearchResultKind::Commit { .. } => "commit".to_string(),
                };

                SearchResultItem {
                    kind,
                    name: r.name,
                    file_path: r.file.display().to_string(),
                    line: r.line,
                    score: r.score,
                    snippet: r.snippet,
                }
            })
            .collect();

        InlineSearchResults {
            query: query.to_string(),
            filter: filter.to_string(),
            results: items,
        }
    }

    // ==================== Pane Interaction Polling ====================

    /// Poll all QueryPanes for pending actions (like drilldown clicks).
    /// Call this after rendering to handle chart interactions.
    pub fn poll_pane_interactions(&mut self) {
        // Collect actions first to avoid borrow issues
        let mut drilldown_actions = Vec::new();

        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    if let Some(action) = query_pane.take_pending_action() {
                        drilldown_actions.push(action);
                    }
                }
            }
        }

        // Process collected actions
        for action in drilldown_actions {
            match action {
                QueryPaneAction::DrilldownLogs {
                    timestamp_secs,
                    metric_name,
                } => {
                    // Convert timestamp to nanoseconds and create a 5-minute window around it
                    let center_ns = (timestamp_secs * 1_000_000_000.0) as i64;
                    let window_ns = 5 * 60 * 1_000_000_000_i64; // 5 minutes in nanoseconds
                    let start_ns = center_ns - window_ns / 2;
                    let end_ns = center_ns + window_ns / 2;

                    log::info!(
                        "Opening logs pane for drilldown at {timestamp_secs} (metric: {metric_name})"
                    );

                    self.add_logs_pane(start_ns, end_ns);
                }
                QueryPaneAction::QueryChanged | QueryPaneAction::None => {
                    // These actions are handled elsewhere or are no-ops
                }
            }
        }

        // Poll SQL panes for share-to-agent actions
        self.poll_sql_pane_actions();
    }

    /// Propagate Flight SQL connection definitions from Settings to all open SQL panes.
    pub fn sync_sql_connections(
        &mut self,
        connections: &[crate::ui::settings_screen::FlightSqlConnection],
    ) {
        use crate::components::SqlPane;
        use egui_tiles::Tile;

        log::info!(
            "sync_sql_connections: {} definitions, caching",
            connections.len()
        );

        // Cache for new SQL panes created later
        self.cached_flight_sql_connections = connections.to_vec();

        let tile_ids = self.get_pane_tile_ids();
        let mut sql_count = 0;
        for tile_id in tile_ids {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(sql_pane) = component.as_any_mut().downcast_mut::<SqlPane>() {
                    sql_count += 1;
                    sql_pane.sync_connections(connections);
                }
            }
        }
        log::info!(
            "sync_sql_connections: synced {} SQL panes",
            sql_count
        );
    }

    /// Poll all SqlPanes for pending actions (like share-to-agent).
    fn poll_sql_pane_actions(&mut self) {
        use crate::components::{SqlPane, SqlPaneAction};

        let mut inline_tables = Vec::new();

        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(sql_pane) = component.as_any_mut().downcast_mut::<SqlPane>() {
                    match sql_pane.take_action() {
                        SqlPaneAction::ShareResultToAgent(table) => {
                            inline_tables.push(table);
                        }
                        SqlPaneAction::OpenSettings => {
                            self.pending_open_settings = true;
                        }
                        SqlPaneAction::None => {}
                    }
                }
            }
        }

        for table in inline_tables {
            self.inject_inline_content_to_agent_pane(InlineContent::Table(table));
            log::info!("Shared SQL result to agent panel");
        }
    }

    // ==================== Floating Panes ====================

    /// Dock all floating panes back into the tile layout.
    ///
    /// This removes all floating panes and adds them back to the tile tree.
    pub(super) fn dock_all_floating_panes(&mut self) {
        // Collect all floating pane IDs first to avoid borrow issues
        let pane_ids: Vec<_> = self.floating_panes.panes.iter().map(|p| p.id).collect();

        for pane_id in pane_ids {
            if let Some(component) = self.floating_panes.remove_pane(pane_id) {
                let pane_tile = self.viewport_tree.tiles.insert_pane(component);
                if self.add_tile_to_viewport(pane_tile) {
                    self.show_landing = false;
                }
            }
        }

        log::info!("Docked all floating panes");
    }

    /// Float the currently focused pane (detach from tile layout to floating window).
    ///
    /// This removes the pane from the tile tree and adds it to the floating pane manager.
    /// The pane appears as a draggable, resizable window above the tile layout.
    ///
    /// If `tile_rect` is provided, the floating pane will use that size. Otherwise,
    /// it will use a reasonable default.
    pub(super) fn float_focused_pane(&mut self, tile_rect: Option<egui::Rect>) {
        let focused_tile = match self.behavior.focused_tile() {
            Some(tile_id) => tile_id,
            None => {
                log::debug!("No focused pane to float");
                return;
            }
        };

        // Capture parent container info BEFORE removal for undo
        let parent_info = self.find_parent_container_info(focused_tile);
        let container_kind = parent_info.and_then(|(parent_id, _)| {
            if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(parent_id) {
                Some(container.kind())
            } else {
                None
            }
        });

        // Remove the pane from the tile tree
        if let Some(egui_tiles::Tile::Pane(component)) =
            self.viewport_tree.tiles.remove(focused_tile)
        {
            // Use a default starting position, offset by floating pane count for stacking
            let offset = (self.floating_panes.count() as f32) * 30.0;

            // Use the tile rect if provided, otherwise use defaults
            let (position, size) = if let Some(rect) = tile_rect {
                // Use the original tile's position and size
                (rect.min + egui::vec2(offset, offset), rect.size())
            } else {
                // Default position and size
                (
                    egui::pos2(100.0 + offset, 100.0 + offset),
                    egui::vec2(500.0, 350.0),
                )
            };

            // Add to floating pane manager with the determined size
            let floating_pane_id = self
                .floating_panes
                .add_pane_with_size(component, position, size);

            // Push undo action
            let floated_info = super::FloatedPaneInfo {
                floating_pane_id,
                parent_id: parent_info.map(|(id, _)| id),
                child_index: parent_info.map(|(_, idx)| idx).unwrap_or(0),
                container_kind: container_kind.unwrap_or(egui_tiles::ContainerKind::Tabs),
                was_tile_focused: true, // It was focused since we got it from focused_tile
            };
            self.undo_stack
                .push(super::UndoAction::UnfloatPane(floated_info));
            log::debug!("Pushed float pane to undo stack");

            // Clear the focus from the tile tree since we removed the tile
            self.behavior.set_focused_tile(None);

            // Clean up the tree structure (remove empty containers)
            self.viewport_tree.tiles.remove(focused_tile);

            log::info!("Floated pane from tile tree with size {size:?}");
        }
    }

    // ==================== Plugin API Methods ====================

    /// Add a logs pane from a plugin (uses current time range).
    ///
    /// This is a public wrapper for plugins to create logs panes without
    /// needing to specify the time range explicitly.
    pub fn add_logs_pane_from_plugin(&mut self) {
        let (start_ns, end_ns) = self.time_range_toolbar.get_range_ns();
        self.add_logs_pane(start_ns as i64, end_ns as i64);
    }

    /// Close the currently focused pane (public wrapper for plugins).
    pub fn close_focused_pane(&mut self) {
        if let Some(focused_tile) = self.behavior.focused_tile() {
            self.close_tile(focused_tile);
        }
    }

    /// Focus pane in a direction (public wrapper for plugins).
    ///
    /// # Arguments
    /// * `direction` - One of "left", "right", "up", "down"
    pub fn focus_pane_in_direction(&mut self, direction: &str) {
        let nav_direction = match direction.to_lowercase().as_str() {
            "left" => NavDirection::Left,
            "right" => NavDirection::Right,
            "up" => NavDirection::Up,
            "down" => NavDirection::Down,
            _ => {
                log::warn!("Invalid pane focus direction: {direction}");
                return;
            }
        };

        // Find the currently focused tile
        let Some(current_id) = self.behavior.focused_tile() else {
            log::debug!("No focused pane to navigate from");
            return;
        };

        // Find sibling in the requested direction
        if let Some(target_id) = self.find_sibling_in_direction(current_id, nav_direction) {
            self.behavior.set_focused_tile(Some(target_id));
            log::debug!("Plugin focused pane in direction: {nav_direction:?}");
        }
    }

    /// Set time range preset from a plugin.
    ///
    /// # Arguments
    /// * `preset` - One of "5m", "15m", "30m", "1h", "6h", "24h", "7d"
    pub fn set_time_range_preset_from_plugin(&mut self, preset: &str) {
        if let Some(preset_enum) = Self::parse_time_preset(preset) {
            self.time_range_toolbar.set_preset(preset_enum);
            log::info!("Plugin set time range preset: {preset}");
        } else {
            log::warn!("Plugin: unknown time range preset '{preset}'");
        }
    }

    /// Set absolute time range from a plugin.
    ///
    /// # Arguments
    /// * `start_secs` - Start time in seconds since Unix epoch
    /// * `end_secs` - End time in seconds since Unix epoch
    pub fn set_time_range_absolute_from_plugin(&mut self, start_secs: f64, end_secs: f64) {
        self.time_range_toolbar
            .set_custom_range(start_secs, end_secs);
        log::info!("Plugin set absolute time range: {start_secs} to {end_secs}");
    }

    // ==================== Plugin Custom Table Panes ====================

    /// Register a custom table pane configuration from a plugin.
    ///
    /// This stores the configuration so custom pane instances can be created later.
    pub fn register_custom_table_pane(&mut self, config: enya_plugin::CustomTableConfig) {
        log::info!(
            "Registered custom table pane '{}' from plugin '{}'",
            config.name,
            config.plugin_name
        );
        self.custom_table_configs
            .insert(config.name.clone(), config);
    }

    /// Add a custom table pane instance to the viewport.
    ///
    /// Creates a new pane using a previously registered custom table configuration.
    pub fn add_custom_table_pane(&mut self, pane_type: &str) {
        if let Some(config) = self.custom_table_configs.get(pane_type).cloned() {
            use crate::components::PluginTablePane;

            let data = self
                .custom_table_data
                .get(pane_type)
                .cloned()
                .unwrap_or_else(|| enya_plugin::CustomTableData::with_rows(Vec::new()));

            let pane: Box<dyn Component> = Box::new(PluginTablePane::new(config, data));
            let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

            if self.add_tile_to_viewport(pane_tile) {
                self.behavior.set_focused_tile(Some(pane_tile));
                self.show_landing = false;
                log::info!("Added custom table pane: {pane_type}");
            }
        } else {
            log::warn!("Unknown custom table pane type: {pane_type}");
        }
    }

    /// Update data for a custom table pane by pane ID.
    ///
    /// This is called by plugins when they have new data to display.
    /// Currently this is a placeholder - we need the PluginTablePane to support updates.
    pub fn update_custom_table_data(&mut self, pane_id: usize, data: enya_plugin::CustomTableData) {
        // For now, we'll update any PluginTablePane with the matching internal ID
        // In practice, we'll need to track pane IDs better
        log::debug!(
            "Update custom table data for pane {pane_id}: {} rows",
            data.rows.len()
        );

        // Update all panes that might match - in a full implementation we'd track by ID
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                use crate::components::PluginTablePane;
                if let Some(table_pane) = component.as_any_mut().downcast_mut::<PluginTablePane>() {
                    // Only update if this is the right pane (by internal ID or first match)
                    table_pane.set_data(data.clone());
                    return;
                }
            }
        }
    }

    /// Update data for all custom table panes of a given type.
    ///
    /// This updates the stored data and refreshes any visible panes of this type.
    pub fn update_custom_table_data_by_type(
        &mut self,
        pane_type: &str,
        data: enya_plugin::CustomTableData,
    ) {
        // Store the data for future pane instances
        self.custom_table_data
            .insert(pane_type.to_string(), data.clone());

        // Update any existing panes of this type
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                use crate::components::PluginTablePane;
                if let Some(table_pane) = component.as_any_mut().downcast_mut::<PluginTablePane>() {
                    if table_pane.pane_type() == pane_type {
                        table_pane.set_data(data.clone());
                    }
                }
            }
        }

        log::debug!(
            "Updated custom table data for type '{}': {} rows",
            pane_type,
            data.rows.len()
        );
    }

    /// Get all registered custom table pane configurations.
    ///
    /// Used by the plugins overlay to show available pane types.
    pub fn custom_table_configs(
        &self,
    ) -> &rustc_hash::FxHashMap<String, enya_plugin::CustomTableConfig> {
        &self.custom_table_configs
    }

    // ==================== Plugin Custom Chart Panes ====================

    /// Register a custom chart pane configuration from a plugin.
    ///
    /// This stores the configuration so custom chart pane instances can be created later.
    pub fn register_custom_chart_pane(&mut self, config: enya_plugin::CustomChartConfig) {
        log::info!(
            "Registered custom chart pane '{}' from plugin '{}'",
            config.name,
            config.plugin_name
        );
        self.custom_chart_configs
            .insert(config.name.clone(), config);
    }

    /// Add a custom chart pane instance to the viewport.
    ///
    /// Creates a new pane using a previously registered custom chart configuration.
    pub fn add_custom_chart_pane(&mut self, pane_type: &str) {
        if let Some(config) = self.custom_chart_configs.get(pane_type).cloned() {
            use crate::components::PluginChartPane;

            let data = self
                .custom_chart_data
                .get(pane_type)
                .cloned()
                .unwrap_or_else(|| enya_plugin::CustomChartData::with_series(Vec::new()));

            let pane: Box<dyn Component> = Box::new(PluginChartPane::new(config, data));
            let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

            if self.add_tile_to_viewport(pane_tile) {
                self.behavior.set_focused_tile(Some(pane_tile));
                self.show_landing = false;
                log::info!("Added custom chart pane: {pane_type}");
            }
        } else {
            log::warn!("Unknown custom chart pane type: {pane_type}");
        }
    }

    /// Update data for all custom chart panes of a given type.
    ///
    /// This updates the stored data and refreshes any visible panes of this type.
    pub fn update_custom_chart_data_by_type(
        &mut self,
        pane_type: &str,
        data: enya_plugin::CustomChartData,
    ) {
        // Store the data for future pane instances
        self.custom_chart_data
            .insert(pane_type.to_string(), data.clone());

        // Update any existing panes of this type
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                use crate::components::PluginChartPane;
                if let Some(chart_pane) = component.as_any_mut().downcast_mut::<PluginChartPane>() {
                    if chart_pane.pane_type() == pane_type {
                        chart_pane.set_data(data.clone());
                    }
                }
            }
        }

        log::debug!(
            "Updated custom chart data for type '{}': {} series",
            pane_type,
            data.series.len()
        );
    }

    /// Get all registered custom chart pane configurations.
    ///
    /// Used by the plugins overlay to show available pane types.
    pub fn custom_chart_configs(
        &self,
    ) -> &rustc_hash::FxHashMap<String, enya_plugin::CustomChartConfig> {
        &self.custom_chart_configs
    }

    // ==================== Custom Stat Panes ====================

    /// Register a custom stat pane type from a plugin.
    ///
    /// This stores the configuration so custom stat pane instances can be created later.
    pub fn register_custom_stat_pane(&mut self, config: enya_plugin::StatPaneConfig) {
        log::info!(
            "Registered custom stat pane '{}' from plugin '{}'",
            config.name,
            config.plugin_name
        );
        self.custom_stat_configs.insert(config.name.clone(), config);
    }

    /// Add a custom stat pane instance to the viewport.
    ///
    /// Creates a new pane using a previously registered custom stat configuration.
    pub fn add_custom_stat_pane(&mut self, pane_type: &str) {
        if let Some(config) = self.custom_stat_configs.get(pane_type).cloned() {
            use crate::components::PluginStatPane;

            let data = self
                .custom_stat_data
                .get(pane_type)
                .cloned()
                .unwrap_or_else(|| enya_plugin::StatPaneData::with_value(0.0));

            let pane: Box<dyn Component> = Box::new(PluginStatPane::new(config, data));
            let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

            if self.add_tile_to_viewport(pane_tile) {
                self.behavior.set_focused_tile(Some(pane_tile));
                self.show_landing = false;
                log::info!("Added custom stat pane of type '{pane_type}'");
            }
        } else {
            log::warn!("Unknown custom stat pane type: {pane_type}");
        }
    }

    /// Update data for all custom stat panes of a given type.
    ///
    /// This updates the stored data and refreshes any visible panes of this type.
    pub fn update_custom_stat_data_by_type(
        &mut self,
        pane_type: &str,
        data: enya_plugin::StatPaneData,
    ) {
        // Store the data for future pane instances
        self.custom_stat_data
            .insert(pane_type.to_string(), data.clone());

        // Update any existing panes of this type
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                use crate::components::PluginStatPane;
                if let Some(stat_pane) = component.as_any_mut().downcast_mut::<PluginStatPane>() {
                    if stat_pane.pane_type() == pane_type {
                        stat_pane.set_data(data.clone());
                    }
                }
            }
        }

        log::debug!(
            "Updated custom stat data for type '{}': value={}",
            pane_type,
            data.value
        );
    }

    // ==================== Custom Gauge Panes ====================

    /// Register a custom gauge pane type from a plugin.
    ///
    /// This stores the configuration so custom gauge pane instances can be created later.
    pub fn register_custom_gauge_pane(&mut self, config: enya_plugin::GaugePaneConfig) {
        log::info!(
            "Registered custom gauge pane '{}' from plugin '{}'",
            config.name,
            config.plugin_name
        );
        self.custom_gauge_configs
            .insert(config.name.clone(), config);
    }

    /// Add a custom gauge pane instance to the viewport.
    ///
    /// Creates a new pane using a previously registered custom gauge configuration.
    pub fn add_custom_gauge_pane(&mut self, pane_type: &str) {
        if let Some(config) = self.custom_gauge_configs.get(pane_type).cloned() {
            use crate::components::PluginGaugePane;

            let data = self
                .custom_gauge_data
                .get(pane_type)
                .cloned()
                .unwrap_or_else(|| enya_plugin::GaugePaneData::with_value(0.0));

            let pane: Box<dyn Component> = Box::new(PluginGaugePane::new(config, data));
            let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

            if self.add_tile_to_viewport(pane_tile) {
                self.behavior.set_focused_tile(Some(pane_tile));
                self.show_landing = false;
                log::info!("Added custom gauge pane of type '{pane_type}'");
            }
        } else {
            log::warn!("Unknown custom gauge pane type: {pane_type}");
        }
    }

    /// Update data for all custom gauge panes of a given type.
    ///
    /// This updates the stored data and refreshes any visible panes of this type.
    pub fn update_custom_gauge_data_by_type(
        &mut self,
        pane_type: &str,
        data: enya_plugin::GaugePaneData,
    ) {
        // Store the data for future pane instances
        self.custom_gauge_data
            .insert(pane_type.to_string(), data.clone());

        // Update any existing panes of this type
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                use crate::components::PluginGaugePane;
                if let Some(gauge_pane) = component.as_any_mut().downcast_mut::<PluginGaugePane>() {
                    if gauge_pane.pane_type() == pane_type {
                        gauge_pane.set_data(data.clone());
                    }
                }
            }
        }

        log::debug!(
            "Updated custom gauge data for type '{}': value={}",
            pane_type,
            data.value
        );
    }

    // ==================== Plugin Pane Refresh ====================

    /// Get plugin pane types that need to be refreshed based on their refresh intervals.
    ///
    /// Returns a list of pane type names that have exceeded their refresh interval
    /// since the last refresh.
    pub fn get_pending_plugin_refreshes(&self, refreshable_panes: &[(String, u32)]) -> Vec<String> {
        use std::time::Duration;

        let now = crate::util::Instant::now();
        let mut pending = Vec::new();

        for (pane_type, interval_secs) in refreshable_panes {
            if *interval_secs == 0 {
                continue; // No auto-refresh
            }

            // Check if we have active panes of this type (only refresh if visible)
            let has_active_pane = self.has_custom_pane_of_type(pane_type);
            if !has_active_pane {
                continue;
            }

            let should_refresh = match self.plugin_pane_last_refresh.get(pane_type) {
                Some(last) => {
                    now.duration_since(*last) >= Duration::from_secs(*interval_secs as u64)
                }
                None => true, // Never refreshed - refresh now
            };

            if should_refresh {
                pending.push(pane_type.clone());
            }
        }

        pending
    }

    /// Check if there are any custom panes of the given type currently visible.
    fn has_custom_pane_of_type(&self, pane_type: &str) -> bool {
        use crate::components::{
            PluginChartPane, PluginGaugePane, PluginStatPane, PluginTablePane,
        };

        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                // Check table panes
                if let Some(table_pane) = component.as_any().downcast_ref::<PluginTablePane>() {
                    if table_pane.pane_type() == pane_type {
                        return true;
                    }
                }
                // Check chart panes
                if let Some(chart_pane) = component.as_any().downcast_ref::<PluginChartPane>() {
                    if chart_pane.pane_type() == pane_type {
                        return true;
                    }
                }
                // Check stat panes
                if let Some(stat_pane) = component.as_any().downcast_ref::<PluginStatPane>() {
                    if stat_pane.pane_type() == pane_type {
                        return true;
                    }
                }
                // Check gauge panes
                if let Some(gauge_pane) = component.as_any().downcast_ref::<PluginGaugePane>() {
                    if gauge_pane.pane_type() == pane_type {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Mark a plugin pane type as refreshed (update its last refresh time).
    pub fn mark_plugin_pane_refreshed(&mut self, pane_type: &str) {
        self.plugin_pane_last_refresh
            .insert(pane_type.to_string(), crate::util::Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Constants Tests ====================

    #[test]
    fn test_max_tree_depth_value() {
        // Document the current value for change detection.
        // Value should be large enough for practical layouts (50+)
        // but small enough to prevent stack overflow (200 or less).
        assert_eq!(MAX_TREE_DEPTH, 100);
    }

    // ==================== Documentation Tests ====================
    //
    // The following behaviors are tested through integration tests and
    // manual testing since they require egui_tiles tree structures:
    //
    // Focus Validation (close_tile):
    // - When closing a pane, focus is set to a sibling in priority order:
    //   Right > Left > Down > Up > first remaining pane
    // - After tree mutation, focus is validated to ensure the tile exists
    // - If validation fails, focus falls back to first available pane
    //
    // Recursion Depth Guards:
    // - find_parent_tab_recursive: Returns None if depth > MAX_TREE_DEPTH
    // - find_parent_info_recursive: Returns None if depth > MAX_TREE_DEPTH
    // - collect_pane_ids: Returns partial results if depth > MAX_TREE_DEPTH
    // - All guards log warnings when triggered
    //
    // These guards prevent stack overflow on pathological tree structures
    // while allowing normal workspace layouts to function correctly.
}
