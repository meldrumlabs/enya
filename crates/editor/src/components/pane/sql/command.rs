//! SQL pane command system.
//!
//! Commands are triggered with `/` prefix (e.g., `/explain`, `/analyze`).

/// Available SQL pane commands (triggered with `/`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlCommand {
    /// Compare query results across two environments.
    Diff,
    /// Show query execution plan (EXPLAIN).
    Explain,
    /// Show query execution plan with timing (EXPLAIN ANALYZE).
    Analyze,
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
    /// Load demo query plan.
    Demo,
    /// Toggle/set plan viewer mode.
    Plan,
    /// Show available commands.
    Help,
}

impl SqlCommand {
    /// All available commands.
    pub fn all() -> &'static [SqlCommand] {
        &[
            SqlCommand::Explain,
            SqlCommand::Analyze,
            SqlCommand::Demo,
            SqlCommand::Plan,
            SqlCommand::Schema,
            SqlCommand::Connect,
            SqlCommand::Diff,
            SqlCommand::Profile,
            SqlCommand::Export,
            SqlCommand::History,
            SqlCommand::Watch,
            SqlCommand::Sample,
            SqlCommand::Help,
        ]
    }

    /// Command name (what user types).
    pub fn name(&self) -> &'static str {
        match self {
            SqlCommand::Diff => "diff",
            SqlCommand::Explain => "explain",
            SqlCommand::Analyze => "analyze",
            SqlCommand::Profile => "profile",
            SqlCommand::Schema => "schema",
            SqlCommand::Connect => "connect",
            SqlCommand::Export => "export",
            SqlCommand::History => "history",
            SqlCommand::Watch => "watch",
            SqlCommand::Sample => "sample",
            SqlCommand::Demo => "demo",
            SqlCommand::Plan => "plan",
            SqlCommand::Help => "help",
        }
    }

    /// Short description of the command.
    pub fn description(&self) -> &'static str {
        match self {
            SqlCommand::Diff => "Compare across envs",
            SqlCommand::Explain => "Show query plan (EXPLAIN)",
            SqlCommand::Analyze => "Query plan with timing (EXPLAIN ANALYZE)",
            SqlCommand::Profile => "Profile execution",
            SqlCommand::Schema => "Table structure",
            SqlCommand::Connect => "Connect to database",
            SqlCommand::Export => "Export results",
            SqlCommand::History => "Query history",
            SqlCommand::Watch => "Auto-refresh query",
            SqlCommand::Sample => "Sample table rows",
            SqlCommand::Demo => "Load demo query plan",
            SqlCommand::Plan => "Toggle plan viewer",
            SqlCommand::Help => "Show commands",
        }
    }

    /// Parse command from input string.
    #[allow(dead_code)] // Will be used for command execution
    pub fn parse(input: &str) -> Option<SqlCommand> {
        let cmd = input.trim().strip_prefix('/')?.split_whitespace().next()?;
        SqlCommand::all().iter().find(|c| c.name() == cmd).cloned()
    }
}
