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

#![cfg(not(target_arch = "wasm32"))]

use egui::text::LayoutJob;
use egui::{Color32, RichText, TextEdit, TextFormat};
use enya_datafusion::arrow::array::{Array, RecordBatch};
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::{
    ConnectionState, ExecutionStats, FlightClient, PlanNode, QueryEvent, QueryId, QueryRequest,
    Session, TableInfo,
};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::mpsc;

use super::plan_view::{PlanViewMode, PlanViewer};
use crate::components::util::id_generator::next_id_usize;
use crate::ui::semantic_icons::{action, category, file, nav, status, time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

// ============================================================================
// SQL Syntax Highlighting
// ============================================================================

/// SQL keywords for syntax highlighting.
const SQL_KEYWORDS: &[&str] = &[
    // DML
    "SELECT",
    "FROM",
    "WHERE",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    // Joins
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "OUTER",
    "CROSS",
    "ON",
    "USING",
    // Clauses
    "ORDER",
    "BY",
    "GROUP",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "DISTINCT",
    "ALL",
    "AS",
    // Logical operators
    "AND",
    "OR",
    "NOT",
    "IN",
    "BETWEEN",
    "LIKE",
    "ILIKE",
    "IS",
    "NULL",
    "EXISTS",
    // DDL
    "CREATE",
    "DROP",
    "ALTER",
    "TABLE",
    "VIEW",
    "INDEX",
    "DATABASE",
    "SCHEMA",
    // Other
    "UNION",
    "INTERSECT",
    "EXCEPT",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "WITH",
    "RECURSIVE",
    "OVER",
    "PARTITION",
    "WINDOW",
    "ROWS",
    "RANGE",
    "EXPLAIN",
    "ANALYZE",
    "VERBOSE",
    "FORMAT",
    "ASC",
    "DESC",
    "NULLS",
    "FIRST",
    "LAST",
    "CAST",
    "FILTER",
    "WITHIN",
    "FETCH",
    "NEXT",
    "ONLY",
    // Boolean values
    "TRUE",
    "FALSE",
    // Types (common)
    "INT",
    "INTEGER",
    "BIGINT",
    "SMALLINT",
    "TINYINT",
    "FLOAT",
    "DOUBLE",
    "REAL",
    "DECIMAL",
    "NUMERIC",
    "VARCHAR",
    "CHAR",
    "TEXT",
    "STRING",
    "BOOLEAN",
    "BOOL",
    "DATE",
    "TIME",
    "TIMESTAMP",
    "INTERVAL",
    "BINARY",
    "VARBINARY",
    "BLOB",
];

/// SQL aggregate and common functions for highlighting.
const SQL_FUNCTIONS: &[&str] = &[
    // Aggregate
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "ARRAY_AGG",
    "STRING_AGG",
    "FIRST_VALUE",
    "LAST_VALUE",
    "NTH_VALUE",
    // Window
    "ROW_NUMBER",
    "RANK",
    "DENSE_RANK",
    "NTILE",
    "LAG",
    "LEAD",
    // String
    "CONCAT",
    "SUBSTRING",
    "SUBSTR",
    "TRIM",
    "LTRIM",
    "RTRIM",
    "UPPER",
    "LOWER",
    "LENGTH",
    "CHAR_LENGTH",
    "REPLACE",
    "SPLIT_PART",
    "REGEXP_REPLACE",
    // Numeric
    "ABS",
    "CEIL",
    "CEILING",
    "FLOOR",
    "ROUND",
    "TRUNC",
    "MOD",
    "POWER",
    "SQRT",
    "LOG",
    "LOG10",
    "LN",
    "EXP",
    "SIGN",
    "RANDOM",
    // Date/Time
    "NOW",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "DATE_TRUNC",
    "DATE_PART",
    "EXTRACT",
    "TO_DATE",
    "TO_TIMESTAMP",
    // Null handling
    "COALESCE",
    "NULLIF",
    "IFNULL",
    "NVL",
    // Type conversion
    "CAST",
    "TRY_CAST",
    "TYPEOF",
    // Conditional
    "IF",
    "IIF",
    "GREATEST",
    "LEAST",
    // DataFusion specific
    "ARROW_TYPEOF",
    "TO_CHAR",
];

/// Token types for SQL highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlToken {
    Keyword,
    Function,
    String,
    Number,
    Comment,
    Operator,
    Identifier,
    Whitespace,
    DotCommand,
}

/// A token with its position and type.
struct Token {
    start: usize,
    end: usize,
    kind: SqlToken,
}

/// Tokenize SQL text for syntax highlighting.
fn tokenize_sql(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let start = i;
        let ch = chars[i];

        // Dot commands (.help, .open, etc.)
        if ch == '.' && start == 0 || (start > 0 && chars[start - 1] == '\n') {
            // Check if this looks like a dot command at start of line
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if j > i + 1 {
                tokens.push(Token {
                    start,
                    end: j,
                    kind: SqlToken::DotCommand,
                });
                i = j;
                continue;
            }
        }

        // Whitespace
        if ch.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Whitespace,
            });
            continue;
        }

        // Single-line comment: --
        if ch == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Comment,
            });
            continue;
        }

        // Block comment: /* ... */
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < chars.len() {
                i += 2; // Skip */
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Comment,
            });
            continue;
        }

        // String: 'single quoted'
        if ch == '\'' {
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    // Check for escaped quote ''
                    if i + 1 < chars.len() && chars[i + 1] == '\'' {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::String,
            });
            continue;
        }

        // Double-quoted identifier: "identifier"
        if ch == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            if i < chars.len() {
                i += 1; // Skip closing "
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Identifier,
            });
            continue;
        }

        // Number
        if ch.is_ascii_digit()
            || (ch == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'e'
                    || chars[i] == 'E')
            {
                if (chars[i] == 'e' || chars[i] == 'E')
                    && i + 1 < chars.len()
                    && (chars[i + 1] == '+' || chars[i + 1] == '-')
                {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Number,
            });
            continue;
        }

        // Identifier or keyword
        if ch.is_alphabetic() || ch == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let upper = word.to_uppercase();

            // Check if it's a function (followed by open paren)
            let is_function = SQL_FUNCTIONS.contains(&upper.as_str());
            let is_keyword = SQL_KEYWORDS.contains(&upper.as_str());

            let kind = if is_function {
                // Look ahead for ( to confirm it's a function call
                let mut peek = i;
                while peek < chars.len() && chars[peek].is_whitespace() {
                    peek += 1;
                }
                if peek < chars.len() && chars[peek] == '(' {
                    SqlToken::Function
                } else if is_keyword {
                    SqlToken::Keyword
                } else {
                    SqlToken::Identifier
                }
            } else if is_keyword {
                SqlToken::Keyword
            } else {
                SqlToken::Identifier
            };

            tokens.push(Token {
                start,
                end: i,
                kind,
            });
            continue;
        }

        // Operators and punctuation
        i += 1;
        tokens.push(Token {
            start,
            end: i,
            kind: SqlToken::Operator,
        });
    }

    tokens
}

/// Create a highlighted LayoutJob for SQL text.
fn highlight_sql(text: &str, theme: AppTheme) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = typography::monospace(typography::SM);

    if text.is_empty() {
        return job;
    }

    let tokens = tokenize_sql(text);
    let mut last_end = 0;

    for token in tokens {
        // Handle any gap (shouldn't happen, but be safe)
        if token.start > last_end {
            if let Some(gap_text) = text.get(last_end..token.start) {
                job.append(
                    gap_text,
                    0.0,
                    TextFormat::simple(font_id.clone(), theme.text_primary()),
                );
            }
        }

        if let Some(token_text) = text.get(token.start..token.end) {
            let color = match token.kind {
                SqlToken::Keyword => theme.syntax_keyword(),
                SqlToken::Function => theme.syntax_function(),
                SqlToken::String => theme.syntax_value(),
                SqlToken::Number => theme.syntax_number(),
                SqlToken::Comment => theme.syntax_comment(),
                SqlToken::Operator => theme.syntax_punctuation(),
                SqlToken::DotCommand => theme.accent_primary(),
                SqlToken::Identifier => theme.text_primary(),
                SqlToken::Whitespace => theme.text_primary(),
            };

            job.append(token_text, 0.0, TextFormat::simple(font_id.clone(), color));
        }

        last_end = token.end;
    }

    // Handle any remaining text
    if last_end < text.len() {
        if let Some(remaining) = text.get(last_end..) {
            job.append(
                remaining,
                0.0,
                TextFormat::simple(font_id.clone(), theme.text_primary()),
            );
        }
    }

    job
}

// ============================================================================
// SQL Pane Types
// ============================================================================

/// Actions that can be triggered by the SQL pane.
#[derive(Debug, Clone)]
pub enum SqlPaneAction {
    /// No action.
    None,
}

// ============================================================================
// Command System
// ============================================================================

/// Available SQL pane commands (triggered with `/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlCommand {
    /// Compare query results across two environments.
    Diff,
    /// Show query execution plan.
    Explain,
    /// Profile query with detailed timing.
    Profile,
    /// Show table schema/structure.
    Schema,
    /// Switch active connection.
    Connect,
    /// Export results to file.
    Export,
    /// Show query history.
    History,
    /// Watch mode - re-run query periodically.
    Watch,
    /// Quick sample of a table.
    Sample,
    /// Show available commands.
    Help,
}

impl SqlCommand {
    /// All available commands.
    fn all() -> &'static [SqlCommand] {
        &[
            SqlCommand::Diff,
            SqlCommand::Explain,
            SqlCommand::Profile,
            SqlCommand::Schema,
            SqlCommand::Connect,
            SqlCommand::Export,
            SqlCommand::History,
            SqlCommand::Watch,
            SqlCommand::Sample,
            SqlCommand::Help,
        ]
    }

    /// Command name (what user types).
    fn name(&self) -> &'static str {
        match self {
            SqlCommand::Diff => "diff",
            SqlCommand::Explain => "explain",
            SqlCommand::Profile => "profile",
            SqlCommand::Schema => "schema",
            SqlCommand::Connect => "connect",
            SqlCommand::Export => "export",
            SqlCommand::History => "history",
            SqlCommand::Watch => "watch",
            SqlCommand::Sample => "sample",
            SqlCommand::Help => "help",
        }
    }

    /// Short description of the command.
    fn description(&self) -> &'static str {
        match self {
            SqlCommand::Diff => "Compare across envs",
            SqlCommand::Explain => "Show query plan",
            SqlCommand::Profile => "Profile execution",
            SqlCommand::Schema => "Table structure",
            SqlCommand::Connect => "Connect to database",
            SqlCommand::Export => "Export results",
            SqlCommand::History => "Query history",
            SqlCommand::Watch => "Auto-refresh query",
            SqlCommand::Sample => "Sample table rows",
            SqlCommand::Help => "Show commands",
        }
    }

    /// Parse command from input string.
    #[allow(dead_code)] // Will be used for command execution
    fn parse(input: &str) -> Option<SqlCommand> {
        let cmd = input.trim().strip_prefix('/')?.split_whitespace().next()?;
        SqlCommand::all().iter().find(|c| c.name() == cmd).cloned()
    }
}

/// Current mode of the SQL pane.
#[derive(Debug, Clone, Default)]
pub enum SqlMode {
    /// Normal query mode.
    #[default]
    Normal,
    /// Diff mode - comparing two environments.
    Diff {
        left: ConnectionId,
        right: ConnectionId,
    },
    /// Explain mode - showing query plan.
    Explain,
    /// Profile mode - detailed execution stats.
    Profile,
}

/// A suggestion item for the completion popup.
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Display text.
    pub label: String,
    /// Secondary text (description, row count, etc.).
    pub detail: String,
    /// Text to insert when selected.
    pub insert: String,
    /// Icon/indicator.
    pub icon: SuggestionIcon,
    /// Fuzzy match score (higher is better).
    pub score: i64,
    /// Positions in label that matched the query.
    pub match_positions: Vec<usize>,
}

/// Icon type for suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionIcon {
    Command,
    Table,
    Column,
    Connection,
    History,
}

/// State for the suggestion popup.
#[derive(Debug, Clone, Default)]
pub struct SuggestionState {
    /// Current suggestions to show.
    pub items: Vec<Suggestion>,
    /// Selected index.
    pub selected: usize,
    /// Whether the popup is visible.
    pub visible: bool,
}

/// Unique identifier for a saved connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(usize);

impl ConnectionId {
    fn new() -> Self {
        Self(next_id_usize())
    }
}

/// A saved database connection configuration.
#[derive(Debug, Clone)]
pub struct SavedConnection {
    /// Unique identifier.
    pub id: ConnectionId,
    /// Display name (e.g., "Production", "Staging", "Local").
    pub name: String,
    /// Flight SQL endpoint URL.
    pub endpoint: String,
    /// Connection state.
    pub state: ConnectionState,
    /// Tables discovered from this connection.
    pub tables: Vec<TableInfo>,
    /// Whether this connection is the currently active one.
    pub active: bool,
}

impl SavedConnection {
    fn new(name: &str, endpoint: &str) -> Self {
        Self {
            id: ConnectionId::new(),
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            state: ConnectionState::Disconnected,
            tables: Vec::new(),
            active: false,
        }
    }
}

/// State for the connection tree sidebar.
#[derive(Debug, Clone, Default)]
pub struct ConnectionTreeState {
    /// IDs of expanded connections (showing their tables).
    pub expanded: FxHashSet<ConnectionId>,
    /// Currently selected item in the tree.
    pub selected: Option<TreeSelection>,
    /// Whether the "Add Connection" dialog is open.
    pub show_add_dialog: bool,
    /// Name input for new connection dialog.
    pub new_conn_name: String,
    /// Endpoint input for new connection dialog.
    pub new_conn_endpoint: String,
}

/// What is selected in the connection tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeSelection {
    /// A connection is selected.
    Connection(ConnectionId),
    /// A table within a connection is selected.
    Table {
        connection: ConnectionId,
        table: String,
    },
}

/// Backend for SQL execution - either local DataFusion or remote Flight.
#[allow(dead_code)] // Local variant will be used for file-based queries
enum SqlBackend {
    /// Local DataFusion session (for file queries).
    Local {
        session: Session,
        event_rx: mpsc::Receiver<QueryEvent>,
    },
    /// Remote Flight SQL connection.
    Flight {
        #[allow(dead_code)] // Client stored for reconnection; queries use endpoint
        client: Box<FlightClient>, // Boxed to avoid large enum variant warning
        tables: Vec<TableInfo>,
    },
}

/// A single executed query with its results.
struct QueryCell {
    /// The SQL query that was executed.
    sql: String,
    /// Query ID for tracking.
    id: QueryId,
    /// Current status of the query.
    status: QueryStatus,
    /// Schema of results (if available).
    schema: Option<SchemaRef>,
    /// Result batches.
    batches: Vec<RecordBatch>,
    /// Execution statistics.
    stats: Option<ExecutionStats>,
    /// Error message if query failed.
    error: Option<String>,
    /// Whether this is an info/system message (not a user query).
    is_info: bool,
}

/// Status of a query execution.
#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryStatus {
    /// Query is running.
    Running,
    /// Query completed successfully.
    Completed,
    /// Query failed with an error.
    Failed,
    /// Query was cancelled.
    Cancelled,
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
            connections: Vec::new(),
            tree_state: ConnectionTreeState::default(),
            sidebar_width: 0.0, // Used as popup visibility flag (0.0 = closed, 1.0 = open)
            mode: SqlMode::default(),
            suggestions: SuggestionState::default(),
            prev_input: String::new(),
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.plan_viewer.set_theme(theme);
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

Slash Commands (type / to see suggestions):
  /help              Show this help message
  /diff <e1> <e2>    Compare query results across two environments
  /explain           Show query execution plan
  /profile           Profile query with detailed timing
  /schema <table>    Show table structure
  /connect <endpoint> Connect to Flight SQL (e.g., localhost:50051)
  /export <format>   Export results to file
  /history           Show query history

Dot Commands (DuckDB/SQLite style):
  .open <endpoint>   Connect to a Flight SQL server
  .close             Disconnect from the server
  .tables            List all tables
  .explain <sql>     Show logical plan for query
  .analyze <sql>     Show physical plan with timing

Keyboard Shortcuts:
  ⌘↵ or Ctrl+Enter   Execute query
  Tab                Insert suggestion
  ↑↓                 Navigate suggestions
  Escape             Close suggestions / release focus"#;
                self.add_info_cell(help_text);
            }
            "diff" => {
                // /diff <left> <right> - Compare query across environments
                if parts.len() < 3 {
                    self.add_info_cell(
                        "Usage: /diff <left-connection> <right-connection>\n\
                         Example: /diff staging production\n\n\
                         After setting diff mode, run a query to compare results.",
                    );
                } else {
                    let left_name = parts[1];
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
                                "Diff mode: {left_name} ↔ {right_name}\nEnter a query to compare results."
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
            }
            "explain" => {
                self.mode = SqlMode::Explain;
                self.add_info_cell("Explain mode: Enter a query to see its execution plan.");
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
                    Some("timeline") => {
                        self.plan_viewer.mode = PlanViewMode::Timeline;
                        self.show_plan_viewer = true;
                    }
                    Some("diff") => {
                        self.plan_viewer.mode = PlanViewMode::Diff;
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
                     .plan [tree|timeline|diff|hide] - Toggle plan viewer\n\
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
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: false,
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

    /// Add an error message cell to history.
    fn add_error_cell(&mut self, message: &str) {
        self.history.push(QueryCell {
            sql: String::new(),
            id: QueryId::new(),
            status: QueryStatus::Failed,
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: Some(message.to_string()),
            is_info: true, // Error messages are system info, not user queries
        });
        self.scroll_to_bottom = true;
    }

    /// Add an info message cell to history.
    fn add_info_cell(&mut self, message: &str) {
        self.history.push(QueryCell {
            sql: message.to_string(),
            id: QueryId::new(),
            status: QueryStatus::Completed,
            schema: None,
            batches: Vec::new(),
            stats: None,
            error: None,
            is_info: true,
        });
        self.scroll_to_bottom = true;
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
                    self.add_info_cell("Query plan loaded. Use j/k/h/l to navigate.");
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
    fn parse_metrics(description: &str) -> Option<enya_datafusion::OperatorMetrics> {
        use std::time::Duration;

        // Look for patterns like [rows=100, time=5.2ms]
        if !description.contains('[') {
            return None;
        }

        let mut metrics = enya_datafusion::OperatorMetrics::default();
        let mut found = false;

        // Parse rows
        if let Some(start) = description.find("rows=") {
            let rest = &description[start + 5..];
            if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                if let Ok(rows) = rest[..end].parse::<usize>() {
                    metrics.output_rows = rows;
                    found = true;
                }
            }
        }

        // Parse time
        if let Some(start) = description.find("time=") {
            let rest = &description[start + 5..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(rest.len());
            if let Ok(time_val) = rest[..end].parse::<f64>() {
                // Check unit
                let unit = &rest[end..];
                let duration = if unit.starts_with("ms") {
                    Duration::from_secs_f64(time_val / 1000.0)
                } else if unit.starts_with("µs") || unit.starts_with("us") {
                    Duration::from_secs_f64(time_val / 1_000_000.0)
                } else if unit.starts_with('s') {
                    Duration::from_secs_f64(time_val)
                } else {
                    Duration::from_secs_f64(time_val / 1000.0) // Default to ms
                };
                metrics.elapsed_time = duration;
                found = true;
            }
        }

        // Parse memory
        if let Some(start) = description.find("mem=") {
            let rest = &description[start + 4..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(rest.len());
            if let Ok(mem_val) = rest[..end].parse::<f64>() {
                let unit = &rest[end..];
                let bytes = if unit.starts_with("KB") {
                    (mem_val * 1024.0) as usize
                } else if unit.starts_with("MB") {
                    (mem_val * 1024.0 * 1024.0) as usize
                } else if unit.starts_with("GB") {
                    (mem_val * 1024.0 * 1024.0 * 1024.0) as usize
                } else {
                    mem_val as usize
                };
                metrics.memory_bytes = bytes;
                found = true;
            }
        }

        if found { Some(metrics) } else { None }
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

                if has_results {
                    // With results: top-aligned layout, input at top, results scroll below
                    ui.allocate_ui_with_layout(
                        egui::vec2(available_width, available_height),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            ui.set_max_width(content_width);
                            ui.vertical(|ui| {
                                self.render_mode_badge(ui);
                                self.render_suggestions_popup(ui, accent);
                                self.render_input_bar(ui, accent);
                                self.render_input_hints(ui, text_secondary);
                                ui.add_space(24.0);
                                self.render_results(ui);
                            });
                        },
                    );
                } else {
                    // Empty state: vertically centered input bar
                    // Estimate input section height: input ~48px + hints ~24px ≈ 80px
                    let estimated_content_height = 80.0;
                    let vertical_offset =
                        ((available_height - estimated_content_height) / 2.0).max(0.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(available_width, available_height),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            // Add space to push content to vertical center
                            ui.add_space(vertical_offset);

                            ui.set_max_width(content_width);
                            ui.vertical(|ui| {
                                self.render_mode_badge(ui);
                                self.render_suggestions_popup(ui, accent);
                                self.render_input_bar(ui, accent);
                                self.render_input_hints(ui, text_secondary);
                            });
                        },
                    );
                }
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

    /// Render query results (only called when history is non-empty).
    fn render_results(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        // Separator line
        ui.add_space(8.0);
        let rect = ui.available_rect_before_wrap();
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            egui::Stroke::new(1.0, self.theme.border_default().gamma_multiply(0.5)),
        );
        ui.add_space(16.0);

        // Results scroll area
        let scroll_id = egui::Id::new(format!("sql_results_{}", self.id));
        egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, cell) in self.history.iter().enumerate() {
                    self.render_result_cell(ui, cell, idx, text_primary, text_secondary, accent);
                    ui.add_space(16.0);
                }

                if self.scroll_to_bottom {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    self.scroll_to_bottom = false;
                }
            });
    }

    /// Render a single result cell.
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
                                            (PlanViewMode::Timeline, "Timeline"),
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
