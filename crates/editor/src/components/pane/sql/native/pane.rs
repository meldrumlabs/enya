//! SQL Pane - single-cell SQL query execution using Arrow Flight SQL.
//!
//! This pane provides a SQL interface for connecting to Flight SQL servers
//! (DataFusion, DuckDB, InfluxDB, etc.) and executing queries. Features:
//! - Connections managed via Settings page
//! - Single result cell updated in place (use additional panes for multiple results)
//! - Results rendered as tables with export options
//! - Schema browser sidebar with remote table metadata
//! - Query plan visualization with `/explain` and `/analyze` commands
//! - Slash commands: `/close`, `/tables`, `/plan`, `/explain`, `/analyze`, `/demo`, etc.

use super::super::highlighting::highlight_sql;
use super::command::SqlCommand;
use super::connections::{
    ConnectionAction, ConnectionId, ConnectionSnapshot, ConnectionTreeState, SavedConnection,
    SqlBackend, TreeSelection,
};
use super::diff::{compute_table_diff, schemas_compatible};
use super::diff_rendering::{
    render_data_diff_content, render_plan_diff_content, render_profile_diff_content,
    render_schema_diff_content,
};
use super::plan_parsing::{
    create_demo_plan, create_diff_demo, create_profile_diff_demo, create_schema_diff_demo,
    parse_plan_text,
};
use super::plan_view::{PlanViewMode, PlanViewer};
use super::suggestions::{
    COLUMN_KEYWORDS, Suggestion, SuggestionIcon, SuggestionState, TABLE_KEYWORDS,
};
use super::types::{
    Cell, CellKind, CellViewState, DiffQueryResult, DiffType, QueryStatus, ResultOverlay,
    SchemaDiffResult, SqlMode, SqlPaneAction, StatusMessage,
};
use crate::components::util::id_generator::next_id_usize;
use crate::components::util::{
    render_colored_badge, render_stat_badge, render_stat_badge_with_icon,
};
use crate::components::{OverlayColors, OverlayStyle};
use crate::ui::semantic_icons::{action, category, empty, file, nav, status, time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use egui::{Color32, RichText, TextEdit, TextFormat};
use enya_datafusion::arrow::array::{Array, RecordBatch};
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::{
    BenchmarkRequest, ConnectionState, DescribeRequest, ExecutionStats, FlightClient, PlanNode,
    QueryEvent, QueryId, QueryRequest, TableInfo, format_array_value, format_duration, format_rows,
};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

/// Result of checking backend type for local-only operations.
enum LocalBackendCheck {
    Local,
    Flight,
    NotConnected,
}

/// A SQL pane with REPL-style interface.
pub struct SqlPane {
    /// Unique identifier for this pane.
    id: usize,
    /// Current theme.
    theme: AppTheme,
    /// Pane title.
    title: String,
    /// Description for the pane.
    description: String,
    /// Tokio runtime handle for async operations.
    runtime_handle: tokio::runtime::Handle,
    /// SQL backend (local or flight).
    backend: Option<SqlBackend>,
    /// Connection state for UI display (legacy, kept for compatibility).
    connection_state: ConnectionState,
    /// Connected endpoint URL (if any).
    endpoint: Option<String>,
    /// The single result cell (replaced in-place when a new query runs).
    result_cell: Option<Cell>,
    /// UI state for the result cell.
    cell_view_state: CellViewState,
    /// Transient info/error message banner.
    status_message: Option<StatusMessage>,
    /// Current input buffer.
    input: String,
    /// Whether the input is focused.
    input_focused: bool,
    /// Move cursor to end of input on next frame.
    move_cursor_to_end: bool,
    /// Scroll to bottom on next frame.
    scroll_to_bottom: bool,
    /// Show plan viewer panel (right side).
    show_plan_viewer: bool,
    /// Plan viewer for query plan visualization.
    plan_viewer: PlanViewer,
    /// Pending connection task result receiver.
    pending_connect: Option<tokio::sync::oneshot::Receiver<Result<FlightClient, String>>>,
    /// Pending query task result receiver.
    #[allow(clippy::type_complexity)]
    pending_query:
        Option<tokio::sync::oneshot::Receiver<Result<(SchemaRef, Vec<RecordBatch>), String>>>,
    /// ID of the query currently being executed via Flight.
    pending_query_id: Option<QueryId>,
    /// Abort handle for the currently running Flight SQL query task.
    pending_flight_abort: Option<tokio::task::AbortHandle>,
    /// Pending explain query result receiver.
    #[allow(clippy::type_complexity)]
    pending_explain: Option<tokio::sync::oneshot::Receiver<Result<String, String>>>,
    /// ID of connection being connected.
    pending_connect_id: Option<ConnectionId>,
    /// Whether auto-connect has been attempted (prevents re-connecting on settings re-sync).
    auto_connected: bool,
    /// Pending table fetch result receiver.
    pending_tables: Option<tokio::sync::oneshot::Receiver<Result<Vec<TableInfo>, String>>>,
    /// ID of connection for pending table fetch.
    pending_tables_id: Option<ConnectionId>,
    /// Pending diff query result receiver.
    /// Contains: query_id, left_name, right_name, is_analyze, receiver for (left_result, right_result)
    #[allow(clippy::type_complexity)]
    pending_diff: Option<(
        QueryId,
        String,   // left_name
        String,   // right_name
        DiffType, // diff type (Data, Plan, Profile)
        tokio::sync::oneshot::Receiver<(
            Result<(SchemaRef, Vec<RecordBatch>, Option<String>), String>, // left: schema, batches, plan_text
            Result<(SchemaRef, Vec<RecordBatch>, Option<String>), String>, // right: schema, batches, plan_text
        )>,
    )>,
    /// Pending schema diff result receiver.
    /// Contains: query_id, left_name, right_name, table_name, receiver for (left_columns, right_columns)
    #[allow(clippy::type_complexity)]
    pending_schema_diff: Option<(
        QueryId,
        String, // left_name
        String, // right_name
        String, // table_name
        tokio::sync::oneshot::Receiver<(
            Result<Vec<enya_datafusion::ColumnInfo>, String>, // left columns
            Result<Vec<enya_datafusion::ColumnInfo>, String>, // right columns
        )>,
    )>,
    // ========================
    // New connection management
    // ========================
    /// Saved connections list.
    connections: Vec<SavedConnection>,
    /// Connection tree sidebar state.
    tree_state: ConnectionTreeState,
    /// Connection popup visibility (0.0 = hidden, 1.0 = visible).
    /// Note: Repurposed from original sidebar_width for minimal layout.
    sidebar_width: f32,
    /// True on the frame the popup just opened — suppresses the "click outside
    /// to close" check so the pill's opening click doesn't immediately close it.
    popup_just_opened: bool,
    /// Screen-space rect of the connection pill, used to anchor the popup.
    pill_rect: egui::Rect,
    // ========================
    // Command system
    // ========================
    /// Current SQL mode (normal, diff, explain, etc.).
    mode: SqlMode,
    /// Suggestion popup state.
    suggestions: SuggestionState,
    /// Previous input for detecting changes.
    prev_input: String,
    /// Nucleo fuzzy matcher for suggestions.
    matcher: Matcher,
    // ========================
    // Result overlay system
    // ========================
    /// Currently active overlay (None = compact preview mode).
    active_overlay: ResultOverlay,
    /// Whether the result overlay is active.
    overlay_active: bool,
    /// Current page in table overlay (0-indexed).
    overlay_table_page: usize,
    /// Filter text for table overlay.
    overlay_filter: String,
    /// Whether a workspace overlay is open that should block our keyboard input.
    overlay_blocks_input: bool,
    /// Timestamp of last copy-to-clipboard action (for "Copied!" feedback).
    copied_feedback: Option<crate::util::Instant>,
    /// Pending action to deliver to the workspace on next poll.
    pending_action: SqlPaneAction,
    /// Input history for Up/Down navigation (most recent at end).
    input_history: Vec<String>,
    /// Current position in input history (None = not browsing history).
    history_index: Option<usize>,
    /// Saved current input when entering history browsing.
    history_saved_input: String,
    /// Column index to sort by in table overlay (None = original order).
    overlay_sort_column: Option<usize>,
    /// Sort direction in table overlay (true = ascending).
    overlay_sort_ascending: bool,
}

impl SqlPane {
    /// Create a new SQL pane.
    ///
    /// The runtime handle is used to spawn async operations.
    pub fn new(theme: AppTheme, runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            id: next_id_usize(),
            theme,
            title: "SQL".to_string(),
            description: "Flight SQL Client".to_string(),
            runtime_handle,
            backend: None,
            connection_state: ConnectionState::Disconnected,
            endpoint: None,
            result_cell: None,
            cell_view_state: CellViewState::default(),
            status_message: None,
            input: String::new(),
            input_focused: true,
            move_cursor_to_end: false,
            scroll_to_bottom: false,
            show_plan_viewer: false,
            plan_viewer: PlanViewer::new(theme),
            pending_connect: None,
            pending_query: None,
            pending_query_id: None,
            pending_flight_abort: None,
            pending_explain: None,
            pending_connect_id: None,
            auto_connected: false,
            pending_tables: None,
            pending_tables_id: None,
            pending_diff: None,
            pending_schema_diff: None,
            connections: Vec::new(),
            tree_state: ConnectionTreeState::default(),
            sidebar_width: 0.0, // Used as popup visibility flag (0.0 = closed, 1.0 = open)
            popup_just_opened: false,
            pill_rect: egui::Rect::NOTHING,
            mode: SqlMode::default(),
            suggestions: SuggestionState::default(),
            prev_input: String::new(),
            matcher: Matcher::new(Config::DEFAULT),
            active_overlay: ResultOverlay::None,
            overlay_active: false,
            overlay_table_page: 0,
            overlay_filter: String::new(),
            overlay_blocks_input: false,
            copied_feedback: None,
            pending_action: SqlPaneAction::None,
            input_history: Vec::new(),
            history_index: None,
            history_saved_input: String::new(),
            overlay_sort_column: None,
            overlay_sort_ascending: true,
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.plan_viewer.set_theme(theme);
    }

    /// Set whether a workspace overlay is blocking keyboard input.
    pub fn set_overlay_blocks_input(&mut self, blocks: bool) {
        self.overlay_blocks_input = blocks;
    }

    /// Disconnect from current server.
    fn disconnect(&mut self) {
        self.backend = None;
        self.connection_state = ConnectionState::Disconnected;
        self.endpoint = None;
    }

    /// Replace the single result cell in-place.
    fn set_result_cell(&mut self, cell: Cell) {
        self.status_message = None;
        self.cell_view_state = CellViewState::default();
        self.result_cell = Some(cell);
        self.scroll_to_bottom = true;
    }

    // ========================================================================
    // Connection Management (new multi-connection support)
    // ========================================================================

    /// Connect to a saved connection by ID.
    fn connect_saved(&mut self, id: ConnectionId) {
        // Check state and extract endpoint before mutating active flags.
        let endpoint = {
            let Some(conn) = self.connections.iter().find(|c| c.id == id) else {
                return;
            };
            if matches!(
                conn.state,
                ConnectionState::Connecting | ConnectionState::Connected
            ) {
                return;
            }
            conn.endpoint.clone()
        };

        // Mark this connection as active immediately so the UI (pill, run
        // button) reflects the connecting state right away.
        for conn in &mut self.connections {
            if conn.id == id {
                conn.state = ConnectionState::Connecting;
                conn.active = true;
            } else {
                conn.active = false;
            }
        }

        // Also update legacy state
        self.connection_state = ConnectionState::Connecting;
        self.endpoint = Some(endpoint.clone());
        self.pending_connect_id = Some(id);

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_connect = Some(rx);

        self.runtime_handle.spawn(async move {
            let result = FlightClient::connect(&endpoint).await;
            let _ = tx.send(result.map_err(|e| e.to_string()));
        });
    }

    /// Disconnect a saved connection by ID.
    fn disconnect_saved(&mut self, id: ConnectionId) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.id == id) {
            conn.state = ConnectionState::Disconnected;
            conn.tables.clear();

            // If this was the active connection, clear backend
            if conn.active {
                conn.active = false;
                self.backend = None;
                self.connection_state = ConnectionState::Disconnected;
                self.endpoint = None;
            }
        }
    }

    /// Set a connection as the active one (queries go through it).
    fn set_active_connection(&mut self, id: ConnectionId) {
        for conn in &mut self.connections {
            conn.active = conn.id == id;
        }

        // Update legacy endpoint tracking
        if let Some(conn) = self.connections.iter().find(|c| c.id == id) {
            self.endpoint = Some(conn.endpoint.clone());
            self.connection_state = conn.state.clone();
        }
    }

    /// Toggle expansion of a connection in the tree.
    fn toggle_connection_expanded(&mut self, id: ConnectionId) {
        if self.tree_state.expanded.contains(&id) {
            self.tree_state.expanded.remove(&id);
        } else {
            self.tree_state.expanded.insert(id);
        }
    }

    /// Get the currently active connection.
    fn active_connection(&self) -> Option<&SavedConnection> {
        self.connections.iter().find(|c| c.active)
    }

    /// Remove a saved connection by ID.
    fn remove_connection(&mut self, id: ConnectionId) {
        // Disconnect first if connected
        self.disconnect_saved(id);
        self.connections.retain(|c| c.id != id);
        // Clear selection if it was this connection
        if let Some(TreeSelection::Connection(sel_id)) = &self.tree_state.selected {
            if *sel_id == id {
                self.tree_state.selected = None;
            }
        }
        if let Some(TreeSelection::Table { connection, .. }) = &self.tree_state.selected {
            if *connection == id {
                self.tree_state.selected = None;
            }
        }
    }

    /// Handle a connection action from the connection UI.
    fn handle_connection_action(&mut self, action: ConnectionAction) {
        match action {
            ConnectionAction::Connect(id) => {
                self.connect_saved(id);
            }
            ConnectionAction::Disconnect(id) => {
                self.disconnect_saved(id);
            }
            ConnectionAction::SetActive(id) => {
                self.set_active_connection(id);
            }
            ConnectionAction::Remove(id) => {
                self.remove_connection(id);
            }
            ConnectionAction::ToggleExpanded(id) => {
                self.toggle_connection_expanded(id);
            }
            ConnectionAction::OpenSettings => {
                self.pending_action = SqlPaneAction::OpenSettings;
            }
            ConnectionAction::ClosePopup => {
                self.sidebar_width = 0.0;
            }
            ConnectionAction::TogglePlanViewer => {
                self.show_plan_viewer = !self.show_plan_viewer;
            }
            ConnectionAction::InsertTableName(table_name) => {
                if !self.input.is_empty() && !self.input.ends_with(' ') {
                    self.input.push(' ');
                }
                self.input.push_str(&table_name);
            }
        }
    }

    /// Execute the current input as a SQL query or command.
    fn execute_input(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Save to input history (dedup consecutive identical entries)
        if self.input_history.last() != Some(&input) {
            self.input_history.push(input.clone());
        }
        self.history_index = None;

        // Check for slash-commands (/diff, /explain, etc.)
        if let Some(cmd) = input.strip_prefix('/') {
            self.handle_slash_command(cmd);
            self.input.clear();
            return;
        }

        // Execute as SQL query
        self.execute_query(&input);
        self.input.clear();
        self.scroll_to_bottom = true;
    }

    /// Handle a slash-command (/diff, /explain, etc.).
    fn handle_slash_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts.first().copied().unwrap_or("");

        match command {
            "diff" => {
                // /diff [demo|analyze|schema|profile] <left> <right> <query|table>
                // Check if it's a demo request
                match parts.get(1).copied() {
                    Some("demo") => {
                        self.load_diff_demo();
                    }
                    Some("schema") => {
                        // /diff schema [demo] <left> <right> <table>
                        if parts.get(2) == Some(&"demo") {
                            self.load_schema_diff_demo();
                        } else if parts.len() < 5 {
                            self.add_info_cell(
                                "Usage: /diff schema <left> <right> <table>\n\
                                 Example: /diff schema staging prod users\n\n\
                                 Compares table schema between two connections.\n\
                                 Use /diff schema demo to preview with sample data.",
                            );
                        } else {
                            let left_name = parts[2];
                            let right_name = parts[3];
                            let table = parts[4];
                            self.execute_schema_diff(left_name, right_name, table);
                        }
                    }
                    Some("profile") => {
                        // /diff profile [demo] <left> <right> <query>
                        if parts.get(2) == Some(&"demo") {
                            self.load_profile_diff_demo();
                        } else if parts.len() < 5 {
                            self.add_info_cell(
                                "Usage: /diff profile <left> <right> <query>\n\
                                 Example: /diff profile staging prod SELECT * FROM orders\n\n\
                                 Compares EXPLAIN ANALYZE profiles with metric highlighting.\n\
                                 Use /diff profile demo to preview with sample data.",
                            );
                        } else {
                            let left_name = parts[2];
                            let right_name = parts[3];
                            let sql = parts[4..].join(" ");
                            self.execute_profile_diff(left_name, right_name, &sql);
                        }
                    }
                    Some("analyze") => {
                        // /diff analyze <left> <right> <query>
                        if parts.len() < 5 {
                            self.add_info_cell(
                                "Usage: /diff analyze <left> <right> <query>\n\
                                 Example: /diff analyze staging prod SELECT * FROM users\n\n\
                                 Compares EXPLAIN ANALYZE plans between two connections.",
                            );
                        } else {
                            let left_name = parts[2];
                            let right_name = parts[3];
                            let sql = parts[4..].join(" ");
                            self.execute_diff_query(left_name, right_name, &sql, true);
                        }
                    }
                    Some(left_name) if parts.len() >= 4 => {
                        // /diff <left> <right> <query> - inline execution
                        let right_name = parts[2];
                        let sql = parts[3..].join(" ");
                        self.execute_diff_query(left_name, right_name, &sql, false);
                    }
                    Some(left_name) if parts.len() == 3 => {
                        // /diff <left> <right> - set diff mode (legacy)
                        let right_name = parts[2];

                        // Find connections by name
                        let left_conn = self
                            .connections
                            .iter()
                            .find(|c| c.name.eq_ignore_ascii_case(left_name))
                            .map(|c| c.id);
                        let right_conn = self
                            .connections
                            .iter()
                            .find(|c| c.name.eq_ignore_ascii_case(right_name))
                            .map(|c| c.id);

                        match (left_conn, right_conn) {
                            (Some(left), Some(right)) => {
                                self.mode = SqlMode::Diff { left, right };
                                self.add_info_cell(&format!(
                                    "Diff mode: {left_name} ↔ {right_name}\nEnter a query to compare results.\n\n\
                                     Tip: Use inline syntax: /diff {left_name} {right_name} SELECT * FROM table"
                                ));
                            }
                            (None, _) => {
                                self.add_error_cell(&format!("Connection not found: {left_name}"));
                            }
                            (_, None) => {
                                self.add_error_cell(&format!("Connection not found: {right_name}"));
                            }
                        }
                    }
                    _ => {
                        self.add_info_cell(
                            "Usage:\n\
                             /diff <left> <right> <query>           Compare query results\n\
                             /diff analyze <left> <right> <query>   Compare EXPLAIN ANALYZE plans\n\
                             /diff schema <left> <right> <table>    Compare table schemas\n\
                             /diff profile <left> <right> <query>   Compare execution profiles\n\
                             /diff demo                             Show demo data diff\n\
                             /diff schema demo                      Show demo schema diff\n\
                             /diff profile demo                     Show demo profile diff\n\n\
                             Example: /diff staging prod SELECT * FROM users LIMIT 10",
                        );
                    }
                }
            }
            "explain" => {
                // /explain <query> - run EXPLAIN on the query
                let sql = parts[1..].join(" ");
                if sql.is_empty() {
                    self.add_info_cell(
                        "Usage: /explain <query>\nExample: /explain SELECT * FROM users",
                    );
                } else {
                    self.execute_explain(&sql, false);
                }
            }
            "analyze" => {
                // /analyze <query> - run EXPLAIN ANALYZE on the query
                let sql = parts[1..].join(" ");
                if sql.is_empty() {
                    self.add_info_cell(
                        "Usage: /analyze <query>\nExample: /analyze SELECT * FROM users",
                    );
                } else {
                    self.execute_explain(&sql, true);
                }
            }
            "bench" => {
                // /bench [N] <query> - benchmark query over N iterations
                let rest = parts[1..].join(" ");
                if rest.is_empty() {
                    self.add_info_cell(
                        "Usage: /bench [iterations] <query>\n\
                         Example: /bench SELECT * FROM users\n\
                         Example: /bench 20 SELECT * FROM users\n\n\
                         Runs the query N times (default: 10) and shows min/median/max timings.\n\
                         Local DataFusion sessions only.",
                    );
                } else {
                    // Check if first arg is a number (iteration count)
                    let (iterations, sql) = match parts.get(1).and_then(|s| s.parse::<usize>().ok())
                    {
                        Some(n) if n > 0 && parts.len() > 2 => (n, parts[2..].join(" ")),
                        _ => (10, rest),
                    };
                    if sql.is_empty() {
                        self.add_info_cell(
                            "Usage: /bench [iterations] <query>\n\
                             Provide a SQL query to benchmark.",
                        );
                    } else {
                        self.execute_benchmark(&sql, iterations);
                    }
                }
            }
            "describe" => {
                if let Some(table_name) = parts.get(1) {
                    self.execute_describe(table_name);
                } else {
                    self.add_info_cell(
                        "Usage: /describe <table>\n\
                         Example: /describe users\n\
                         Example: /describe public.orders\n\n\
                         Shows per-column statistics: count, nulls, distinct, min, max, mean.",
                    );
                }
            }
            "demo" => {
                // Load a demo plan for testing the visualization
                self.load_demo_plan();
                // Add a placeholder result so we can open the overlay
                self.set_result_cell(Cell::explain(
                    "-- Demo Query Plan",
                    enya_datafusion::QueryId::new(),
                ));
            }
            "schema" => {
                if let Some(table_name) = parts.get(1) {
                    // Show table schema
                    self.execute_query(&format!("DESCRIBE {table_name}"));
                } else {
                    self.add_info_cell("Usage: /schema <table-name>\nExample: /schema users");
                }
            }
            "close" => {
                self.disconnect();
                self.add_info_cell("Disconnected");
            }
            "normal" | "reset" => {
                // Reset to normal mode
                self.mode = SqlMode::Normal;
                self.add_info_cell("Switched to normal SQL mode.");
            }
            _ => {
                self.add_error_cell(&format!(
                    "Unknown command: /{command}\nType / to see available commands."
                ));
            }
        }
        self.scroll_to_bottom = true;
    }

    /// Execute an EXPLAIN or EXPLAIN ANALYZE query.
    fn execute_explain(&mut self, sql: &str, analyze: bool) {
        let explain_sql = if analyze {
            format!("EXPLAIN ANALYZE {sql}")
        } else {
            format!("EXPLAIN {sql}")
        };

        self.add_info_cell(if analyze {
            "Running EXPLAIN ANALYZE..."
        } else {
            "Running EXPLAIN..."
        });

        // Execute via Flight and parse the result as a plan
        if let Some(endpoint) = self.endpoint.clone() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.pending_explain = Some(rx);

            self.runtime_handle.spawn(async move {
                let result = async {
                    let mut client = FlightClient::connect(&endpoint)
                        .await
                        .map_err(|e| e.to_string())?;
                    let mut stream = client
                        .execute(&explain_sql)
                        .await
                        .map_err(|e| e.to_string())?;
                    let batches = stream.collect().await.map_err(|e| e.to_string())?;

                    // Extract plan text from result batches
                    let mut plan_text = String::new();
                    for batch in batches {
                        for col_idx in 0..batch.num_columns() {
                            let col = batch.column(col_idx);
                            if let Some(arr) =
                                col.as_any()
                                    .downcast_ref::<enya_datafusion::arrow::array::StringArray>()
                            {
                                for i in 0..arr.len() {
                                    if !arr.is_null(i) {
                                        plan_text.push_str(arr.value(i));
                                        plan_text.push('\n');
                                    }
                                }
                            }
                        }
                    }

                    Ok::<_, String>(plan_text)
                }
                .await;
                let _ = tx.send(result);
            });
        } else {
            self.add_error_cell("Not connected. Configure connections in Settings.");
        }
    }

    /// Check the local backend type (avoids E0502 borrow conflicts).
    fn check_local_backend(&self) -> LocalBackendCheck {
        match &self.backend {
            Some(SqlBackend::Local { .. }) => LocalBackendCheck::Local,
            Some(SqlBackend::Flight { .. }) => LocalBackendCheck::Flight,
            _ => LocalBackendCheck::NotConnected,
        }
    }

    /// Execute a benchmark on the local backend.
    fn execute_benchmark(&mut self, sql: &str, iterations: usize) {
        match self.check_local_backend() {
            LocalBackendCheck::Local => {
                let query_id = QueryId::new();
                let display = format!("/bench {iterations} {sql}");
                self.set_result_cell(Cell::benchmark(display, query_id));

                let request = BenchmarkRequest::new(sql)
                    .with_id(query_id)
                    .with_iterations(iterations);

                if let Some(SqlBackend::Local { session, .. }) = &self.backend {
                    if let Err(e) = session.benchmark(request) {
                        if let Some(cell) = self.result_cell.as_mut() {
                            cell.set_status(QueryStatus::Failed);
                            cell.set_error(e.to_string());
                        }
                    }
                }
            }
            LocalBackendCheck::Flight => {
                self.add_error_cell("Benchmarking is only supported on local DataFusion sessions.");
            }
            LocalBackendCheck::NotConnected => {
                self.add_error_cell("Not connected. Configure connections in Settings.");
            }
        }
    }

    /// Execute a describe on a table (local backend only).
    fn execute_describe(&mut self, table_name: &str) {
        match self.check_local_backend() {
            LocalBackendCheck::Local => {
                let query_id = QueryId::new();
                let display = format!("/describe {table_name}");
                self.set_result_cell(Cell::describe(display, query_id));

                let request = DescribeRequest::new(table_name).with_id(query_id);

                if let Some(SqlBackend::Local { session, .. }) = &self.backend {
                    if let Err(e) = session.describe(request) {
                        if let Some(cell) = self.result_cell.as_mut() {
                            cell.set_status(QueryStatus::Failed);
                            cell.set_error(e.to_string());
                        }
                    }
                }
            }
            LocalBackendCheck::Flight => {
                self.add_error_cell("Describe is only supported on local DataFusion sessions.");
            }
            LocalBackendCheck::NotConnected => {
                self.add_error_cell("Not connected. Configure connections in Settings.");
            }
        }
    }

    /// Execute a SQL query.
    fn execute_query(&mut self, sql: &str) {
        let query_id = QueryId::new();

        // Replace result cell
        self.set_result_cell(Cell::query(sql, query_id));

        match &mut self.backend {
            Some(SqlBackend::Flight { .. }) => {
                let sql = sql.to_string();
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.pending_query = Some(rx);
                self.pending_query_id = Some(query_id);

                if let Some(endpoint) = self.endpoint.clone() {
                    let join_handle = self.runtime_handle.spawn(async move {
                        let result = async {
                            let mut client = FlightClient::connect(&endpoint)
                                .await
                                .map_err(|e| e.to_string())?;
                            let mut stream =
                                client.execute(&sql).await.map_err(|e| e.to_string())?;
                            let schema = stream.schema();
                            let batches = stream.collect().await.map_err(|e| e.to_string())?;
                            Ok::<_, String>((schema, batches))
                        }
                        .await;
                        let _ = tx.send(result);
                    });
                    self.pending_flight_abort = Some(join_handle.abort_handle());
                }
            }
            Some(SqlBackend::Local { session, .. }) => {
                let request = QueryRequest::new(sql).with_id(query_id);
                if let Err(e) = session.execute(request) {
                    if let Some(cell) = self.result_cell.as_mut() {
                        cell.set_status(QueryStatus::Failed);
                        cell.set_error(e.to_string());
                    }
                }
            }
            None => {
                if let Some(cell) = self.result_cell.as_mut() {
                    cell.set_status(QueryStatus::Failed);
                    cell.set_error("Not connected. Configure connections in Settings.".to_string());
                }
            }
        }
    }

    /// Cancel the currently running query.
    fn cancel_query(&mut self) {
        // Only cancel if there's a running cell
        let query_id = match &self.result_cell {
            Some(cell) if cell.status() == QueryStatus::Running => cell.id(),
            _ => return,
        };

        match &self.backend {
            Some(SqlBackend::Local { session, .. }) => {
                if let Err(e) = session.cancel(query_id) {
                    log::warn!("Failed to cancel query: {e}");
                }
                // Executor will send QueryEvent::Cancelled via the event channel
            }
            Some(SqlBackend::Flight { .. }) => {
                // Abort the spawned tokio task
                if let Some(abort_handle) = self.pending_flight_abort.take() {
                    abort_handle.abort();
                }
                // Manually set cancelled since Flight has no event channel
                if let Some(cell) = self.result_cell.as_mut() {
                    cell.set_status(QueryStatus::Cancelled);
                }
                self.pending_query = None;
                self.pending_query_id = None;
            }
            None => {}
        }
    }

    /// Execute a diff query comparing results between two connections.
    /// If is_analyze is true, compare EXPLAIN ANALYZE plans instead of data.
    fn execute_diff_query(
        &mut self,
        left_name: &str,
        right_name: &str,
        sql: &str,
        is_analyze: bool,
    ) {
        let diff_type = if is_analyze {
            DiffType::Plan
        } else {
            DiffType::Data
        };
        self.execute_diff_query_with_type(left_name, right_name, sql, diff_type);
    }

    /// Execute a diff query with explicit diff type.
    fn execute_diff_query_with_type(
        &mut self,
        left_name: &str,
        right_name: &str,
        sql: &str,
        diff_type: DiffType,
    ) {
        // Find both connections by name
        let left_conn = self
            .connections
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(left_name));
        let right_conn = self
            .connections
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(right_name));

        // Validate connections exist and are connected
        let (left_endpoint, left_name_owned) = match left_conn {
            Some(c) if matches!(c.state, ConnectionState::Connected) => {
                (c.endpoint.clone(), c.name.clone())
            }
            Some(c) => {
                self.add_error_cell(&format!(
                    "Connection '{}' is not connected. Connect via the sidebar.",
                    c.name
                ));
                return;
            }
            None => {
                self.add_error_cell(&format!("Connection not found: {left_name}"));
                return;
            }
        };

        let (right_endpoint, right_name_owned) = match right_conn {
            Some(c) if matches!(c.state, ConnectionState::Connected) => {
                (c.endpoint.clone(), c.name.clone())
            }
            Some(c) => {
                self.add_error_cell(&format!(
                    "Connection '{}' is not connected. Connect via the sidebar.",
                    c.name
                ));
                return;
            }
            None => {
                self.add_error_cell(&format!("Connection not found: {right_name}"));
                return;
            }
        };

        let query_id = QueryId::new();
        let is_analyze = matches!(diff_type, DiffType::Plan | DiffType::Profile);
        let display_sql = match diff_type {
            DiffType::Plan => format!("/diff analyze {left_name} {right_name} {sql}"),
            DiffType::Profile => format!("/diff profile {left_name} {right_name} {sql}"),
            _ => format!("/diff {left_name} {right_name} {sql}"),
        };

        // Replace result cell with Running status
        self.set_result_cell(Cell::diff(display_sql, query_id));

        // Spawn async task to run both queries
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_diff = Some((
            query_id,
            left_name_owned.clone(),
            right_name_owned.clone(),
            diff_type,
            rx,
        ));

        let sql_owned = sql.to_string();
        let runtime = self.runtime_handle.clone();

        runtime.spawn(async move {
            // Prepare the SQL - wrap in EXPLAIN ANALYZE if needed
            let execute_sql = if is_analyze {
                format!("EXPLAIN ANALYZE {sql_owned}")
            } else {
                sql_owned.clone()
            };

            // Run both queries concurrently
            let (left_result, right_result) = tokio::join!(
                Self::run_diff_query(&left_endpoint, &execute_sql, is_analyze),
                Self::run_diff_query(&right_endpoint, &execute_sql, is_analyze),
            );

            let _ = tx.send((left_result, right_result));
        });

        self.scroll_to_bottom = true;
    }

    /// Execute a profile diff (EXPLAIN ANALYZE with metric highlighting).
    fn execute_profile_diff(&mut self, left_name: &str, right_name: &str, sql: &str) {
        self.execute_diff_query_with_type(left_name, right_name, sql, DiffType::Profile);
    }

    /// Helper function to run a query on a specific endpoint for diff comparison.
    async fn run_diff_query(
        endpoint: &str,
        sql: &str,
        is_analyze: bool,
    ) -> Result<(SchemaRef, Vec<RecordBatch>, Option<String>), String> {
        let mut client = FlightClient::connect(endpoint)
            .await
            .map_err(|e| e.to_string())?;

        let mut stream = client.execute(sql).await.map_err(|e| e.to_string())?;

        let schema = stream.schema();
        let batches = stream.collect().await.map_err(|e| e.to_string())?;

        // If this is an analyze query, extract the plan text from the result
        let plan_text = if is_analyze {
            // EXPLAIN ANALYZE typically returns a single column with the plan text
            if let Some(batch) = batches.first() {
                if batch.num_columns() > 0 {
                    use enya_datafusion::arrow::array::StringArray;
                    if let Some(arr) = batch.column(0).as_any().downcast_ref::<StringArray>() {
                        let mut plan_lines = Vec::new();
                        for i in 0..arr.len() {
                            if !arr.is_null(i) {
                                plan_lines.push(arr.value(i).to_string());
                            }
                        }
                        Some(plan_lines.join("\n"))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok((schema, batches, plan_text))
    }

    /// Execute a schema diff between two connections.
    fn execute_schema_diff(&mut self, left_name: &str, right_name: &str, table: &str) {
        // Find both connections by name
        let left_conn = self
            .connections
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(left_name));
        let right_conn = self
            .connections
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(right_name));

        // Validate connections exist and are connected
        let (left_endpoint, left_name_owned) = match left_conn {
            Some(c) if matches!(c.state, ConnectionState::Connected) => {
                (c.endpoint.clone(), c.name.clone())
            }
            Some(c) => {
                self.add_error_cell(&format!(
                    "Connection '{}' is not connected. Connect via the sidebar.",
                    c.name
                ));
                return;
            }
            None => {
                self.add_error_cell(&format!("Connection not found: {left_name}"));
                return;
            }
        };

        let (right_endpoint, right_name_owned) = match right_conn {
            Some(c) if matches!(c.state, ConnectionState::Connected) => {
                (c.endpoint.clone(), c.name.clone())
            }
            Some(c) => {
                self.add_error_cell(&format!(
                    "Connection '{}' is not connected. Connect via the sidebar.",
                    c.name
                ));
                return;
            }
            None => {
                self.add_error_cell(&format!("Connection not found: {right_name}"));
                return;
            }
        };

        let query_id = QueryId::new();
        let display_sql = format!("/diff schema {left_name} {right_name} {table}");
        let table_owned = table.to_string();

        // Replace result cell with Running status
        self.set_result_cell(Cell::diff(display_sql, query_id));

        // Spawn async task to fetch schemas from both connections
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_schema_diff = Some((
            query_id,
            left_name_owned.clone(),
            right_name_owned.clone(),
            table_owned.clone(),
            rx,
        ));

        let runtime = self.runtime_handle.clone();

        runtime.spawn(async move {
            // Run both schema fetches concurrently
            let (left_result, right_result) = tokio::join!(
                Self::fetch_table_schema(&left_endpoint, &table_owned),
                Self::fetch_table_schema(&right_endpoint, &table_owned),
            );

            let _ = tx.send((left_result, right_result));
        });

        self.scroll_to_bottom = true;
    }

    /// Helper function to fetch table schema from an endpoint.
    async fn fetch_table_schema(
        endpoint: &str,
        table: &str,
    ) -> Result<Vec<enya_datafusion::ColumnInfo>, String> {
        let mut client = FlightClient::connect(endpoint)
            .await
            .map_err(|e| e.to_string())?;

        client
            .get_columns(None, None, table)
            .await
            .map_err(|e| e.to_string())
    }

    /// Show an error status banner (does not replace the result cell).
    fn add_error_cell(&mut self, message: &str) {
        self.status_message = Some(StatusMessage {
            text: message.to_string(),
            is_error: true,
        });
    }

    /// Show an info status banner (does not replace the result cell).
    fn add_info_cell(&mut self, message: &str) {
        self.status_message = Some(StatusMessage {
            text: message.to_string(),
            is_error: false,
        });
    }

    /// Show an info status banner (alias for add_info_cell).
    fn set_status_info(&mut self, message: &str) {
        self.add_info_cell(message);
    }

    /// Whether the result cell exists and is navigable.
    fn has_result(&self) -> bool {
        self.result_cell.as_ref().is_some_and(|c| c.is_navigable())
    }

    /// Process card actions from the query card renderer.
    fn handle_card_actions(&mut self, actions: Vec<super::query_card::CardAction>) {
        use super::query_card::CardAction;
        for action in actions {
            match action {
                CardAction::Collapse => {
                    // Refocus input bar
                    self.input_focused = true;
                }
                CardAction::CopyToClipboard(text) => {
                    self.copy_to_clipboard(&text);
                }
                CardAction::ShareToAgent => {
                    if let Some(table) = self.result_to_inline_table() {
                        self.pending_action = SqlPaneAction::ShareResultToAgent(table);
                    }
                }
                CardAction::Delete => {
                    self.result_cell = None;
                    self.cell_view_state = CellViewState::default();
                    self.input_focused = true;
                }
                CardAction::ExpandTable => {
                    self.open_overlay(ResultOverlay::Table);
                }
                CardAction::Cancel => {
                    self.cancel_query();
                }
                CardAction::NextPage => {
                    let total_rows: usize = self
                        .result_cell
                        .as_ref()
                        .map(|c| c.batches().iter().map(|b| b.num_rows()).sum())
                        .unwrap_or(0);
                    let total_pages = total_rows.div_ceil(super::query_card::ROWS_PER_PAGE).max(1);
                    if self.cell_view_state.table_page < total_pages - 1 {
                        self.cell_view_state.table_page += 1;
                    }
                }
                CardAction::PrevPage => {
                    if self.cell_view_state.table_page > 0 {
                        self.cell_view_state.table_page -= 1;
                    }
                }
            }
        }
    }

    /// Poll for async operation results.
    fn poll_async(&mut self) {
        // Poll pending connection
        if let Some(mut rx) = self.pending_connect.take() {
            match rx.try_recv() {
                Ok(Ok(client)) => {
                    // Connection successful
                    self.connection_state = ConnectionState::Connected;
                    self.backend = Some(SqlBackend::Flight {
                        client: Box::new(client),
                        tables: Vec::new(),
                    });

                    // Update saved connection state if this was a saved connection
                    if let Some(conn_id) = self.pending_connect_id.take() {
                        // Get the connection name first
                        let conn_name = self
                            .connections
                            .iter()
                            .find(|c| c.id == conn_id)
                            .map(|c| c.name.clone());

                        // Update all connections
                        for conn in &mut self.connections {
                            if conn.id == conn_id {
                                conn.state = ConnectionState::Connected;
                                conn.active = true;
                            } else {
                                conn.active = false;
                            }
                        }

                        if let Some(name) = &conn_name {
                            log::info!("Connected to {name}");
                        }
                        self.tree_state.expanded.insert(conn_id);
                    }
                    // Fetch tables in background
                    self.fetch_tables();
                }
                Ok(Err(e)) => {
                    // Connection failed
                    self.connection_state = ConnectionState::Failed(e.clone());

                    // Update saved connection state
                    if let Some(conn_id) = self.pending_connect_id.take() {
                        // Get the connection name first
                        let conn_name = self
                            .connections
                            .iter()
                            .find(|c| c.id == conn_id)
                            .map(|c| c.name.clone());

                        // Update the connection state
                        if let Some(conn) = self.connections.iter_mut().find(|c| c.id == conn_id) {
                            conn.state = ConnectionState::Failed(e.clone());
                        }

                        // Add error message
                        if let Some(name) = conn_name {
                            self.add_error_cell(&format!("Failed to connect to {name}: {e}"));
                        }
                    } else {
                        self.add_error_cell(&format!("Connection failed: {e}"));
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still connecting
                    self.pending_connect = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Connection task dropped
                    self.connection_state =
                        ConnectionState::Failed("Connection task dropped".to_string());
                    self.pending_connect_id = None;
                }
            }
        }

        // Poll pending query
        if let Some(mut rx) = self.pending_query.take() {
            match rx.try_recv() {
                Ok(Ok((schema, batches))) => {
                    // Query successful
                    self.pending_flight_abort = None;
                    if let Some(query_id) = self.pending_query_id.take() {
                        if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id)
                        {
                            if let Some(q) = cell.as_query_mut() {
                                let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();
                                q.status = QueryStatus::Completed;
                                q.schema = Some(schema);
                                q.batches = batches;
                                q.stats = Some(ExecutionStats {
                                    rows_returned: row_count,
                                    ..Default::default()
                                });
                            }
                        }
                        self.scroll_to_bottom = true;
                    }
                }
                Ok(Err(e)) => {
                    // Query failed
                    self.pending_flight_abort = None;
                    if let Some(query_id) = self.pending_query_id.take() {
                        if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id)
                        {
                            cell.set_status(QueryStatus::Failed);
                            cell.set_error(e);
                        }
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running
                    self.pending_query = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Query task dropped (possibly cancelled)
                    self.pending_flight_abort = None;
                    if let Some(query_id) = self.pending_query_id.take() {
                        if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id)
                        {
                            // If already cancelled, don't overwrite with Failed
                            if cell.status() != QueryStatus::Cancelled {
                                cell.set_status(QueryStatus::Failed);
                                cell.set_error("Query task dropped".to_string());
                            }
                        }
                    }
                }
            }
        }

        // Poll pending explain
        if let Some(mut rx) = self.pending_explain.take() {
            match rx.try_recv() {
                Ok(Ok(plan_text)) => {
                    // Parse plan text and load into viewer
                    let plan = parse_plan_text(&plan_text);
                    self.plan_viewer.load_plan(&plan);
                    self.show_plan_viewer = true;

                    // Replace result cell with explain cell
                    let explain_id = enya_datafusion::QueryId::new();
                    self.set_result_cell(Cell::explain(
                        format!("EXPLAIN {}", plan_text.lines().next().unwrap_or("...")),
                        explain_id,
                    ));
                }
                Ok(Err(e)) => {
                    self.add_error_cell(&format!("EXPLAIN failed: {e}"));
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running
                    self.pending_explain = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    self.add_error_cell("EXPLAIN task dropped");
                }
            }
        }

        // Poll pending table fetch
        if let Some(mut rx) = self.pending_tables.take() {
            match rx.try_recv() {
                Ok(Ok(tables)) => {
                    // Store tables in the active connection
                    if let Some(conn_id) = self.pending_tables_id.take() {
                        if let Some(conn) = self.connections.iter_mut().find(|c| c.id == conn_id) {
                            let table_count = tables.len();
                            conn.tables = tables;
                            log::debug!(
                                "Fetched {} tables for connection '{}'",
                                table_count,
                                conn.name
                            );
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Table fetch failed - log but don't show error to user
                    log::warn!("Failed to fetch tables: {e}");
                    self.pending_tables_id = None;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still fetching
                    self.pending_tables = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Task dropped
                    self.pending_tables_id = None;
                }
            }
        }

        // Poll pending diff query
        if let Some((query_id, left_name, right_name, diff_type, mut rx)) = self.pending_diff.take()
        {
            match rx.try_recv() {
                Ok((left_result, right_result)) => {
                    let is_analyze = matches!(diff_type, DiffType::Plan | DiffType::Profile);

                    // Build the diff result
                    let mut diff_result = DiffQueryResult {
                        left_name: left_name.clone(),
                        right_name: right_name.clone(),
                        left_schema: None,
                        left_batches: Vec::new(),
                        left_error: None,
                        right_schema: None,
                        right_batches: Vec::new(),
                        right_error: None,
                        schemas_match: false,
                        diff_stats: None,
                        left_plan: None,
                        right_plan: None,
                        diff_type: diff_type.clone(),
                        schema_diff: None,
                    };

                    // Process left result
                    match left_result {
                        Ok((schema, batches, plan_text)) => {
                            diff_result.left_schema = Some(schema);
                            diff_result.left_batches = batches;
                            if is_analyze {
                                if let Some(text) = plan_text {
                                    diff_result.left_plan = Some(parse_plan_text(&text));
                                }
                            }
                        }
                        Err(e) => {
                            diff_result.left_error = Some(e);
                        }
                    }

                    // Process right result
                    match right_result {
                        Ok((schema, batches, plan_text)) => {
                            diff_result.right_schema = Some(schema);
                            diff_result.right_batches = batches;
                            if is_analyze {
                                if let Some(text) = plan_text {
                                    diff_result.right_plan = Some(parse_plan_text(&text));
                                }
                            }
                        }
                        Err(e) => {
                            diff_result.right_error = Some(e);
                        }
                    }

                    // Check schema compatibility and compute diff stats if both succeeded
                    if let (Some(left_schema), Some(right_schema)) =
                        (&diff_result.left_schema, &diff_result.right_schema)
                    {
                        diff_result.schemas_match = schemas_compatible(left_schema, right_schema);

                        if diff_result.schemas_match && !is_analyze {
                            diff_result.diff_stats = Some(compute_table_diff(
                                &diff_result.left_batches,
                                &diff_result.right_batches,
                            ));
                        }
                    }

                    // Determine final status
                    let has_error =
                        diff_result.left_error.is_some() || diff_result.right_error.is_some();

                    // Update the cell
                    if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id) {
                        if let Some(d) = cell.as_diff_mut() {
                            if has_error {
                                d.status = QueryStatus::Failed;
                                let mut errors = Vec::new();
                                if let Some(e) = &diff_result.left_error {
                                    errors.push(format!("{left_name}: {e}"));
                                }
                                if let Some(e) = &diff_result.right_error {
                                    errors.push(format!("{right_name}: {e}"));
                                }
                                d.error = Some(errors.join("\n"));
                            } else {
                                d.status = QueryStatus::Completed;
                            }
                            d.diff_result = Some(diff_result);
                        }
                    }

                    // Open the diff overlay
                    if self
                        .result_cell
                        .as_ref()
                        .is_some_and(|c| c.id() == query_id)
                    {
                        self.open_overlay(ResultOverlay::Diff { other_idx: 0 });
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running
                    self.pending_diff = Some((query_id, left_name, right_name, diff_type, rx));
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Task dropped
                    if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id) {
                        cell.set_status(QueryStatus::Failed);
                        cell.set_error("Diff query task dropped".to_string());
                    }
                }
            }
        }

        // Poll pending schema diff
        if let Some((query_id, left_name, right_name, table_name, mut rx)) =
            self.pending_schema_diff.take()
        {
            match rx.try_recv() {
                Ok((left_result, right_result)) => {
                    // Build the schema diff result
                    let mut diff_result = DiffQueryResult {
                        left_name: left_name.clone(),
                        right_name: right_name.clone(),
                        left_schema: None,
                        left_batches: Vec::new(),
                        left_error: None,
                        right_schema: None,
                        right_batches: Vec::new(),
                        right_error: None,
                        schemas_match: true,
                        diff_stats: None,
                        left_plan: None,
                        right_plan: None,
                        diff_type: DiffType::Schema,
                        schema_diff: None,
                    };

                    // Process results and build schema diff
                    match (&left_result, &right_result) {
                        (Ok(left_cols), Ok(right_cols)) => {
                            let schema_diff =
                                SchemaDiffResult::from_columns(&table_name, left_cols, right_cols);
                            diff_result.schema_diff = Some(schema_diff);
                        }
                        (Err(e), _) => {
                            diff_result.left_error = Some(e.clone());
                        }
                        (_, Err(e)) => {
                            diff_result.right_error = Some(e.clone());
                        }
                    }

                    // Determine final status
                    let has_error =
                        diff_result.left_error.is_some() || diff_result.right_error.is_some();

                    // Update the cell
                    if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id) {
                        if let Some(d) = cell.as_diff_mut() {
                            if has_error {
                                d.status = QueryStatus::Failed;
                                let mut errors = Vec::new();
                                if let Some(e) = &diff_result.left_error {
                                    errors.push(format!("{left_name}: {e}"));
                                }
                                if let Some(e) = &diff_result.right_error {
                                    errors.push(format!("{right_name}: {e}"));
                                }
                                d.error = Some(errors.join("\n"));
                            } else {
                                d.status = QueryStatus::Completed;
                            }
                            d.diff_result = Some(diff_result);
                        }
                    }

                    // Open the diff overlay
                    if self
                        .result_cell
                        .as_ref()
                        .is_some_and(|c| c.id() == query_id)
                    {
                        self.open_overlay(ResultOverlay::Diff { other_idx: 0 });
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running
                    self.pending_schema_diff =
                        Some((query_id, left_name, right_name, table_name, rx));
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Task dropped
                    if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id) {
                        cell.set_status(QueryStatus::Failed);
                        cell.set_error("Schema diff task dropped".to_string());
                    }
                }
            }
        }

        // Poll local session events
        if let Some(SqlBackend::Local { event_rx, .. }) = &mut self.backend {
            while let Ok(event) = event_rx.try_recv() {
                let query_id = event.query_id();
                if let Some(cell) = self.result_cell.as_mut().filter(|c| c.id() == query_id) {
                    if let Some(q) = cell.as_query_mut() {
                        match event {
                            QueryEvent::Started { schema, .. } => {
                                q.schema = Some(schema);
                            }
                            QueryEvent::Batch { batch, .. } => {
                                q.batches.push(batch);
                            }
                            QueryEvent::Completed { stats, .. } => {
                                q.status = QueryStatus::Completed;
                                q.stats = Some(stats);
                            }
                            QueryEvent::Failed { error, .. } => {
                                q.status = QueryStatus::Failed;
                                q.error = Some(error);
                            }
                            QueryEvent::Cancelled { .. } => {
                                q.status = QueryStatus::Cancelled;
                            }
                            QueryEvent::Progress { .. } => {}
                            _ => {} // Benchmark events not applicable to query cells
                        }
                    } else if let Some(b) = cell.as_benchmark_mut() {
                        match event {
                            QueryEvent::BenchmarkProgress {
                                iteration,
                                total_iterations,
                                last_duration,
                                ..
                            } => {
                                b.progress = Some((iteration, total_iterations));
                                b.last_duration = Some(last_duration);
                            }
                            QueryEvent::BenchmarkCompleted { stats, .. } => {
                                b.status = QueryStatus::Completed;
                                b.stats = Some(*stats);
                            }
                            QueryEvent::Failed { error, .. } => {
                                b.status = QueryStatus::Failed;
                                b.error = Some(error);
                            }
                            QueryEvent::Cancelled { .. } => {
                                b.status = QueryStatus::Cancelled;
                            }
                            _ => {}
                        }
                    } else if let Some(d) = cell.as_describe_mut() {
                        match event {
                            QueryEvent::DescribeCompleted { stats, .. } => {
                                d.status = QueryStatus::Completed;
                                d.stats = Some(*stats);
                            }
                            QueryEvent::Failed { error, .. } => {
                                d.status = QueryStatus::Failed;
                                d.error = Some(error);
                            }
                            QueryEvent::Cancelled { .. } => {
                                d.status = QueryStatus::Cancelled;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    /// Load a demo query plan for testing the visualization.
    fn load_demo_plan(&mut self) {
        let plan = create_demo_plan();
        self.plan_viewer.load_plan(&plan);
        self.show_plan_viewer = true;
    }

    /// Load a demo diff result for testing the diff overlay.
    fn load_diff_demo(&mut self) {
        let diff_result = create_diff_demo();
        let query_id = QueryId::new();
        self.set_result_cell(Cell::diff_completed(
            "/diff demo (staging vs production)",
            query_id,
            diff_result,
        ));
        self.open_overlay(ResultOverlay::Diff { other_idx: 0 });
    }

    /// Load a demo schema diff result for testing the schema diff overlay.
    fn load_schema_diff_demo(&mut self) {
        let diff_result = create_schema_diff_demo();
        let query_id = QueryId::new();
        self.set_result_cell(Cell::diff_completed(
            "/diff schema demo (staging vs production users)",
            query_id,
            diff_result,
        ));
        self.open_overlay(ResultOverlay::Diff { other_idx: 0 });
    }

    /// Load a demo profile diff result for testing the profile diff overlay.
    fn load_profile_diff_demo(&mut self) {
        let (left_plan, right_plan) = create_profile_diff_demo();

        let diff_result = DiffQueryResult {
            left_name: "staging".to_string(),
            right_name: "production".to_string(),
            left_schema: None,
            left_batches: Vec::new(),
            left_error: None,
            right_schema: None,
            right_batches: Vec::new(),
            right_error: None,
            schemas_match: true,
            diff_stats: None,
            left_plan: Some(left_plan),
            right_plan: Some(right_plan),
            diff_type: DiffType::Profile,
            schema_diff: None,
        };

        let query_id = QueryId::new();
        self.set_result_cell(Cell::diff_completed(
            "/diff profile demo (staging vs production)",
            query_id,
            diff_result,
        ));
        self.open_overlay(ResultOverlay::Diff { other_idx: 0 });
    }

    /// Fetch table list from connected server.
    fn fetch_tables(&mut self) {
        // Get the active connection ID and endpoint
        let (conn_id, endpoint) = match self.connections.iter().find(|c| c.active) {
            Some(conn) => (conn.id, conn.endpoint.clone()),
            None => return,
        };

        // Check if we already have a pending fetch for this connection
        if self.pending_tables_id == Some(conn_id) {
            return;
        }

        // Ensure we have a Flight backend
        if !matches!(self.backend, Some(SqlBackend::Flight { .. })) {
            return;
        }

        self.pending_tables_id = Some(conn_id);

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_tables = Some(rx);

        // Spawn async task to fetch tables (reconnects like query execution)
        self.runtime_handle.spawn(async move {
            let result = async {
                let mut client = FlightClient::connect(&endpoint)
                    .await
                    .map_err(|e| e.to_string())?;
                client
                    .get_tables(None, None)
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;
            let _ = tx.send(result);
        });
    }

    /// Show the SQL pane with notebook-cell layout.
    ///
    /// Layout:
    /// ```text
    /// ┌────────────────────────────────┐
    /// │ [cell 1 - collapsed]           │
    /// │ [cell 2 - collapsed]           │
    /// │ [cell 3 - EXPANDED]            │
    /// │ [cell 4 - collapsed]           │
    /// │                                │
    /// │ [input bar + hints]            │
    /// └────────────────────────────────┘
    /// ```
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Poll for async results
        self.poll_async();

        // Update suggestions when input changes
        if self.input != self.prev_input {
            self.update_suggestions();
            self.prev_input = self.input.clone();
        }

        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();
        let bg_base = self.theme.bg_base();

        egui::Frame::new()
            .fill(bg_base)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                // Center content with max width, scrollbar stays at pane edge
                let available_width = ui.available_width();
                let max_content_width = available_width.min(900.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(available_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(max_content_width);

                        self.render_mode_badge(ui);

                        // Reserve space for input section at bottom
                        let input_height = 100.0;
                        let scroll_height = (ui.available_height() - input_height).max(100.0);

                        // === Single result cell (scrollable) ===
                        let mut card_actions: Vec<super::query_card::CardAction> = Vec::new();

                        let has_connections = !self.connections.is_empty();
                        let is_connected = self.active_connection().is_some_and(|c| {
                            matches!(c.state, ConnectionState::Connected)
                        });
                        let theme = self.theme;

                        // Use a floating scrollbar so the scroll area content
                        // width matches the input bar width below it.
                        ui.style_mut().spacing.scroll.floating = true;

                        egui::ScrollArea::vertical()
                            .id_salt("notebook_cells")
                            .max_height(scroll_height)
                            .stick_to_bottom(self.scroll_to_bottom)
                            .show(ui, |ui| {
                                if let Some(cell) = &self.result_cell {
                                    if cell.is_navigable() {
                                        let vs = &mut self.cell_view_state;
                                        let plan_viewer = &mut self.plan_viewer;
                                        let overlay_blocks =
                                            self.overlay_blocks_input || self.overlay_active;
                                        let input_id =
                                            egui::Id::new(format!("sql_input_{}", self.id));
                                        let input_has_focus = ui
                                            .ctx()
                                            .memory(|m| m.has_focus(input_id))
                                            || self.input_focused;
                                        let actions = super::query_card::render_query_card(
                                            ui,
                                            cell,
                                            0,
                                            vs,
                                            theme,
                                            overlay_blocks,
                                            plan_viewer,
                                            input_has_focus,
                                        );
                                        card_actions = actions;
                                        ui.add_space(8.0);
                                    }
                                }

                                if !self.has_result() {
                                    // Empty state placeholder
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(scroll_height / 4.0);
                                        ui.label(
                                            RichText::new(empty::NO_QUERIES)
                                                .color(theme.text_secondary().gamma_multiply(0.2))
                                                .size(32.0),
                                        );
                                        ui.add_space(8.0);
                                        let (title, subtitle) = if !has_connections {
                                            (
                                                "No connections configured",
                                                "Add Flight SQL connections in Settings",
                                            )
                                        } else if !is_connected {
                                            (
                                                "Not connected",
                                                "Click the connection pill below to choose an endpoint",
                                            )
                                        } else {
                                            (
                                                "Run a query to get started",
                                                "Type SQL below and press Enter",
                                            )
                                        };
                                        ui.label(
                                            RichText::new(title)
                                                .color(theme.text_secondary())
                                                .size(12.0),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(subtitle)
                                                .color(theme.text_secondary().gamma_multiply(0.6))
                                                .size(10.0),
                                        );
                                    });
                                }
                            });

                        self.scroll_to_bottom = false;

                        // Process card actions
                        if !card_actions.is_empty() {
                            self.handle_card_actions(card_actions);
                        }

                        // Render status message banner (between cell and input)
                        let mut dismiss_status = false;
                        if let Some(msg) = &self.status_message {
                            ui.add_space(4.0);
                            let color = if msg.is_error {
                                self.theme.semantic_error()
                            } else {
                                self.theme.text_secondary()
                            };
                            let text = msg.text.clone();
                            let close_color =
                                self.theme.text_secondary().gamma_multiply(0.5);
                            egui::Frame::new()
                                .fill(self.theme.bg_elevated())
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&text).color(color).size(11.0),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button(
                                                        RichText::new(action::CLOSE)
                                                            .color(close_color)
                                                            .size(10.0),
                                                    )
                                                    .clicked()
                                                {
                                                    dismiss_status = true;
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                        if dismiss_status {
                            self.status_message = None;
                        }

                        // === Input section (pinned at bottom) ===
                        ui.add_space(8.0);
                        self.render_suggestions_popup(ui, accent);
                        self.render_input_bar(ui, accent);
                        self.render_input_hints(ui, text_secondary);
                    },
                );
            });

        // Render connection popup when open
        if self.sidebar_width == 1.0 {
            let snapshots: Vec<ConnectionSnapshot> = self
                .connections
                .iter()
                .map(ConnectionSnapshot::from)
                .collect();
            let just_opened = self.popup_just_opened;
            self.popup_just_opened = false;
            let actions = super::connections::render_connection_popup(
                ui,
                self.theme,
                &snapshots,
                self.pill_rect,
                just_opened,
            );
            for action in actions {
                self.handle_connection_action(action);
            }
        }

        // Render result overlay if active (kept for now during transition)
        if self.active_overlay != ResultOverlay::None {
            self.render_result_overlay(ui);
        }
    }

    // ========================================================================
    // Result Overlay System
    // ========================================================================

    /// Compare two cell values for sorting (numeric-aware, NULL-last).
    fn compare_cell_values(a: &str, b: &str) -> std::cmp::Ordering {
        match (a, b) {
            ("NULL", "NULL") => std::cmp::Ordering::Equal,
            ("NULL", _) => std::cmp::Ordering::Greater,
            (_, "NULL") => std::cmp::Ordering::Less,
            _ => {
                if let (Ok(an), Ok(bn)) = (a.parse::<f64>(), b.parse::<f64>()) {
                    an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.cmp(b)
                }
            }
        }
    }

    /// Open an overlay for the result cell.
    fn open_overlay(&mut self, overlay: ResultOverlay) {
        self.active_overlay = overlay;
        self.overlay_active = true;
        self.overlay_table_page = 0;
        self.overlay_filter.clear();
    }

    /// Close the active overlay and refocus the input bar.
    fn close_overlay(&mut self) {
        self.active_overlay = ResultOverlay::None;
        self.overlay_active = false;
        self.overlay_table_page = 0;
        self.overlay_filter.clear();
        self.overlay_sort_column = None;
        self.overlay_sort_ascending = true;
        self.input_focused = true;
    }

    /// Render the result overlay.
    fn render_result_overlay(&mut self, ui: &mut egui::Ui) {
        if !self.overlay_active || self.result_cell.is_none() {
            self.close_overlay();
            return;
        }

        // Handle Esc to close (before drawing anything) — skip if a workspace overlay is open
        if !self.overlay_blocks_input
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.close_overlay();
            return;
        }

        // Draw dimmed backdrop
        ui.painter().rect_filled(
            ui.ctx().available_rect(),
            0.0,
            Color32::from_black_alpha(180),
        );

        // Calculate popup dimensions - responsive, sidebar-aware
        let popup_width = crate::util::overlay_width(ui.ctx(), 0.85, 700.0, 1400.0);
        let popup_height = crate::util::overlay_height(ui.ctx(), 0.85, 500.0, 900.0);

        // Render overlay content in a centered Area
        egui::Area::new(egui::Id::new("result_overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .constrain_to(crate::util::overlay_content_rect(ui.ctx()))
            .show(ui.ctx(), |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);

                overlay_style.frame().inner_margin(0.0).show(ui, |ui| {
                    ui.set_width(popup_width);
                    ui.set_max_width(popup_width);
                    ui.set_max_height(popup_height);

                    match &self.active_overlay {
                        ResultOverlay::None => {}
                        ResultOverlay::Table => {
                            self.render_table_overlay(ui);
                        }
                        ResultOverlay::Plan => {
                            self.render_plan_overlay(ui);
                        }
                        ResultOverlay::Diff { .. } => {
                            self.render_diff_overlay(ui);
                        }
                    }
                });
            });
    }

    /// Render the table overlay view.
    fn render_table_overlay(&mut self, ui: &mut egui::Ui) {
        // Clear egui focus so vim navigation keys (h/j/k/l) don't leak into the input bar.
        ui.ctx()
            .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));

        let colors = OverlayColors::new(self.theme);
        let bg_surface = self.theme.bg_surface();
        let bg_base = self.theme.bg_base();
        let rows_per_page = 50;

        // Extract data from cell first to avoid borrow conflicts
        let (total_rows, num_cols, execution_time_ms, has_schema, column_widths) = {
            let cell = self.result_cell.as_ref().unwrap();
            let total: usize = cell.batches().iter().map(|b| b.num_rows()).sum();
            let cols = cell.schema().map(|s| s.fields().len()).unwrap_or(0);
            let time_ms = cell.stats().map(|s| s.total_time.as_millis());

            // Calculate column widths based on header names and data types
            let widths: Vec<f32> = if let Some(schema) = cell.schema() {
                schema
                    .fields()
                    .iter()
                    .map(|field| {
                        // Base width on column name length (monospace ~7px per char)
                        let name_width = field.name().len() as f32 * 7.0;
                        let type_width = format!("{}", field.data_type()).len() as f32 * 6.0;
                        // Use max of name/type width, with min 80 and max 200
                        name_width.max(type_width).clamp(80.0, 200.0)
                    })
                    .collect()
            } else {
                vec![]
            };

            (total, cols, time_ms, cell.schema().is_some(), widths)
        };

        let total_pages = total_rows.div_ceil(rows_per_page);
        let mut should_close = false;
        let mut next_page = false;
        let mut prev_page = false;
        let mut scroll_delta = egui::Vec2::ZERO;
        let mut should_copy = false;
        let mut should_share_to_agent = false;

        // Handle keyboard navigation — skip if a workspace overlay (style picker, etc.) is open
        if !self.overlay_blocks_input {
            ui.ctx().input_mut(|i| {
                // Vim-style scrolling: h/l horizontal, j/k vertical
                let scroll_step = 50.0;
                if i.consume_key(egui::Modifiers::NONE, egui::Key::L) {
                    scroll_delta.x += scroll_step;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::H) {
                    scroll_delta.x -= scroll_step;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::J) {
                    scroll_delta.y += scroll_step;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::K) {
                    scroll_delta.y -= scroll_step;
                }

                // Page navigation: [ ] or arrow keys
                if i.consume_key(egui::Modifiers::NONE, egui::Key::CloseBracket)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                {
                    next_page = true;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::OpenBracket)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                {
                    prev_page = true;
                }

                // Copy to clipboard
                if i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)
                    || i.consume_key(egui::Modifiers::CTRL, egui::Key::C)
                {
                    should_copy = true;
                }

                // Share to agent panel
                if i.consume_key(egui::Modifiers::NONE, egui::Key::S) {
                    should_share_to_agent = true;
                }

                // Close on Escape
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    should_close = true;
                }
            });
        }

        // Share to agent panel
        if should_share_to_agent {
            if let Some(table) = self.result_to_inline_table() {
                self.pending_action = SqlPaneAction::ShareResultToAgent(table);
            }
        }

        // Copy all results as TSV
        if should_copy {
            if let Some(cell) = &self.result_cell {
                if let Some(schema) = cell.schema() {
                    let tsv = Self::format_results_as_tsv(schema, cell.batches());
                    self.copy_to_clipboard(&tsv);
                }
            }
        }

        // ===== Header with icon and stats badges =====
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Table icon
            ui.label(RichText::new(file::DATA).color(colors.accent).size(16.0));
            ui.add_space(6.0);

            // Title
            ui.label(
                RichText::new("Table View")
                    .color(colors.accent)
                    .font(typography::proportional(typography::XL))
                    .strong(),
            );

            // Right side: stats badges
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // "Copied!" feedback badge
                if self.show_copied_badge() {
                    render_stat_badge(ui, "Copied!", &colors);
                    ui.add_space(4.0);
                }

                // Row count badge
                render_stat_badge(ui, &format!("{total_rows} rows"), &colors);
                ui.add_space(4.0);

                // Column count badge
                render_stat_badge(ui, &format!("{num_cols} cols"), &colors);

                // Execution time badge
                if let Some(ms) = execution_time_ms {
                    ui.add_space(4.0);
                    render_stat_badge_with_icon(ui, time::TIMER, &format!("{ms}ms"), &colors);
                }
            });
        });
        ui.add_space(8.0);

        // Separator below header
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );

        // Table content
        if !has_schema {
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("No schema available")
                        .color(colors.faint_text)
                        .font(typography::proportional(typography::MD)),
                );
            });
            ui.add_space(24.0);
            if should_close {
                self.close_overlay();
            }
            return;
        }

        // Now we can safely access cell data for rendering the table
        let cell = self.result_cell.as_ref().unwrap();
        let schema = cell.schema().unwrap(); // Safe because has_schema is true

        // Calculate row number width for alignment
        let max_row_num = (self.overlay_table_page + 1) * rows_per_page;
        let row_num_width = max_row_num.to_string().len().max(3);
        let row_num_gutter_width = (row_num_width + 2) as f32 * 8.0;

        let header_height = typography::SM + typography::XS + 8.0;
        let row_height = typography::SM + 8.0;
        let start_row = self.overlay_table_page * rows_per_page;

        // Build sorted row indices for column sort
        let sort_col = self.overlay_sort_column;
        let sort_asc = self.overlay_sort_ascending;
        let sorted_row_indices: Vec<(usize, usize)> = {
            let mut indices: Vec<(usize, usize)> = Vec::new();
            for (batch_idx, batch) in cell.batches().iter().enumerate() {
                for row_idx in 0..batch.num_rows() {
                    indices.push((batch_idx, row_idx));
                }
            }
            if let Some(sc) = sort_col {
                if sc < num_cols {
                    indices.sort_by(|a, b| {
                        let val_a =
                            format_array_value(cell.batches()[a.0].column(sc).as_ref(), a.1);
                        let val_b =
                            format_array_value(cell.batches()[b.0].column(sc).as_ref(), b.1);
                        let ord = Self::compare_cell_values(&val_a, &val_b);
                        if sort_asc { ord } else { ord.reverse() }
                    });
                }
            }
            indices
        };

        // Single ScrollArea for both header and body - ensures aligned scrolling
        egui::Frame::new()
            .fill(bg_base)
            .inner_margin(0.0)
            .show(ui, |ui| {
                let scroll_id = egui::Id::new("table_overlay_scroll");

                // Apply keyboard scroll delta
                if scroll_delta != egui::Vec2::ZERO {
                    let current_offset = ui
                        .ctx()
                        .memory(|m| m.data.get_temp::<egui::Vec2>(scroll_id))
                        .unwrap_or(egui::Vec2::ZERO);
                    let new_offset = (current_offset + scroll_delta).max(egui::Vec2::ZERO);
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(scroll_id, new_offset));
                }

                let stored_offset = ui
                    .ctx()
                    .memory(|m| m.data.get_temp::<egui::Vec2>(scroll_id))
                    .unwrap_or(egui::Vec2::ZERO);

                let scroll_output = egui::ScrollArea::both()
                    .id_salt("overlay_table")
                    .scroll_offset(stored_offset)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;

                        // ===== Column headers row =====
                        ui.horizontal(|ui| {
                            ui.style_mut().spacing.item_spacing.x = 0.0;

                            // Row number gutter placeholder
                            let (gutter_rect, _) = ui.allocate_exact_size(
                                egui::vec2(row_num_gutter_width, header_height),
                                egui::Sense::hover(),
                            );
                            ui.painter()
                                .rect_filled(gutter_rect, 0.0, self.theme.bg_base());
                            ui.painter().text(
                                gutter_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "#",
                                typography::monospace(typography::XS),
                                colors.faint_text,
                            );

                            // Column headers with fixed widths (clickable for sort)
                            for (idx, field) in schema.fields().iter().enumerate() {
                                let col_width = column_widths.get(idx).copied().unwrap_or(100.0);
                                let col_spacing = 16.0;

                                // Allocate fixed-width cell (clickable for column sort)
                                let (col_rect, col_response) = ui.allocate_exact_size(
                                    egui::vec2(col_width + col_spacing, header_height),
                                    egui::Sense::click(),
                                );

                                // Toggle sort on click: asc → desc → none
                                if col_response.clicked() {
                                    if self.overlay_sort_column == Some(idx) {
                                        if self.overlay_sort_ascending {
                                            self.overlay_sort_ascending = false;
                                        } else {
                                            self.overlay_sort_column = None;
                                        }
                                    } else {
                                        self.overlay_sort_column = Some(idx);
                                        self.overlay_sort_ascending = true;
                                    }
                                    // Reset to first page on sort change
                                    self.overlay_table_page = 0;
                                }

                                // Hover cursor
                                if col_response.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                // Header background (highlight on hover or active sort)
                                let is_sort_col = self.overlay_sort_column == Some(idx);
                                let header_bg = if col_response.hovered() {
                                    self.theme.bg_hover()
                                } else if is_sort_col {
                                    self.theme.bg_hover().gamma_multiply(0.5)
                                } else {
                                    bg_surface
                                };
                                ui.painter().rect_filled(col_rect, 0.0, header_bg);

                                // Sort indicator
                                let sort_indicator = if is_sort_col {
                                    if self.overlay_sort_ascending {
                                        " ▲"
                                    } else {
                                        " ▼"
                                    }
                                } else {
                                    ""
                                };

                                // Draw column name with sort indicator
                                ui.painter().text(
                                    col_rect.left_center() + egui::vec2(8.0, -6.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{}{sort_indicator}", field.name()),
                                    typography::monospace(typography::SM),
                                    if is_sort_col {
                                        colors.accent
                                    } else {
                                        colors.text
                                    },
                                );

                                // Draw data type below
                                ui.painter().text(
                                    col_rect.left_center() + egui::vec2(8.0, 6.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{}", field.data_type()),
                                    typography::monospace(typography::XS),
                                    colors.faint_text,
                                );
                            }
                        });

                        // ===== Data rows (using sorted indices) =====
                        let page_end = (start_row + rows_per_page).min(sorted_row_indices.len());
                        let page_start = start_row.min(sorted_row_indices.len());

                        for (display_idx, &(batch_idx, row_idx)) in
                            sorted_row_indices[page_start..page_end].iter().enumerate()
                        {
                            let absolute_row = start_row + display_idx + 1;
                            let batch = &cell.batches()[batch_idx];

                            // Alternate row background
                            let row_bg = if display_idx % 2 == 0 {
                                Color32::TRANSPARENT
                            } else {
                                self.theme.bg_hover().gamma_multiply(0.3)
                            };

                            ui.horizontal(|ui| {
                                ui.style_mut().spacing.item_spacing.x = 0.0;

                                // Row number gutter
                                let (gutter_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(row_num_gutter_width, row_height),
                                    egui::Sense::hover(),
                                );
                                ui.painter()
                                    .rect_filled(gutter_rect, 0.0, self.theme.bg_base());
                                let row_num_str = format!("{absolute_row:>row_num_width$}");
                                ui.painter().text(
                                    gutter_rect.left_center() + egui::vec2(8.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    row_num_str,
                                    typography::monospace(typography::XS),
                                    colors.faint_text,
                                );

                                // Cell values with fixed widths
                                for col_idx in 0..batch.num_columns() {
                                    let col_width =
                                        column_widths.get(col_idx).copied().unwrap_or(100.0);
                                    let col_spacing = 16.0;

                                    // Allocate fixed-width cell
                                    let (cell_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(col_width + col_spacing, row_height),
                                        egui::Sense::hover(),
                                    );

                                    // Draw row background
                                    ui.painter().rect_filled(cell_rect, 0.0, row_bg);

                                    let col = batch.column(col_idx);
                                    let value = format_array_value(col.as_ref(), row_idx);

                                    if value == "NULL" {
                                        // NULL cells: subtle background tint + italic text
                                        let null_bg = colors.faint_text.gamma_multiply(0.06);
                                        ui.painter().rect_filled(cell_rect, 0.0, null_bg);
                                        let job = egui::text::LayoutJob::single_section(
                                            "null".to_string(),
                                            egui::TextFormat {
                                                font_id: typography::monospace(typography::SM),
                                                color: colors.faint_text,
                                                italics: true,
                                                ..Default::default()
                                            },
                                        );
                                        let galley = ui.fonts_mut(|f| f.layout_job(job));
                                        ui.painter().galley(
                                            cell_rect.left_center()
                                                + egui::vec2(8.0, -galley.size().y / 2.0),
                                            galley,
                                            colors.faint_text,
                                        );
                                    } else {
                                        // Truncate long values
                                        let max_chars = ((col_width - 8.0) / 7.0) as usize;
                                        let display_val =
                                            if value.len() > max_chars && max_chars > 3 {
                                                let truncated: String = value
                                                    .chars()
                                                    .take(max_chars.saturating_sub(1))
                                                    .collect();
                                                format!("{truncated}…")
                                            } else {
                                                value
                                            };

                                        ui.painter().text(
                                            cell_rect.left_center() + egui::vec2(8.0, 0.0),
                                            egui::Align2::LEFT_CENTER,
                                            display_val,
                                            typography::monospace(typography::SM),
                                            colors.muted_text,
                                        );
                                    }
                                }
                            });
                        }
                    });

                // Store the new scroll offset after user interaction
                ui.ctx().memory_mut(|m| {
                    m.data.insert_temp(scroll_id, scroll_output.state.offset);
                });
            });

        // ===== Footer with query and pagination =====
        // Separator above footer
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
        ui.add_space(6.0);

        let current_page = self.overlay_table_page;

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Keyboard hint
                ui.label(
                    RichText::new("hjkl scroll • [/] page • \u{2318}C copy • S share • Esc")
                        .color(colors.faint_text.gamma_multiply(0.7))
                        .font(typography::proportional(typography::XS)),
                );

                ui.add_space(12.0);

                // Next page button
                let can_next = current_page < total_pages.saturating_sub(1);
                if ui
                    .add_enabled(
                        can_next,
                        egui::Button::new(RichText::new(nav::FORWARD).color(if can_next {
                            colors.accent
                        } else {
                            colors.faint_text.gamma_multiply(0.3)
                        }))
                        .frame(false),
                    )
                    .clicked()
                {
                    next_page = true;
                }

                // Page indicator
                ui.label(
                    RichText::new(format!("{} / {}", current_page + 1, total_pages.max(1)))
                        .color(colors.muted_text)
                        .font(typography::proportional(typography::SM)),
                );

                // Previous page button
                let can_prev = current_page > 0;
                if ui
                    .add_enabled(
                        can_prev,
                        egui::Button::new(RichText::new(nav::BACK).color(if can_prev {
                            colors.accent
                        } else {
                            colors.faint_text.gamma_multiply(0.3)
                        }))
                        .frame(false),
                    )
                    .clicked()
                {
                    prev_page = true;
                }
            });
        });
        ui.add_space(8.0);

        // Apply page changes
        if next_page && self.overlay_table_page < total_pages.saturating_sub(1) {
            self.overlay_table_page += 1;
        }
        if prev_page && self.overlay_table_page > 0 {
            self.overlay_table_page -= 1;
        }
        if should_close {
            self.close_overlay();
        }
    }

    /// Render the plan overlay view.
    fn render_plan_overlay(&mut self, ui: &mut egui::Ui) {
        // Clear egui focus so vim navigation works and ':' can open command palette.
        // Without this, the SQL input TextEdit might retain focus, causing
        // listen_for_kb_shortcut() to return early before processing ':'.
        ui.ctx()
            .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));

        // Propagate overlay_blocks_input to plan_viewer
        self.plan_viewer
            .set_overlay_blocks_input(self.overlay_blocks_input);

        let colors = OverlayColors::new(self.theme);
        let bg_base = self.theme.bg_base();

        // Extract query for footer
        let sql_query = self
            .result_cell
            .as_ref()
            .map(|c| c.sql().to_string())
            .unwrap_or_default();

        // Get plan stats
        let (total_time, operator_count, bottleneck_count) = self.plan_viewer.stats();

        let mut should_close = false;
        let mut should_copy = false;

        // Handle keyboard navigation — skip if a workspace overlay is open
        if !self.overlay_blocks_input {
            ui.ctx().input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    should_close = true;
                }
                if i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)
                    || i.consume_key(egui::Modifiers::CTRL, egui::Key::C)
                {
                    should_copy = true;
                }
            });
        }

        // Copy plan as formatted text
        if should_copy {
            if let Some(plan_node) = self.plan_viewer.root_plan() {
                let text = Self::format_plan_as_text(plan_node, 0);
                self.copy_to_clipboard(&text);
            }
        }

        // ===== Header with icon and stats badges =====
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Plan icon
            ui.label(RichText::new(nav::TREE).color(colors.accent).size(16.0));
            ui.add_space(6.0);

            // Title
            ui.label(
                RichText::new("Execution Plan")
                    .color(colors.accent)
                    .font(typography::proportional(typography::XL))
                    .strong(),
            );

            // Right side: stats badges
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // "Copied!" feedback badge
                if self.show_copied_badge() {
                    render_stat_badge(ui, "Copied!", &colors);
                    ui.add_space(4.0);
                }

                // Execution time badge
                if !total_time.is_zero() {
                    render_stat_badge_with_icon(
                        ui,
                        time::TIMER,
                        &format!("{}ms", total_time.as_millis()),
                        &colors,
                    );
                    ui.add_space(4.0);
                }

                // Operator count badge
                render_stat_badge(ui, &format!("{operator_count} ops"), &colors);

                // Bottleneck badge
                if bottleneck_count > 0 {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(self.theme.semantic_warning().gamma_multiply(0.2))
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 3.0;
                                ui.label(
                                    RichText::new(status::WARNING)
                                        .color(self.theme.semantic_warning())
                                        .size(10.0),
                                );
                                ui.label(
                                    RichText::new(format!("{bottleneck_count} bottleneck"))
                                        .color(self.theme.semantic_warning())
                                        .font(typography::proportional(typography::XS)),
                                );
                            });
                        });
                }
            });
        });
        ui.add_space(8.0);

        // Separator below header
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );

        // Plan viewer content
        egui::Frame::new()
            .fill(bg_base)
            .inner_margin(12.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.plan_viewer.show(ui);
                    });
            });

        // ===== Footer with query and keyboard hints =====
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // SQL query preview (truncated)
            let sql_preview =
                crate::components::util::text_formatting::truncate_with_ellipsis(&sql_query, 50);
            ui.label(
                RichText::new(&sql_preview)
                    .color(colors.faint_text)
                    .font(typography::monospace(typography::XS)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Keyboard hints based on current mode
                let hints = match self.plan_viewer.mode {
                    PlanViewMode::Tree => {
                        "j/k nav • h/l fold • b bottleneck • \u{2318}C copy • Esc"
                    }
                    PlanViewMode::Stats => "scroll to explore • \u{2318}C copy • Esc",
                    PlanViewMode::Waterfall => "j/k nav • b bottleneck • \u{2318}C copy • Esc",
                };
                ui.label(
                    RichText::new(hints)
                        .color(colors.faint_text.gamma_multiply(0.7))
                        .font(typography::proportional(typography::XS)),
                );
            });
        });
        ui.add_space(8.0);

        if should_close {
            self.close_overlay();
        }
    }

    /// Render the diff overlay view.
    fn render_diff_overlay(&mut self, ui: &mut egui::Ui) {
        use crate::ui::semantic_icons::diff;

        // Clear egui focus so keyboard shortcuts don't leak into the input bar.
        ui.ctx()
            .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));

        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let colors = OverlayColors::new(self.theme);
        let theme = self.theme;

        // Handle Escape to close — skip if a workspace overlay is open
        let mut should_close = false;
        if !self.overlay_blocks_input {
            ui.ctx().input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    should_close = true;
                }
            });
        }

        // Helper to calculate total time from a plan tree
        fn calc_plan_time(node: &PlanNode) -> u64 {
            let self_time = node
                .metrics
                .as_ref()
                .map(|m| m.elapsed_time.as_micros() as u64)
                .unwrap_or(0);
            let children_time: u64 = node.children.iter().map(calc_plan_time).sum();
            self_time + children_time
        }

        // Extract data from diff result to avoid borrow conflicts in closures
        let diff_data = self
            .result_cell
            .as_ref()
            .and_then(|c| c.diff_result())
            .map(|d| {
                // Calculate profile timing if plans are available
                let left_time_ms = d.left_plan.as_ref().map(|p| calc_plan_time(p) / 1000);
                let right_time_ms = d.right_plan.as_ref().map(|p| calc_plan_time(p) / 1000);

                (
                    d.left_name.clone(),
                    d.right_name.clone(),
                    d.diff_type.clone(),
                    d.diff_stats.clone(),
                    d.schemas_match,
                    d.left_schema.is_some(),
                    d.right_schema.is_some(),
                    d.left_error.clone(),
                    d.right_error.clone(),
                    d.schema_diff.clone(),
                    left_time_ms,
                    right_time_ms,
                )
            });

        let Some((
            left_name,
            right_name,
            diff_type,
            diff_stats,
            schemas_match,
            has_left_schema,
            has_right_schema,
            left_error,
            right_error,
            schema_diff,
            left_time_ms,
            right_time_ms,
        )) = diff_data
        else {
            // No diff result - show placeholder
            egui::Frame::new()
                .fill(theme.bg_surface())
                .inner_margin(16.0)
                .corner_radius(12.0)
                .show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("No diff data available")
                                .color(text_secondary)
                                .size(14.0),
                        );
                    });
                });
            if should_close {
                self.close_overlay();
            }
            return;
        };

        // ===== Header with icon, title, and stats badges =====
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Back button
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(format!("{} Back", nav::BACK))
                            .color(text_secondary)
                            .size(12.0),
                    )
                    .frame(false),
                )
                .clicked()
            {
                should_close = true;
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            // Diff icon and title
            ui.label(RichText::new(diff::DIFF).color(colors.accent).size(16.0));
            ui.add_space(4.0);

            let title = match &diff_type {
                DiffType::Plan => format!("{left_name} vs {right_name} (Plan)"),
                DiffType::Profile => format!("{left_name} vs {right_name} (Profile)"),
                DiffType::Schema => {
                    let table = schema_diff
                        .as_ref()
                        .map(|s| s.table_name.as_str())
                        .unwrap_or("table");
                    format!("{left_name} vs {right_name} ({table} Schema)")
                }
                DiffType::Data => format!("{left_name} vs {right_name}"),
            };
            ui.label(RichText::new(title).color(text_primary).size(14.0).strong());

            // Stats badges on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // For schema diff, use schema_diff stats
                if let Some(sd) = &schema_diff {
                    render_colored_badge(
                        ui,
                        &format!("{} matching", sd.matching),
                        theme.semantic_success(),
                    );
                    ui.add_space(4.0);

                    if sd.changed > 0 {
                        render_colored_badge(
                            ui,
                            &format!("{} changed", sd.changed),
                            theme.semantic_warning(),
                        );
                        ui.add_space(4.0);
                    }

                    if sd.left_only > 0 {
                        render_colored_badge(
                            ui,
                            &format!("{} removed", sd.left_only),
                            theme.semantic_error(),
                        );
                        ui.add_space(4.0);
                    }

                    if sd.right_only > 0 {
                        render_colored_badge(
                            ui,
                            &format!("{} added", sd.right_only),
                            theme.accent_muted(),
                        );
                    }
                } else if let Some(stats) = &diff_stats {
                    // Matching badge
                    render_colored_badge(
                        ui,
                        &format!("{} matching", stats.matching),
                        theme.semantic_success(),
                    );
                    ui.add_space(4.0);

                    if stats.left_only > 0 {
                        render_colored_badge(
                            ui,
                            &format!("{} left only", stats.left_only),
                            theme.semantic_warning(),
                        );
                        ui.add_space(4.0);
                    }

                    if stats.right_only > 0 {
                        render_colored_badge(
                            ui,
                            &format!("{} right only", stats.right_only),
                            theme.accent_muted(),
                        );
                        ui.add_space(4.0);
                    }

                    if stats.different > 0 {
                        render_colored_badge(
                            ui,
                            &format!("{} different", stats.different),
                            theme.semantic_error(),
                        );
                    }
                } else if !schemas_match && has_left_schema && has_right_schema {
                    render_colored_badge(ui, "Schema mismatch", theme.semantic_warning());
                }

                // Profile diff timing summary
                if matches!(diff_type, DiffType::Profile | DiffType::Plan) {
                    if let (Some(left_ms), Some(right_ms)) = (left_time_ms, right_time_ms) {
                        let delta = right_ms as i64 - left_ms as i64;
                        let pct = if left_ms > 0 {
                            ((right_ms as f64 - left_ms as f64) / left_ms as f64) * 100.0
                        } else {
                            0.0
                        };

                        let (text, color) = if delta < 0 {
                            (
                                format!("{left_ms}ms → {right_ms}ms ({pct:.0}%)"),
                                theme.semantic_success(),
                            )
                        } else if delta > 0 {
                            (
                                format!("{left_ms}ms → {right_ms}ms (+{pct:.0}%)"),
                                theme.semantic_error(),
                            )
                        } else {
                            (format!("{left_ms}ms"), text_secondary)
                        };

                        render_colored_badge(ui, &text, color);
                    }
                }
            });
        });
        ui.add_space(8.0);

        // Separator below header
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );

        // Main content - side by side view
        egui::Frame::new()
            .fill(theme.bg_base())
            .inner_margin(8.0)
            .show(ui, |ui| {
                // Error display if either side failed
                if left_error.is_some() || right_error.is_some() {
                    if let Some(err) = &left_error {
                        egui::Frame::new()
                            .fill(theme.semantic_error().gamma_multiply(0.1))
                            .stroke(egui::Stroke::new(1.0, theme.semantic_error()))
                            .corner_radius(4.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("{left_name}: {err}"))
                                        .color(theme.semantic_error()),
                                );
                            });
                        ui.add_space(4.0);
                    }
                    if let Some(err) = &right_error {
                        egui::Frame::new()
                            .fill(theme.semantic_error().gamma_multiply(0.1))
                            .stroke(egui::Stroke::new(1.0, theme.semantic_error()))
                            .corner_radius(4.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("{right_name}: {err}"))
                                        .color(theme.semantic_error()),
                                );
                            });
                    }
                } else {
                    // Re-borrow diff_result for content rendering (no mutable self access here)
                    if let Some(diff_result) =
                        self.result_cell.as_ref().and_then(|c| c.diff_result())
                    {
                        match diff_type {
                            DiffType::Schema => {
                                render_schema_diff_content(ui, self.theme, diff_result);
                            }
                            DiffType::Profile => {
                                render_profile_diff_content(ui, self.theme, diff_result);
                            }
                            DiffType::Plan => {
                                render_plan_diff_content(ui, self.theme, diff_result);
                            }
                            DiffType::Data => {
                                render_data_diff_content(ui, self.theme, diff_result);
                            }
                        }
                    }
                }
            });

        // ===== Footer with keyboard hints =====
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let key_bg = theme.bg_elevated();
            crate::components::util::render_key_badge(ui, "Esc", key_bg, text_secondary);
            ui.label(
                RichText::new("close")
                    .color(text_secondary.gamma_multiply(0.7))
                    .size(10.0),
            );
        });
        ui.add_space(8.0);

        if should_close {
            self.close_overlay();
        }
    }

    // ========================================================================
    // Input Bar UI Components
    // ========================================================================

    /// Update suggestions based on current input.
    fn update_suggestions(&mut self) {
        // Clone input to avoid borrow issues with mutable methods
        let input = self.input.trim().to_string();

        if input.is_empty() {
            self.suggestions.clear();
            return;
        }

        // Check if typing a command (starts with /)
        if let Some(cmd_query) = input.strip_prefix('/') {
            // /describe <table> and /schema <table> - show table name suggestions directly
            if let Some(partial) = cmd_query
                .strip_prefix("describe ")
                .or_else(|| cmd_query.strip_prefix("schema "))
            {
                let partial = partial.trim();
                let items = self.get_schema_suggestions(partial);
                self.suggestions.set(items);
                return;
            }

            // Special handling for /explain, /analyze, and /bench - show table suggestions for SQL part
            let sql_part = cmd_query
                .strip_prefix("explain ")
                .or_else(|| cmd_query.strip_prefix("analyze "))
                .or_else(|| {
                    // /bench may have an optional iteration count before the SQL
                    let rest = cmd_query.strip_prefix("bench ")?;
                    rest.split_once(' ')
                        .filter(|(first, _)| first.parse::<usize>().is_ok())
                        .map(|(_, sql)| sql)
                        .or(Some(rest))
                });

            if let Some(sql_part) = sql_part {
                // Check for table completion in the SQL part
                if Self::ends_with_table_keyword(sql_part) {
                    let items = self.get_schema_suggestions("");
                    self.suggestions.set(items);
                    return;
                }

                // Check for partial table/schema name
                let words: Vec<&str> = sql_part.split_whitespace().collect();
                if words.len() >= 2 {
                    let second_last = words[words.len() - 2].to_uppercase();
                    let last = words[words.len() - 1];

                    if TABLE_KEYWORDS.contains(&second_last.as_str()) {
                        let items = self.get_schema_suggestions(last);
                        self.suggestions.set(items);
                        return;
                    }
                }

                // No SQL suggestions, fall through to command matching
            }

            // Standard command matching with nucleo
            let items = self.fuzzy_match_commands(cmd_query);
            self.suggestions.set(items);
            return;
        }

        // Check for table completion (after FROM, JOIN, etc.)
        if Self::ends_with_table_keyword(&input) {
            let items = self.get_schema_suggestions("");
            self.suggestions.set(items);
            return;
        }

        // Split input into words once for all remaining checks
        let words: Vec<&str> = input.split_whitespace().collect();

        // Check for partial table/schema name after table keywords
        if words.len() >= 2 {
            let second_last = words[words.len() - 2].to_uppercase();
            let last = words[words.len() - 1];

            if TABLE_KEYWORDS.contains(&second_last.as_str()) {
                let items = self.get_schema_suggestions(last);
                self.suggestions.set(items);
                return;
            }
        }

        // Column completion context: after SELECT, WHERE, HAVING, ON, SET, ORDER BY, GROUP BY
        let upper_words: Vec<String> = words.iter().map(|w| w.to_uppercase()).collect();

        let in_column_context = upper_words
            .iter()
            .rposition(|w| {
                COLUMN_KEYWORDS.contains(&w.as_str()) || TABLE_KEYWORDS.contains(&w.as_str())
            })
            .is_some_and(|idx| {
                // Only column context if the keyword is a column-position keyword,
                // not a table-position keyword (those are handled above)
                COLUMN_KEYWORDS.contains(&upper_words[idx].as_str())
            });

        if in_column_context {
            let from_tables = self.extract_from_tables(&input);
            let partial = words.last().copied().unwrap_or("");
            let partial_upper = partial.to_uppercase();

            if !COLUMN_KEYWORDS.contains(&partial_upper.as_str()) {
                let mut items = if !from_tables.is_empty() {
                    self.get_column_suggestions(partial, &from_tables)
                } else {
                    Vec::new()
                };
                if partial.len() >= 2 {
                    items.extend(self.get_keyword_suggestions(partial));
                }
                items.sort_by(|a, b| b.score.cmp(&a.score));
                self.suggestions.set(items);
                return;
            }
        }

        // Generic keyword/function completion for any context
        if let Some(last_word) = words.last() {
            if last_word.len() >= 2 && !last_word.starts_with('/') {
                let items = self.get_keyword_suggestions(last_word);
                self.suggestions.set(items);
                return;
            }
        }

        self.suggestions.clear();
    }

    /// Check if text ends with a table keyword followed by a space (e.g., "FROM ").
    fn ends_with_table_keyword(text: &str) -> bool {
        let upper = text.to_uppercase();
        TABLE_KEYWORDS
            .iter()
            .any(|kw| upper.ends_with(&format!("{kw} ")))
    }

    /// Fuzzy match SQL commands using nucleo.
    fn fuzzy_match_commands(&mut self, query: &str) -> Vec<Suggestion> {
        if query.is_empty() {
            // Show all commands
            return SqlCommand::all()
                .iter()
                .map(|cmd| Suggestion {
                    label: format!("/{}", cmd.name()),
                    detail: cmd.description().to_string(),
                    insert: format!("/{}", cmd.name()),
                    icon: SuggestionIcon::Command,
                    score: 0,
                    match_positions: Vec::new(),
                })
                .collect();
        }

        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut results = Vec::new();

        for cmd in SqlCommand::all() {
            indices.clear();
            let haystack = Utf32Str::new(cmd.name(), &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                results.push(Suggestion {
                    label: format!("/{}", cmd.name()),
                    detail: cmd.description().to_string(),
                    insert: format!("/{}", cmd.name()),
                    icon: SuggestionIcon::Command,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize + 1).collect(), // +1 for leading /
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Get table suggestions with optional fuzzy filtering.
    fn get_table_suggestions(&mut self, query: &str) -> Vec<Suggestion> {
        let tables: Vec<_> = self
            .active_connection()
            .map(|c| c.tables.clone())
            .unwrap_or_default();

        if query.is_empty() {
            // Return all tables
            return tables
                .iter()
                .map(|t| Suggestion {
                    label: t.name.clone(),
                    detail: Self::table_detail(t),
                    insert: t.name.clone(),
                    icon: SuggestionIcon::Table,
                    score: 0,
                    match_positions: Vec::new(),
                })
                .collect();
        }

        // Fuzzy match with nucleo
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut results = Vec::new();

        for table in &tables {
            indices.clear();
            let haystack = Utf32Str::new(&table.name, &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                results.push(Suggestion {
                    label: table.name.clone(),
                    detail: Self::table_detail(table),
                    insert: table.name.clone(),
                    icon: SuggestionIcon::Table,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize).collect(),
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Get schema/catalog/table suggestions with hierarchical navigation.
    /// - Empty query: show catalogs (or schemas if single catalog)
    /// - "catalog.": show schemas in that catalog
    /// - "catalog.schema.": show tables
    /// - Partial without dot: fuzzy match catalogs/schemas/tables
    fn get_schema_suggestions(&mut self, query: &str) -> Vec<Suggestion> {
        let tables: Vec<_> = self
            .active_connection()
            .map(|c| c.tables.clone())
            .unwrap_or_default();

        if tables.is_empty() {
            return Vec::new();
        }

        // Collect unique catalogs and schemas
        let mut catalogs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut schemas: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
            std::collections::BTreeMap::new();

        for t in &tables {
            if !t.catalog.is_empty() {
                catalogs.insert(t.catalog.clone());
                schemas
                    .entry(t.catalog.clone())
                    .or_default()
                    .insert(t.schema.clone());
            } else if !t.schema.is_empty() {
                // No catalog, just schema
                schemas
                    .entry(String::new())
                    .or_default()
                    .insert(t.schema.clone());
            }
        }

        // Parse query to determine hierarchy level
        let parts: Vec<&str> = query.split('.').collect();

        match parts.as_slice() {
            // Empty or no dot: show catalogs (or schemas if only one catalog)
            [partial] if !query.contains('.') => {
                if catalogs.len() <= 1 && !schemas.is_empty() {
                    // Single or no catalog - show schemas directly
                    let default_catalog = catalogs.iter().next().cloned().unwrap_or_default();
                    let schema_set = schemas.get(&default_catalog).cloned().unwrap_or_default();

                    if partial.is_empty() {
                        // Show all schemas
                        schema_set
                            .iter()
                            .map(|s| Suggestion {
                                label: format!("{s}."),
                                detail: format!(
                                    "{} tables",
                                    tables.iter().filter(|t| &t.schema == s).count()
                                ),
                                insert: format!("{s}."),
                                icon: SuggestionIcon::Column, // Using Column icon for schema
                                score: 0,
                                match_positions: Vec::new(),
                            })
                            .collect()
                    } else {
                        // Fuzzy match schemas
                        self.fuzzy_match_schemas(partial, &schema_set, &tables)
                    }
                } else if catalogs.len() > 1 {
                    // Multiple catalogs - show them
                    if partial.is_empty() {
                        catalogs
                            .iter()
                            .map(|c| {
                                let schema_count = schemas.get(c).map(|s| s.len()).unwrap_or(0);
                                Suggestion {
                                    label: format!("{c}."),
                                    detail: format!("{schema_count} schemas"),
                                    insert: format!("{c}."),
                                    icon: SuggestionIcon::Connection, // Using Connection icon for catalog
                                    score: 0,
                                    match_positions: Vec::new(),
                                }
                            })
                            .collect()
                    } else {
                        // Fuzzy match catalogs
                        self.fuzzy_match_catalogs(partial, &catalogs, &schemas)
                    }
                } else {
                    // No catalogs or schemas - show tables directly
                    self.get_table_suggestions(partial)
                }
            }

            // "catalog." - show schemas in that catalog
            [catalog, partial] if query.ends_with('.') || !partial.is_empty() => {
                let catalog_str = *catalog;
                if let Some(schema_set) = schemas.get(catalog_str) {
                    if partial.is_empty() || query.ends_with('.') {
                        // Show all schemas in this catalog
                        schema_set
                            .iter()
                            .map(|s| Suggestion {
                                label: format!("{s}."),
                                detail: format!(
                                    "{} tables",
                                    tables
                                        .iter()
                                        .filter(|t| t.catalog == catalog_str && &t.schema == s)
                                        .count()
                                ),
                                insert: format!("{catalog_str}.{s}."),
                                icon: SuggestionIcon::Column,
                                score: 0,
                                match_positions: Vec::new(),
                            })
                            .collect()
                    } else {
                        // Fuzzy match schemas within this catalog
                        self.fuzzy_match_schemas_in_catalog(
                            partial,
                            catalog_str,
                            schema_set,
                            &tables,
                        )
                    }
                } else {
                    Vec::new()
                }
            }

            // "catalog.schema." or "schema." - show tables
            [catalog, schema, partial] => {
                let matching_tables: Vec<_> = tables
                    .iter()
                    .filter(|t| {
                        (t.catalog == *catalog || catalog.is_empty()) && t.schema == *schema
                    })
                    .cloned()
                    .collect();

                if partial.is_empty() {
                    // Show all tables in this schema
                    matching_tables
                        .iter()
                        .map(|t| Suggestion {
                            label: t.name.clone(),
                            detail: Self::table_detail(t),
                            insert: if catalog.is_empty() {
                                format!("{schema}.{}", t.name)
                            } else {
                                format!("{catalog}.{schema}.{}", t.name)
                            },
                            icon: SuggestionIcon::Table,
                            score: 0,
                            match_positions: Vec::new(),
                        })
                        .collect()
                } else {
                    // Fuzzy match tables in this schema
                    self.fuzzy_match_tables_in_schema(partial, &matching_tables, catalog, schema)
                }
            }

            _ => Vec::new(),
        }
    }

    /// Fuzzy match schemas.
    fn fuzzy_match_schemas(
        &mut self,
        query: &str,
        schemas: &std::collections::BTreeSet<String>,
        tables: &[TableInfo],
    ) -> Vec<Suggestion> {
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut results = Vec::new();

        for schema in schemas {
            indices.clear();
            let haystack = Utf32Str::new(schema, &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                let table_count = tables.iter().filter(|t| &t.schema == schema).count();
                results.push(Suggestion {
                    label: format!("{schema}."),
                    detail: format!("{table_count} tables"),
                    insert: format!("{schema}."),
                    icon: SuggestionIcon::Column,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize).collect(),
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Fuzzy match catalogs.
    fn fuzzy_match_catalogs(
        &mut self,
        query: &str,
        catalogs: &std::collections::BTreeSet<String>,
        schemas: &std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    ) -> Vec<Suggestion> {
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut results = Vec::new();

        for catalog in catalogs {
            indices.clear();
            let haystack = Utf32Str::new(catalog, &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                let schema_count = schemas.get(catalog).map(|s| s.len()).unwrap_or(0);
                results.push(Suggestion {
                    label: format!("{catalog}."),
                    detail: format!("{schema_count} schemas"),
                    insert: format!("{catalog}."),
                    icon: SuggestionIcon::Connection,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize).collect(),
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Fuzzy match schemas within a specific catalog.
    fn fuzzy_match_schemas_in_catalog(
        &mut self,
        query: &str,
        catalog: &str,
        schemas: &std::collections::BTreeSet<String>,
        tables: &[TableInfo],
    ) -> Vec<Suggestion> {
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut results = Vec::new();

        for schema in schemas {
            indices.clear();
            let haystack = Utf32Str::new(schema, &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                let table_count = tables
                    .iter()
                    .filter(|t| t.catalog == catalog && &t.schema == schema)
                    .count();
                results.push(Suggestion {
                    label: format!("{schema}."),
                    detail: format!("{table_count} tables"),
                    insert: format!("{catalog}.{schema}."),
                    icon: SuggestionIcon::Column,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize).collect(),
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Fuzzy match tables within a specific schema.
    fn fuzzy_match_tables_in_schema(
        &mut self,
        query: &str,
        tables: &[TableInfo],
        catalog: &str,
        schema: &str,
    ) -> Vec<Suggestion> {
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut results = Vec::new();

        for table in tables {
            indices.clear();
            let haystack = Utf32Str::new(&table.name, &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                results.push(Suggestion {
                    label: table.name.clone(),
                    detail: Self::table_detail(table),
                    insert: if catalog.is_empty() {
                        format!("{schema}.{}", table.name)
                    } else {
                        format!("{catalog}.{schema}.{}", table.name)
                    },
                    icon: SuggestionIcon::Table,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize).collect(),
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Generate detail string for a table suggestion.
    fn table_detail(t: &TableInfo) -> String {
        let mut parts = Vec::new();

        // Show schema.catalog if available
        if !t.schema.is_empty() && !t.catalog.is_empty() {
            let catalog = &t.catalog;
            let schema = &t.schema;
            parts.push(format!("{catalog}.{schema}"));
        } else if !t.schema.is_empty() {
            parts.push(t.schema.clone());
        }

        // Show column count if available
        if !t.columns.is_empty() {
            let cols = t.columns.len();
            parts.push(format!("{cols} cols"));
        }

        // Show row count if known
        if let Some(rows) = t.row_count {
            if rows > 0 {
                parts.push(format!("{rows} rows"));
            }
        }

        if parts.is_empty() {
            "table".to_string()
        } else {
            parts.join(" · ")
        }
    }

    /// Extract table references from FROM/JOIN clauses in the current SQL input.
    fn extract_from_tables(&self, input: &str) -> Vec<(String, TableInfo)> {
        let tables = match self.active_connection() {
            Some(conn) => &conn.tables,
            None => return Vec::new(),
        };

        let words: Vec<&str> = input.split_whitespace().collect();
        let upper_words: Vec<String> = words.iter().map(|w| w.to_uppercase()).collect();
        let mut result = Vec::new();

        // Non-table keywords that indicate end of a table reference
        const CLAUSE_KEYWORDS: &[&str] = &[
            "WHERE",
            "JOIN",
            "INNER",
            "LEFT",
            "RIGHT",
            "FULL",
            "CROSS",
            "ON",
            "GROUP",
            "ORDER",
            "HAVING",
            "LIMIT",
            "UNION",
            "INTERSECT",
            "EXCEPT",
            "SET",
            "SELECT",
            "VALUES",
        ];

        for (i, uw) in upper_words.iter().enumerate() {
            if (*uw == "FROM" || *uw == "JOIN") && i + 1 < words.len() {
                let table_ref = words[i + 1].trim_end_matches(',');
                // Handle schema.table or just table
                let table_name = table_ref.rsplit('.').next().unwrap_or(table_ref);

                if let Some(table_info) = tables
                    .iter()
                    .find(|t| t.name.eq_ignore_ascii_case(table_name))
                {
                    // Check for alias: FROM table AS alias, or FROM table alias
                    let alias = if i + 2 < upper_words.len() {
                        if upper_words[i + 2] == "AS" && i + 3 < words.len() {
                            words[i + 3].trim_end_matches(',').to_string()
                        } else if !CLAUSE_KEYWORDS
                            .contains(&upper_words[i + 2].trim_end_matches(','))
                            && !upper_words[i + 2].starts_with(',')
                        {
                            words[i + 2].trim_end_matches(',').to_string()
                        } else {
                            table_name.to_string()
                        }
                    } else {
                        table_name.to_string()
                    };

                    result.push((alias, table_info.clone()));
                }
            }
        }

        result
    }

    /// Get column suggestions from the given tables, with optional fuzzy matching.
    fn get_column_suggestions(
        &mut self,
        partial: &str,
        tables: &[(String, TableInfo)],
    ) -> Vec<Suggestion> {
        let mut results = Vec::new();

        // Handle table.column prefix pattern (e.g., "t." or "users.")
        if let Some(dot_pos) = partial.rfind('.') {
            let table_prefix = &partial[..dot_pos];
            let col_partial = &partial[dot_pos + 1..];

            let matching_tables: Vec<_> = tables
                .iter()
                .filter(|(alias, t)| {
                    alias.eq_ignore_ascii_case(table_prefix)
                        || t.name.eq_ignore_ascii_case(table_prefix)
                })
                .collect();

            for (alias, table_info) in &matching_tables {
                if col_partial.is_empty() {
                    for col in &table_info.columns {
                        results.push(Suggestion {
                            label: col.name.clone(),
                            detail: format!("{} · {}", alias, col.data_type),
                            insert: format!("{table_prefix}.{}", col.name),
                            icon: SuggestionIcon::Column,
                            score: 0,
                            match_positions: Vec::new(),
                        });
                    }
                } else {
                    let pattern = Pattern::new(
                        col_partial,
                        CaseMatching::Ignore,
                        Normalization::Smart,
                        AtomKind::Fuzzy,
                    );
                    let mut indices: Vec<u32> = Vec::new();
                    let mut buf = Vec::new();

                    for col in &table_info.columns {
                        indices.clear();
                        let haystack = Utf32Str::new(&col.name, &mut buf);
                        if let Some(score) =
                            pattern.indices(haystack, &mut self.matcher, &mut indices)
                        {
                            results.push(Suggestion {
                                label: col.name.clone(),
                                detail: format!("{} · {}", alias, col.data_type),
                                insert: format!("{table_prefix}.{}", col.name),
                                icon: SuggestionIcon::Column,
                                score: i64::from(score),
                                match_positions: indices.iter().map(|&i| i as usize).collect(),
                            });
                        }
                    }
                }
            }

            results.sort_by(|a, b| b.score.cmp(&a.score));
            return results;
        }

        if partial.is_empty() {
            // Show all columns from all referenced tables
            for (alias, table_info) in tables {
                for col in &table_info.columns {
                    results.push(Suggestion {
                        label: col.name.clone(),
                        detail: format!("{alias} · {}", col.data_type),
                        insert: col.name.clone(),
                        icon: SuggestionIcon::Column,
                        score: 0,
                        match_positions: Vec::new(),
                    });
                }
            }
            return results;
        }

        // Fuzzy match all columns across tables
        let pattern = Pattern::new(
            partial,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();

        for (alias, table_info) in tables {
            for col in &table_info.columns {
                indices.clear();
                let haystack = Utf32Str::new(&col.name, &mut buf);
                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    results.push(Suggestion {
                        label: col.name.clone(),
                        detail: format!("{alias} · {}", col.data_type),
                        insert: col.name.clone(),
                        icon: SuggestionIcon::Column,
                        score: i64::from(score),
                        match_positions: indices.iter().map(|&i| i as usize).collect(),
                    });
                }
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    /// Get keyword and function suggestions for a partial word.
    fn get_keyword_suggestions(&mut self, partial: &str) -> Vec<Suggestion> {
        use super::super::highlighting::{SQL_FUNCTIONS, SQL_KEYWORDS};

        if partial.len() < 2 {
            return Vec::new();
        }

        let pattern = Pattern::new(
            partial,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();
        let mut results = Vec::new();

        for kw in SQL_KEYWORDS {
            indices.clear();
            let haystack = Utf32Str::new(kw, &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                results.push(Suggestion {
                    label: kw.to_string(),
                    detail: "keyword".to_string(),
                    insert: kw.to_string(),
                    icon: SuggestionIcon::Keyword,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize).collect(),
                });
            }
        }

        for func in SQL_FUNCTIONS {
            indices.clear();
            let haystack = Utf32Str::new(func, &mut buf);
            if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                results.push(Suggestion {
                    label: func.to_string(),
                    detail: "function".to_string(),
                    insert: format!("{func}("),
                    icon: SuggestionIcon::Function,
                    score: i64::from(score),
                    match_positions: indices.iter().map(|&i| i as usize).collect(),
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(15);
        results
    }

    /// Render mode badge (for diff, explain, etc.).
    fn render_mode_badge(&self, ui: &mut egui::Ui) {
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        match &self.mode {
            SqlMode::Normal => {}
            SqlMode::Diff { left, right } => {
                let left_name = self
                    .connections
                    .iter()
                    .find(|c| c.id == *left)
                    .map(|c| c.name.as_str())
                    .unwrap_or("?");
                let right_name = self
                    .connections
                    .iter()
                    .find(|c| c.id == *right)
                    .map(|c| c.name.as_str())
                    .unwrap_or("?");

                egui::Frame::new()
                    .fill(accent.gamma_multiply(0.1))
                    .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("DIFF").color(accent).size(10.0).strong());
                            ui.label(
                                RichText::new(format!("{left_name} ↔ {right_name}"))
                                    .color(text_secondary)
                                    .size(11.0),
                            );
                        });
                    });
                ui.add_space(8.0);
            }
            SqlMode::Explain => {
                egui::Frame::new()
                    .fill(accent.gamma_multiply(0.1))
                    .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.label(RichText::new("EXPLAIN").color(accent).size(10.0).strong());
                    });
                ui.add_space(8.0);
            }
        }
    }

    /// Render suggestions popup above input.
    fn render_suggestions_popup(&mut self, ui: &mut egui::Ui, accent: Color32) {
        if !self.suggestions.visible || self.suggestions.items.is_empty() {
            return;
        }

        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let bg_elevated = self.theme.bg_elevated();
        let border = self.theme.border_default();
        let selected_idx = self.suggestions.selected;

        // Clone suggestion data to avoid borrow issues
        let suggestions: Vec<_> = self
            .suggestions
            .items
            .iter()
            .map(|s| {
                (
                    s.label.clone(),
                    s.detail.clone(),
                    s.icon,
                    s.match_positions.clone(),
                )
            })
            .collect();

        let mut clicked_idx: Option<usize> = None;

        // Limit max height to show ~8 items (each row ~30px)
        let max_height = 250.0;

        egui::Frame::new()
            .fill(bg_elevated)
            .stroke(egui::Stroke::new(1.0, border))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(4, 4))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(max_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (idx, (label, detail, icon_type, match_positions)) in
                            suggestions.iter().enumerate()
                        {
                            let is_selected = idx == selected_idx;
                            let row_bg = if is_selected {
                                accent.gamma_multiply(0.15)
                            } else {
                                Color32::TRANSPARENT
                            };

                            let row = egui::Frame::new()
                                .fill(row_bg)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(10, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Icon
                                        let icon = icon_type.icon_str();
                                        ui.label(
                                            RichText::new(icon)
                                                .color(if is_selected {
                                                    accent
                                                } else {
                                                    text_secondary
                                                })
                                                .size(12.0),
                                        );

                                        ui.add_space(8.0);

                                        // Label with match position highlighting
                                        if !match_positions.is_empty() && !is_selected {
                                            let font_id = egui::FontId::proportional(12.0);
                                            let mut label_job = egui::text::LayoutJob::default();
                                            for (char_idx, ch) in label.chars().enumerate() {
                                                let color = if match_positions.contains(&char_idx) {
                                                    accent
                                                } else {
                                                    text_primary
                                                };
                                                let mut buf = [0u8; 4];
                                                let s = ch.encode_utf8(&mut buf);
                                                label_job.append(
                                                    s,
                                                    0.0,
                                                    TextFormat::simple(font_id.clone(), color),
                                                );
                                            }
                                            ui.label(label_job);
                                        } else {
                                            ui.label(
                                                RichText::new(label)
                                                    .color(if is_selected {
                                                        accent
                                                    } else {
                                                        text_primary
                                                    })
                                                    .size(12.0),
                                            );
                                        }

                                        // Detail (right-aligned)
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(detail)
                                                        .color(text_secondary.gamma_multiply(0.7))
                                                        .size(11.0),
                                                );
                                            },
                                        );
                                    });
                                });

                            // Scroll to selected item
                            if is_selected {
                                row.response.scroll_to_me(Some(egui::Align::Center));
                            }

                            if row.response.clicked() {
                                clicked_idx = Some(idx);
                            }
                        }
                    });
            });

        // Handle click outside the borrow
        if let Some(idx) = clicked_idx {
            self.insert_suggestion(idx);
        }

        ui.add_space(8.0);
    }

    /// Insert the selected suggestion into the input.
    fn insert_suggestion(&mut self, idx: usize) {
        if let Some(suggestion) = self.suggestions.items.get(idx) {
            let input = self.input.trim();

            if suggestion.icon == SuggestionIcon::Command {
                // Replace entire input with command
                self.input = suggestion.insert.clone();
            } else {
                // For functions ending with '(', don't add trailing space
                let suffix = if suggestion.insert.ends_with('(') {
                    ""
                } else {
                    " "
                };
                // Replace last partial word with suggestion
                let words: Vec<&str> = input.split_whitespace().collect();
                if words.len() >= 2 {
                    let prefix = words[..words.len() - 1].join(" ");
                    self.input = format!("{} {}{}", prefix, suggestion.insert, suffix);
                } else {
                    self.input = format!("{}{}", suggestion.insert, suffix);
                }
            }

            self.suggestions.visible = false;
            self.move_cursor_to_end = true;
        }
    }

    /// Render the main input bar with SQL syntax highlighting.
    fn render_input_bar(&mut self, ui: &mut egui::Ui, accent: Color32) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();

        // Input container
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .stroke(egui::Stroke::new(
                1.0,
                if self.input_focused {
                    accent.gamma_multiply(0.5)
                } else {
                    self.theme.border_default()
                },
            ))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                // Force the frame to fill the available width so it aligns
                // with the query cards in the scroll area above.
                ui.set_width(ui.available_width());

                ui.horizontal(|ui| {
                    // Prompt indicator
                    let prompt = match &self.mode {
                        SqlMode::Normal => "SQL",
                        SqlMode::Diff { .. } => "DIFF",
                        SqlMode::Explain => "EXPLAIN",
                    };
                    ui.label(RichText::new(prompt).color(accent).size(11.0).strong());

                    ui.label(
                        RichText::new(">")
                            .color(text_secondary.gamma_multiply(0.5))
                            .size(12.0),
                    );

                    ui.add_space(8.0);

                    // Connection indicator (small pill) — always show if connections exist
                    if !self.connections.is_empty() {
                        let (pill_label, dot_color, label_color) = if let Some(conn) =
                            self.active_connection()
                        {
                            let dot = match &conn.state {
                                ConnectionState::Connected => self.theme.semantic_success(),
                                ConnectionState::Connecting => accent,
                                ConnectionState::Failed(_) => self.theme.semantic_error(),
                                ConnectionState::Disconnected => text_secondary.gamma_multiply(0.5),
                            };
                            (conn.name.clone(), dot, text_secondary)
                        } else {
                            (
                                "Not connected".to_string(),
                                accent.gamma_multiply(0.5),
                                accent.gamma_multiply(0.7),
                            )
                        };

                        let pill_resp = ui.add(
                            egui::Button::new(
                                RichText::new(format!("● {pill_label}"))
                                    .color(label_color)
                                    .size(10.0),
                            )
                            .fill(self.theme.bg_surface())
                            .stroke(egui::Stroke::new(1.0, dot_color.gamma_multiply(0.4)))
                            .corner_radius(10.0),
                        );
                        self.pill_rect = pill_resp.rect;
                        if pill_resp.clicked() {
                            let was_closed = self.sidebar_width == 0.0;
                            self.sidebar_width = if was_closed { 1.0 } else { 0.0 };
                            self.popup_just_opened = was_closed;
                        }
                        pill_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                        ui.add_space(8.0);
                    }

                    // Main text input with syntax highlighting
                    let theme = self.theme;
                    let table_names: Vec<String> = self
                        .active_connection()
                        .map(|c| c.tables.iter().map(|t| t.name.clone()).collect())
                        .unwrap_or_default();
                    let mut layouter =
                        move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                            let refs: Vec<&str> = table_names.iter().map(|s| s.as_str()).collect();
                            let mut job = highlight_sql(text.as_str(), theme, &refs);
                            job.wrap.max_width = wrap_width;
                            ui.fonts_mut(|f| f.layout_job(job))
                        };

                    // Use stable ID for focus tracking
                    let input_id = egui::Id::new(format!("sql_input_{}", self.id));

                    // Intercept bare Enter BEFORE TextEdit to submit query.
                    // Shift+Enter passes through to TextEdit naturally as a newline.
                    let mut enter_to_submit = false;
                    let mut history_up = false;
                    let mut history_down = false;
                    let has_input_focus =
                        ui.ctx().memory(|m| m.has_focus(input_id)) || self.input_focused;
                    if has_input_focus {
                        ui.ctx().input_mut(|input| {
                            // When suggestions are visible, Enter accepts the suggestion
                            if self.suggestions.visible {
                                if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                                    // Will be handled below as suggestion insert
                                }
                            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                                enter_to_submit = true;
                            }
                            // Ctrl/Cmd+Enter always submits
                            if input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                                || input.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)
                            {
                                enter_to_submit = true;
                            }
                            // Consume Tab when suggestions aren't visible to prevent indentation
                            if !self.suggestions.visible {
                                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                                // Consume Up/Down for input history navigation
                                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                                    history_up = true;
                                }
                                if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                                    history_down = true;
                                }
                            }
                        });
                    }

                    // Auto-expand height based on line count
                    let line_count = self.input.lines().count().max(1);
                    let input_height = (line_count as f32 * 16.0).clamp(22.0, 120.0);

                    // Reserve space for the run button (32px + spacing)
                    let run_button_width = 40.0;
                    let response = ui.add_sized(
                        egui::vec2((ui.available_width() - run_button_width).max(1.0), input_height),
                        TextEdit::multiline(&mut self.input)
                            .id(input_id)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(1)
                            .frame(false)
                            .layouter(&mut layouter)
                            .hint_text(
                                RichText::new("SELECT * FROM ... or / for commands")
                                    .color(text_secondary.gamma_multiply(0.4))
                                    .monospace(),
                            ),
                    );

                    // Request focus on initial render or when suggestions are visible
                    if self.input_focused {
                        response.request_focus();
                        self.input_focused = false;
                    } else if self.suggestions.visible && !response.has_focus() {
                        response.request_focus();
                    }

                    // (No multi-cell navigation — input always has focus)

                    // Move cursor to end after inserting suggestion
                    if self.move_cursor_to_end {
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), input_id) {
                            let cursor = egui::text::CCursor::new(self.input.len());
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                            state.store(ui.ctx(), input_id);
                        }
                        self.move_cursor_to_end = false;
                    }

                    // Handle keyboard navigation for suggestions
                    if response.has_focus() {
                        // Handle Escape key — dismiss suggestions
                        if ui.input(|i| i.key_pressed(egui::Key::Escape))
                            && self.suggestions.visible
                        {
                            self.suggestions.visible = false;
                        }

                        // Up/Down for suggestion navigation (only when suggestions visible)
                        if self.suggestions.visible {
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                                && self.suggestions.selected > 0
                            {
                                self.suggestions.selected -= 1;
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                                && self.suggestions.selected
                                    < self.suggestions.items.len().saturating_sub(1)
                            {
                                self.suggestions.selected += 1;
                            }
                            // Tab or Enter to insert suggestion
                            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                self.insert_suggestion(self.suggestions.selected);
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                self.insert_suggestion(self.suggestions.selected);
                            }
                        }
                    }

                    // Handle input history navigation
                    if history_up && !self.input_history.is_empty() {
                        match self.history_index {
                            None => {
                                // Entering history mode — save current input
                                self.history_saved_input = self.input.clone();
                                self.history_index = Some(self.input_history.len() - 1);
                                self.input = self.input_history.last().unwrap().clone();
                                self.move_cursor_to_end = true;
                            }
                            Some(idx) if idx > 0 => {
                                self.history_index = Some(idx - 1);
                                self.input = self.input_history[idx - 1].clone();
                                self.move_cursor_to_end = true;
                            }
                            _ => {} // Already at oldest entry
                        }
                    }
                    if history_down {
                        if let Some(idx) = self.history_index {
                            if idx < self.input_history.len() - 1 {
                                self.history_index = Some(idx + 1);
                                self.input = self.input_history[idx + 1].clone();
                                self.move_cursor_to_end = true;
                            } else {
                                // Return to saved input
                                self.history_index = None;
                                self.input = self.history_saved_input.clone();
                                self.move_cursor_to_end = true;
                            }
                        }
                    }

                    // Execute after all UI is rendered
                    if enter_to_submit {
                        self.execute_input();
                    }

                    // Run button (small, subtle)
                    let has_connection = self.active_connection().is_some();
                    let run_btn = ui.add_enabled(
                        has_connection && !self.input.trim().is_empty(),
                        egui::Button::new(RichText::new("↵").color(text_primary).size(11.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(32.0, 20.0)),
                    );
                    if run_btn.clicked() {
                        self.execute_input();
                    }
                    run_btn.on_hover_text("Run query (Enter)");
                });
            });
    }

    /// Render input hints line.
    fn render_input_hints(&self, ui: &mut egui::Ui, text_secondary: Color32) {
        let hint_color = text_secondary.gamma_multiply(0.5);
        let dot_color = text_secondary.gamma_multiply(0.3);

        /// Render a single hint label.
        fn hint(ui: &mut egui::Ui, text: &str, color: Color32) {
            ui.label(RichText::new(text).color(color).size(10.0));
        }
        /// Render a dot separator.
        fn dot(ui: &mut egui::Ui, color: Color32) {
            ui.label(RichText::new("·").color(color).size(10.0));
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            // === INPUT mode hints ===
            if self.suggestions.visible {
                hint(ui, "↑↓ navigate", hint_color);
                dot(ui, dot_color);
                hint(ui, "Tab insert", hint_color);
                dot(ui, dot_color);
            }
            hint(ui, "↵ run", hint_color);
            dot(ui, dot_color);
            hint(ui, "↑↓ history", hint_color);
            dot(ui, dot_color);
            hint(ui, "⇧↵ newline", hint_color);

            // Connection status on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(conn) = self.active_connection() {
                    let is_connected = matches!(conn.state, ConnectionState::Connected);
                    let status_color = if is_connected {
                        self.theme.semantic_success()
                    } else {
                        text_secondary.gamma_multiply(0.4)
                    };
                    ui.label(RichText::new("●").color(status_color).size(8.0));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&conn.name)
                            .color(text_secondary.gamma_multiply(0.6))
                            .size(10.0),
                    );
                } else if self.connections.is_empty() {
                    ui.label(
                        RichText::new("No connections · configure in Settings")
                            .color(text_secondary.gamma_multiply(0.5))
                            .size(10.0),
                    );
                }
            });
        });
    }

    /// Take the pending action, if any.
    pub fn take_action(&mut self) -> SqlPaneAction {
        std::mem::replace(&mut self.pending_action, SqlPaneAction::None)
    }

    /// Synchronize connections from the Settings-defined list.
    ///
    /// Preserves existing connections that match by (name, endpoint) so
    /// runtime state (connected, tables, active) is kept. Creates new
    /// entries for additions and removes connections no longer in Settings.
    pub fn sync_connections(
        &mut self,
        definitions: &[crate::ui::settings_screen::FlightSqlConnection],
    ) {
        log::debug!(
            "SqlPane::sync_connections: received {} definitions, had {} connections",
            definitions.len(),
            self.connections.len()
        );
        let mut new_connections = Vec::with_capacity(definitions.len());
        for def in definitions {
            // Try to find an existing connection with the same name+endpoint
            if let Some(pos) = self
                .connections
                .iter()
                .position(|c| c.name == def.name && c.endpoint == def.endpoint)
            {
                // Reuse existing (preserves state, tables, active)
                new_connections.push(self.connections.swap_remove(pos));
            } else {
                // New definition — create a fresh SavedConnection
                log::debug!(
                    "SqlPane::sync_connections: creating new connection '{}' -> '{}'",
                    def.name,
                    def.endpoint
                );
                new_connections.push(SavedConnection::new(&def.name, &def.endpoint));
            }
        }
        self.connections = new_connections;
        log::debug!(
            "SqlPane::sync_connections: now has {} connections",
            self.connections.len()
        );

        // Auto-connect to the first connection on initial sync
        if !self.auto_connected
            && self.active_connection().is_none()
            && !self.connections.is_empty()
        {
            self.auto_connected = true;
            let first_id = self.connections[0].id;
            log::debug!("SqlPane: auto-connecting to '{}'", self.connections[0].name);
            self.connect_saved(first_id);
        }
    }

    /// Convert the result cell to an InlineTable.
    /// Truncates to at most 20 rows of pre-formatted string data.
    fn result_to_inline_table(
        &self,
    ) -> Option<crate::components::pane::inline_content::InlineTable> {
        use crate::components::pane::inline_content::{InlineTable, InlineTableColumn};

        let cell = self.result_cell.as_ref()?;
        let schema = cell.schema()?;
        if cell.batches().is_empty() {
            return None;
        }

        let columns: Vec<InlineTableColumn> = schema
            .fields()
            .iter()
            .map(|f| InlineTableColumn {
                name: f.name().clone(),
                data_type: format!("{}", f.data_type()),
            })
            .collect();

        let total_rows: usize = cell.batches().iter().map(|b| b.num_rows()).sum();
        let max_rows = 20;
        let mut rows = Vec::new();
        'outer: for batch in cell.batches() {
            for row_idx in 0..batch.num_rows() {
                if rows.len() >= max_rows {
                    break 'outer;
                }
                let row: Vec<String> = (0..batch.num_columns())
                    .map(|col| format_array_value(batch.column(col).as_ref(), row_idx))
                    .collect();
                rows.push(row);
            }
        }

        Some(InlineTable {
            title: cell.sql().to_string(),
            columns,
            rows,
            total_rows,
            execution_time_ms: cell.stats().map(|s| s.total_time.as_millis() as u64),
        })
    }

    /// Get the result cell as an InlineTable.
    /// Used by the workspace to serve `show_inline_table` agent commands.
    pub fn get_inline_table(
        &self,
        query: Option<&str>,
    ) -> Option<crate::components::pane::inline_content::InlineTable> {
        // With single-cell model, optionally filter by query match
        if let Some(q) = query {
            let cell = self.result_cell.as_ref()?;
            if cell.sql().trim() != q.trim() {
                return None;
            }
        }
        self.result_to_inline_table()
    }

    /// Copy text to the system clipboard (no-op on WASM).
    fn copy_to_clipboard(&mut self, text: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(text);
            }
        }
        let _ = text; // suppress unused warning on WASM
        self.copied_feedback = Some(crate::util::Instant::now());
    }

    /// Format query results as tab-separated values for clipboard.
    fn format_results_as_tsv(schema: &SchemaRef, batches: &[RecordBatch]) -> String {
        let mut out = String::new();

        // Header row
        let fields = schema.fields();
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            out.push_str(field.name());
        }
        out.push('\n');

        // Data rows
        for batch in batches {
            for row in 0..batch.num_rows() {
                for col in 0..batch.num_columns() {
                    if col > 0 {
                        out.push('\t');
                    }
                    out.push_str(&format_array_value(batch.column(col).as_ref(), row));
                }
                out.push('\n');
            }
        }

        out
    }

    /// Format a plan tree as indented text for clipboard.
    fn format_plan_as_text(node: &PlanNode, depth: usize) -> String {
        let mut out = String::new();
        let indent = "  ".repeat(depth);

        out.push_str(&indent);
        out.push_str(&node.operator);
        if let Some(metrics) = &node.metrics {
            let mut parts = Vec::new();
            if !metrics.elapsed_time.is_zero() {
                parts.push(format_duration(metrics.elapsed_time));
            }
            if metrics.output_rows > 0 {
                parts.push(format!("{} rows", format_rows(metrics.output_rows)));
            }
            if !parts.is_empty() {
                out.push_str(&format!(" [{}]", parts.join(", ")));
            }
        }
        out.push('\n');

        if !node.description.is_empty() {
            out.push_str(&indent);
            out.push_str("  ");
            out.push_str(&node.description);
            out.push('\n');
        }

        for child in &node.children {
            out.push_str(&Self::format_plan_as_text(child, depth + 1));
        }

        out
    }

    /// Whether the "Copied!" feedback badge should be shown.
    fn show_copied_badge(&self) -> bool {
        self.copied_feedback
            .is_some_and(|t| t.elapsed().as_secs_f32() < 1.5)
    }

    /// Extract snapshot data from the SQL pane's query history.
    ///
    /// Converts each Cell to a SnapshotQueryCell, preserving cell kind
    /// (Query, Info, Diff, Explain). Skips in-progress queries.
    /// Returns None if no cells to snapshot.
    pub fn extract_snapshot_data(&self) -> Option<enya_config::SnapshotSqlPane> {
        use super::types::ColumnDiffStatus;
        use enya_config::{
            SnapshotBenchmarkData, SnapshotCellKind, SnapshotColumnDiffStatus, SnapshotDiffData,
            SnapshotDiffStats, SnapshotDiffType, SnapshotOperatorMetrics, SnapshotPhaseTiming,
            SnapshotPlanNode, SnapshotQueryCell, SnapshotQueryStats, SnapshotSchemaDiff,
            SnapshotSchemaDiffColumn, SnapshotSqlPane, SnapshotTableColumn,
        };

        let max_rows_per_cell = 500;

        fn plan_node_to_snapshot(node: &PlanNode) -> SnapshotPlanNode {
            SnapshotPlanNode {
                operator: node.operator.clone(),
                description: node.description.clone(),
                properties: node
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                children: node.children.iter().map(plan_node_to_snapshot).collect(),
                metrics: node.metrics.as_ref().map(|m| SnapshotOperatorMetrics {
                    output_rows: m.output_rows as u64,
                    elapsed_time_ms: m.elapsed_time.as_millis() as u64,
                    memory_bytes: m.memory_bytes as u64,
                    spill_count: m.spill_count as u32,
                    spill_bytes: m.spill_bytes as u64,
                }),
            }
        }

        fn extract_batches(
            schema: Option<&SchemaRef>,
            batches: &[RecordBatch],
            max_rows: usize,
        ) -> (Vec<SnapshotTableColumn>, Vec<Vec<String>>, u64) {
            if let Some(schema) = schema {
                let columns: Vec<SnapshotTableColumn> = schema
                    .fields()
                    .iter()
                    .map(|f| SnapshotTableColumn {
                        name: f.name().clone(),
                        data_type: format!("{}", f.data_type()),
                    })
                    .collect();

                let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
                let mut rows = Vec::new();
                'outer: for batch in batches {
                    for row_idx in 0..batch.num_rows() {
                        if rows.len() >= max_rows {
                            break 'outer;
                        }
                        let row: Vec<String> = (0..batch.num_columns())
                            .map(|col| format_array_value(batch.column(col).as_ref(), row_idx))
                            .collect();
                        rows.push(row);
                    }
                }
                (columns, rows, total_rows as u64)
            } else {
                (Vec::new(), Vec::new(), 0)
            }
        }

        let cells: Vec<SnapshotQueryCell> = self
            .result_cell
            .iter()
            .filter(|cell| {
                // Skip running cells (incomplete data)
                let status = cell.status();
                status == QueryStatus::Completed || status == QueryStatus::Failed
            })
            .map(|cell| match &cell.kind {
                CellKind::Query(q) => {
                    let (columns, rows, total_rows) =
                        extract_batches(q.schema.as_ref(), &q.batches, max_rows_per_cell);
                    let stats = q.stats.as_ref().map(|s| SnapshotQueryStats {
                        total_time_ms: s.total_time.as_millis() as u64,
                        planning_time_ms: s.planning_time.as_millis() as u64,
                        execution_time_ms: s.execution_time.as_millis() as u64,
                        rows_returned: s.rows_returned as u64,
                        bytes_scanned: s.bytes_scanned as u64,
                        partitions_scanned: s.partitions_scanned as u32,
                    });
                    SnapshotQueryCell {
                        kind: SnapshotCellKind::Query,
                        sql: cell.sql().to_string(),
                        columns,
                        rows,
                        total_rows,
                        stats,
                        error: q.error.clone(),
                        plan: None,
                        diff: None,
                        benchmark: None,
                        describe: None,
                    }
                }
                CellKind::Info(i) => SnapshotQueryCell {
                    kind: SnapshotCellKind::Info,
                    sql: cell.sql().to_string(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    stats: None,
                    error: i.error.clone(),
                    plan: None,
                    diff: None,
                    benchmark: None,
                    describe: None,
                },
                CellKind::Diff(d) => {
                    let diff_data = d.diff_result.as_ref().map(|dr| {
                        let (left_columns, left_rows, left_total_rows) = extract_batches(
                            dr.left_schema.as_ref(),
                            &dr.left_batches,
                            max_rows_per_cell,
                        );
                        let (right_columns, right_rows, right_total_rows) = extract_batches(
                            dr.right_schema.as_ref(),
                            &dr.right_batches,
                            max_rows_per_cell,
                        );
                        SnapshotDiffData {
                            left_name: dr.left_name.clone(),
                            right_name: dr.right_name.clone(),
                            left_columns,
                            left_rows,
                            left_total_rows,
                            left_error: dr.left_error.clone(),
                            right_columns,
                            right_rows,
                            right_total_rows,
                            right_error: dr.right_error.clone(),
                            schemas_match: dr.schemas_match,
                            diff_stats: dr.diff_stats.as_ref().map(|s| SnapshotDiffStats {
                                left_only: s.left_only as u64,
                                right_only: s.right_only as u64,
                                different: s.different as u64,
                                matching: s.matching as u64,
                            }),
                            left_plan: dr.left_plan.as_ref().map(plan_node_to_snapshot),
                            right_plan: dr.right_plan.as_ref().map(plan_node_to_snapshot),
                            diff_type: match dr.diff_type {
                                DiffType::Data => SnapshotDiffType::Data,
                                DiffType::Plan => SnapshotDiffType::Plan,
                                DiffType::Schema => SnapshotDiffType::Schema,
                                DiffType::Profile => SnapshotDiffType::Profile,
                            },
                            schema_diff: dr.schema_diff.as_ref().map(|sd| SnapshotSchemaDiff {
                                table_name: sd.table_name.clone(),
                                columns: sd
                                    .columns
                                    .iter()
                                    .map(|c| SnapshotSchemaDiffColumn {
                                        name: c.name.clone(),
                                        left_type: c.left_type.clone(),
                                        left_nullable: c.left_nullable,
                                        right_type: c.right_type.clone(),
                                        right_nullable: c.right_nullable,
                                        status: match c.status {
                                            ColumnDiffStatus::Matching => {
                                                SnapshotColumnDiffStatus::Matching
                                            }
                                            ColumnDiffStatus::LeftOnly => {
                                                SnapshotColumnDiffStatus::LeftOnly
                                            }
                                            ColumnDiffStatus::RightOnly => {
                                                SnapshotColumnDiffStatus::RightOnly
                                            }
                                            ColumnDiffStatus::Changed => {
                                                SnapshotColumnDiffStatus::Changed
                                            }
                                        },
                                    })
                                    .collect(),
                                matching: sd.matching as u64,
                                left_only: sd.left_only as u64,
                                right_only: sd.right_only as u64,
                                changed: sd.changed as u64,
                            }),
                        }
                    });
                    SnapshotQueryCell {
                        kind: SnapshotCellKind::Diff,
                        sql: cell.sql().to_string(),
                        columns: Vec::new(),
                        rows: Vec::new(),
                        total_rows: 0,
                        stats: None,
                        error: d.error.clone(),
                        plan: None,
                        diff: diff_data,
                        benchmark: None,
                        describe: None,
                    }
                }
                CellKind::Explain(_e) => SnapshotQueryCell {
                    kind: SnapshotCellKind::Explain,
                    sql: cell.sql().to_string(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    stats: None,
                    error: cell.get_error().map(|e| e.to_string()),
                    plan: None,
                    diff: None,
                    benchmark: None,
                    describe: None,
                },
                CellKind::Benchmark(b) => {
                    let phase_to_snapshot =
                        |p: &enya_datafusion::PhaseTiming| SnapshotPhaseTiming {
                            min_us: p.min.as_micros() as u64,
                            max_us: p.max.as_micros() as u64,
                            mean_us: p.mean.as_micros() as u64,
                            median_us: p.median.as_micros() as u64,
                            percent_of_total: p.percent_of_total,
                        };
                    let benchmark = b.stats.as_ref().map(|s| SnapshotBenchmarkData {
                        iterations: s.iterations as u64,
                        rows_per_iteration: s.rows_per_iteration as u64,
                        logical_planning: phase_to_snapshot(&s.logical_planning),
                        physical_planning: phase_to_snapshot(&s.physical_planning),
                        execution: phase_to_snapshot(&s.execution),
                        total: phase_to_snapshot(&s.total),
                    });
                    SnapshotQueryCell {
                        kind: SnapshotCellKind::Benchmark,
                        sql: cell.sql().to_string(),
                        columns: Vec::new(),
                        rows: Vec::new(),
                        total_rows: 0,
                        stats: None,
                        error: b.error.clone(),
                        plan: None,
                        diff: None,
                        benchmark,
                        describe: None,
                    }
                }
                CellKind::Describe(d) => {
                    let describe = d.stats.as_ref().map(|s| enya_config::SnapshotDescribeData {
                        table_name: s.table_name.clone(),
                        total_rows: s.total_rows as u64,
                        columns: s
                            .columns
                            .iter()
                            .map(|c| enya_config::SnapshotColumnStats {
                                name: c.name.clone(),
                                data_type: c.data_type.clone(),
                                count: c.count as u64,
                                null_count: c.null_count as u64,
                                distinct_count: c.distinct_count as u64,
                                min: c.min.clone(),
                                max: c.max.clone(),
                                mean: c.mean,
                            })
                            .collect(),
                        elapsed_ms: s.elapsed.as_millis() as u64,
                    });
                    SnapshotQueryCell {
                        kind: SnapshotCellKind::Describe,
                        sql: cell.sql().to_string(),
                        columns: Vec::new(),
                        rows: Vec::new(),
                        total_rows: 0,
                        stats: None,
                        error: d.error.clone(),
                        plan: None,
                        diff: None,
                        benchmark: None,
                        describe,
                    }
                }
            })
            .collect();

        if cells.is_empty() {
            return None;
        }

        // Attach plan from plan_viewer to the last explain or query cell
        let mut cells = cells;
        if let Some(root) = self.plan_viewer.root_plan() {
            // Prefer attaching to the last explain cell, fall back to last cell
            let target = cells
                .iter()
                .rposition(|c| c.kind == SnapshotCellKind::Explain)
                .or_else(|| cells.len().checked_sub(1));
            if let Some(idx) = target {
                cells[idx].plan = Some(plan_node_to_snapshot(root));
            }
        }

        Some(SnapshotSqlPane { cells })
    }

    /// Load snapshot data into the SQL pane, replacing current history.
    ///
    /// Dispatches on cell kind to create the appropriate Cell variant.
    /// String row data is converted back into Arrow StringArray RecordBatches
    /// so the existing rendering code works unchanged.
    pub fn load_snapshot_data(&mut self, data: &enya_config::SnapshotSqlPane) {
        use enya_config::SnapshotCellKind;
        use enya_datafusion::arrow::array::StringArray;
        use enya_datafusion::arrow::datatypes::{DataType, Field, Schema};
        use std::sync::Arc;

        use super::types::{
            ColumnDiffStatus, DiffStats, DiffType, SchemaDiffColumn, SchemaDiffResult,
        };

        fn strings_to_batch(
            columns: &[enya_config::SnapshotTableColumn],
            rows: &[Vec<String>],
        ) -> (Option<SchemaRef>, Vec<RecordBatch>) {
            if columns.is_empty() || rows.is_empty() {
                return (None, Vec::new());
            }
            let fields: Vec<Field> = columns
                .iter()
                .map(|c| Field::new(&c.name, DataType::Utf8, true))
                .collect();
            let schema = Arc::new(Schema::new(fields));
            let num_cols = columns.len();
            let arrays: Vec<Arc<dyn Array>> = (0..num_cols)
                .map(|col_idx| {
                    let values: Vec<Option<&str>> = rows
                        .iter()
                        .map(|row| {
                            row.get(col_idx)
                                .map(|s| if s == "NULL" { None } else { Some(s.as_str()) })
                                .unwrap_or(None)
                        })
                        .collect();
                    Arc::new(StringArray::from(values)) as Arc<dyn Array>
                })
                .collect();
            match RecordBatch::try_new(schema.clone(), arrays) {
                Ok(batch) => (Some(schema as SchemaRef), vec![batch]),
                Err(_) => (None, Vec::new()),
            }
        }

        fn snapshot_plan_to_node(node: &enya_config::SnapshotPlanNode) -> PlanNode {
            PlanNode {
                operator: node.operator.clone(),
                description: node.description.clone(),
                properties: node.properties.iter().cloned().collect(),
                children: node.children.iter().map(snapshot_plan_to_node).collect(),
                metrics: node
                    .metrics
                    .as_ref()
                    .map(|m| enya_datafusion::OperatorMetrics {
                        output_rows: m.output_rows as usize,
                        elapsed_time: std::time::Duration::from_millis(m.elapsed_time_ms),
                        memory_bytes: m.memory_bytes as usize,
                        spill_count: m.spill_count as usize,
                        spill_bytes: m.spill_bytes as usize,
                    }),
            }
        }

        self.result_cell = None;
        self.cell_view_state = CellViewState::default();

        for cell_data in &data.cells {
            let id = QueryId::new();

            let cell = match cell_data.kind {
                SnapshotCellKind::Query => {
                    let (schema, batches) = strings_to_batch(&cell_data.columns, &cell_data.rows);
                    let stats = cell_data.stats.as_ref().map(|s| ExecutionStats {
                        total_time: std::time::Duration::from_millis(s.total_time_ms),
                        planning_time: std::time::Duration::from_millis(s.planning_time_ms),
                        execution_time: std::time::Duration::from_millis(s.execution_time_ms),
                        rows_returned: s.rows_returned as usize,
                        bytes_scanned: s.bytes_scanned as usize,
                        partitions_scanned: s.partitions_scanned as usize,
                    });
                    Cell::query_completed(
                        cell_data.sql.clone(),
                        id,
                        schema,
                        batches,
                        stats,
                        cell_data.error.clone(),
                    )
                }
                SnapshotCellKind::Info => {
                    if let Some(error) = &cell_data.error {
                        Cell::error(error.clone())
                    } else {
                        Cell::info(cell_data.sql.clone())
                    }
                }
                SnapshotCellKind::Explain => Cell::explain(cell_data.sql.clone(), id),
                SnapshotCellKind::Diff => {
                    if let Some(diff) = &cell_data.diff {
                        let (left_schema, left_batches) =
                            strings_to_batch(&diff.left_columns, &diff.left_rows);
                        let (right_schema, right_batches) =
                            strings_to_batch(&diff.right_columns, &diff.right_rows);

                        let diff_result = DiffQueryResult {
                            left_name: diff.left_name.clone(),
                            right_name: diff.right_name.clone(),
                            left_schema,
                            left_batches,
                            left_error: diff.left_error.clone(),
                            right_schema,
                            right_batches,
                            right_error: diff.right_error.clone(),
                            schemas_match: diff.schemas_match,
                            diff_stats: diff.diff_stats.as_ref().map(|s| DiffStats {
                                left_only: s.left_only as usize,
                                right_only: s.right_only as usize,
                                different: s.different as usize,
                                matching: s.matching as usize,
                            }),
                            left_plan: diff.left_plan.as_ref().map(snapshot_plan_to_node),
                            right_plan: diff.right_plan.as_ref().map(snapshot_plan_to_node),
                            diff_type: match diff.diff_type {
                                enya_config::SnapshotDiffType::Data => DiffType::Data,
                                enya_config::SnapshotDiffType::Plan => DiffType::Plan,
                                enya_config::SnapshotDiffType::Schema => DiffType::Schema,
                                enya_config::SnapshotDiffType::Profile => DiffType::Profile,
                            },
                            schema_diff: diff.schema_diff.as_ref().map(|sd| SchemaDiffResult {
                                table_name: sd.table_name.clone(),
                                columns: sd
                                    .columns
                                    .iter()
                                    .map(|c| SchemaDiffColumn {
                                        name: c.name.clone(),
                                        left_type: c.left_type.clone(),
                                        left_nullable: c.left_nullable,
                                        right_type: c.right_type.clone(),
                                        right_nullable: c.right_nullable,
                                        status: match c.status {
                                            enya_config::SnapshotColumnDiffStatus::Matching => {
                                                ColumnDiffStatus::Matching
                                            }
                                            enya_config::SnapshotColumnDiffStatus::LeftOnly => {
                                                ColumnDiffStatus::LeftOnly
                                            }
                                            enya_config::SnapshotColumnDiffStatus::RightOnly => {
                                                ColumnDiffStatus::RightOnly
                                            }
                                            enya_config::SnapshotColumnDiffStatus::Changed => {
                                                ColumnDiffStatus::Changed
                                            }
                                        },
                                    })
                                    .collect(),
                                matching: sd.matching as usize,
                                left_only: sd.left_only as usize,
                                right_only: sd.right_only as usize,
                                changed: sd.changed as usize,
                            }),
                        };
                        Cell::diff_completed(cell_data.sql.clone(), id, diff_result)
                    } else {
                        // Diff cell with no data (shouldn't happen, but handle gracefully)
                        Cell::diff(cell_data.sql.clone(), id)
                    }
                }
                SnapshotCellKind::Benchmark => {
                    let snapshot_to_phase =
                        |p: &enya_config::SnapshotPhaseTiming| enya_datafusion::PhaseTiming {
                            min: std::time::Duration::from_micros(p.min_us),
                            max: std::time::Duration::from_micros(p.max_us),
                            mean: std::time::Duration::from_micros(p.mean_us),
                            median: std::time::Duration::from_micros(p.median_us),
                            percent_of_total: p.percent_of_total,
                        };
                    let stats =
                        cell_data
                            .benchmark
                            .as_ref()
                            .map(|b| enya_datafusion::BenchmarkStats {
                                iterations: b.iterations as usize,
                                rows_per_iteration: b.rows_per_iteration as usize,
                                logical_planning: snapshot_to_phase(&b.logical_planning),
                                physical_planning: snapshot_to_phase(&b.physical_planning),
                                execution: snapshot_to_phase(&b.execution),
                                total: snapshot_to_phase(&b.total),
                            });
                    Cell::benchmark_completed(cell_data.sql.clone(), id, stats)
                }
                SnapshotCellKind::Describe => {
                    let stats =
                        cell_data
                            .describe
                            .as_ref()
                            .map(|d| enya_datafusion::DescribeStats {
                                table_name: d.table_name.clone(),
                                total_rows: d.total_rows as usize,
                                columns: d
                                    .columns
                                    .iter()
                                    .map(|c| enya_datafusion::ColumnStats {
                                        name: c.name.clone(),
                                        data_type: c.data_type.clone(),
                                        count: c.count as usize,
                                        null_count: c.null_count as usize,
                                        distinct_count: c.distinct_count as usize,
                                        min: c.min.clone(),
                                        max: c.max.clone(),
                                        mean: c.mean,
                                    })
                                    .collect(),
                                elapsed: std::time::Duration::from_millis(d.elapsed_ms),
                            });
                    Cell::describe_completed(cell_data.sql.clone(), id, stats)
                }
            };

            // Load the last non-info cell as the result cell (single-cell model)
            if !cell.is_info() {
                self.result_cell = Some(cell);
                self.cell_view_state = CellViewState::default();
            }
        }

        // Load plan into plan_viewer from the last cell that has one
        for cell_data in data.cells.iter().rev() {
            if let Some(plan) = &cell_data.plan {
                let plan_node = snapshot_plan_to_node(plan);
                self.plan_viewer.load_plan(&plan_node);
                break;
            }
        }

        self.scroll_to_bottom = true;
    }
}

impl crate::components::Component for SqlPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        SqlPane::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.title.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        SqlPane::set_theme(self, theme);
    }

    fn label(&self) -> RichText {
        RichText::new(format!("{} {}", category::DATAFUSION, self.title))
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn set_overlay_blocks_input(&mut self, blocks: bool) {
        SqlPane::set_overlay_blocks_input(self, blocks);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
