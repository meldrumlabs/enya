//! SQL Pane - REPL-style SQL query execution using Arrow Flight SQL.
//!
//! This pane provides a SQL interface for connecting to Flight SQL servers
//! (DataFusion, DuckDB, InfluxDB, etc.) and executing queries. Features:
//! - Connect to any Flight SQL server via `.open <endpoint>`
//! - Query history displayed as cells with execution timing
//! - Results rendered as tables with export options
//! - Schema browser sidebar with remote table metadata
//! - Query plan visualization with `.explain` and `.analyze` commands
//!
//! Dot-commands (inspired by DuckDB/SQLite):
//! - `.open <endpoint>` - Connect to Flight SQL server
//! - `.close` - Disconnect
//! - `.tables` - List tables
//! - `.schema <table>` - Show table schema
//! - `.explain <query>` - Show query plan
//! - `.timer on|off` - Toggle query timing
//! - `.mode <format>` - Set output mode (table, csv, json)
//! - `.help` - Show help

use egui::{Color32, RichText, TextEdit};
use enya_datafusion::arrow::array::{Array, RecordBatch};
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::{
    ConnectionState, ExecutionStats, FlightClient, PlanNode, QueryEvent, QueryId, QueryRequest,
    TableInfo,
};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use rustc_hash::FxHashMap;

use super::command::SqlCommand;
use super::connections::{
    ConnectionId, ConnectionTreeState, SavedConnection, SqlBackend, TreeSelection,
};
use super::diff::{
    DiffRow, RowDiffStatus, compute_detailed_diff, compute_table_diff, schemas_compatible,
};
use super::highlighting::highlight_sql;
use super::plan_view::{PlanViewMode, PlanViewer};
use super::suggestions::{Suggestion, SuggestionIcon, SuggestionState};
use super::types::{
    DiffQueryResult, DiffType, ProfileRow, QueryCell, QueryStatus, ResultOverlay, SchemaDiffResult,
    SqlMode, SqlPaneAction,
};
use crate::components::util::id_generator::next_id_usize;
use crate::components::{OverlayColors, OverlayStyle};
use crate::ui::semantic_icons::{action, category, file, nav, status, time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;

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
    /// Query history (executed queries with results).
    history: Vec<QueryCell>,
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
    /// Pending explain query result receiver.
    #[allow(clippy::type_complexity)]
    pending_explain: Option<tokio::sync::oneshot::Receiver<Result<String, String>>>,
    /// ID of connection being connected.
    pending_connect_id: Option<ConnectionId>,
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
    /// Index of the result being viewed in the overlay.
    overlay_result_idx: Option<usize>,
    /// Current page in table overlay (0-indexed).
    overlay_table_page: usize,
    /// Filter text for table overlay.
    overlay_filter: String,
    /// Whether a workspace overlay is open that should block our keyboard input.
    overlay_blocks_input: bool,
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
            history: Vec::new(),
            input: String::new(),
            input_focused: true,
            move_cursor_to_end: false,
            scroll_to_bottom: false,
            show_plan_viewer: false,
            plan_viewer: PlanViewer::new(theme),
            pending_connect: None,
            pending_query: None,
            pending_query_id: None,
            pending_explain: None,
            pending_connect_id: None,
            pending_tables: None,
            pending_tables_id: None,
            pending_diff: None,
            pending_schema_diff: None,
            connections: Vec::new(),
            tree_state: ConnectionTreeState::default(),
            sidebar_width: 0.0, // Used as popup visibility flag (0.0 = closed, 1.0 = open)
            mode: SqlMode::default(),
            suggestions: SuggestionState::default(),
            prev_input: String::new(),
            matcher: Matcher::new(Config::DEFAULT),
            active_overlay: ResultOverlay::None,
            overlay_result_idx: None,
            overlay_table_page: 0,
            overlay_filter: String::new(),
            overlay_blocks_input: false,
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

    /// Connect to a Flight SQL server.
    fn connect(&mut self, endpoint: &str) {
        let endpoint = endpoint.to_string();

        // Check if we already have a connection to this endpoint
        let existing_id = self
            .connections
            .iter()
            .find(|c| c.endpoint == endpoint)
            .map(|c| c.id);

        if let Some(id) = existing_id {
            // Reuse existing connection
            self.connect_saved(id);
            return;
        }

        // Create a new saved connection with a name derived from the endpoint
        let name = if endpoint.starts_with("localhost") || endpoint.starts_with("127.") {
            "local".to_string()
        } else {
            // Use first part of hostname as name
            endpoint
                .split(':')
                .next()
                .unwrap_or(&endpoint)
                .split('.')
                .next()
                .unwrap_or("server")
                .to_string()
        };

        // Ensure unique name
        let mut final_name = name.clone();
        let mut counter = 1;
        while self.connections.iter().any(|c| c.name == final_name) {
            counter += 1;
            final_name = format!("{name}-{counter}");
        }

        // Create and add the connection
        let conn = SavedConnection::new(&final_name, &endpoint);
        let conn_id = conn.id;
        self.connections.push(conn);

        // Now connect to it
        self.connect_saved(conn_id);
    }

    /// Disconnect from current server.
    fn disconnect(&mut self) {
        self.backend = None;
        self.connection_state = ConnectionState::Disconnected;
        self.endpoint = None;
    }

    // ========================================================================
    // Connection Management (new multi-connection support)
    // ========================================================================

    /// Add a new saved connection.
    fn add_connection(&mut self, name: &str, endpoint: &str) {
        let conn = SavedConnection::new(name, endpoint);
        self.connections.push(conn);
    }

    /// Connect to a saved connection by ID.
    fn connect_saved(&mut self, id: ConnectionId) {
        let Some(conn) = self.connections.iter_mut().find(|c| c.id == id) else {
            return;
        };

        // Already connecting or connected
        if matches!(
            conn.state,
            ConnectionState::Connecting | ConnectionState::Connected
        ) {
            return;
        }

        let endpoint = conn.endpoint.clone();
        conn.state = ConnectionState::Connecting;

        // Also update legacy state if this becomes active
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

    /// Execute the current input as a SQL query or command.
    fn execute_input(&mut self) {
        let input = self.input.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Check for slash-commands (/help, /diff, etc.)
        if let Some(cmd) = input.strip_prefix('/') {
            self.handle_slash_command(cmd);
            self.input.clear();
            return;
        }

        // Check for dot-commands (like DuckDB/SQLite)
        if let Some(cmd) = input.strip_prefix('.') {
            self.handle_command(cmd);
            self.input.clear();
            return;
        }

        // Execute as SQL query
        self.execute_query(&input);
        self.input.clear();
        self.scroll_to_bottom = true;
    }

    /// Handle a slash-command (/help, /diff, etc.).
    fn handle_slash_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts.first().copied().unwrap_or("");

        match command {
            "help" => {
                let help_text = r#"SQL Pane Commands:

Query Execution:
  /explain <query>   Show query execution plan (EXPLAIN)
  /analyze <query>   Show execution plan with timing (EXPLAIN ANALYZE)
  /schema <table>    Show table structure

Plan Viewer:
  /demo              Load a demo plan for testing
  /plan              Toggle plan viewer
  /plan tree         Show tree view
  /plan timeline     Show timeline view
  /plan waterfall    Show waterfall view
  /plan hide         Hide plan viewer

Connections:
  /connect <endpoint> Connect to Flight SQL (e.g., localhost:50051)

Diff Comparison:
  /diff <e1> <e2> <q>          Compare query results
  /diff analyze <e1> <e2> <q>  Compare EXPLAIN ANALYZE plans
  /diff schema <e1> <e2> <t>   Compare table schemas
  /diff profile <e1> <e2> <q>  Compare execution profiles
  /diff demo                   Demo data diff
  /diff schema demo            Demo schema diff
  /diff profile demo           Demo profile diff

Other:
  /help              Show this help message
  /history           Show query history
  /export <format>   Export results to file

Keyboard Shortcuts:
  ⌘↵ or Ctrl+Enter   Execute query
  Tab                Insert suggestion
  ↑↓                 Navigate suggestions
  Escape             Close suggestions / release focus"#;
                self.add_info_cell(help_text);
            }
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
            "demo" => {
                // Load a demo plan for testing the visualization
                self.load_demo_plan();
                // Add a placeholder result so we can open the overlay
                self.history.push(QueryCell {
                    sql: "-- Demo Query Plan".to_string(),
                    id: enya_datafusion::QueryId::new(),
                    status: QueryStatus::Completed,
                    started_at: Instant::now(),
                    schema: None,
                    batches: Vec::new(),
                    stats: None,
                    error: None,
                    is_info: false,
                    diff_result: None,
                });
                let idx = self.history.len() - 1;
                self.open_overlay(ResultOverlay::Plan, idx);
            }
            "plan" => {
                // Toggle or set plan viewer mode
                match parts.get(1).copied() {
                    Some("tree") => {
                        self.plan_viewer.mode = PlanViewMode::Tree;
                        self.show_plan_viewer = true;
                    }
                    Some("stats") => {
                        self.plan_viewer.mode = PlanViewMode::Stats;
                        self.show_plan_viewer = true;
                    }
                    Some("waterfall") => {
                        self.plan_viewer.mode = PlanViewMode::Waterfall;
                        self.show_plan_viewer = true;
                    }
                    Some("hide") => {
                        self.show_plan_viewer = false;
                    }
                    _ => {
                        self.show_plan_viewer = !self.show_plan_viewer;
                    }
                }
            }
            "profile" => {
                self.mode = SqlMode::Profile;
                self.add_info_cell("Profile mode: Enter a query to see detailed execution timing.");
            }
            "schema" => {
                if let Some(table_name) = parts.get(1) {
                    // Show table schema
                    self.execute_query(&format!("DESCRIBE {table_name}"));
                } else {
                    self.add_info_cell("Usage: /schema <table-name>\nExample: /schema users");
                }
            }
            "connect" => {
                if let Some(arg) = parts.get(1) {
                    // Check if it looks like an endpoint (contains : or is an IP/hostname)
                    let is_endpoint = arg.contains(':')
                        || arg.starts_with("localhost")
                        || arg.starts_with("127.")
                        || arg.parse::<std::net::IpAddr>().is_ok();

                    if is_endpoint {
                        // Connect to a new endpoint directly
                        self.connect(arg);
                    } else {
                        // Try to find connection by name and make it active
                        let conn_id = self
                            .connections
                            .iter()
                            .find(|c| c.name.eq_ignore_ascii_case(arg))
                            .map(|c| c.id);

                        if let Some(id) = conn_id {
                            let is_connected = self
                                .connections
                                .iter()
                                .find(|c| c.id == id)
                                .map(|c| matches!(c.state, ConnectionState::Connected))
                                .unwrap_or(false);

                            if is_connected {
                                self.set_active_connection(id);
                                self.add_info_cell(&format!("Switched to connection: {arg}"));
                            } else {
                                self.connect_saved(id);
                                self.add_info_cell(&format!("Connecting to: {arg}"));
                            }
                        } else {
                            // Show available connections
                            let available: Vec<_> =
                                self.connections.iter().map(|c| c.name.as_str()).collect();
                            if available.is_empty() {
                                self.add_info_cell(
                                    "No connections available.\n\
                                     Use /connect <endpoint> to connect (e.g., /connect localhost:50051)",
                                );
                            } else {
                                self.add_error_cell(&format!(
                                    "Connection not found: {}\nAvailable: {}",
                                    arg,
                                    available.join(", ")
                                ));
                            }
                        }
                    }
                } else {
                    // Show usage and available connections
                    let available: Vec<_> = self
                        .connections
                        .iter()
                        .map(|c| {
                            let status = if matches!(c.state, ConnectionState::Connected) {
                                "●"
                            } else {
                                "○"
                            };
                            format!(
                                "{} {} {}",
                                status,
                                c.name,
                                if c.active { "(active)" } else { "" }
                            )
                        })
                        .collect();

                    if available.is_empty() {
                        self.add_info_cell(
                            "Usage: /connect <endpoint>\n\
                             Example: /connect localhost:50051\n\n\
                             No existing connections.",
                        );
                    } else {
                        self.add_info_cell(&format!(
                            "Usage: /connect <endpoint|name>\n\
                             Example: /connect localhost:50051\n\n\
                             Existing connections:\n{}",
                            available.join("\n")
                        ));
                    }
                }
            }
            "export" => {
                self.add_info_cell(
                    "Export not yet implemented.\nUsage: /export csv or /export json",
                );
            }
            "history" => {
                if self.history.is_empty() {
                    self.add_info_cell("No query history yet.");
                } else {
                    let history_text: Vec<_> = self
                        .history
                        .iter()
                        .enumerate()
                        .filter(|(_, cell)| !cell.sql.is_empty() && !cell.sql.starts_with('/'))
                        .take(10)
                        .map(|(i, cell)| {
                            let status = match cell.status {
                                QueryStatus::Completed => "✓",
                                QueryStatus::Failed => "✗",
                                QueryStatus::Running => "…",
                                QueryStatus::Cancelled => "○",
                            };
                            format!(
                                "[{}] {} {}",
                                i + 1,
                                status,
                                cell.sql.lines().next().unwrap_or("")
                            )
                        })
                        .collect();
                    self.add_info_cell(&format!("Recent queries:\n{}", history_text.join("\n")));
                }
            }
            "normal" | "reset" => {
                // Reset to normal mode
                self.mode = SqlMode::Normal;
                self.add_info_cell("Switched to normal SQL mode.");
            }
            _ => {
                self.add_error_cell(&format!(
                    "Unknown command: /{command}\nType /help to see available commands."
                ));
            }
        }
        self.scroll_to_bottom = true;
    }

    /// Handle a dot-command (like DuckDB/SQLite).
    fn handle_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let command = parts.first().copied().unwrap_or("");

        match command {
            "open" => {
                if let Some(endpoint) = parts.get(1) {
                    self.connect(endpoint);
                } else {
                    self.add_error_cell(".open requires an endpoint (e.g., .open localhost:50051)");
                }
            }
            "close" => {
                self.disconnect();
                self.add_info_cell("Disconnected");
            }
            "tables" => {
                self.execute_query("SHOW TABLES");
            }
            "explain" => {
                // EXPLAIN query (logical plan)
                let sql = parts[1..].join(" ");
                if sql.is_empty() {
                    self.add_error_cell(".explain requires a SQL query");
                } else {
                    self.execute_explain(&sql, false);
                }
            }
            "analyze" => {
                // EXPLAIN ANALYZE query (physical plan with timing)
                let sql = parts[1..].join(" ");
                if sql.is_empty() {
                    self.add_error_cell(".analyze requires a SQL query");
                } else {
                    self.execute_explain(&sql, true);
                }
            }
            "plan" => {
                // Toggle or set plan viewer mode
                match parts.get(1).copied() {
                    Some("tree") => {
                        self.plan_viewer.mode = PlanViewMode::Tree;
                        self.show_plan_viewer = true;
                    }
                    Some("stats") => {
                        self.plan_viewer.mode = PlanViewMode::Stats;
                        self.show_plan_viewer = true;
                    }
                    Some("waterfall") => {
                        self.plan_viewer.mode = PlanViewMode::Waterfall;
                        self.show_plan_viewer = true;
                    }
                    Some("hide") => {
                        self.show_plan_viewer = false;
                    }
                    _ => {
                        self.show_plan_viewer = !self.show_plan_viewer;
                    }
                }
            }
            "demo" => {
                // Load a demo query plan for testing the visualization
                self.load_demo_plan();
                self.add_info_cell("Demo plan loaded. Use j/k/h/l to navigate.");
            }
            "help" => {
                self.add_info_cell(
                    "Dot-Commands (like DuckDB/SQLite):\n\
                     .open <endpoint>  - Connect to Flight SQL server\n\
                     .close            - Disconnect from server\n\
                     .tables           - List available tables\n\
                     .explain <query>  - Show query plan (EXPLAIN)\n\
                     .analyze <query>  - Show query plan with timing (EXPLAIN ANALYZE)\n\
                     .plan [tree|timeline|waterfall|hide] - Toggle plan viewer\n\
                     .demo             - Load a demo query plan\n\
                     .help             - Show this help\n\n\
                     Enter SQL queries directly and press Ctrl+Enter to execute.\n\n\
                     Plan Viewer Keys (when visible):\n\
                     j/k - Navigate up/down\n\
                     h/l - Collapse/expand nodes\n\
                     b   - Jump to bottleneck\n\
                     Space - Toggle expand/collapse",
                );
            }
            _ => {
                self.add_error_cell(&format!(
                    "Unknown command: .{command}. Type .help for help."
                ));
            }
        }
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
            self.add_error_cell("Not connected. Use .open <endpoint> first.");
        }
    }

    /// Execute a SQL query.
    fn execute_query(&mut self, sql: &str) {
        let query_id = QueryId::new();

        // Add to history
        self.history.push(QueryCell {
            sql: sql.to_string(),
            id: query_id,
            status: QueryStatus::Running,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: false,
            diff_result: None,
        });

        match &mut self.backend {
            Some(SqlBackend::Flight { .. }) => {
                // Execute via Flight - we need to take ownership temporarily
                // This is a workaround for the borrow checker
                let sql = sql.to_string();
                let (tx, rx) = tokio::sync::oneshot::channel();
                self.pending_query = Some(rx);
                self.pending_query_id = Some(query_id);

                // We can't easily move client out, so we'll do blocking connect
                // This is a temporary solution - proper implementation would use Arc<Mutex>
                if let Some(endpoint) = self.endpoint.clone() {
                    self.runtime_handle.spawn(async move {
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
                }
            }
            Some(SqlBackend::Local { session, .. }) => {
                // Execute via local session
                let request = QueryRequest::new(sql).with_id(query_id);
                if let Err(e) = session.execute(request) {
                    if let Some(cell) = self.history.last_mut() {
                        cell.status = QueryStatus::Failed;
                        cell.error = Some(e.to_string());
                    }
                }
            }
            None => {
                // No backend connected - show error
                if let Some(cell) = self.history.last_mut() {
                    cell.status = QueryStatus::Failed;
                    cell.error = Some(
                        "Not connected. Use .open <endpoint> to connect to a Flight SQL server."
                            .to_string(),
                    );
                }
            }
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
                    "Connection '{}' is not connected. Use /connect {} first.",
                    c.name, c.name
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
                    "Connection '{}' is not connected. Use /connect {} first.",
                    c.name, c.name
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

        // Add to history with Running status
        self.history.push(QueryCell {
            sql: display_sql,
            id: query_id,
            status: QueryStatus::Running,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: false,
            diff_result: None,
        });

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
                    "Connection '{}' is not connected. Use /connect {} first.",
                    c.name, c.name
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
                    "Connection '{}' is not connected. Use /connect {} first.",
                    c.name, c.name
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

        // Add to history with Running status
        self.history.push(QueryCell {
            sql: display_sql,
            id: query_id,
            status: QueryStatus::Running,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: false,
            diff_result: None,
        });

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

    /// Add an error message cell to history.
    fn add_error_cell(&mut self, message: &str) {
        self.history.push(QueryCell {
            sql: String::new(),
            id: QueryId::new(),
            status: QueryStatus::Failed,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: Some(message.to_string()),
            is_info: true, // Error messages are system info, not user queries
            diff_result: None,
        });
        self.scroll_to_bottom = true;
    }

    /// Add an info message cell to history.
    fn add_info_cell(&mut self, message: &str) {
        self.history.push(QueryCell {
            sql: message.to_string(),
            id: QueryId::new(),
            status: QueryStatus::Completed,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: true,
            diff_result: None,
        });
        self.scroll_to_bottom = true;
    }

    /// Clear all query results from history.
    fn clear_results(&mut self) {
        // Remove all non-running query results (keep running queries)
        self.history
            .retain(|cell| cell.status == QueryStatus::Running);
        self.active_overlay = ResultOverlay::None;
        self.overlay_result_idx = None;
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

                        // Add info cell and expand tree
                        if let Some(name) = conn_name {
                            self.add_info_cell(&format!("Connected to {name}"));
                        }
                        self.tree_state.expanded.insert(conn_id);
                    } else {
                        self.add_info_cell(&format!(
                            "Connected to {}",
                            self.endpoint.as_deref().unwrap_or("server")
                        ));
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
                    if let Some(query_id) = self.pending_query_id.take() {
                        if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                            let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();
                            cell.status = QueryStatus::Completed;
                            cell.schema = Some(schema);
                            cell.batches = batches;
                            cell.stats = Some(ExecutionStats {
                                rows_returned: row_count,
                                ..Default::default()
                            });
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Query failed
                    if let Some(query_id) = self.pending_query_id.take() {
                        if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                            cell.status = QueryStatus::Failed;
                            cell.error = Some(e);
                        }
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running
                    self.pending_query = Some(rx);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Query task dropped
                    if let Some(query_id) = self.pending_query_id.take() {
                        if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                            cell.status = QueryStatus::Failed;
                            cell.error = Some("Query task dropped".to_string());
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
                    let plan = self.parse_plan_text(&plan_text);
                    self.plan_viewer.load_plan(&plan);
                    self.show_plan_viewer = true;

                    // Add a result cell and open the Plan overlay
                    self.history.push(QueryCell {
                        sql: format!("EXPLAIN {}", plan_text.lines().next().unwrap_or("...")),
                        id: enya_datafusion::QueryId::new(),
                        status: QueryStatus::Completed,
                        started_at: Instant::now(),
                        schema: None,
                        batches: Vec::new(),
                        stats: None,
                        error: None,
                        is_info: false,
                        diff_result: None,
                    });
                    let idx = self.history.len() - 1;
                    self.open_overlay(ResultOverlay::Plan, idx);
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
                            log::info!(
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
                                    diff_result.left_plan = Some(self.parse_plan_text(&text));
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
                                    diff_result.right_plan = Some(self.parse_plan_text(&text));
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
                    if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                        if has_error {
                            cell.status = QueryStatus::Failed;
                            // Combine errors for display
                            let mut errors = Vec::new();
                            if let Some(e) = &diff_result.left_error {
                                errors.push(format!("{left_name}: {e}"));
                            }
                            if let Some(e) = &diff_result.right_error {
                                errors.push(format!("{right_name}: {e}"));
                            }
                            cell.error = Some(errors.join("\n"));
                        } else {
                            cell.status = QueryStatus::Completed;
                        }
                        cell.diff_result = Some(diff_result);
                    }

                    // Open the diff overlay
                    let idx = self.history.iter().position(|c| c.id == query_id);
                    if let Some(idx) = idx {
                        // For plan diff, use a different overlay approach if desired
                        // For now, use the Diff overlay for both
                        self.open_overlay(ResultOverlay::Diff { other_idx: idx }, idx);
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running
                    self.pending_diff = Some((query_id, left_name, right_name, diff_type, rx));
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Task dropped
                    if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                        cell.status = QueryStatus::Failed;
                        cell.error = Some("Diff query task dropped".to_string());
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
                    if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                        if has_error {
                            cell.status = QueryStatus::Failed;
                            let mut errors = Vec::new();
                            if let Some(e) = &diff_result.left_error {
                                errors.push(format!("{left_name}: {e}"));
                            }
                            if let Some(e) = &diff_result.right_error {
                                errors.push(format!("{right_name}: {e}"));
                            }
                            cell.error = Some(errors.join("\n"));
                        } else {
                            cell.status = QueryStatus::Completed;
                        }
                        cell.diff_result = Some(diff_result);
                    }

                    // Open the diff overlay
                    let idx = self.history.iter().position(|c| c.id == query_id);
                    if let Some(idx) = idx {
                        self.open_overlay(ResultOverlay::Diff { other_idx: idx }, idx);
                    }
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still running
                    self.pending_schema_diff =
                        Some((query_id, left_name, right_name, table_name, rx));
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Task dropped
                    if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                        cell.status = QueryStatus::Failed;
                        cell.error = Some("Schema diff task dropped".to_string());
                    }
                }
            }
        }

        // Poll local session events
        if let Some(SqlBackend::Local { event_rx, .. }) = &mut self.backend {
            while let Ok(event) = event_rx.try_recv() {
                let query_id = event.query_id();
                if let Some(cell) = self.history.iter_mut().find(|c| c.id == query_id) {
                    match event {
                        QueryEvent::Started { schema, .. } => {
                            cell.schema = Some(schema);
                        }
                        QueryEvent::Batch { batch, .. } => {
                            cell.batches.push(batch);
                        }
                        QueryEvent::Completed { stats, .. } => {
                            cell.status = QueryStatus::Completed;
                            cell.stats = Some(stats);
                        }
                        QueryEvent::Failed { error, .. } => {
                            cell.status = QueryStatus::Failed;
                            cell.error = Some(error);
                        }
                        QueryEvent::Cancelled { .. } => {
                            cell.status = QueryStatus::Cancelled;
                        }
                        QueryEvent::Progress { .. } => {}
                    }
                }
            }
        }
    }

    /// Parse EXPLAIN output text into a PlanNode structure.
    fn parse_plan_text(&self, text: &str) -> PlanNode {
        // Parse indented plan text into a tree structure.
        // Format is typically:
        // ProjectionExec: ...
        //   FilterExec: ...
        //     TableScan: ...

        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            return PlanNode {
                operator: "EmptyPlan".to_string(),
                description: String::new(),
                properties: FxHashMap::default(),
                children: vec![],
                metrics: None,
            };
        }

        // Parse with a stack-based approach
        let mut root: Option<PlanNode> = None;
        let mut stack: Vec<(usize, PlanNode)> = Vec::new();

        for line in lines {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            let depth = indent / 2; // Assume 2-space indentation

            // Parse operator and description
            let (operator, description) = if let Some(colon_pos) = trimmed.find(':') {
                let op = trimmed[..colon_pos].trim().to_string();
                let desc = trimmed[colon_pos + 1..].trim().to_string();
                (op, desc)
            } else {
                (trimmed.to_string(), String::new())
            };

            // Parse metrics if present (format: [rows=N, time=Xms])
            let metrics = Self::parse_metrics(&description);

            let node = PlanNode {
                operator,
                description: description.clone(),
                properties: FxHashMap::default(),
                children: vec![],
                metrics,
            };

            // Pop nodes from stack that are at same or deeper level
            while let Some((d, _)) = stack.last() {
                if *d >= depth {
                    let (_, child) = stack.pop().unwrap();
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.children.insert(0, child);
                    } else {
                        root = Some(child);
                    }
                } else {
                    break;
                }
            }

            stack.push((depth, node));
        }

        // Pop remaining nodes
        while let Some((_, child)) = stack.pop() {
            if let Some((_, parent)) = stack.last_mut() {
                parent.children.insert(0, child);
            } else {
                root = Some(child);
            }
        }

        root.unwrap_or(PlanNode {
            operator: "Unknown".to_string(),
            description: String::new(),
            properties: FxHashMap::default(),
            children: vec![],
            metrics: None,
        })
    }

    /// Parse metrics from a description string.
    ///
    /// Handles formats like:
    /// - `metrics=[output_rows=5, elapsed_compute=52.06µs, output_bytes=1920.0 B]`
    /// - `[rows=100, time=5.2ms, mem=1KB]`
    fn parse_metrics(description: &str) -> Option<enya_datafusion::OperatorMetrics> {
        // Look for metrics section
        if !description.contains('[') {
            return None;
        }

        let mut metrics = enya_datafusion::OperatorMetrics::default();
        let mut found = false;

        // Parse output_rows (new format) or rows (old format)
        if let Some(rows) = Self::parse_metric_usize(description, "output_rows=")
            .or_else(|| Self::parse_metric_usize(description, "rows="))
        {
            metrics.output_rows = rows;
            found = true;
        }

        // Parse elapsed_compute (new format) or time (old format)
        if let Some(duration) = Self::parse_metric_duration(description, "elapsed_compute=")
            .or_else(|| Self::parse_metric_duration(description, "time="))
        {
            metrics.elapsed_time = duration;
            found = true;
        }

        // Parse output_bytes (new format) or mem (old format)
        if let Some(bytes) = Self::parse_metric_bytes(description, "output_bytes=")
            .or_else(|| Self::parse_metric_bytes(description, "mem="))
        {
            metrics.memory_bytes = bytes;
            found = true;
        }

        // Parse spill metrics
        if let Some(spill_count) = Self::parse_metric_usize(description, "spill_count=") {
            metrics.spill_count = spill_count;
            found = true;
        }
        if let Some(spill_bytes) = Self::parse_metric_bytes(description, "spilled_bytes=") {
            metrics.spill_bytes = spill_bytes;
            found = true;
        }

        if found { Some(metrics) } else { None }
    }

    /// Parse a usize metric value.
    fn parse_metric_usize(description: &str, key: &str) -> Option<usize> {
        let start = description.find(key)?;
        let rest = &description[start + key.len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse().ok()
    }

    /// Parse a duration metric value (e.g., "52.06µs", "5.2ms", "1.5s").
    fn parse_metric_duration(description: &str, key: &str) -> Option<std::time::Duration> {
        use std::time::Duration;

        let start = description.find(key)?;
        let rest = &description[start + key.len()..];

        // Find end of numeric part
        let end = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());

        let time_val: f64 = rest[..end].parse().ok()?;
        let unit = &rest[end..];

        let duration = if unit.starts_with("ms") {
            Duration::from_secs_f64(time_val / 1000.0)
        } else if unit.starts_with("µs") || unit.starts_with("us") {
            Duration::from_secs_f64(time_val / 1_000_000.0)
        } else if unit.starts_with("ns") {
            Duration::from_nanos(time_val as u64)
        } else if unit.starts_with('s') {
            Duration::from_secs_f64(time_val)
        } else {
            // Default to microseconds for elapsed_compute
            Duration::from_secs_f64(time_val / 1_000_000.0)
        };

        Some(duration)
    }

    /// Parse a bytes metric value (e.g., "1920.0 B", "1.5 KB", "256 MB").
    fn parse_metric_bytes(description: &str, key: &str) -> Option<usize> {
        let start = description.find(key)?;
        let rest = &description[start + key.len()..];

        // Find end of numeric part (allow spaces before unit)
        let end = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.' && c != ' ')
            .unwrap_or(rest.len());

        // Parse the number, handling potential spaces
        let num_str = rest[..end].trim();
        let mem_val: f64 = num_str.parse().ok()?;

        // Get the unit (skip any spaces)
        let unit = rest[end..].trim_start();

        let bytes = if unit.starts_with("KB") || unit.starts_with("KiB") {
            (mem_val * 1024.0) as usize
        } else if unit.starts_with("MB") || unit.starts_with("MiB") {
            (mem_val * 1024.0 * 1024.0) as usize
        } else if unit.starts_with("GB") || unit.starts_with("GiB") {
            (mem_val * 1024.0 * 1024.0 * 1024.0) as usize
        } else if unit.starts_with('B') {
            mem_val as usize
        } else {
            // Default to bytes
            mem_val as usize
        };

        Some(bytes)
    }

    /// Load a demo query plan for testing the visualization.
    fn load_demo_plan(&mut self) {
        use std::time::Duration;

        // Create a realistic demo query plan
        let plan = PlanNode {
            operator: "ProjectionExec".to_string(),
            description: "user_id, name, total_orders, last_order_date".to_string(),
            properties: FxHashMap::default(),
            metrics: Some(enya_datafusion::OperatorMetrics {
                output_rows: 1000,
                elapsed_time: Duration::from_millis(5),
                memory_bytes: 32768,
                spill_count: 0,
                spill_bytes: 0,
            }),
            children: vec![PlanNode {
                operator: "SortExec".to_string(),
                description: "total_orders DESC".to_string(),
                properties: FxHashMap::default(),
                metrics: Some(enya_datafusion::OperatorMetrics {
                    output_rows: 1000,
                    elapsed_time: Duration::from_millis(25),
                    memory_bytes: 65536,
                    spill_count: 0,
                    spill_bytes: 0,
                }),
                children: vec![PlanNode {
                    operator: "HashAggregateExec".to_string(),
                    description: "group_by=[user_id], aggr=[COUNT(*), MAX(order_date)]".to_string(),
                    properties: FxHashMap::default(),
                    metrics: Some(enya_datafusion::OperatorMetrics {
                        output_rows: 1000,
                        elapsed_time: Duration::from_millis(45),
                        memory_bytes: 131072,
                        spill_count: 0,
                        spill_bytes: 0,
                    }),
                    children: vec![PlanNode {
                        operator: "HashJoinExec".to_string(),
                        description: "users.id = orders.user_id, type=Inner".to_string(),
                        properties: FxHashMap::default(),
                        metrics: Some(enya_datafusion::OperatorMetrics {
                            output_rows: 50000,
                            elapsed_time: Duration::from_millis(120),
                            memory_bytes: 524288,
                            spill_count: 0,
                            spill_bytes: 0,
                        }),
                        children: vec![
                            PlanNode {
                                operator: "ParquetExec".to_string(),
                                description: "users.parquet, projection=[id, name]".to_string(),
                                properties: FxHashMap::default(),
                                metrics: Some(enya_datafusion::OperatorMetrics {
                                    output_rows: 10000,
                                    elapsed_time: Duration::from_millis(85),
                                    memory_bytes: 262144,
                                    spill_count: 0,
                                    spill_bytes: 0,
                                }),
                                children: vec![],
                            },
                            PlanNode {
                                operator: "FilterExec".to_string(),
                                description: "order_date >= '2024-01-01'".to_string(),
                                properties: FxHashMap::default(),
                                metrics: Some(enya_datafusion::OperatorMetrics {
                                    output_rows: 50000,
                                    elapsed_time: Duration::from_millis(15),
                                    memory_bytes: 8192,
                                    spill_count: 0,
                                    spill_bytes: 0,
                                }),
                                children: vec![PlanNode {
                                    operator: "ParquetExec".to_string(),
                                    description: "orders.parquet, projection=[user_id, order_date]"
                                        .to_string(),
                                    properties: FxHashMap::default(),
                                    metrics: Some(enya_datafusion::OperatorMetrics {
                                        output_rows: 100000,
                                        elapsed_time: Duration::from_millis(150),
                                        memory_bytes: 1048576,
                                        spill_count: 0,
                                        spill_bytes: 0,
                                    }),
                                    children: vec![],
                                }],
                            },
                        ],
                    }],
                }],
            }],
        };

        self.plan_viewer.load_plan(&plan);
        self.show_plan_viewer = true;
    }

    /// Load a demo diff result for testing the diff overlay.
    fn load_diff_demo(&mut self) {
        use enya_datafusion::arrow::array::{Int32Array, StringArray};
        use enya_datafusion::arrow::datatypes::{DataType, Field, Schema};
        use std::sync::Arc;

        // Create schema for demo data - realistic users table
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("email", DataType::Utf8, true),
            Field::new("name", DataType::Utf8, true),
            Field::new("role", DataType::Utf8, true),
        ]));

        // STAGING environment - includes test users and some synced production users
        // Test users (staging-only): test@, qa@, demo@
        // Synced from prod: alice@, bob@, carol@
        let left_batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3, 100, 101, 102])),
                Arc::new(StringArray::from(vec![
                    "alice@acme.com",
                    "bob@acme.com",
                    "carol@acme.com",
                    "test@staging.local",
                    "qa@staging.local",
                    "demo@staging.local",
                ])),
                Arc::new(StringArray::from(vec![
                    "Alice Chen",
                    "Bob Smith",
                    "Carol Jones",
                    "Test User",
                    "QA Engineer",
                    "Demo Account",
                ])),
                Arc::new(StringArray::from(vec![
                    "admin", "editor", "viewer", "admin", "editor", "viewer",
                ])),
            ],
        )
        .unwrap();

        // PRODUCTION environment - real users only
        // Synced users: alice@, bob@, carol@ (same as staging)
        // Production-only: dave@, emma@, frank@ (new users since staging snapshot)
        let right_batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5, 6])),
                Arc::new(StringArray::from(vec![
                    "alice@acme.com",
                    "bob@acme.com",
                    "carol@acme.com",
                    "dave@acme.com",
                    "emma@acme.com",
                    "frank@acme.com",
                ])),
                Arc::new(StringArray::from(vec![
                    "Alice Chen",
                    "Bob Smith",
                    "Carol Jones",
                    "Dave Wilson",
                    "Emma Brown",
                    "Frank Garcia",
                ])),
                Arc::new(StringArray::from(vec![
                    "admin", "editor", "viewer", "editor", "viewer", "viewer",
                ])),
            ],
        )
        .unwrap();

        // Create the diff result (no plans - to show data diff view)
        let diff_result = DiffQueryResult {
            left_name: "staging".to_string(),
            right_name: "production".to_string(),
            left_schema: Some(schema.clone()),
            left_batches: vec![left_batch],
            left_error: None,
            right_schema: Some(schema),
            right_batches: vec![right_batch],
            right_error: None,
            schemas_match: true,
            diff_stats: Some(super::types::DiffStats {
                matching: 3,   // Alice, Bob, Carol (same in both)
                left_only: 3,  // Test User, QA Engineer, Demo Account (staging only)
                right_only: 3, // Dave, Emma, Frank (production only)
                different: 0,
            }),
            left_plan: None,
            right_plan: None,
            diff_type: DiffType::Data,
            schema_diff: None,
        };

        // Create a query cell with the diff result
        let query_id = QueryId::new();
        self.history.push(QueryCell {
            sql: "/diff demo (staging vs production)".to_string(),
            id: query_id,
            status: QueryStatus::Completed,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: false,
            diff_result: Some(diff_result),
        });

        // Open the diff overlay
        let idx = self.history.len() - 1;
        self.open_overlay(ResultOverlay::Diff { other_idx: idx }, idx);
        self.scroll_to_bottom = true;
    }

    /// Load a demo schema diff result for testing the schema diff overlay.
    fn load_schema_diff_demo(&mut self) {
        use super::types::{ColumnDiffStatus, SchemaDiffColumn, SchemaDiffResult};

        // Create a realistic schema diff scenario:
        // - staging has some test columns, missing some production columns
        // - production has evolved with new columns, some type changes
        let schema_diff = SchemaDiffResult {
            table_name: "users".to_string(),
            columns: vec![
                // Matching columns
                SchemaDiffColumn {
                    name: "id".to_string(),
                    left_type: Some("INT".to_string()),
                    left_nullable: Some(false),
                    right_type: Some("INT".to_string()),
                    right_nullable: Some(false),
                    status: ColumnDiffStatus::Matching,
                },
                SchemaDiffColumn {
                    name: "email".to_string(),
                    left_type: Some("VARCHAR(255)".to_string()),
                    left_nullable: Some(false),
                    right_type: Some("VARCHAR(255)".to_string()),
                    right_nullable: Some(false),
                    status: ColumnDiffStatus::Matching,
                },
                SchemaDiffColumn {
                    name: "name".to_string(),
                    left_type: Some("VARCHAR(100)".to_string()),
                    left_nullable: Some(true),
                    right_type: Some("VARCHAR(100)".to_string()),
                    right_nullable: Some(true),
                    status: ColumnDiffStatus::Matching,
                },
                // Changed column - type difference
                SchemaDiffColumn {
                    name: "status".to_string(),
                    left_type: Some("VARCHAR(20)".to_string()),
                    left_nullable: Some(true),
                    right_type: Some("INT".to_string()),
                    right_nullable: Some(false),
                    status: ColumnDiffStatus::Changed,
                },
                // Left-only column (staging has it, production doesn't)
                SchemaDiffColumn {
                    name: "test_flag".to_string(),
                    left_type: Some("BOOLEAN".to_string()),
                    left_nullable: Some(true),
                    right_type: None,
                    right_nullable: None,
                    status: ColumnDiffStatus::LeftOnly,
                },
                SchemaDiffColumn {
                    name: "debug_info".to_string(),
                    left_type: Some("TEXT".to_string()),
                    left_nullable: Some(true),
                    right_type: None,
                    right_nullable: None,
                    status: ColumnDiffStatus::LeftOnly,
                },
                // Right-only columns (production has them, staging doesn't)
                SchemaDiffColumn {
                    name: "created_at".to_string(),
                    left_type: None,
                    left_nullable: None,
                    right_type: Some("TIMESTAMP".to_string()),
                    right_nullable: Some(false),
                    status: ColumnDiffStatus::RightOnly,
                },
                SchemaDiffColumn {
                    name: "updated_at".to_string(),
                    left_type: None,
                    left_nullable: None,
                    right_type: Some("TIMESTAMP".to_string()),
                    right_nullable: Some(true),
                    status: ColumnDiffStatus::RightOnly,
                },
                SchemaDiffColumn {
                    name: "deleted_at".to_string(),
                    left_type: None,
                    left_nullable: None,
                    right_type: Some("TIMESTAMP".to_string()),
                    right_nullable: Some(true),
                    status: ColumnDiffStatus::RightOnly,
                },
            ],
            matching: 3,
            left_only: 2,
            right_only: 3,
            changed: 1,
        };

        // Create the diff result
        let diff_result = DiffQueryResult {
            left_name: "staging".to_string(),
            right_name: "production".to_string(),
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
            diff_type: DiffType::Schema,
            schema_diff: Some(schema_diff),
        };

        // Create a query cell with the diff result
        let query_id = QueryId::new();
        self.history.push(QueryCell {
            sql: "/diff schema demo (staging vs production users)".to_string(),
            id: query_id,
            status: QueryStatus::Completed,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: false,
            diff_result: Some(diff_result),
        });

        // Open the diff overlay
        let idx = self.history.len() - 1;
        self.open_overlay(ResultOverlay::Diff { other_idx: idx }, idx);
        self.scroll_to_bottom = true;
    }

    /// Load a demo profile diff result for testing the profile diff overlay.
    fn load_profile_diff_demo(&mut self) {
        use enya_datafusion::{OperatorMetrics, PlanNode};
        use std::time::Duration;

        // Realistic query plan for:
        // SELECT c.name, COUNT(*) as order_count, SUM(o.total) as revenue
        // FROM orders o JOIN customers c ON o.customer_id = c.id
        // WHERE o.status = 'completed' AND o.created_at > '2024-01-01'
        // GROUP BY c.name ORDER BY revenue DESC LIMIT 100

        // Staging: slower - full table scans, no partition pruning, memory pressure
        let left_plan = PlanNode {
            operator: "GlobalLimitExec".to_string(),
            description: "limit=100".to_string(),
            properties: Default::default(),
            metrics: Some(OperatorMetrics {
                output_rows: 100,
                elapsed_time: Duration::from_micros(850),
                memory_bytes: 8192,
                spill_count: 0,
                spill_bytes: 0,
            }),
            children: vec![PlanNode {
                operator: "SortExec".to_string(),
                description: "revenue DESC".to_string(),
                properties: Default::default(),
                metrics: Some(OperatorMetrics {
                    output_rows: 2847,
                    elapsed_time: Duration::from_millis(45),
                    memory_bytes: 524288,
                    spill_count: 0,
                    spill_bytes: 0,
                }),
                children: vec![PlanNode {
                    operator: "AggregateExec".to_string(),
                    description: "GROUP BY c.name, SUM(total), COUNT(*)".to_string(),
                    properties: Default::default(),
                    metrics: Some(OperatorMetrics {
                        output_rows: 2847,
                        elapsed_time: Duration::from_millis(125),
                        memory_bytes: 4194304,
                        spill_count: 2,
                        spill_bytes: 8388608,
                    }),
                    children: vec![PlanNode {
                        operator: "CoalesceBatchesExec".to_string(),
                        description: "target_batch_size=8192".to_string(),
                        properties: Default::default(),
                        metrics: Some(OperatorMetrics {
                            output_rows: 156420,
                            elapsed_time: Duration::from_millis(12),
                            memory_bytes: 262144,
                            spill_count: 0,
                            spill_bytes: 0,
                        }),
                        children: vec![PlanNode {
                            operator: "HashJoinExec".to_string(),
                            description: "INNER JOIN on customer_id = id".to_string(),
                            properties: Default::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 156420,
                                elapsed_time: Duration::from_millis(340),
                                memory_bytes: 67108864,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![
                                PlanNode {
                                    operator: "FilterExec".to_string(),
                                    description:
                                        "status = 'completed' AND created_at > '2024-01-01'"
                                            .to_string(),
                                    properties: Default::default(),
                                    metrics: Some(OperatorMetrics {
                                        output_rows: 156420,
                                        elapsed_time: Duration::from_millis(85),
                                        memory_bytes: 131072,
                                        spill_count: 0,
                                        spill_bytes: 0,
                                    }),
                                    children: vec![PlanNode {
                                        operator: "ParquetExec".to_string(),
                                        description: "orders.parquet [full scan, 48 partitions]"
                                            .to_string(),
                                        properties: Default::default(),
                                        metrics: Some(OperatorMetrics {
                                            output_rows: 1250000,
                                            elapsed_time: Duration::from_millis(420),
                                            memory_bytes: 134217728,
                                            spill_count: 0,
                                            spill_bytes: 0,
                                        }),
                                        children: vec![],
                                    }],
                                },
                                PlanNode {
                                    operator: "ParquetExec".to_string(),
                                    description: "customers.parquet [full scan, 4 partitions]"
                                        .to_string(),
                                    properties: Default::default(),
                                    metrics: Some(OperatorMetrics {
                                        output_rows: 50000,
                                        elapsed_time: Duration::from_millis(65),
                                        memory_bytes: 16777216,
                                        spill_count: 0,
                                        spill_bytes: 0,
                                    }),
                                    children: vec![],
                                },
                            ],
                        }],
                    }],
                }],
            }],
        };

        // Production: faster - partition pruning, bloom filters, optimized join
        let right_plan = PlanNode {
            operator: "GlobalLimitExec".to_string(),
            description: "limit=100".to_string(),
            properties: Default::default(),
            metrics: Some(OperatorMetrics {
                output_rows: 100,
                elapsed_time: Duration::from_micros(420),
                memory_bytes: 8192,
                spill_count: 0,
                spill_bytes: 0,
            }),
            children: vec![PlanNode {
                operator: "SortExec".to_string(),
                description: "revenue DESC".to_string(),
                properties: Default::default(),
                metrics: Some(OperatorMetrics {
                    output_rows: 2891,
                    elapsed_time: Duration::from_millis(28),
                    memory_bytes: 524288,
                    spill_count: 0,
                    spill_bytes: 0,
                }),
                children: vec![PlanNode {
                    operator: "AggregateExec".to_string(),
                    description: "GROUP BY c.name, SUM(total), COUNT(*)".to_string(),
                    properties: Default::default(),
                    metrics: Some(OperatorMetrics {
                        output_rows: 2891,
                        elapsed_time: Duration::from_millis(42),
                        memory_bytes: 2097152,
                        spill_count: 0,
                        spill_bytes: 0,
                    }),
                    children: vec![PlanNode {
                        operator: "CoalesceBatchesExec".to_string(),
                        description: "target_batch_size=8192".to_string(),
                        properties: Default::default(),
                        metrics: Some(OperatorMetrics {
                            output_rows: 162350,
                            elapsed_time: Duration::from_millis(8),
                            memory_bytes: 262144,
                            spill_count: 0,
                            spill_bytes: 0,
                        }),
                        children: vec![PlanNode {
                            operator: "HashJoinExec".to_string(),
                            description: "INNER JOIN on customer_id = id".to_string(),
                            properties: Default::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 162350,
                                elapsed_time: Duration::from_millis(95),
                                memory_bytes: 33554432,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![
                                PlanNode {
                                    operator: "FilterExec".to_string(),
                                    description: "status = 'completed' AND created_at > '2024-01-01'".to_string(),
                                    properties: Default::default(),
                                    metrics: Some(OperatorMetrics {
                                        output_rows: 162350,
                                        elapsed_time: Duration::from_millis(22),
                                        memory_bytes: 131072,
                                        spill_count: 0,
                                        spill_bytes: 0,
                                    }),
                                    children: vec![PlanNode {
                                        operator: "ParquetExec".to_string(),
                                        description: "orders.parquet [pruned: 12/48 partitions, bloom filter]".to_string(),
                                        properties: Default::default(),
                                        metrics: Some(OperatorMetrics {
                                            output_rows: 312500,
                                            elapsed_time: Duration::from_millis(78),
                                            memory_bytes: 33554432,
                                            spill_count: 0,
                                            spill_bytes: 0,
                                        }),
                                        children: vec![],
                                    }],
                                },
                                PlanNode {
                                    operator: "ParquetExec".to_string(),
                                    description: "customers.parquet [cached, 4 partitions]".to_string(),
                                    properties: Default::default(),
                                    metrics: Some(OperatorMetrics {
                                        output_rows: 50000,
                                        elapsed_time: Duration::from_millis(12),
                                        memory_bytes: 16777216,
                                        spill_count: 0,
                                        spill_bytes: 0,
                                    }),
                                    children: vec![],
                                },
                            ],
                        }],
                    }],
                }],
            }],
        };

        // Create the diff result
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

        // Create a query cell with the diff result
        let query_id = QueryId::new();
        self.history.push(QueryCell {
            sql: "/diff profile demo (staging vs production)".to_string(),
            id: query_id,
            status: QueryStatus::Completed,
            started_at: Instant::now(),
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: false,
            diff_result: Some(diff_result),
        });

        // Open the diff overlay
        let idx = self.history.len() - 1;
        self.open_overlay(ResultOverlay::Diff { other_idx: idx }, idx);
        self.scroll_to_bottom = true;
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

    /// Get the list of tables from the current backend.
    #[allow(dead_code)] // Will be used for table info population
    fn get_tables(&self) -> Vec<TableInfo> {
        match &self.backend {
            Some(SqlBackend::Flight { tables, .. }) => tables.clone(),
            Some(SqlBackend::Local { session, .. }) => session.tables(),
            None => Vec::new(),
        }
    }

    /// Show the SQL pane with three-panel layout.
    ///
    /// Layout:
    /// ```text
    /// ┌─────────────┬────────────────────────┬─────────────┐
    /// │ CONNECTIONS │  Query Results         │ PLAN VIEWER │
    /// │             │                        │ (toggleable)│
    /// │ ▼ staging   │                        │             │
    /// │   └─ users  ├────────────────────────┤             │
    /// │   └─ orders │  Query Input           │             │
    /// │             │                        │             │
    /// │ + Add       │        [Run]           │             │
    /// └─────────────┴────────────────────────┴─────────────┘
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

        // Only count actual query results (not info/system messages) for layout
        let has_results = self.history.iter().any(|c| !c.is_info);

        // Centered input with suggestions above, results below
        egui::Frame::new()
            .fill(self.theme.bg_base())
            .inner_margin(egui::Margin::symmetric(48, 32))
            .show(ui, |ui| {
                let max_width = 700.0;
                let available_width = ui.available_width();
                let available_height = ui.available_height();
                let content_width = available_width.min(max_width);

                // Always keep input bar centered, with results appearing below
                // Estimate input section height: input ~48px + hints ~24px + results ~200px
                let input_section_height = 80.0;
                let results_height = if has_results { 180.0 } else { 0.0 };
                let total_content_height = input_section_height + results_height;
                let vertical_offset = ((available_height - total_content_height) / 2.0).max(32.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(available_width, available_height),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        // Add space to push content toward center
                        ui.add_space(vertical_offset);

                        ui.set_max_width(content_width);
                        ui.vertical(|ui| {
                            self.render_mode_badge(ui);
                            self.render_suggestions_popup(ui, accent);
                            self.render_input_bar(ui, accent);
                            self.render_input_hints(ui, text_secondary);

                            if has_results {
                                ui.add_space(16.0);
                                self.render_results(ui);
                            }
                        });
                    },
                );
            });

        // Render add connection dialog if open
        if self.tree_state.show_add_dialog {
            self.render_add_connection_dialog(
                ui,
                self.theme.text_primary(),
                text_secondary,
                accent,
            );
        }

        // Render result overlay if active
        if self.active_overlay != ResultOverlay::None {
            self.render_result_overlay(ui);
        }
    }

    // ========================================================================
    // Result Overlay System
    // ========================================================================

    /// Open an overlay for the specified result.
    fn open_overlay(&mut self, overlay: ResultOverlay, result_idx: usize) {
        self.active_overlay = overlay;
        self.overlay_result_idx = Some(result_idx);
        self.overlay_table_page = 0;
        self.overlay_filter.clear();
    }

    /// Close the active overlay.
    fn close_overlay(&mut self) {
        self.active_overlay = ResultOverlay::None;
        self.overlay_result_idx = None;
        self.overlay_table_page = 0;
        self.overlay_filter.clear();
    }

    /// Render the result overlay.
    fn render_result_overlay(&mut self, ui: &mut egui::Ui) {
        let Some(result_idx) = self.overlay_result_idx else {
            return;
        };

        // Get result - need to check bounds
        if result_idx >= self.history.len() {
            self.close_overlay();
            return;
        }

        let screen_rect = ui.ctx().available_rect();

        // Handle Esc to close (before drawing anything)
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close_overlay();
            return;
        }

        // Draw dimmed backdrop
        ui.painter()
            .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        // Calculate popup dimensions - consistent with diff viewer and other overlays
        let popup_width = (screen_rect.width() * 0.85).clamp(700.0, 1400.0);
        let popup_height = (screen_rect.height() * 0.85).clamp(500.0, 900.0);

        // Render overlay content in a centered Area
        egui::Area::new(egui::Id::new("result_overlay"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);

                overlay_style.frame().inner_margin(0.0).show(ui, |ui| {
                    ui.set_width(popup_width);
                    ui.set_max_width(popup_width);
                    ui.set_max_height(popup_height);

                    match &self.active_overlay {
                        ResultOverlay::None => {}
                        ResultOverlay::Table => {
                            self.render_table_overlay(ui, result_idx);
                        }
                        ResultOverlay::Plan => {
                            self.render_plan_overlay(ui, result_idx);
                        }
                        ResultOverlay::Diff { other_idx } => {
                            let other = *other_idx;
                            self.render_diff_overlay(ui, result_idx, other);
                        }
                    }
                });
            });
    }

    /// Render the table overlay view.
    fn render_table_overlay(&mut self, ui: &mut egui::Ui, result_idx: usize) {
        let colors = OverlayColors::new(self.theme);
        let bg_surface = self.theme.bg_surface();
        let bg_base = self.theme.bg_base();
        let rows_per_page = 50;

        // Extract data from cell first to avoid borrow conflicts
        let (total_rows, num_cols, execution_time_ms, sql_query, has_schema, column_widths) = {
            let cell = &self.history[result_idx];
            let total: usize = cell.batches.iter().map(|b| b.num_rows()).sum();
            let cols = cell.schema.as_ref().map(|s| s.fields().len()).unwrap_or(0);
            let time_ms = cell.stats.as_ref().map(|s| s.total_time.as_millis());

            // Calculate column widths based on header names and data types
            let widths: Vec<f32> = if let Some(schema) = &cell.schema {
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

            (
                total,
                cols,
                time_ms,
                cell.sql.clone(),
                cell.schema.is_some(),
                widths,
            )
        };

        let total_pages = total_rows.div_ceil(rows_per_page);
        let mut should_close = false;
        let mut next_page = false;
        let mut prev_page = false;
        let mut scroll_delta = egui::Vec2::ZERO;

        // Handle keyboard navigation and consume events to prevent propagation to underlying panes
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

            // Close on Escape
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_close = true;
            }
        });

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

                // Row count badge
                self.render_stat_badge(ui, &format!("{total_rows} rows"), &colors);
                ui.add_space(4.0);

                // Column count badge
                self.render_stat_badge(ui, &format!("{num_cols} cols"), &colors);

                // Execution time badge
                if let Some(ms) = execution_time_ms {
                    ui.add_space(4.0);
                    self.render_time_badge(ui, ms, &colors);
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
        let cell = &self.history[result_idx];
        let schema = cell.schema.as_ref().unwrap(); // Safe because has_schema is true

        // Calculate row number width for alignment
        let max_row_num = (self.overlay_table_page + 1) * rows_per_page;
        let row_num_width = max_row_num.to_string().len().max(3);
        let row_num_gutter_width = (row_num_width + 2) as f32 * 8.0;

        let header_height = typography::SM + typography::XS + 8.0;
        let row_height = typography::SM + 8.0;
        let start_row = self.overlay_table_page * rows_per_page;

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

                            // Column headers with fixed widths
                            for (idx, field) in schema.fields().iter().enumerate() {
                                let col_width = column_widths.get(idx).copied().unwrap_or(100.0);
                                let col_spacing = 16.0;

                                // Allocate fixed-width cell
                                let (col_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(col_width + col_spacing, header_height),
                                    egui::Sense::hover(),
                                );

                                // Header background
                                ui.painter().rect_filled(col_rect, 0.0, bg_surface);

                                // Draw column name
                                ui.painter().text(
                                    col_rect.left_center() + egui::vec2(8.0, -6.0),
                                    egui::Align2::LEFT_CENTER,
                                    field.name(),
                                    typography::monospace(typography::SM),
                                    colors.text,
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

                        // ===== Data rows =====
                        let mut rows_shown = 0;
                        let mut current_row = 0;

                        'outer: for batch in &cell.batches {
                            for row_idx in 0..batch.num_rows() {
                                if current_row < start_row {
                                    current_row += 1;
                                    continue;
                                }
                                if rows_shown >= rows_per_page {
                                    break 'outer;
                                }

                                let absolute_row = start_row + rows_shown + 1;

                                // Alternate row background
                                let row_bg = if rows_shown % 2 == 0 {
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
                                    ui.painter().rect_filled(
                                        gutter_rect,
                                        0.0,
                                        self.theme.bg_base(),
                                    );
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

                                        let (display_val, color) = if value == "NULL" {
                                            ("null".to_string(), colors.faint_text)
                                        } else {
                                            // Truncate long values
                                            let max_chars = ((col_width - 8.0) / 7.0) as usize;
                                            if value.len() > max_chars && max_chars > 3 {
                                                (
                                                    format!(
                                                        "{}…",
                                                        &value[..max_chars.saturating_sub(1)]
                                                    ),
                                                    colors.muted_text,
                                                )
                                            } else {
                                                (value, colors.muted_text)
                                            }
                                        };

                                        ui.painter().text(
                                            cell_rect.left_center() + egui::vec2(8.0, 0.0),
                                            egui::Align2::LEFT_CENTER,
                                            display_val,
                                            typography::monospace(typography::SM),
                                            color,
                                        );
                                    }
                                });

                                rows_shown += 1;
                                current_row += 1;
                            }
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

            // SQL query preview (truncated)
            let sql_preview = if sql_query.len() > 60 {
                format!("{}...", &sql_query[..60])
            } else {
                sql_query.clone()
            };
            ui.label(
                RichText::new(&sql_preview)
                    .color(colors.faint_text)
                    .font(typography::monospace(typography::XS)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Keyboard hint
                ui.label(
                    RichText::new("hjkl scroll • [/] page • Esc")
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

    /// Renders a stat badge (e.g., "128 rows", "5 cols").
    fn render_stat_badge(&self, ui: &mut egui::Ui, text: &str, colors: &OverlayColors) {
        egui::Frame::new()
            .fill(colors.badge_bg)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(text)
                        .color(colors.muted_text)
                        .font(typography::proportional(typography::XS)),
                );
            });
    }

    /// Renders a time badge with clock icon.
    fn render_time_badge(&self, ui: &mut egui::Ui, ms: u128, colors: &OverlayColors) {
        egui::Frame::new()
            .fill(colors.badge_bg)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.label(
                        RichText::new(time::TIMER)
                            .color(colors.faint_text)
                            .size(10.0),
                    );
                    ui.label(
                        RichText::new(format!("{ms}ms"))
                            .color(colors.muted_text)
                            .font(typography::proportional(typography::XS)),
                    );
                });
            });
    }

    /// Render the plan overlay view.
    fn render_plan_overlay(&mut self, ui: &mut egui::Ui, result_idx: usize) {
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
            .history
            .get(result_idx)
            .map(|c| c.sql.clone())
            .unwrap_or_default();

        // Get plan stats
        let (total_time, operator_count, bottleneck_count) = self.plan_viewer.stats();

        let mut should_close = false;

        // Handle keyboard navigation
        ui.ctx().input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_close = true;
            }
        });

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

                // Execution time badge
                if !total_time.is_zero() {
                    self.render_time_badge(ui, total_time.as_millis(), &colors);
                    ui.add_space(4.0);
                }

                // Operator count badge
                self.render_stat_badge(ui, &format!("{operator_count} ops"), &colors);

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
            let sql_preview = if sql_query.len() > 50 {
                format!("{}...", &sql_query[..50])
            } else {
                sql_query.clone()
            };
            ui.label(
                RichText::new(&sql_preview)
                    .color(colors.faint_text)
                    .font(typography::monospace(typography::XS)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Keyboard hints based on current mode
                let hints = match self.plan_viewer.mode {
                    PlanViewMode::Tree => "j/k nav • h/l fold • b bottleneck • Esc",
                    PlanViewMode::Stats => "scroll to explore • Esc",
                    PlanViewMode::Waterfall => "j/k nav • b bottleneck • Esc",
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
    fn render_diff_overlay(&mut self, ui: &mut egui::Ui, result_idx: usize, _other_idx: usize) {
        use crate::ui::semantic_icons::diff;

        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let colors = OverlayColors::new(self.theme);
        let theme = self.theme;

        // Handle Escape to close
        let mut should_close = false;
        ui.ctx().input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_close = true;
            }
        });

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
            .history
            .get(result_idx)
            .and_then(|c| c.diff_result.as_ref())
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
                    Self::render_diff_stat_badge_static(
                        ui,
                        &format!("{} matching", sd.matching),
                        theme.semantic_success(),
                    );
                    ui.add_space(4.0);

                    if sd.changed > 0 {
                        Self::render_diff_stat_badge_static(
                            ui,
                            &format!("{} changed", sd.changed),
                            theme.semantic_warning(),
                        );
                        ui.add_space(4.0);
                    }

                    if sd.left_only > 0 {
                        Self::render_diff_stat_badge_static(
                            ui,
                            &format!("{} removed", sd.left_only),
                            theme.semantic_error(),
                        );
                        ui.add_space(4.0);
                    }

                    if sd.right_only > 0 {
                        Self::render_diff_stat_badge_static(
                            ui,
                            &format!("{} added", sd.right_only),
                            theme.accent_muted(),
                        );
                    }
                } else if let Some(stats) = &diff_stats {
                    // Matching badge
                    Self::render_diff_stat_badge_static(
                        ui,
                        &format!("{} matching", stats.matching),
                        theme.semantic_success(),
                    );
                    ui.add_space(4.0);

                    if stats.left_only > 0 {
                        Self::render_diff_stat_badge_static(
                            ui,
                            &format!("{} left only", stats.left_only),
                            theme.semantic_warning(),
                        );
                        ui.add_space(4.0);
                    }

                    if stats.right_only > 0 {
                        Self::render_diff_stat_badge_static(
                            ui,
                            &format!("{} right only", stats.right_only),
                            theme.accent_muted(),
                        );
                        ui.add_space(4.0);
                    }

                    if stats.different > 0 {
                        Self::render_diff_stat_badge_static(
                            ui,
                            &format!("{} different", stats.different),
                            theme.semantic_error(),
                        );
                    }
                } else if !schemas_match && has_left_schema && has_right_schema {
                    Self::render_diff_stat_badge_static(
                        ui,
                        "Schema mismatch",
                        theme.semantic_warning(),
                    );
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

                        Self::render_diff_stat_badge_static(ui, &text, color);
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
                    if let Some(diff_result) = self
                        .history
                        .get(result_idx)
                        .and_then(|c| c.diff_result.as_ref())
                    {
                        match diff_type {
                            DiffType::Schema => {
                                // Schema diff view - column comparison table
                                self.render_schema_diff_content(ui, diff_result);
                            }
                            DiffType::Profile => {
                                // Profile diff view - side by side trees with metric highlighting
                                self.render_profile_diff_content(ui, diff_result);
                            }
                            DiffType::Plan => {
                                // Plan diff view - side by side trees
                                self.render_plan_diff_content(ui, diff_result);
                            }
                            DiffType::Data => {
                                // Data diff view - side by side tables
                                self.render_data_diff_content(ui, diff_result);
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

    /// Static version of render_diff_stat_badge that doesn't borrow self.
    fn render_diff_stat_badge_static(ui: &mut egui::Ui, text: &str, color: Color32) {
        egui::Frame::new()
            .fill(color.gamma_multiply(0.15))
            .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.5)))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(RichText::new(text).color(color).size(11.0));
            });
    }

    /// Render plan diff content (side-by-side plan trees).
    fn render_plan_diff_content(&self, ui: &mut egui::Ui, diff_result: &DiffQueryResult) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let available_width = ui.available_width();
        let available_height = ui.available_height().max(300.0);
        let side_width = (available_width - 12.0) / 2.0;
        let colors = OverlayColors::new(self.theme);

        // Column headers
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&diff_result.left_name)
                            .color(text_primary)
                            .strong(),
                    );
                },
            );
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&diff_result.right_name)
                            .color(text_primary)
                            .strong(),
                    );
                },
            );
        });

        // Separator below headers
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
        ui.add_space(4.0);

        // Side-by-side content
        let content_height = (available_height - 40.0).max(200.0);

        ui.horizontal(|ui| {
            // Left plan panel
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, content_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_max_width(side_width);
                    egui::ScrollArea::vertical()
                        .id_salt("sql_diff_left_plan")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(plan) = &diff_result.left_plan {
                                self.render_plan_tree(ui, plan, 0);
                            } else {
                                ui.label(
                                    RichText::new("No plan data")
                                        .color(text_secondary)
                                        .italics(),
                                );
                            }
                        });
                },
            );

            // Center separator
            let separator_rect = ui.available_rect_before_wrap();
            ui.painter().vline(
                separator_rect.left(),
                separator_rect.y_range(),
                egui::Stroke::new(1.0, colors.separator),
            );
            ui.add_space(4.0);

            // Right plan panel
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, content_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_max_width(side_width);
                    egui::ScrollArea::vertical()
                        .id_salt("sql_diff_right_plan")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(plan) = &diff_result.right_plan {
                                self.render_plan_tree(ui, plan, 0);
                            } else {
                                ui.label(
                                    RichText::new("No plan data")
                                        .color(text_secondary)
                                        .italics(),
                                );
                            }
                        });
                },
            );
        });
    }

    /// Render a simple plan tree (non-interactive, for diff view).
    fn render_plan_tree(&self, ui: &mut egui::Ui, node: &PlanNode, depth: usize) {
        let text_secondary = self.theme.text_secondary();
        let indent = depth as f32 * 16.0;

        ui.horizontal(|ui| {
            ui.add_space(indent);

            // Operator name with color based on category
            let category = enya_datafusion::OperatorCategory::from_operator(&node.operator);
            let color = self.theme.plan_color(category.color_index());

            ui.label(
                RichText::new(&node.operator)
                    .color(color)
                    .strong()
                    .size(12.0),
            );

            // Metrics if available
            if let Some(metrics) = &node.metrics {
                ui.label(
                    RichText::new(format!(
                        " ({}, {}r)",
                        enya_datafusion::format_duration(metrics.elapsed_time),
                        metrics.output_rows
                    ))
                    .color(text_secondary)
                    .size(10.0),
                );
            }
        });

        // Description
        if !node.description.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(indent + 16.0);
                ui.label(
                    RichText::new(&node.description)
                        .color(text_secondary)
                        .size(10.0),
                );
            });
        }

        // Recursively render children
        for child in &node.children {
            self.render_plan_tree(ui, child, depth + 1);
        }
    }

    /// Render schema diff content (unified table showing column differences).
    fn render_schema_diff_content(&self, ui: &mut egui::Ui, diff_result: &DiffQueryResult) {
        use super::types::ColumnDiffStatus;

        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let available_width = ui.available_width();
        let available_height = ui.available_height().max(300.0);
        let colors = OverlayColors::new(self.theme);

        let Some(schema_diff) = &diff_result.schema_diff else {
            ui.label(
                RichText::new("No schema diff data available")
                    .color(text_secondary)
                    .italics(),
            );
            return;
        };

        // Stats summary
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "{} matching  {} changed  {} removed  {} added",
                    schema_diff.matching,
                    schema_diff.changed,
                    schema_diff.left_only,
                    schema_diff.right_only
                ))
                .color(text_secondary)
                .size(10.0),
            );
        });
        ui.add_space(4.0);

        // Column widths - proportional to available width (25%, 30%, 30%, 15%)
        let usable_width = (available_width - 24.0).max(400.0); // Account for padding
        let col_widths = [
            usable_width * 0.25, // Column name
            usable_width * 0.30, // Left type
            usable_width * 0.30, // Right type
            usable_width * 0.15, // Status
        ];
        let row_height = 22.0;

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.add_sized(
                [col_widths[0], row_height],
                egui::Label::new(
                    RichText::new("Column")
                        .color(text_primary)
                        .strong()
                        .size(11.0),
                ),
            );
            ui.add_sized(
                [col_widths[1], row_height],
                egui::Label::new(
                    RichText::new(&diff_result.left_name)
                        .color(self.theme.diff_removed_text())
                        .strong()
                        .size(11.0),
                ),
            );
            ui.add_sized(
                [col_widths[2], row_height],
                egui::Label::new(
                    RichText::new(&diff_result.right_name)
                        .color(self.theme.diff_added_text())
                        .strong()
                        .size(11.0),
                ),
            );
            ui.add_sized(
                [col_widths[3], row_height],
                egui::Label::new(
                    RichText::new("Status")
                        .color(text_primary)
                        .strong()
                        .size(11.0),
                ),
            );
        });

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
        ui.add_space(2.0);

        // Scrollable column rows
        let content_height = (available_height - 80.0).max(200.0);
        egui::ScrollArea::vertical()
            .id_salt("sql_schema_diff_rows")
            .max_height(content_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;

                for col in &schema_diff.columns {
                    // Determine row background and status display based on column status
                    let (bg_color, status_text, status_color) = match &col.status {
                        ColumnDiffStatus::Matching => {
                            (Color32::TRANSPARENT, "✓", self.theme.semantic_success())
                        }
                        ColumnDiffStatus::Changed => (
                            self.theme.semantic_warning().gamma_multiply(0.1),
                            "changed",
                            self.theme.semantic_warning(),
                        ),
                        ColumnDiffStatus::LeftOnly => (
                            self.theme.diff_removed_bg(),
                            "removed",
                            self.theme.diff_removed_text(),
                        ),
                        ColumnDiffStatus::RightOnly => (
                            self.theme.diff_added_bg(),
                            "added",
                            self.theme.diff_added_text(),
                        ),
                    };

                    // Row frame
                    egui::Frame::new()
                        .fill(bg_color)
                        .inner_margin(egui::Margin::symmetric(0, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);

                                // Column name
                                ui.add_sized(
                                    [col_widths[0], row_height],
                                    egui::Label::new(
                                        RichText::new(&col.name).color(text_primary).size(11.0),
                                    ),
                                );

                                // Left type
                                let left_type = col
                                    .left_type
                                    .as_ref()
                                    .map(|t| {
                                        let nullable = col
                                            .left_nullable
                                            .map(|n| if n { " NULL" } else { " NOT NULL" })
                                            .unwrap_or("");
                                        format!("{t}{nullable}")
                                    })
                                    .unwrap_or_else(|| "—".to_string());
                                ui.add_sized(
                                    [col_widths[1], row_height],
                                    egui::Label::new(
                                        RichText::new(&left_type)
                                            .color(if col.left_type.is_some() {
                                                text_secondary
                                            } else {
                                                text_secondary.gamma_multiply(0.5)
                                            })
                                            .size(10.0),
                                    ),
                                );

                                // Right type
                                let right_type = col
                                    .right_type
                                    .as_ref()
                                    .map(|t| {
                                        let nullable = col
                                            .right_nullable
                                            .map(|n| if n { " NULL" } else { " NOT NULL" })
                                            .unwrap_or("");
                                        format!("{t}{nullable}")
                                    })
                                    .unwrap_or_else(|| "—".to_string());
                                ui.add_sized(
                                    [col_widths[2], row_height],
                                    egui::Label::new(
                                        RichText::new(&right_type)
                                            .color(if col.right_type.is_some() {
                                                text_secondary
                                            } else {
                                                text_secondary.gamma_multiply(0.5)
                                            })
                                            .size(10.0),
                                    ),
                                );

                                // Status
                                ui.add_sized(
                                    [col_widths[3], row_height],
                                    egui::Label::new(
                                        RichText::new(status_text).color(status_color).size(10.0),
                                    ),
                                );
                            });
                        });
                }
            });
    }

    /// Render profile diff content (side-by-side trees with timing deltas).
    fn render_profile_diff_content(&self, ui: &mut egui::Ui, diff_result: &DiffQueryResult) {
        let text_secondary = self.theme.text_secondary();
        let available_height = ui.available_height().max(300.0);
        let available_width = ui.available_width();

        // Side-by-side layout
        let separator_width = 2.0;
        let side_width = (available_width - separator_width) / 2.0;

        // Side-by-side scrolling content
        let content_height = (available_height - 80.0).max(200.0);

        egui::ScrollArea::vertical()
            .id_salt("sql_profile_diff_split")
            .max_height(content_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(available_width);
                ui.set_max_width(available_width);

                if let Some(left_plan) = &diff_result.left_plan {
                    self.render_split_profile_tree(
                        ui,
                        left_plan,
                        diff_result.right_plan.as_ref(),
                        side_width,
                    );
                } else if let Some(right_plan) = &diff_result.right_plan {
                    self.render_split_profile_tree(ui, right_plan, None, side_width);
                } else {
                    ui.label(
                        RichText::new("No plan data available")
                            .color(text_secondary)
                            .italics(),
                    );
                }
            });
    }

    /// Render side-by-side profile trees like git diff.
    fn render_split_profile_tree(
        &self,
        ui: &mut egui::Ui,
        left_node: &PlanNode,
        right_root: Option<&PlanNode>,
        side_width: f32,
    ) {
        let mut paired_rows: Vec<(Option<ProfileRow>, Option<ProfileRow>)> = Vec::new();
        Self::build_paired_profile_rows(left_node, right_root, 0, &mut paired_rows);

        let text_secondary = self.theme.text_secondary();
        let row_height = 28.0;
        let separator_width = 2.0;
        let total_width = side_width * 2.0 + separator_width;

        for (left_row, right_row) in &paired_rows {
            // Main operator row
            ui.horizontal(|ui| {
                ui.set_min_width(total_width);
                ui.set_max_width(total_width);

                // Left side panel
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(side_width, row_height));
                        ui.set_max_width(side_width);
                        ui.horizontal(|ui| {
                            self.render_profile_row(ui, left_row.as_ref(), true, side_width);
                        });
                    });

                // Center separator line
                let rect = ui.available_rect_before_wrap();
                ui.painter().vline(
                    rect.left() + 1.0,
                    rect.y_range(),
                    egui::Stroke::new(1.0, self.theme.border_default()),
                );
                ui.add_space(separator_width);

                // Right side panel
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(side_width, row_height));
                        ui.set_max_width(side_width);
                        ui.horizontal(|ui| {
                            self.render_profile_row(ui, right_row.as_ref(), false, side_width);
                        });
                    });
            });

            // Description row (if present) - show on both sides
            let left_desc = left_row
                .as_ref()
                .filter(|r| !r.description.is_empty())
                .map(|r| (r.description.as_str(), r.depth));
            let right_desc = right_row
                .as_ref()
                .filter(|r| !r.description.is_empty())
                .map(|r| r.description.as_str());

            if left_desc.is_some() || right_desc.is_some() {
                let depth = left_desc.map(|(_, d)| d).unwrap_or(0);
                let indent = 16.0 + depth as f32 * 16.0;

                ui.horizontal(|ui| {
                    ui.set_min_width(total_width);
                    ui.set_max_width(total_width);

                    // Left description
                    egui::Frame::new()
                        .fill(Color32::TRANSPARENT)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(side_width, 16.0));
                            ui.set_max_width(side_width);
                            ui.add_space(indent);
                            if let Some((desc_text, _)) = left_desc {
                                ui.label(
                                    RichText::new(desc_text)
                                        .color(text_secondary.gamma_multiply(0.6))
                                        .size(10.0),
                                );
                            }
                        });

                    ui.add_space(separator_width);

                    // Right description
                    egui::Frame::new()
                        .fill(Color32::TRANSPARENT)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(side_width, 16.0));
                            ui.set_max_width(side_width);
                            ui.add_space(indent);
                            if let Some(desc_text) = right_desc {
                                ui.label(
                                    RichText::new(desc_text)
                                        .color(text_secondary.gamma_multiply(0.6))
                                        .size(10.0),
                                );
                            }
                        });
                });
            }
        }
    }

    /// Render a single row in the split profile view.
    fn render_profile_row(
        &self,
        ui: &mut egui::Ui,
        row: Option<&ProfileRow>,
        is_left: bool,
        side_width: f32,
    ) {
        let text_secondary = self.theme.text_secondary();

        let Some(row) = row else {
            // Empty row placeholder
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(side_width - 8.0, 24.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 0.0, self.theme.bg_base().gamma_multiply(0.3));
            return;
        };

        let indent = row.depth as f32 * 16.0;

        // Delta calculation: how much slower/faster is THIS side compared to OTHER side
        // delta > 0 means other side is slower (I am faster)
        // delta < 0 means other side is faster (I am slower)
        let delta_ms = row
            .other_time_ms
            .map(|other| other as i64 - row.time_ms as i64);

        // For left side (staging): negative delta = staging slower = bad (red)
        // For right side (production): positive delta = production faster = good (green)
        let is_this_side_slower = delta_ms.map(|d| d < -5).unwrap_or(false); // other is faster
        let is_this_side_faster = delta_ms.map(|d| d > 5).unwrap_or(false); // other is slower

        // Determine highlighting based on which side and whether it's significant
        let (should_highlight_red, should_highlight_green) = if is_left {
            // Left side: highlight red if this side is slower
            (is_this_side_slower, false)
        } else {
            // Right side: highlight green if this side is faster
            (false, is_this_side_faster)
        };

        // Background color
        let bg_color = if should_highlight_red {
            self.theme.diff_removed_bg().gamma_multiply(0.4)
        } else if should_highlight_green {
            self.theme.diff_added_bg().gamma_multiply(0.4)
        } else {
            Color32::TRANSPARENT
        };

        // Gutter stripe color
        let gutter_color = if should_highlight_red {
            self.theme.diff_removed_text()
        } else if should_highlight_green {
            self.theme.diff_added_text()
        } else if is_left {
            self.theme.diff_removed_bg().gamma_multiply(0.3)
        } else {
            self.theme.diff_added_bg().gamma_multiply(0.3)
        };

        // Draw row background
        let row_rect = ui.available_rect_before_wrap();
        let bg_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(side_width - 4.0, 24.0));
        ui.painter().rect_filled(bg_rect, 2.0, bg_color);

        // Draw gutter stripe
        let gutter_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(3.0, 24.0));
        ui.painter().rect_filled(gutter_rect, 0.0, gutter_color);

        ui.add_space(8.0 + indent);

        // Tree connector
        if row.depth > 0 {
            ui.label(
                RichText::new("└")
                    .color(text_secondary.gamma_multiply(0.3))
                    .size(10.0),
            );
            ui.add_space(2.0);
        }

        // Operator name with category color
        let category = enya_datafusion::OperatorCategory::from_operator(&row.operator);
        let op_color = self.theme.plan_color(category.color_index());
        ui.label(
            RichText::new(&row.operator)
                .color(op_color)
                .strong()
                .size(11.0),
        );

        ui.add_space(8.0);

        // Timing display - always neutral color, only delta is highlighted
        ui.label(
            RichText::new(format!("{}ms", row.time_ms))
                .color(text_secondary)
                .size(11.0),
        );

        // Delta badge - only show on the SLOWER side
        if let Some(delta) = delta_ms {
            // Only show badge if this side is slower and the difference is significant
            if is_this_side_slower && delta.abs() > 5 {
                ui.add_space(6.0);
                let diff = delta.abs();
                ui.label(
                    RichText::new(format!("+{diff}ms"))
                        .color(self.theme.semantic_error())
                        .size(10.0)
                        .strong(),
                );
            }
        }

        // Row count (compact)
        if row.rows > 0 {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("{}rows", enya_datafusion::format_rows(row.rows)))
                    .color(text_secondary.gamma_multiply(0.5))
                    .size(9.0),
            );
        }
    }

    /// Build paired rows from two plan trees for side-by-side rendering.
    fn build_paired_profile_rows(
        left_node: &PlanNode,
        right_root: Option<&PlanNode>,
        depth: usize,
        rows: &mut Vec<(Option<ProfileRow>, Option<ProfileRow>)>,
    ) {
        let right_node = Self::find_matching_node(left_node, right_root, depth);

        let left_time_ms = left_node
            .metrics
            .as_ref()
            .map(|m| m.elapsed_time.as_millis() as u64)
            .unwrap_or(0);
        let right_time_ms = right_node
            .and_then(|n| n.metrics.as_ref())
            .map(|m| m.elapsed_time.as_millis() as u64);

        let left_rows = left_node
            .metrics
            .as_ref()
            .map(|m| m.output_rows)
            .unwrap_or(0);
        let right_rows = right_node
            .and_then(|n| n.metrics.as_ref())
            .map(|m| m.output_rows)
            .unwrap_or(0);

        let left_row = ProfileRow {
            operator: left_node.operator.clone(),
            description: left_node.description.clone(),
            depth,
            time_ms: left_time_ms,
            other_time_ms: right_time_ms,
            rows: left_rows,
        };

        let right_row = right_node.map(|rn| ProfileRow {
            operator: rn.operator.clone(),
            description: rn.description.clone(),
            depth,
            time_ms: right_time_ms.unwrap_or(0),
            other_time_ms: Some(left_time_ms),
            rows: right_rows,
        });

        rows.push((Some(left_row), right_row));

        for child in &left_node.children {
            let right_child = right_node
                .and_then(|rn| rn.children.iter().find(|rc| rc.operator == child.operator));
            Self::build_paired_profile_rows(child, right_child, depth + 1, rows);
        }
    }

    /// Find matching node in the other plan tree by operator name.
    /// Note: When called recursively, `other` is already the matched child from the parent level.
    fn find_matching_node<'a>(
        node: &PlanNode,
        other: Option<&'a PlanNode>,
        _depth: usize,
    ) -> Option<&'a PlanNode> {
        let other = other?;
        if other.operator == node.operator {
            Some(other)
        } else {
            None
        }
    }

    /// Render data diff content (side-by-side tables with row highlighting).
    fn render_data_diff_content(&self, ui: &mut egui::Ui, diff_result: &DiffQueryResult) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let available_width = ui.available_width();
        let available_height = ui.available_height().max(300.0);
        let side_width = (available_width - 12.0) / 2.0;
        let colors = OverlayColors::new(self.theme);

        // Compute detailed diff with paired rows
        let table_diff = compute_detailed_diff(
            diff_result.left_schema.as_ref(),
            &diff_result.left_batches,
            diff_result.right_schema.as_ref(),
            &diff_result.right_batches,
        );

        // Schema mismatch warning
        if !diff_result.schemas_match
            && diff_result.left_schema.is_some()
            && diff_result.right_schema.is_some()
        {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{} Schemas don't match", status::WARNING))
                        .color(self.theme.semantic_warning())
                        .size(11.0),
                );
            });
            ui.add_space(4.0);
        }

        // Row counts for headers
        let left_rows: usize = diff_result.left_batches.iter().map(|b| b.num_rows()).sum();
        let right_rows: usize = diff_result.right_batches.iter().map(|b| b.num_rows()).sum();

        // Diff stats summary
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "{} matching  {} left only  {} right only",
                    table_diff.stats.matching,
                    table_diff.stats.left_only,
                    table_diff.stats.right_only
                ))
                .color(text_secondary)
                .size(10.0),
            );
        });
        ui.add_space(4.0);

        // Column headers with connection names
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&diff_result.left_name)
                            .color(self.theme.diff_removed_text())
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!("({left_rows} rows)"))
                            .color(text_secondary)
                            .size(10.0),
                    );
                },
            );
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&diff_result.right_name)
                            .color(self.theme.diff_added_text())
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!("({right_rows} rows)"))
                            .color(text_secondary)
                            .size(10.0),
                    );
                },
            );
        });

        // Separator below headers
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
        ui.add_space(2.0);

        // Calculate dimensions
        let content_height = (available_height - 80.0).max(200.0);
        let num_cols = table_diff.columns.len().max(1);
        let col_width = ((side_width - 16.0) / num_cols as f32).clamp(60.0, 120.0);
        let row_height = 18.0;

        // Render column headers
        ui.horizontal(|ui| {
            // Left header
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, row_height),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(4.0);
                    for col in &table_diff.columns {
                        let display_name = if col.len() > 12 {
                            format!("{}…", &col[..11])
                        } else {
                            col.clone()
                        };
                        ui.add_sized(
                            [col_width, row_height],
                            egui::Label::new(
                                RichText::new(display_name)
                                    .color(text_primary)
                                    .strong()
                                    .size(10.0),
                            ),
                        );
                    }
                },
            );
            // Right header
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, row_height),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(4.0);
                    for col in &table_diff.columns {
                        let display_name = if col.len() > 12 {
                            format!("{}…", &col[..11])
                        } else {
                            col.clone()
                        };
                        ui.add_sized(
                            [col_width, row_height],
                            egui::Label::new(
                                RichText::new(display_name)
                                    .color(text_primary)
                                    .strong()
                                    .size(10.0),
                            ),
                        );
                    }
                },
            );
        });

        // Separator below column headers
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, self.theme.border_default()),
        );

        // Scrollable paired rows
        egui::ScrollArea::vertical()
            .id_salt("sql_diff_paired_rows")
            .max_height(content_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;

                let max_rows = 100;
                for pair in table_diff.rows.iter().take(max_rows) {
                    self.render_diff_row_pair(ui, pair, side_width, col_width, row_height);
                }

                if table_diff.rows.len() > max_rows {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("… {}+ rows", table_diff.rows.len()))
                            .color(text_secondary)
                            .italics()
                            .size(9.0),
                    );
                }
            });
    }

    /// Render a single paired row in the diff view.
    fn render_diff_row_pair(
        &self,
        ui: &mut egui::Ui,
        pair: &super::diff::DiffRowPair,
        side_width: f32,
        col_width: f32,
        row_height: f32,
    ) {
        let text_secondary = self.theme.text_secondary();
        let empty_bg = self.theme.bg_base().gamma_multiply(0.7);

        // Determine colors based on row status
        let (left_bg, left_text, right_bg, right_text) = match (&pair.left, &pair.right) {
            (Some(left), Some(_right)) => {
                // Both present - matching rows
                if left.status == RowDiffStatus::Matching {
                    (None, text_secondary, None, text_secondary)
                } else {
                    // Hash collision or mismatch - treat as different
                    (
                        Some(self.theme.diff_removed_bg()),
                        self.theme.diff_removed_text(),
                        Some(self.theme.diff_added_bg()),
                        self.theme.diff_added_text(),
                    )
                }
            }
            (Some(_), None) => {
                // Left only - red/removed, right empty
                (
                    Some(self.theme.diff_removed_bg()),
                    self.theme.diff_removed_text(),
                    Some(empty_bg),
                    text_secondary,
                )
            }
            (None, Some(_)) => {
                // Right only - green/added, left empty
                (
                    Some(empty_bg),
                    text_secondary,
                    Some(self.theme.diff_added_bg()),
                    self.theme.diff_added_text(),
                )
            }
            (None, None) => return, // Shouldn't happen
        };

        // Allocate the full row first to get proper rect for backgrounds
        let (row_rect, _) = ui.allocate_exact_size(
            egui::vec2(side_width * 2.0 + 8.0, row_height),
            egui::Sense::hover(),
        );

        // Draw backgrounds first (behind content)
        let left_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(side_width, row_height));
        let right_rect = egui::Rect::from_min_size(
            egui::pos2(row_rect.min.x + side_width + 8.0, row_rect.min.y),
            egui::vec2(side_width, row_height),
        );

        if let Some(bg) = left_bg {
            ui.painter().rect_filled(left_rect, 0.0, bg);
        }
        if let Some(bg) = right_bg {
            ui.painter().rect_filled(right_rect, 0.0, bg);
        }

        // Draw vertical separator
        ui.painter().vline(
            row_rect.min.x + side_width + 4.0,
            egui::Rangef::new(row_rect.top(), row_rect.bottom()),
            egui::Stroke::new(1.0, self.theme.border_default().gamma_multiply(0.5)),
        );

        // Render left side content
        if let Some(row) = &pair.left {
            self.render_diff_row_cells_at(ui, row, left_rect, col_width, left_text);
        }

        // Render right side content
        if let Some(row) = &pair.right {
            self.render_diff_row_cells_at(ui, row, right_rect, col_width, right_text);
        }
    }

    /// Render cells for a single diff row at a specific position.
    fn render_diff_row_cells_at(
        &self,
        ui: &mut egui::Ui,
        row: &DiffRow,
        rect: egui::Rect,
        col_width: f32,
        text_color: egui::Color32,
    ) {
        let mut x = rect.left() + 4.0;
        let y_center = rect.center().y;

        for value in &row.values {
            let max_chars = (col_width / 7.0) as usize;
            let display_value = if value.chars().count() > max_chars && max_chars > 3 {
                let truncated: String = value.chars().take(max_chars - 1).collect();
                format!("{truncated}…")
            } else {
                value.clone()
            };

            ui.painter().text(
                egui::pos2(x, y_center),
                egui::Align2::LEFT_CENTER,
                display_value,
                egui::FontId::monospace(9.0),
                text_color,
            );

            x += col_width;
        }
    }

    // ========================================================================
    // Input Bar UI Components
    // ========================================================================

    /// Update suggestions based on current input.
    fn update_suggestions(&mut self) {
        // Clone input to avoid borrow issues with mutable methods
        let input = self.input.trim().to_string();

        // Clear if empty
        if input.is_empty() {
            self.suggestions.items.clear();
            self.suggestions.visible = false;
            self.suggestions.selected = 0;
            return;
        }

        // Check if typing a command (starts with /)
        if let Some(cmd_query) = input.strip_prefix('/') {
            // Special handling for /explain and /analyze - show table suggestions for SQL part
            let sql_part = cmd_query
                .strip_prefix("explain ")
                .or_else(|| cmd_query.strip_prefix("analyze "));

            if let Some(sql_part) = sql_part {
                // Check for table completion in the SQL part
                let upper = sql_part.to_uppercase();
                let needs_table = upper.ends_with("FROM ")
                    || upper.ends_with("JOIN ")
                    || upper.ends_with("UPDATE ")
                    || upper.ends_with("INTO ")
                    || upper.ends_with("TABLE ");

                if needs_table {
                    self.suggestions.items = self.get_schema_suggestions("");
                    self.suggestions.visible = !self.suggestions.items.is_empty();
                    self.suggestions.selected = 0;
                    return;
                }

                // Check for partial table/schema name
                let words: Vec<&str> = sql_part.split_whitespace().collect();
                if words.len() >= 2 {
                    let second_last = words[words.len() - 2].to_uppercase();
                    let last = words[words.len() - 1];

                    if ["FROM", "JOIN", "UPDATE", "INTO", "TABLE"].contains(&second_last.as_str()) {
                        self.suggestions.items = self.get_schema_suggestions(last);
                        self.suggestions.visible = !self.suggestions.items.is_empty();
                        self.suggestions.selected = 0;
                        return;
                    }
                }

                // No SQL suggestions, fall through to command matching
            }

            // Special handling for /connect - show endpoint suggestions
            if cmd_query == "connect" || cmd_query.starts_with("connect ") {
                let partial = cmd_query.strip_prefix("connect").unwrap_or("").trim_start();

                let mut items = Vec::new();

                if partial.is_empty() {
                    // Show all options when no partial typed
                    items.push(Suggestion {
                        label: "localhost:50051".to_string(),
                        detail: "Local Flight SQL".to_string(),
                        insert: "/connect localhost:50051".to_string(),
                        icon: SuggestionIcon::Connection,
                        score: 0,
                        match_positions: Vec::new(),
                    });

                    for conn in &self.connections {
                        let status = if matches!(conn.state, ConnectionState::Connected) {
                            "connected"
                        } else {
                            "saved"
                        };
                        items.push(Suggestion {
                            label: conn.name.clone(),
                            detail: format!("{} ({})", conn.endpoint, status),
                            insert: format!("/connect {}", conn.name),
                            icon: SuggestionIcon::Connection,
                            score: 0,
                            match_positions: Vec::new(),
                        });
                    }
                } else {
                    // Fuzzy match with nucleo
                    let pattern = Pattern::new(
                        partial,
                        CaseMatching::Ignore,
                        Normalization::Smart,
                        AtomKind::Fuzzy,
                    );
                    let mut indices: Vec<u32> = Vec::new();
                    let mut buf = Vec::new();

                    // Check localhost
                    indices.clear();
                    let haystack = Utf32Str::new("localhost:50051", &mut buf);
                    if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices)
                    {
                        items.push(Suggestion {
                            label: "localhost:50051".to_string(),
                            detail: "Local Flight SQL".to_string(),
                            insert: "/connect localhost:50051".to_string(),
                            icon: SuggestionIcon::Connection,
                            score: i64::from(score),
                            match_positions: indices.iter().map(|&i| i as usize).collect(),
                        });
                    }

                    // Check existing connections
                    for conn in &self.connections {
                        indices.clear();
                        let haystack = Utf32Str::new(&conn.name, &mut buf);
                        if let Some(score) =
                            pattern.indices(haystack, &mut self.matcher, &mut indices)
                        {
                            let status = if matches!(conn.state, ConnectionState::Connected) {
                                "connected"
                            } else {
                                "saved"
                            };
                            items.push(Suggestion {
                                label: conn.name.clone(),
                                detail: format!("{} ({})", conn.endpoint, status),
                                insert: format!("/connect {}", conn.name),
                                icon: SuggestionIcon::Connection,
                                score: i64::from(score),
                                match_positions: indices.iter().map(|&i| i as usize).collect(),
                            });
                        }
                    }

                    // Sort by score descending
                    items.sort_by(|a, b| b.score.cmp(&a.score));
                }

                self.suggestions.items = items;
                self.suggestions.visible = !self.suggestions.items.is_empty();
                self.suggestions.selected = 0;
                return;
            }

            // Standard command matching with nucleo
            self.suggestions.items = self.fuzzy_match_commands(cmd_query);
            self.suggestions.visible = !self.suggestions.items.is_empty();
            self.suggestions.selected = 0;
            return;
        }

        // Check for table completion (after FROM, JOIN, etc.)
        let upper = input.to_uppercase();
        let needs_table = upper.ends_with("FROM ")
            || upper.ends_with("JOIN ")
            || upper.ends_with("UPDATE ")
            || upper.ends_with("INTO ")
            || upper.ends_with("TABLE ");

        if needs_table {
            // Show catalogs/schemas hierarchy (no partial typed yet)
            self.suggestions.items = self.get_schema_suggestions("");
            self.suggestions.visible = !self.suggestions.items.is_empty();
            self.suggestions.selected = 0;
            return;
        }

        // Check for partial table/schema name after keywords
        let words: Vec<&str> = input.split_whitespace().collect();
        if words.len() >= 2 {
            let second_last = words[words.len() - 2].to_uppercase();
            let last = words[words.len() - 1];

            if ["FROM", "JOIN", "UPDATE", "INTO", "TABLE"].contains(&second_last.as_str()) {
                // Hierarchical matching: catalog.schema.table
                self.suggestions.items = self.get_schema_suggestions(last);
                self.suggestions.visible = !self.suggestions.items.is_empty();
                self.suggestions.selected = 0;
                return;
            }
        }

        // No suggestions
        self.suggestions.items.clear();
        self.suggestions.visible = false;
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
            SqlMode::Profile => {
                egui::Frame::new()
                    .fill(accent.gamma_multiply(0.1))
                    .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.label(RichText::new("PROFILE").color(accent).size(10.0).strong());
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
            .map(|s| (s.label.clone(), s.detail.clone(), s.icon))
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
                        for (idx, (label, detail, icon_type)) in suggestions.iter().enumerate() {
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
                                        let icon = match icon_type {
                                            SuggestionIcon::Command => action::TERMINAL,
                                            SuggestionIcon::Table => file::DATA,
                                            SuggestionIcon::Column => file::DATA,
                                            SuggestionIcon::Connection => category::DATAFUSION,
                                            SuggestionIcon::History => time::HISTORY,
                                        };
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

                                        // Label
                                        ui.label(
                                            RichText::new(label)
                                                .color(if is_selected {
                                                    accent
                                                } else {
                                                    text_primary
                                                })
                                                .size(12.0),
                                        );

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
                // Replace last partial word with suggestion
                let words: Vec<&str> = input.split_whitespace().collect();
                if words.len() >= 2 {
                    let prefix = words[..words.len() - 1].join(" ");
                    self.input = format!("{} {} ", prefix, suggestion.insert);
                } else {
                    self.input = format!("{} ", suggestion.insert);
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
                ui.horizontal(|ui| {
                    // Prompt indicator
                    let prompt = match &self.mode {
                        SqlMode::Normal => "SQL",
                        SqlMode::Diff { .. } => "DIFF",
                        SqlMode::Explain => "EXPLAIN",
                        SqlMode::Profile => "PROFILE",
                    };
                    ui.label(RichText::new(prompt).color(accent).size(11.0).strong());

                    ui.label(
                        RichText::new(">")
                            .color(text_secondary.gamma_multiply(0.5))
                            .size(12.0),
                    );

                    ui.add_space(8.0);

                    // Connection indicator (small pill)
                    if let Some(conn) = self.active_connection() {
                        let conn_name = conn.name.clone();
                        let is_connected = matches!(conn.state, ConnectionState::Connected);
                        let status_color = if is_connected {
                            self.theme.semantic_success()
                        } else {
                            text_secondary.gamma_multiply(0.4)
                        };

                        let pill = egui::Frame::new()
                            .fill(self.theme.bg_surface())
                            .corner_radius(10.0)
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("●").color(status_color).size(6.0));
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(&conn_name).color(text_secondary).size(10.0),
                                    );
                                });
                            });
                        if pill.response.clicked() {
                            self.sidebar_width = if self.sidebar_width == 0.0 { 1.0 } else { 0.0 };
                        }
                        pill.response
                            .on_hover_cursor(egui::CursorIcon::PointingHand);

                        ui.add_space(8.0);
                    }

                    // Main text input with syntax highlighting
                    let theme = self.theme;
                    let mut layouter =
                        move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                            let mut job = highlight_sql(text.as_str(), theme);
                            job.wrap.max_width = wrap_width;
                            ui.fonts_mut(|f| f.layout_job(job))
                        };

                    // Use stable ID for focus tracking
                    let input_id = egui::Id::new(format!("sql_input_{}", self.id));
                    let response = ui.add(
                        TextEdit::singleline(&mut self.input)
                            .id(input_id)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(ui.available_width() - 50.0)
                            .frame(false)
                            .layouter(&mut layouter)
                            .hint_text(
                                RichText::new("SELECT * FROM ... or /help")
                                    .color(text_secondary.gamma_multiply(0.4))
                                    .monospace(),
                            ),
                    );

                    // Request focus on initial render or when suggestions are visible
                    // This ensures the input doesn't lose focus when interacting with suggestions
                    if self.input_focused {
                        response.request_focus();
                        self.input_focused = false;
                    } else if self.suggestions.visible && !response.has_focus() {
                        // Re-request focus if suggestions are visible but input lost focus
                        response.request_focus();
                    }

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
                        let modifiers = ui.input(|i| i.modifiers);

                        // Handle Escape key
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            if self.suggestions.visible {
                                // First Escape closes suggestions
                                self.suggestions.visible = false;
                            } else {
                                // Second Escape releases focus from input
                                response.surrender_focus();
                            }
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
                            // Tab to insert suggestion
                            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                                self.insert_suggestion(self.suggestions.selected);
                            }
                        }

                        // Ctrl+Enter or Cmd+Enter to execute
                        let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if enter_pressed && (modifiers.ctrl || modifiers.command) {
                            self.execute_input();
                        }
                    }

                    // Run button (small, subtle)
                    let has_connection = self.active_connection().is_some();
                    let run_btn = ui.add_enabled(
                        has_connection && !self.input.trim().is_empty(),
                        egui::Button::new(RichText::new("⌘↵").color(text_primary).size(11.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(32.0, 20.0)),
                    );
                    if run_btn.clicked() {
                        self.execute_input();
                    }
                    run_btn.on_hover_text("Run query (Ctrl+Enter)");
                });
            });
    }

    /// Render input hints line.
    fn render_input_hints(&self, ui: &mut egui::Ui, text_secondary: Color32) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if self.suggestions.visible {
                ui.label(
                    RichText::new("↑↓ navigate")
                        .color(text_secondary.gamma_multiply(0.5))
                        .size(10.0),
                );
                ui.label(
                    RichText::new("·")
                        .color(text_secondary.gamma_multiply(0.3))
                        .size(10.0),
                );
                ui.label(
                    RichText::new("Tab insert")
                        .color(text_secondary.gamma_multiply(0.5))
                        .size(10.0),
                );
                ui.label(
                    RichText::new("·")
                        .color(text_secondary.gamma_multiply(0.3))
                        .size(10.0),
                );
            }
            ui.label(
                RichText::new("⌘↵ run")
                    .color(text_secondary.gamma_multiply(0.5))
                    .size(10.0),
            );
            ui.label(
                RichText::new("·")
                    .color(text_secondary.gamma_multiply(0.3))
                    .size(10.0),
            );
            ui.label(
                RichText::new("/help commands")
                    .color(text_secondary.gamma_multiply(0.5))
                    .size(10.0),
            );

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
                        RichText::new("No connection · /connect to add")
                            .color(text_secondary.gamma_multiply(0.5))
                            .size(10.0),
                    );
                }
            });
        });
    }

    // ========================================================================
    // Legacy Minimal Premium Layout Components (kept for reference)
    // ========================================================================

    /// Render minimal header with title and connection pill.
    #[allow(dead_code)]
    fn render_minimal_header(
        &mut self,
        ui: &mut egui::Ui,
        text_secondary: Color32,
        accent: Color32,
    ) {
        ui.horizontal(|ui| {
            // Left: "SQL" title
            ui.label(
                RichText::new("SQL")
                    .color(self.theme.text_primary())
                    .size(18.0)
                    .strong(),
            );

            // Right: Connection pill
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pill_response = self.render_connection_pill(ui, text_secondary, accent);

                // Toggle popup on click
                if pill_response.clicked() {
                    self.tree_state.show_add_dialog = false; // Close add dialog if open
                    // Toggle popup state (we'll use sidebar_width as a flag, repurposed)
                    if self.sidebar_width == 0.0 {
                        self.sidebar_width = 1.0; // Show popup
                    } else {
                        self.sidebar_width = 0.0; // Hide popup
                    }
                }
            });
        });
    }

    /// Render the connection status pill (clickable).
    #[allow(dead_code)]
    fn render_connection_pill(
        &self,
        ui: &mut egui::Ui,
        text_secondary: Color32,
        accent: Color32,
    ) -> egui::Response {
        let (pill_text, pill_color, status_color) = if let Some(conn) = self.active_connection() {
            match &conn.state {
                ConnectionState::Connected => (
                    conn.name.clone(),
                    text_secondary,
                    self.theme.semantic_success(),
                ),
                ConnectionState::Connecting => (conn.name.clone(), accent, accent),
                ConnectionState::Disconnected => (
                    conn.name.clone(),
                    text_secondary.gamma_multiply(0.7),
                    text_secondary.gamma_multiply(0.5),
                ),
                ConnectionState::Failed(_) => (
                    conn.name.clone(),
                    self.theme.semantic_error(),
                    self.theme.semantic_error(),
                ),
            }
        } else if !self.connections.is_empty() {
            (
                "Select connection".to_string(),
                text_secondary.gamma_multiply(0.7),
                text_secondary.gamma_multiply(0.3),
            )
        } else {
            (
                "No connection".to_string(),
                text_secondary.gamma_multiply(0.7),
                text_secondary.gamma_multiply(0.3),
            )
        };

        // Pill button
        let response = egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .stroke(egui::Stroke::new(1.0, self.theme.border_default()))
            .corner_radius(12.0)
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Status dot
                    ui.label(RichText::new("●").color(status_color).size(8.0));
                    ui.add_space(6.0);

                    // Connection name
                    ui.label(RichText::new(&pill_text).color(pill_color).size(12.0));

                    ui.add_space(4.0);

                    // Dropdown arrow
                    ui.label(
                        RichText::new(nav::EXPAND)
                            .color(text_secondary.gamma_multiply(0.5))
                            .size(10.0),
                    );
                });
            })
            .response;

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Render the query editor (hero element).
    #[allow(dead_code)]
    fn render_query_editor(&mut self, ui: &mut egui::Ui, accent: Color32) {
        let text_secondary = self.theme.text_secondary();

        // Query input container with subtle styling
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .stroke(egui::Stroke::new(
                1.0,
                if self.input_focused {
                    accent.gamma_multiply(0.3)
                } else {
                    self.theme.border_default()
                },
            ))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                // SQL syntax-highlighted text input
                let theme = self.theme;
                let mut layouter =
                    move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                        let mut job = highlight_sql(text.as_str(), theme);
                        job.wrap.max_width = wrap_width;
                        ui.fonts_mut(|f| f.layout_job(job))
                    };

                let response = ui.add(
                    TextEdit::multiline(&mut self.input)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(ui.available_width())
                        .desired_rows(4)
                        .frame(false)
                        .layouter(&mut layouter)
                        .hint_text(
                            RichText::new("SELECT * FROM table WHERE ...")
                                .color(text_secondary.gamma_multiply(0.4))
                                .monospace(),
                        ),
                );

                if self.input_focused {
                    response.request_focus();
                    self.input_focused = false;
                }

                // Execute on Ctrl+Enter or Cmd+Enter
                if response.has_focus() {
                    let modifiers = ui.input(|i| i.modifiers);
                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if enter_pressed && (modifiers.ctrl || modifiers.command) {
                        self.execute_input();
                    }
                }
            });

        // Subtle hint below input
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_connection = self.active_connection().is_some();
                let hint_text = if has_connection {
                    "⌘↵ to run"
                } else {
                    "Connect to run queries"
                };
                ui.label(
                    RichText::new(hint_text)
                        .color(text_secondary.gamma_multiply(0.5))
                        .size(11.0),
                );
            });
        });
    }

    /// Render query results - compact preview with overlay expansion.
    fn render_results(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        // Check for running query and extract elapsed time (to avoid borrow issues)
        let running_elapsed_secs = self
            .history
            .iter()
            .find(|cell| !cell.is_info && cell.status == QueryStatus::Running)
            .map(|cell| cell.started_at.elapsed().as_secs_f32());

        // Show loading badge if query is running
        if let Some(elapsed_secs) = running_elapsed_secs {
            egui::Frame::new()
                .fill(self.theme.bg_elevated())
                .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.add_space(8.0);
                        ui.label(RichText::new("Running").color(accent).size(11.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{elapsed_secs:.1}s"))
                                    .color(text_secondary.gamma_multiply(0.7))
                                    .size(10.0)
                                    .monospace(),
                            );
                        });
                    });
                });

            ui.add_space(12.0);

            // Request repaint to update timer
            ui.ctx().request_repaint();
        }

        // Find the most recent non-info result
        let latest_result_idx = self
            .history
            .iter()
            .enumerate()
            .rev()
            .find(|(_, cell)| !cell.is_info && cell.status != QueryStatus::Running)
            .map(|(idx, _)| idx);

        // Handle keyboard shortcuts for overlay (only when not in overlay and input not focused)
        let input_focused = ui.ctx().memory(|m| m.focused().is_some());
        let mut should_clear = false;
        if self.active_overlay == ResultOverlay::None && !input_focused {
            if let Some(idx) = latest_result_idx {
                let cell = &self.history[idx];
                let has_data = !cell.batches.is_empty();

                ui.input(|i| {
                    // 't' or Enter to open table view
                    if has_data && (i.key_pressed(egui::Key::T) || i.key_pressed(egui::Key::Enter))
                    {
                        self.open_overlay(ResultOverlay::Table, idx);
                    }
                    // 'p' to open plan view
                    if i.key_pressed(egui::Key::P) {
                        self.open_overlay(ResultOverlay::Plan, idx);
                    }
                    // 'c' to clear results
                    if i.key_pressed(egui::Key::C) {
                        should_clear = true;
                    }
                });
            }
        }
        if should_clear {
            self.clear_results();
            return; // Skip rendering since history was just cleared
        }

        // Render compact preview
        if let Some(idx) = latest_result_idx {
            self.render_compact_preview(ui, idx, text_primary, text_secondary, accent);
        } else if running_elapsed_secs.is_none() {
            // Show info messages only
            for cell in self.history.iter().filter(|c| c.is_info) {
                if let Some(error) = &cell.error {
                    ui.label(
                        RichText::new(error)
                            .color(self.theme.semantic_error())
                            .size(12.0),
                    );
                } else {
                    ui.label(RichText::new(&cell.sql).color(text_secondary).size(12.0));
                }
                ui.add_space(8.0);
            }
        }
    }

    /// Render a compact preview of a single result with expand hints.
    fn render_compact_preview(
        &mut self,
        ui: &mut egui::Ui,
        idx: usize,
        text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        let cell = &self.history[idx];
        let max_preview_rows = 3;
        let max_value_len = 16; // Truncate long values for compactness

        // Result container
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .stroke(egui::Stroke::new(1.0, self.theme.border_default()))
            .corner_radius(8.0)
            .inner_margin(0.0)
            .show(ui, |ui| {
                // Header: status and stats
                egui::Frame::new()
                    .fill(self.theme.bg_surface())
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .corner_radius(egui::CornerRadius {
                        nw: 8,
                        ne: 8,
                        sw: 0,
                        se: 0,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| match &cell.status {
                            QueryStatus::Running => {
                                ui.spinner();
                                ui.add_space(8.0);
                                ui.label(RichText::new("Running...").color(accent).size(11.0));
                            }
                            QueryStatus::Completed => {
                                ui.label(
                                    RichText::new(status::SUCCESS)
                                        .color(self.theme.semantic_success())
                                        .size(11.0),
                                );
                                ui.add_space(6.0);

                                let row_count: usize =
                                    cell.batches.iter().map(|b| b.num_rows()).sum();
                                ui.label(
                                    RichText::new(format!("{row_count} rows"))
                                        .color(text_primary)
                                        .size(11.0),
                                );

                                if let Some(stats) = &cell.stats {
                                    ui.label(
                                        RichText::new("·")
                                            .color(text_secondary.gamma_multiply(0.5))
                                            .size(11.0),
                                    );
                                    ui.label(
                                        RichText::new(format!(
                                            "{}ms",
                                            stats.total_time.as_millis()
                                        ))
                                        .color(text_secondary)
                                        .size(11.0),
                                    );
                                }
                            }
                            QueryStatus::Failed => {
                                ui.label(
                                    RichText::new(status::ERROR)
                                        .color(self.theme.semantic_error())
                                        .size(11.0),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new("Error")
                                        .color(self.theme.semantic_error())
                                        .size(11.0),
                                );
                            }
                            QueryStatus::Cancelled => {
                                ui.label(
                                    RichText::new("Cancelled")
                                        .color(text_secondary)
                                        .size(11.0)
                                        .italics(),
                                );
                            }
                        });
                    });

                // Error message if failed
                if let Some(error) = &cell.error {
                    egui::Frame::new()
                        .fill(self.theme.semantic_error().gamma_multiply(0.1))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            // Truncate long errors
                            let display_error = if error.len() > 100 {
                                format!("{}...", &error[..100])
                            } else {
                                error.clone()
                            };
                            ui.label(
                                RichText::new(display_error)
                                    .color(self.theme.semantic_error())
                                    .size(11.0)
                                    .monospace(),
                            );
                        });
                }

                // Compact table preview (if has data)
                if !cell.batches.is_empty() {
                    if let Some(schema) = &cell.schema {
                        let total_cols = schema.fields().len();

                        // Calculate how many columns fit in available width
                        // Available width is ~700px max minus frame margins (~24px)
                        let available_width = ui.available_width() - 24.0;
                        let col_spacing = 16.0; // Space between columns
                        let char_width = 6.5; // Approximate width per monospace char at 10pt
                        let overflow_indicator_width = 40.0; // Space for "+N" indicator

                        // Calculate column widths based on header names (capped at max_value_len)
                        let col_widths: Vec<f32> = schema
                            .fields()
                            .iter()
                            .map(|f| {
                                let name_len = f.name().len().min(max_value_len);
                                (name_len as f32 * char_width).max(40.0) // Min 40px per column
                            })
                            .collect();

                        // Determine how many columns fit
                        let mut total_width = 0.0;
                        let mut show_cols = 0;
                        for (i, &width) in col_widths.iter().enumerate() {
                            let needed = if i == 0 { width } else { col_spacing + width };
                            // Reserve space for overflow indicator if not showing all
                            let reserve = if i + 1 < total_cols {
                                overflow_indicator_width
                            } else {
                                0.0
                            };
                            if total_width + needed + reserve <= available_width {
                                total_width += needed;
                                show_cols = i + 1;
                            } else {
                                break;
                            }
                        }
                        // Show at least 1 column
                        show_cols = show_cols.max(1);

                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                // Table header
                                ui.horizontal(|ui| {
                                    for (col_idx, field) in
                                        schema.fields().iter().take(show_cols).enumerate()
                                    {
                                        if col_idx > 0 {
                                            ui.add_space(col_spacing);
                                        }
                                        let name = field.name();
                                        let display_name = if name.len() > max_value_len {
                                            format!("{}…", &name[..max_value_len - 1])
                                        } else {
                                            name.to_string()
                                        };
                                        ui.label(
                                            RichText::new(display_name)
                                                .color(text_primary)
                                                .size(10.0)
                                                .strong()
                                                .monospace(),
                                        );
                                    }
                                    if total_cols > show_cols {
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(format!("+{}", total_cols - show_cols))
                                                .color(text_secondary.gamma_multiply(0.5))
                                                .size(10.0),
                                        );
                                    }
                                });

                                ui.add_space(2.0);

                                // Preview rows
                                let mut rows_shown = 0;
                                'outer: for batch in &cell.batches {
                                    for row_idx in 0..batch.num_rows() {
                                        if rows_shown >= max_preview_rows {
                                            break 'outer;
                                        }

                                        ui.horizontal(|ui| {
                                            for col_idx in 0..batch.num_columns().min(show_cols) {
                                                if col_idx > 0 {
                                                    ui.add_space(col_spacing);
                                                }
                                                let col = batch.column(col_idx);
                                                let value =
                                                    format_array_value(col.as_ref(), row_idx);

                                                let (display_val, color) = if value == "NULL" {
                                                    (
                                                        "null".to_string(),
                                                        text_secondary.gamma_multiply(0.4),
                                                    )
                                                } else if value.len() > max_value_len {
                                                    (
                                                        format!("{}…", &value[..max_value_len - 1]),
                                                        text_secondary,
                                                    )
                                                } else {
                                                    (value, text_secondary)
                                                };

                                                ui.label(
                                                    RichText::new(display_val)
                                                        .color(color)
                                                        .size(10.0)
                                                        .monospace(),
                                                );
                                            }
                                        });

                                        rows_shown += 1;
                                    }
                                }

                                // "More rows" indicator
                                let total_rows: usize =
                                    cell.batches.iter().map(|b| b.num_rows()).sum();
                                if total_rows > max_preview_rows {
                                    ui.label(
                                        RichText::new(format!(
                                            "… {} more",
                                            total_rows - max_preview_rows
                                        ))
                                        .color(text_secondary.gamma_multiply(0.5))
                                        .size(10.0)
                                        .italics(),
                                    );
                                }
                            });
                    }
                }

                // Footer with expand hints
                egui::Frame::new()
                    .fill(self.theme.bg_surface())
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 8,
                        se: 8,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let has_data = !cell.batches.is_empty();

                            // Expand hints - more compact
                            if has_data {
                                ui.label(RichText::new("t").color(accent).size(10.0).monospace());
                                ui.label(
                                    RichText::new("table")
                                        .color(text_secondary.gamma_multiply(0.6))
                                        .size(9.0),
                                );

                                ui.add_space(8.0);
                            }

                            ui.label(RichText::new("p").color(accent).size(10.0).monospace());
                            ui.label(
                                RichText::new("plan")
                                    .color(text_secondary.gamma_multiply(0.6))
                                    .size(9.0),
                            );

                            ui.add_space(8.0);

                            ui.label(RichText::new("c").color(accent).size(10.0).monospace());
                            ui.label(
                                RichText::new("clear")
                                    .color(text_secondary.gamma_multiply(0.6))
                                    .size(9.0),
                            );

                            // History hint on the right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let query_count =
                                        self.history.iter().filter(|c| !c.is_info).count();
                                    if query_count > 1 {
                                        ui.label(
                                            RichText::new(format!("↑↓ {query_count}"))
                                                .color(text_secondary.gamma_multiply(0.4))
                                                .size(9.0),
                                        );
                                    }
                                },
                            );
                        });
                    });
            });
    }

    /// Render a single result cell (legacy, kept for reference).
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    fn render_result_cell(
        &self,
        ui: &mut egui::Ui,
        cell: &QueryCell,
        idx: usize,
        text_primary: Color32,
        text_secondary: Color32,
        _accent: Color32,
    ) {
        // Info cells are rendered as simple messages without index/row count
        if cell.is_info {
            // For info messages, just show the message
            if let Some(error) = &cell.error {
                // Error info cell
                ui.label(
                    RichText::new(error)
                        .color(self.theme.semantic_error())
                        .size(12.0),
                );
            } else {
                // Regular info cell
                ui.label(RichText::new(&cell.sql).color(text_secondary).size(12.0));
            }
            return;
        }

        // Metadata line: row count and timing (for actual queries)
        ui.horizontal(|ui| {
            // Cell number (subtle)
            ui.label(
                RichText::new(format!("[{}]", idx + 1))
                    .color(text_secondary.gamma_multiply(0.5))
                    .size(10.0)
                    .monospace(),
            );

            ui.add_space(8.0);

            // Status-specific content
            match &cell.status {
                QueryStatus::Running => {
                    ui.spinner();
                    ui.label(RichText::new("Running...").color(text_secondary).size(11.0));
                }
                QueryStatus::Completed => {
                    // Row count
                    let row_count: usize = cell.batches.iter().map(|b| b.num_rows()).sum();
                    ui.label(
                        RichText::new(format!("{row_count} rows"))
                            .color(text_secondary)
                            .size(11.0),
                    );

                    // Timing
                    if let Some(stats) = &cell.stats {
                        ui.label(
                            RichText::new("·")
                                .color(text_secondary.gamma_multiply(0.5))
                                .size(11.0),
                        );
                        ui.label(
                            RichText::new(format!("{}ms", stats.total_time.as_millis()))
                                .color(text_secondary.gamma_multiply(0.7))
                                .size(11.0),
                        );
                    }
                }
                QueryStatus::Failed => {
                    ui.label(
                        RichText::new(status::ERROR)
                            .color(self.theme.semantic_error())
                            .size(11.0),
                    );
                    ui.label(
                        RichText::new("Error")
                            .color(self.theme.semantic_error())
                            .size(11.0),
                    );
                }
                QueryStatus::Cancelled => {
                    ui.label(RichText::new("Cancelled").color(text_secondary).size(11.0));
                }
            }
        });

        ui.add_space(8.0);

        // Error message
        if let Some(error) = &cell.error {
            egui::Frame::new()
                .fill(self.theme.semantic_error().gamma_multiply(0.1))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(error)
                            .color(self.theme.semantic_error())
                            .size(12.0)
                            .monospace(),
                    );
                });
            return;
        }

        // Results table
        if !cell.batches.is_empty() {
            self.render_results_table(ui, cell, text_primary, text_secondary);
        } else if !cell.sql.is_empty() {
            // Info message (from .help, etc.)
            ui.label(RichText::new(&cell.sql).color(text_primary).size(12.0));
        }
    }

    /// Render connection dropdown popup.
    #[allow(dead_code)]
    fn render_connection_popup(&mut self, ui: &mut egui::Ui) {
        // sidebar_width == 1.0 means popup is open (repurposed field)
        if self.sidebar_width != 1.0 {
            return;
        }

        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        // Popup area
        egui::Area::new(egui::Id::new("connection_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(ui.available_width() - 250.0, 60.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(self.theme.bg_elevated())
                    .stroke(egui::Stroke::new(1.0, self.theme.border_default()))
                    .corner_radius(8.0)
                    .shadow(egui::epaint::Shadow {
                        spread: 0,
                        blur: 16,
                        color: Color32::from_black_alpha(40),
                        offset: [0, 4],
                    })
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_min_width(220.0);

                        // Header
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Connections")
                                    .color(text_secondary)
                                    .size(11.0)
                                    .strong(),
                            );
                        });

                        ui.add_space(8.0);

                        // Connection list
                        if self.connections.is_empty() {
                            ui.label(
                                RichText::new("No connections yet")
                                    .color(text_secondary.gamma_multiply(0.6))
                                    .size(11.0),
                            );
                        } else {
                            let connections_snapshot: Vec<_> = self
                                .connections
                                .iter()
                                .map(|c| (c.id, c.name.clone(), c.state.clone(), c.active))
                                .collect();

                            for (id, name, state, active) in connections_snapshot {
                                let is_connected = matches!(state, ConnectionState::Connected);
                                let is_connecting = matches!(state, ConnectionState::Connecting);

                                let status_color = if is_connected {
                                    self.theme.semantic_success()
                                } else if is_connecting {
                                    accent
                                } else {
                                    text_secondary.gamma_multiply(0.4)
                                };

                                let row_bg = if active {
                                    accent.gamma_multiply(0.1)
                                } else {
                                    Color32::TRANSPARENT
                                };

                                let row = egui::Frame::new()
                                    .fill(row_bg)
                                    .corner_radius(4.0)
                                    .inner_margin(egui::Margin::symmetric(8, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            if is_connecting {
                                                ui.spinner();
                                            } else {
                                                ui.label(
                                                    RichText::new("●")
                                                        .color(status_color)
                                                        .size(8.0),
                                                );
                                            }
                                            ui.add_space(8.0);

                                            let name_color =
                                                if active { accent } else { text_primary };
                                            ui.label(
                                                RichText::new(&name).color(name_color).size(12.0),
                                            );
                                        });
                                    });

                                if row.response.clicked() {
                                    if is_connected {
                                        self.set_active_connection(id);
                                    } else if !is_connecting {
                                        self.connect_saved(id);
                                    }
                                    self.sidebar_width = 0.0; // Close popup
                                }

                                row.response.context_menu(|ui| {
                                    if is_connected && ui.button("Disconnect").clicked() {
                                        self.disconnect_saved(id);
                                        ui.close();
                                    }
                                    if ui.button("Remove").clicked() {
                                        self.remove_connection(id);
                                        ui.close();
                                    }
                                });
                            }
                        }

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Add connection button
                        let add_btn = ui.add(
                            egui::Button::new(
                                RichText::new(format!("{} Add Connection", action::ADD))
                                    .color(accent)
                                    .size(11.0),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(200.0, 24.0)),
                        );
                        if add_btn.clicked() {
                            self.tree_state.show_add_dialog = true;
                            self.tree_state.new_conn_name.clear();
                            self.tree_state.new_conn_endpoint.clear();
                            self.sidebar_width = 0.0; // Close popup
                        }
                    });

                // Close popup when clicking outside
                if ui.input(|i| i.pointer.any_click()) {
                    let popup_rect = ui.min_rect();
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if !popup_rect.contains(pos) {
                            self.sidebar_width = 0.0;
                        }
                    }
                }
            });
    }

    // ========================================================================
    // Legacy Panel Components (kept for reference, will be removed)
    // ========================================================================

    /// Render the connection tree sidebar (left panel).
    #[allow(dead_code)]
    fn render_connection_tree(
        &mut self,
        ui: &mut egui::Ui,
        _height: f32,
        text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        // Fill available space - StripBuilder handles sizing
        let available = ui.available_size();

        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .inner_margin(0.0)
            .show(ui, |ui| {
                ui.set_min_size(available);

                ui.vertical(|ui| {
                    // Header
                    egui::Frame::new()
                        .fill(self.theme.bg_surface())
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("CONNECTIONS")
                                        .color(text_secondary)
                                        .size(10.0)
                                        .strong(),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // Plan viewer toggle
                                        let plan_color = if self.show_plan_viewer {
                                            accent
                                        } else {
                                            text_secondary
                                        };
                                        let plan_btn = ui.add(
                                            egui::Button::new(
                                                RichText::new(nav::TREE)
                                                    .color(plan_color)
                                                    .size(12.0),
                                            )
                                            .fill(if self.show_plan_viewer {
                                                accent.gamma_multiply(0.15)
                                            } else {
                                                Color32::TRANSPARENT
                                            })
                                            .stroke(egui::Stroke::NONE)
                                            .corner_radius(4.0)
                                            .min_size(egui::vec2(24.0, 20.0)),
                                        );
                                        if plan_btn.clicked() {
                                            self.show_plan_viewer = !self.show_plan_viewer;
                                        }
                                        plan_btn.on_hover_text("Toggle plan viewer");
                                    },
                                );
                            });
                        });

                    // Connection list
                    egui::ScrollArea::vertical()
                        .id_salt("connection_tree")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(self.sidebar_width - 16.0);
                            ui.add_space(8.0);

                            if self.connections.is_empty() {
                                // Empty state
                                ui.vertical_centered(|ui| {
                                    ui.add_space(40.0);
                                    ui.label(
                                        RichText::new(category::DATAFUSION)
                                            .color(text_secondary.gamma_multiply(0.5))
                                            .size(32.0),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("No connections")
                                            .color(text_secondary)
                                            .size(12.0),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("Add a connection to get started")
                                            .color(text_secondary.gamma_multiply(0.7))
                                            .size(10.0),
                                    );
                                });
                            } else {
                                // Render each connection
                                let connections_snapshot: Vec<_> = self
                                    .connections
                                    .iter()
                                    .map(|c| {
                                        (
                                            c.id,
                                            c.name.clone(),
                                            c.state.clone(),
                                            c.active,
                                            c.tables.clone(),
                                        )
                                    })
                                    .collect();

                                for (id, name, state, active, tables) in connections_snapshot {
                                    self.render_connection_item(
                                        ui,
                                        id,
                                        &name,
                                        &state,
                                        active,
                                        &tables,
                                        text_primary,
                                        text_secondary,
                                        accent,
                                    );
                                }
                            }

                            ui.add_space(16.0);
                        });

                    // Add connection button at bottom
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        egui::Frame::new()
                            .fill(self.theme.bg_surface())
                            .inner_margin(egui::Margin::symmetric(8, 8))
                            .show(ui, |ui| {
                                let add_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(format!("{} Add Connection", action::ADD))
                                            .color(accent)
                                            .size(11.0),
                                    )
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
                                    .corner_radius(4.0)
                                    .min_size(egui::vec2(self.sidebar_width - 24.0, 28.0)),
                                );
                                if add_btn.clicked() {
                                    self.tree_state.show_add_dialog = true;
                                    self.tree_state.new_conn_name.clear();
                                    self.tree_state.new_conn_endpoint.clear();
                                }
                            });
                    });
                });
            });
    }

    /// Render a single connection item in the tree.
    #[allow(clippy::too_many_arguments)]
    fn render_connection_item(
        &mut self,
        ui: &mut egui::Ui,
        id: ConnectionId,
        name: &str,
        state: &ConnectionState,
        active: bool,
        tables: &[TableInfo],
        text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        let is_expanded = self.tree_state.expanded.contains(&id);
        let is_connected = matches!(state, ConnectionState::Connected);
        let is_connecting = matches!(state, ConnectionState::Connecting);

        // Connection row
        let row_bg = if active {
            accent.gamma_multiply(0.1)
        } else {
            Color32::TRANSPARENT
        };

        egui::Frame::new()
            .fill(row_bg)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Expand/collapse arrow
                    let arrow = if is_expanded {
                        nav::EXPAND
                    } else {
                        nav::COLLAPSE
                    };
                    let arrow_btn = ui.add(
                        egui::Button::new(RichText::new(arrow).color(text_secondary).size(10.0))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(16.0, 16.0)),
                    );
                    if arrow_btn.clicked() {
                        self.toggle_connection_expanded(id);
                    }

                    // Connection status indicator
                    let status_color = if is_connected {
                        self.theme.semantic_success()
                    } else if is_connecting {
                        accent
                    } else {
                        text_secondary.gamma_multiply(0.5)
                    };

                    if is_connecting {
                        ui.spinner();
                    } else {
                        ui.label(RichText::new("●").color(status_color).size(8.0));
                    }

                    ui.add_space(4.0);

                    // Connection name (clickable to select/activate)
                    let name_color = if active { accent } else { text_primary };
                    let name_response = ui.add(
                        egui::Label::new(RichText::new(name).color(name_color).size(12.0))
                            .selectable(false)
                            .sense(egui::Sense::click()),
                    );

                    if name_response.clicked() && is_connected {
                        self.set_active_connection(id);
                    }

                    if name_response.double_clicked() && !is_connected && !is_connecting {
                        self.connect_saved(id);
                    }

                    // Context menu
                    name_response.context_menu(|ui| {
                        if is_connected {
                            if ui.button("Disconnect").clicked() {
                                self.disconnect_saved(id);
                                ui.close();
                            }
                            if !active && ui.button("Set as Active").clicked() {
                                self.set_active_connection(id);
                                ui.close();
                            }
                        } else if !is_connecting && ui.button("Connect").clicked() {
                            self.connect_saved(id);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Remove").clicked() {
                            self.remove_connection(id);
                            ui.close();
                        }
                    });
                });
            });

        // Expanded tables
        if is_expanded && is_connected {
            ui.indent(format!("tables_{id:?}"), |ui| {
                if tables.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("No tables")
                                .color(text_secondary.gamma_multiply(0.7))
                                .size(10.0)
                                .italics(),
                        );
                    });
                } else {
                    for table in tables {
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            ui.label(RichText::new(file::DATA).color(text_secondary).size(10.0));
                            ui.add_space(4.0);

                            let table_response = ui.add(
                                egui::Label::new(
                                    RichText::new(&table.name).color(text_secondary).size(11.0),
                                )
                                .selectable(false)
                                .sense(egui::Sense::click()),
                            );

                            // Double-click to insert table name into query
                            if table_response.double_clicked() {
                                if !self.input.is_empty() && !self.input.ends_with(' ') {
                                    self.input.push(' ');
                                }
                                self.input.push_str(&table.name);
                            }
                            table_response.on_hover_text("Double-click to insert into query");
                        });
                    }
                }
            });
        }
    }

    /// Render the plan viewer panel (right side).
    #[allow(dead_code)]
    fn render_plan_viewer_panel(
        &mut self,
        ui: &mut egui::Ui,
        _width: f32,
        _height: f32,
        _text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        // Fill available space - StripBuilder handles sizing
        let available = ui.available_size();

        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .inner_margin(0.0)
            .show(ui, |ui| {
                ui.set_min_size(available);

                ui.vertical(|ui| {
                    // Header
                    egui::Frame::new()
                        .fill(self.theme.bg_surface())
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("PLAN VIEWER")
                                        .color(text_secondary)
                                        .size(10.0)
                                        .strong(),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // Close button
                                        let close_btn = ui.add(
                                            egui::Button::new(
                                                RichText::new(action::CLOSE)
                                                    .color(text_secondary)
                                                    .size(12.0),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(egui::Stroke::NONE)
                                            .min_size(egui::vec2(20.0, 20.0)),
                                        );
                                        if close_btn.clicked() {
                                            self.show_plan_viewer = false;
                                        }
                                        close_btn.on_hover_text("Close plan viewer");

                                        ui.add_space(8.0);

                                        // Mode selector buttons
                                        let modes = [
                                            (PlanViewMode::Tree, "Tree"),
                                            (PlanViewMode::Stats, "Stats"),
                                            (PlanViewMode::Waterfall, "Waterfall"),
                                        ];

                                        for (mode, label) in modes {
                                            let is_active = self.plan_viewer.mode == mode;
                                            let btn = ui.add(
                                                egui::Button::new(
                                                    RichText::new(label)
                                                        .color(if is_active {
                                                            accent
                                                        } else {
                                                            text_secondary
                                                        })
                                                        .size(10.0),
                                                )
                                                .fill(if is_active {
                                                    accent.gamma_multiply(0.15)
                                                } else {
                                                    Color32::TRANSPARENT
                                                })
                                                .stroke(egui::Stroke::NONE)
                                                .corner_radius(4.0),
                                            );
                                            if btn.clicked() {
                                                self.plan_viewer.mode = mode;
                                            }
                                        }
                                    },
                                );
                            });
                        });

                    // Plan viewer content
                    egui::Frame::new()
                        .fill(self.theme.bg_base())
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            if self.plan_viewer.has_plan() {
                                self.plan_viewer.show(ui);
                            } else {
                                // Empty state
                                let empty_height = ui.available_height();
                                ui.vertical_centered(|ui| {
                                    ui.add_space(empty_height / 4.0);
                                    ui.label(
                                        RichText::new(nav::TREE)
                                            .color(text_secondary.gamma_multiply(0.3))
                                            .size(32.0),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("No query plan")
                                            .color(text_secondary)
                                            .size(12.0),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("Run .explain <query> to visualize")
                                            .color(text_secondary.gamma_multiply(0.7))
                                            .size(10.0),
                                    );
                                    ui.add_space(16.0);

                                    // Demo button
                                    let demo_btn = ui.add(
                                        egui::Button::new(
                                            RichText::new("Load Demo Plan")
                                                .color(accent)
                                                .size(11.0),
                                        )
                                        .fill(accent.gamma_multiply(0.1))
                                        .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
                                        .corner_radius(4.0),
                                    );
                                    if demo_btn.clicked() {
                                        self.load_demo_plan();
                                    }
                                });
                            }
                        });
                });
            });
    }

    /// Render the add connection dialog.
    fn render_add_connection_dialog(
        &mut self,
        ui: &mut egui::Ui,
        _text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        egui::Window::new("Add Connection")
            .collapsible(false)
            .resizable(false)
            .default_width(350.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    ui.add_space(8.0);

                    // Name field
                    ui.label(RichText::new("Name").color(text_secondary).size(11.0));
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tree_state.new_conn_name)
                            .hint_text("e.g., Production, Staging, Local")
                            .desired_width(320.0),
                    );

                    ui.add_space(12.0);

                    // Endpoint field
                    ui.label(RichText::new("Endpoint").color(text_secondary).size(11.0));
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.tree_state.new_conn_endpoint)
                            .hint_text("e.g., localhost:50051")
                            .desired_width(320.0)
                            .font(egui::TextStyle::Monospace),
                    );

                    ui.add_space(16.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        let cancel_btn = ui.add(
                            egui::Button::new(
                                RichText::new("Cancel").color(text_secondary).size(12.0),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::new(1.0, self.theme.border_default())),
                        );
                        if cancel_btn.clicked() {
                            self.tree_state.show_add_dialog = false;
                        }

                        ui.add_space(8.0);

                        let can_add = !self.tree_state.new_conn_name.trim().is_empty()
                            && !self.tree_state.new_conn_endpoint.trim().is_empty();

                        let add_btn = ui.add_enabled(
                            can_add,
                            egui::Button::new(
                                RichText::new("Add Connection")
                                    .color(if can_add {
                                        self.theme.bg_base()
                                    } else {
                                        text_secondary
                                    })
                                    .size(12.0),
                            )
                            .fill(if can_add {
                                accent
                            } else {
                                self.theme.bg_surface()
                            }),
                        );

                        if add_btn.clicked() && can_add {
                            let name = self.tree_state.new_conn_name.trim().to_string();
                            let endpoint = self.tree_state.new_conn_endpoint.trim().to_string();
                            self.add_connection(&name, &endpoint);
                            self.tree_state.show_add_dialog = false;
                        }
                    });

                    ui.add_space(8.0);
                });
            });
    }

    #[allow(dead_code)]
    fn render_history(
        &mut self,
        ui: &mut egui::Ui,
        height: f32,
        text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        let scroll_id = egui::Id::new(format!("sql_history_{}", self.id));

        egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .max_height(height)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_min_height(height);

                if self.history.is_empty() {
                    self.render_empty_state(ui, height, text_primary, text_secondary, accent);
                } else {
                    ui.add_space(8.0);
                    let history_len = self.history.len();
                    for (idx, cell) in self.history.iter().enumerate() {
                        self.render_query_cell(
                            ui,
                            cell,
                            idx,
                            history_len,
                            text_primary,
                            text_secondary,
                            accent,
                        );
                        ui.add_space(8.0);
                    }
                }

                if self.scroll_to_bottom {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    self.scroll_to_bottom = false;
                }
            });
    }

    /// Render empty state for the query results area.
    #[allow(dead_code)]
    fn render_empty_state(
        &self,
        ui: &mut egui::Ui,
        height: f32,
        _text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(height / 3.0);

            // Icon
            ui.label(
                RichText::new(category::DATAFUSION)
                    .color(text_secondary.gamma_multiply(0.3))
                    .size(40.0),
            );

            ui.add_space(12.0);

            if self.connections.is_empty() {
                // No connections yet
                ui.label(
                    RichText::new("Add a connection to get started")
                        .color(text_secondary)
                        .size(13.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Use the + Add Connection button in the sidebar")
                        .color(text_secondary.gamma_multiply(0.7))
                        .size(11.0),
                );
            } else if self.active_connection().is_none() {
                // Has connections but none active
                ui.label(
                    RichText::new("Select a connection")
                        .color(text_secondary)
                        .size(13.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Double-click a connection to connect, or click to select")
                        .color(text_secondary.gamma_multiply(0.7))
                        .size(11.0),
                );
            } else {
                // Has active connection, ready to query
                ui.label(
                    RichText::new("Ready to query")
                        .color(text_secondary)
                        .size(13.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Enter a SQL query below and press Ctrl+Enter")
                        .color(text_secondary.gamma_multiply(0.7))
                        .size(11.0),
                );

                ui.add_space(16.0);

                // Quick commands
                egui::Frame::new()
                    .fill(self.theme.bg_elevated())
                    .corner_radius(6.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(".tables")
                                    .color(accent)
                                    .size(11.0)
                                    .monospace(),
                            );
                            ui.label(
                                RichText::new("list tables")
                                    .color(text_secondary.gamma_multiply(0.7))
                                    .size(10.0),
                            );
                            ui.add_space(16.0);
                            ui.label(RichText::new(".help").color(accent).size(11.0).monospace());
                            ui.label(
                                RichText::new("show commands")
                                    .color(text_secondary.gamma_multiply(0.7))
                                    .size(10.0),
                            );
                        });
                    });
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    fn render_query_cell(
        &self,
        ui: &mut egui::Ui,
        cell: &QueryCell,
        idx: usize,
        _history_len: usize,
        text_primary: Color32,
        text_secondary: Color32,
        accent: Color32,
    ) {
        // Determine cell styling based on status
        let (border_color, left_accent) = match &cell.status {
            QueryStatus::Running => (self.theme.accent_primary(), self.theme.accent_primary()),
            QueryStatus::Completed => (self.theme.border_default(), self.theme.semantic_success()),
            QueryStatus::Failed => (
                self.theme.semantic_error().gamma_multiply(0.5),
                self.theme.semantic_error(),
            ),
            QueryStatus::Cancelled => (self.theme.border_default(), text_secondary),
        };

        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .stroke(egui::Stroke::new(1.0, border_color))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                // Left accent bar for visual hierarchy
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        rect.left_top() - egui::vec2(12.0, 0.0),
                        egui::vec2(3.0, rect.height().min(60.0)),
                    ),
                    2.0,
                    left_accent,
                );

                // Cell header with number and status
                ui.horizontal(|ui| {
                    // Cell number badge
                    egui::Frame::new()
                        .fill(self.theme.bg_surface())
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("[{}]", idx + 1))
                                    .color(text_secondary)
                                    .size(10.0)
                                    .monospace(),
                            );
                        });

                    ui.add_space(8.0);

                    // Query text (if present)
                    if !cell.sql.is_empty() && cell.batches.is_empty() && cell.error.is_none() {
                        // Info message - show directly (not SQL, so no highlighting)
                        ui.label(RichText::new(&cell.sql).color(text_primary).size(12.0));
                    } else if !cell.sql.is_empty() {
                        // SQL query - styled with prompt and syntax highlighting
                        ui.label(RichText::new(action::PLAY).color(accent).size(11.0));
                        ui.add_space(4.0);

                        // Truncate long SQL for display
                        let display_sql = if cell.sql.len() > 80 {
                            format!("{}...", &cell.sql[..77])
                        } else {
                            cell.sql.clone()
                        };

                        // Use syntax highlighting for the SQL
                        let job = highlight_sql(&display_sql, self.theme);
                        ui.label(job);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status indicator
                        match &cell.status {
                            QueryStatus::Running => {
                                ui.spinner();
                                ui.add_space(4.0);
                                ui.label(RichText::new("Running").color(accent).size(11.0));
                            }
                            QueryStatus::Completed => {
                                if let Some(stats) = &cell.stats {
                                    if stats.rows_returned > 0 {
                                        ui.label(
                                            RichText::new(format!("{} rows", stats.rows_returned))
                                                .color(text_secondary)
                                                .size(11.0),
                                        );
                                    }
                                }
                            }
                            QueryStatus::Failed => {
                                ui.label(
                                    RichText::new(status::ERROR)
                                        .color(self.theme.semantic_error())
                                        .size(11.0),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Error")
                                        .color(self.theme.semantic_error())
                                        .size(11.0),
                                );
                            }
                            QueryStatus::Cancelled => {
                                ui.label(
                                    RichText::new("Cancelled")
                                        .color(text_secondary)
                                        .size(11.0)
                                        .italics(),
                                );
                            }
                        }
                    });
                });

                // Error message (if failed)
                if let Some(error) = &cell.error {
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(self.theme.semantic_error().gamma_multiply(0.1))
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(error)
                                    .color(self.theme.semantic_error())
                                    .size(12.0),
                            );
                        });
                }

                // Render results table (if completed with data)
                if cell.status == QueryStatus::Completed && !cell.batches.is_empty() {
                    ui.add_space(12.0);
                    self.render_results_table(ui, cell, text_primary, text_secondary);
                }
            });
    }

    fn render_results_table(
        &self,
        ui: &mut egui::Ui,
        cell: &QueryCell,
        text_primary: Color32,
        text_secondary: Color32,
    ) {
        let Some(schema) = &cell.schema else { return };

        let total_rows: usize = cell.batches.iter().map(|b| b.num_rows()).sum();
        let max_display_rows = 100;
        let accent = self.theme.accent_primary();

        // Table container with premium styling
        egui::Frame::new()
            .fill(self.theme.bg_base())
            .stroke(egui::Stroke::new(1.0, self.theme.border_default()))
            .corner_radius(6.0)
            .inner_margin(0.0)
            .show(ui, |ui| {
                // Table header
                egui::Frame::new()
                    .fill(self.theme.bg_surface())
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .corner_radius(egui::CornerRadius {
                        nw: 6,
                        ne: 6,
                        sw: 0,
                        se: 0,
                    })
                    .show(ui, |ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt(format!("header_{:?}", cell.id))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for (idx, field) in schema.fields().iter().enumerate() {
                                        if idx > 0 {
                                            ui.add_space(16.0);
                                        }
                                        ui.set_min_width(80.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(field.name())
                                                    .color(text_primary)
                                                    .size(11.0)
                                                    .strong(),
                                            );
                                            ui.label(
                                                RichText::new(format!("{}", field.data_type()))
                                                    .color(text_secondary.gamma_multiply(0.7))
                                                    .size(9.0),
                                            );
                                        });
                                    }
                                });
                            });
                    });

                // Table body
                egui::ScrollArea::both()
                    .id_salt(format!("body_{:?}", cell.id))
                    .max_height(250.0)
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                let mut rows_shown = 0;
                                'outer: for batch in &cell.batches {
                                    for row_idx in 0..batch.num_rows() {
                                        if rows_shown >= max_display_rows {
                                            break 'outer;
                                        }

                                        // Alternate row background
                                        let row_bg = if rows_shown % 2 == 0 {
                                            Color32::TRANSPARENT
                                        } else {
                                            self.theme.bg_surface().gamma_multiply(0.5)
                                        };

                                        egui::Frame::new()
                                            .fill(row_bg)
                                            .inner_margin(egui::Margin::symmetric(0, 2))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    for col_idx in 0..batch.num_columns() {
                                                        if col_idx > 0 {
                                                            ui.add_space(16.0);
                                                        }
                                                        ui.set_min_width(80.0);
                                                        let col = batch.column(col_idx);
                                                        let value = format_array_value(
                                                            col.as_ref(),
                                                            row_idx,
                                                        );

                                                        // Style NULL values differently
                                                        let (display_val, color) = if value
                                                            == "NULL"
                                                        {
                                                            (
                                                                "null".to_string(),
                                                                text_secondary.gamma_multiply(0.5),
                                                            )
                                                        } else {
                                                            (value, text_secondary)
                                                        };

                                                        ui.label(
                                                            RichText::new(display_val)
                                                                .color(color)
                                                                .size(11.0)
                                                                .monospace(),
                                                        );
                                                    }
                                                });
                                            });
                                        rows_shown += 1;
                                    }
                                }
                            });
                    });

                // Table footer with row count
                egui::Frame::new()
                    .fill(self.theme.bg_surface())
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 6,
                        se: 6,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{total_rows} rows"))
                                    .color(text_secondary)
                                    .size(10.0),
                            );

                            if total_rows > max_display_rows {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(format!("(showing first {max_display_rows})"))
                                        .color(accent.gamma_multiply(0.7))
                                        .size(10.0),
                                );
                            }
                        });
                    });
            });
    }

    #[allow(dead_code)]
    fn render_input(&mut self, ui: &mut egui::Ui, _text_primary: Color32, accent: Color32) {
        let text_secondary = self.theme.text_secondary();

        // Input container with code-editor styling
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
            .inner_margin(0.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Input header bar
                    egui::Frame::new()
                        .fill(self.theme.bg_surface())
                        .inner_margin(egui::Margin::symmetric(12, 6))
                        .corner_radius(egui::CornerRadius {
                            nw: 8,
                            ne: 8,
                            sw: 0,
                            se: 0,
                        })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(category::DATAFUSION).color(accent).size(12.0),
                                );
                                ui.add_space(4.0);
                                ui.label(RichText::new("Query").color(text_secondary).size(11.0));

                                // Show active connection name
                                if let Some(conn) = self.active_connection() {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(nav::COLLAPSE)
                                            .color(text_secondary.gamma_multiply(0.5))
                                            .size(10.0),
                                    );
                                    ui.add_space(4.0);
                                    egui::Frame::new()
                                        .fill(self.theme.semantic_success().gamma_multiply(0.15))
                                        .corner_radius(4.0)
                                        .inner_margin(egui::Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(&conn.name)
                                                    .color(self.theme.semantic_success())
                                                    .size(10.0),
                                            );
                                        });
                                }

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        // Execute button with premium styling
                                        let has_connection = self.active_connection().is_some();
                                        let btn = ui.add_enabled(
                                            has_connection,
                                            egui::Button::new(
                                                RichText::new(format!("{} Run", action::PLAY))
                                                    .color(if has_connection {
                                                        self.theme.bg_base()
                                                    } else {
                                                        text_secondary
                                                    })
                                                    .size(11.0),
                                            )
                                            .fill(if has_connection {
                                                accent
                                            } else {
                                                self.theme.bg_surface()
                                            })
                                            .stroke(egui::Stroke::NONE)
                                            .corner_radius(4.0)
                                            .min_size(egui::vec2(60.0, 22.0)),
                                        );
                                        if btn.clicked() {
                                            self.execute_input();
                                        }
                                        if has_connection {
                                            btn.on_hover_text("Execute query (Ctrl+Enter)");
                                        } else {
                                            btn.on_hover_text("Connect to a database first");
                                        }

                                        ui.add_space(8.0);

                                        // Keyboard shortcut hint
                                        ui.label(
                                            RichText::new("Ctrl+Enter")
                                                .color(text_secondary.gamma_multiply(0.7))
                                                .size(10.0),
                                        );
                                    },
                                );
                            });
                        });

                    // Text input area
                    egui::Frame::new()
                        .fill(self.theme.bg_base())
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Line number gutter
                                ui.vertical(|ui| {
                                    let line_count = self.input.lines().count().max(1);
                                    for i in 1..=line_count.max(3) {
                                        ui.label(
                                            RichText::new(format!("{i:>2}"))
                                                .color(text_secondary.gamma_multiply(0.5))
                                                .size(12.0)
                                                .monospace(),
                                        );
                                    }
                                });

                                ui.add_space(8.0);

                                // Vertical separator
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        ui.cursor().left_top(),
                                        egui::vec2(1.0, 50.0),
                                    ),
                                    0.0,
                                    self.theme.border_default(),
                                );
                                ui.add_space(8.0);

                                // Main text input with SQL syntax highlighting
                                let theme = self.theme;
                                let mut layouter =
                                    move |ui: &egui::Ui,
                                          text: &dyn egui::TextBuffer,
                                          wrap_width: f32| {
                                        let mut job = highlight_sql(text.as_str(), theme);
                                        job.wrap.max_width = wrap_width;
                                        ui.fonts_mut(|f| f.layout_job(job))
                                    };

                                let response = ui.add(
                                    TextEdit::multiline(&mut self.input)
                                        .font(egui::TextStyle::Monospace)
                                        .desired_width(ui.available_width())
                                        .desired_rows(3)
                                        .frame(false)
                                        .layouter(&mut layouter)
                                        .hint_text(
                                            RichText::new("SELECT * FROM table...")
                                                .color(text_secondary.gamma_multiply(0.5))
                                                .monospace(),
                                        ),
                                );

                                if self.input_focused {
                                    response.request_focus();
                                    self.input_focused = false;
                                }

                                // Execute on Ctrl+Enter or Cmd+Enter
                                if response.has_focus() {
                                    let modifiers = ui.input(|i| i.modifiers);
                                    let enter_pressed =
                                        ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if enter_pressed && (modifiers.ctrl || modifiers.command) {
                                        self.execute_input();
                                    }
                                }
                            });
                        });

                    // Footer with command hint
                    egui::Frame::new()
                        .fill(self.theme.bg_surface())
                        .inner_margin(egui::Margin::symmetric(12, 4))
                        .corner_radius(egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 8,
                            se: 8,
                        })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(".help")
                                        .color(accent.gamma_multiply(0.7))
                                        .size(10.0)
                                        .monospace(),
                                );
                                ui.label(
                                    RichText::new("commands")
                                        .color(text_secondary.gamma_multiply(0.6))
                                        .size(10.0),
                                );

                                ui.add_space(16.0);

                                ui.label(
                                    RichText::new(".open")
                                        .color(accent.gamma_multiply(0.7))
                                        .size(10.0)
                                        .monospace(),
                                );
                                ui.label(
                                    RichText::new("connect")
                                        .color(text_secondary.gamma_multiply(0.6))
                                        .size(10.0),
                                );

                                ui.add_space(16.0);

                                ui.label(
                                    RichText::new(".tables")
                                        .color(accent.gamma_multiply(0.7))
                                        .size(10.0)
                                        .monospace(),
                                );
                                ui.label(
                                    RichText::new("list")
                                        .color(text_secondary.gamma_multiply(0.6))
                                        .size(10.0),
                                );
                            });
                        });
                });
            });
    }

    /// Take the pending action, if any.
    pub fn take_action(&mut self) -> SqlPaneAction {
        SqlPaneAction::None
    }
}

/// Format an array value at a given row index.
fn format_array_value(array: &dyn enya_datafusion::arrow::array::Array, row: usize) -> String {
    use enya_datafusion::arrow::array::*;

    if array.is_null(row) {
        return "NULL".to_string();
    }

    // Handle common types
    if let Some(arr) = array.as_any().downcast_ref::<StringArray>() {
        return arr.value(row).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<LargeStringArray>() {
        return arr.value(row).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<Int64Array>() {
        return arr.value(row).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<Int32Array>() {
        return arr.value(row).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<Float64Array>() {
        return format!("{:.4}", arr.value(row));
    }
    if let Some(arr) = array.as_any().downcast_ref::<Float32Array>() {
        return format!("{:.4}", arr.value(row));
    }
    if let Some(arr) = array.as_any().downcast_ref::<BooleanArray>() {
        return arr.value(row).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<TimestampMicrosecondArray>() {
        return arr.value(row).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<TimestampNanosecondArray>() {
        return arr.value(row).to_string();
    }

    // Fallback: use debug format
    format!("{:?}", array.slice(row, 1))
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

    fn set_api_key(&mut self, _key: &str) {
        // Not needed for SQL pane
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed for SQL pane
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
