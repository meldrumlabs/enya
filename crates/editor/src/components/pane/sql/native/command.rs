//! SQL pane command system.
//!
//! Commands are triggered with `/` prefix (e.g., `/explain`, `/analyze`).

/// Available SQL pane commands (triggered with `/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlCommand {
    /// Disconnect from server.
    Close,
    /// List available tables.
    Tables,
    /// Toggle plan viewer.
    Plan,
    /// Compare query results across two environments.
    Diff,
    /// Show query execution plan (EXPLAIN).
    Explain,
    /// Show query execution plan with timing (EXPLAIN ANALYZE).
    Analyze,
    /// Show table schema/structure.
    Schema,
    /// Show query history.
    History,
    /// Load demo query plan.
    Demo,
}

impl SqlCommand {
    /// All available commands.
    pub fn all() -> &'static [SqlCommand] {
        &[
            SqlCommand::Close,
            SqlCommand::Tables,
            SqlCommand::Plan,
            SqlCommand::Explain,
            SqlCommand::Analyze,
            SqlCommand::Demo,
            SqlCommand::Schema,
            SqlCommand::Diff,
            SqlCommand::History,
        ]
    }

    /// Command name (what user types).
    pub fn name(&self) -> &'static str {
        match self {
            SqlCommand::Close => "close",
            SqlCommand::Tables => "tables",
            SqlCommand::Plan => "plan",
            SqlCommand::Diff => "diff",
            SqlCommand::Explain => "explain",
            SqlCommand::Analyze => "analyze",
            SqlCommand::Schema => "schema",
            SqlCommand::History => "history",
            SqlCommand::Demo => "demo",
        }
    }

    /// Short description of the command.
    pub fn description(&self) -> &'static str {
        match self {
            SqlCommand::Close => "Disconnect from server",
            SqlCommand::Tables => "List available tables",
            SqlCommand::Plan => "Toggle plan viewer",
            SqlCommand::Diff => "Compare across envs",
            SqlCommand::Explain => "Show query plan (EXPLAIN)",
            SqlCommand::Analyze => "Query plan with timing (EXPLAIN ANALYZE)",
            SqlCommand::Schema => "Table structure",
            SqlCommand::History => "Query history",
            SqlCommand::Demo => "Load demo query plan",
        }
    }

    /// Parse command from input string.
    #[allow(dead_code)] // Will be used for command execution
    pub fn parse(input: &str) -> Option<SqlCommand> {
        let cmd = input.trim().strip_prefix('/')?.split_whitespace().next()?;
        SqlCommand::all().iter().find(|c| c.name() == cmd).cloned()
    }
}
