//! Editor context for AI agent integration.
//!
//! Provides context about the editor state that can be injected into AI prompts,
//! and parses structured commands from agent responses.

use serde::{Deserialize, Serialize};

/// Context about the editor state for the AI agent.
///
/// This is serialized and injected into the system prompt to give the agent
/// awareness of the current editor state.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EditorContext {
    /// Connection information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionContext>,
    /// Available metrics from Prometheus/backend
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metrics: Vec<String>,
    /// Codebase information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codebase: Option<CodebaseContext>,
    /// Current workspace state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceContext>,
    /// Project-specific context loaded from ENYA.md
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_context: Option<String>,
    /// PR review context (if a PR is currently open)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_review: Option<PrReviewContext>,
}

/// Context about the currently open PR review.
#[derive(Debug, Clone, Serialize)]
pub struct PrReviewContext {
    /// PR number
    pub pr_number: u32,
    /// PR title
    pub pr_title: String,
    /// Number of draft comments
    pub draft_comment_count: usize,
    /// Changed files in the PR
    pub changed_files: Vec<String>,
}

/// Connection context
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionContext {
    /// Backend type (e.g., "prometheus", "demo")
    pub backend: String,
    /// Endpoint URL if connected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Whether the connection is online
    pub is_online: bool,
    /// Backend version if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Codebase context
#[derive(Debug, Clone, Serialize)]
pub struct CodebaseContext {
    /// Repository URL
    pub repo_url: String,
    /// Number of metrics discovered
    pub metric_count: usize,
    /// Number of files scanned
    pub file_count: usize,
    /// Recent commits (last 5)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_commits: Vec<CommitSummary>,
}

/// Summary of a git commit
#[derive(Debug, Clone, Serialize)]
pub struct CommitSummary {
    /// Short hash (7 chars)
    pub hash: String,
    /// Commit message (first line)
    pub message: String,
}

/// Dashboard context
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceContext {
    /// Current time range description
    pub time_range: String,
    /// Number of panes
    pub pane_count: usize,
    /// List of open queries
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub queries: Vec<String>,
    /// Active viewport filter pattern (if filtering panes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

impl EditorContext {
    /// Create an empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the connection context.
    pub fn with_connection(mut self, connection: ConnectionContext) -> Self {
        self.connection = Some(connection);
        self
    }

    /// Set the available metrics (limited to top N).
    pub fn with_metrics(mut self, metrics: Vec<String>) -> Self {
        // Limit to 50 metrics to avoid bloating the prompt
        self.metrics = metrics.into_iter().take(50).collect();
        self
    }

    /// Set the codebase context.
    pub fn with_codebase(mut self, codebase: CodebaseContext) -> Self {
        self.codebase = Some(codebase);
        self
    }

    /// Set the workspace context.
    pub fn with_workspace(mut self, workspace: WorkspaceContext) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Set the project-specific context (loaded from ENYA.md).
    pub fn with_project_context(mut self, context: String) -> Self {
        self.project_context = Some(context);
        self
    }

    /// Set PR review context.
    pub fn with_pr_review(mut self, pr_review: PrReviewContext) -> Self {
        self.pr_review = Some(pr_review);
        self
    }

    /// Generate the context block to inject into the prompt.
    ///
    /// Returns a formatted string that will be prepended to user prompts.
    pub fn to_prompt_block(&self) -> String {
        let mut parts = vec![
            "# Enya Editor Context\n".to_string(),
            "You are integrated with Enya, a metrics visualization editor.\n\n".to_string(),
            "## CRITICAL: Use Enya Commands, NOT Shell Commands\n\n".to_string(),
            "**DO NOT** use bash, grep, ripgrep, find, cat, git, or ANY shell commands.\n".to_string(),
            "**DO NOT** read files directly or scan directories.\n".to_string(),
            "**DO NOT** use your built-in tools for code search or file operations.\n\n".to_string(),
            "**INSTEAD**, output Enya command blocks. The codebase is ALREADY INDEXED.\n\n".to_string(),
            "When the user asks to:\n".to_string(),
            "- Search code → Output `search_codebase` command (NOT grep/ripgrep)\n".to_string(),
            "- Show code → Output `show_inline_source` command (NOT cat/read)\n".to_string(),
            "- Show metrics → Output `show_inline_chart` command\n".to_string(),
            "- Find files → Output `search_codebase` command (NOT find/ls)\n\n".to_string(),
            "Example - if user says \"search for http_requests\":\n".to_string(),
            "```enya-command\n{\"action\": \"search_codebase\", \"query\": \"http_requests\"}\n```\n\n".to_string(),
            "Here is the current state:\n".to_string(),
        ];

        // Connection
        if let Some(ref conn) = self.connection {
            parts.push("\n## Connection\n".to_string());
            parts.push(format!("- Backend: {}\n", conn.backend));
            if let Some(ref endpoint) = conn.endpoint {
                parts.push(format!("- Endpoint: {endpoint}\n"));
            }
            parts.push(format!(
                "- Status: {}\n",
                if conn.is_online { "Online" } else { "Offline" }
            ));
            if let Some(ref version) = conn.version {
                parts.push(format!("- Version: {version}\n"));
            }
        }

        // Metrics
        if !self.metrics.is_empty() {
            parts.push(format!(
                "\n## Available Metrics ({} shown)\n",
                self.metrics.len()
            ));
            for metric in &self.metrics {
                parts.push(format!("- {metric}\n"));
            }
        }

        // Codebase
        if let Some(ref codebase) = self.codebase {
            parts.push("\n## Indexed Codebase\n".to_string());
            parts.push(format!("- Repository: {}\n", codebase.repo_url));
            parts.push(format!("- Metrics found: {}\n", codebase.metric_count));
            parts.push(format!("- Files scanned: {}\n", codebase.file_count));
            if !codebase.recent_commits.is_empty() {
                parts.push("- Recent commits:\n".to_string());
                for commit in &codebase.recent_commits {
                    parts.push(format!("  - {} {}\n", commit.hash, commit.message));
                }
            }
        }

        // Dashboard
        if let Some(ref ws) = self.workspace {
            parts.push("\n## Current Workspace\n".to_string());
            parts.push(format!("- Time range: {}\n", ws.time_range));
            parts.push(format!("- Panes: {}\n", ws.pane_count));
            if let Some(ref filter) = ws.filter {
                parts.push(format!("- Active filter: \"{filter}\"\n"));
            }
            if !ws.queries.is_empty() {
                parts.push("- Active queries:\n".to_string());
                for query in &ws.queries {
                    parts.push(format!("  - {query}\n"));
                }
            }
        }

        // Project Context (from ENYA.md)
        if let Some(ref project_context) = self.project_context {
            parts.push("\n## Project Context\n".to_string());
            parts.push(
                "The following project-specific context was provided by the user in ENYA.md:\n\n"
                    .to_string(),
            );
            parts.push(project_context.clone());
            parts.push("\n".to_string());
        }

        // PR Review Context
        if let Some(ref pr) = self.pr_review {
            parts.push("\n## Active PR Review\n".to_string());
            parts.push(format!("- PR #{}: {}\n", pr.pr_number, pr.pr_title));
            parts.push(format!("- Draft comments: {}\n", pr.draft_comment_count));
            if !pr.changed_files.is_empty() {
                parts.push("- Changed files:\n".to_string());
                for file in &pr.changed_files {
                    parts.push(format!("  - {file}\n"));
                }
            }
            parts.push("\nYou can use `add_pr_comment` to add review comments and `submit_pr_review` to submit.\n".to_string());
        }

        // Commands
        parts.push("\n## Available Commands\n".to_string());
        parts.push(
            "You can execute editor commands by outputting a fenced block like this:\n".to_string(),
        );
        parts.push("```enya-command\n{\"action\": \"create_pane\", \"query\": \"rate(http_requests_total[5m])\", \"title\": \"Request Rate\"}\n```\n".to_string());
        parts.push("\nSupported actions:\n".to_string());
        parts.push(
            "- `create_pane`: Create a new visualization pane with a PromQL query\n".to_string(),
        );
        parts.push("  - Required: `query` (PromQL expression)\n".to_string());
        parts.push("  - Optional: `title` (pane title), `floating` (true for detached pane), `position` ([x, y] pixels)\n".to_string());
        parts.push("- `set_time_range`: Change the dashboard time range\n".to_string());
        parts.push(
            "  - Required: `preset` (e.g., \"15m\", \"1h\", \"6h\", \"24h\", \"7d\")\n".to_string(),
        );
        parts.push(
            "- `search_metrics`: Open the metrics finder with a search pattern\n".to_string(),
        );
        parts.push("  - Required: `pattern` (search string)\n".to_string());
        parts.push(
            "- `show_inline_chart`: Show a time series chart inline in your response (PREFERRED)\n"
                .to_string(),
        );
        parts.push("  - Required: `query` (PromQL expression)\n".to_string());
        parts.push(
            "  - Optional: `title`, `time_range` (e.g., \"1h\"), `height` (pixels)\n".to_string(),
        );
        parts.push(
            "- `show_source`: Show source code for a metric or alert definition (PREFERRED)\n"
                .to_string(),
        );
        parts.push("  - Required: `name` (metric or alert name)\n".to_string());
        parts.push("  - Optional: `source_type` (\"metric\" or \"alert\", default: \"metric\"), `context_lines` (default: 5)\n".to_string());
        parts.push("- `search_codebase`: Search the indexed codebase using full-text search (PREFERRED over git log)\n".to_string());
        parts.push("  - Required: `query` (search terms)\n".to_string());
        parts.push("  - Optional: `filter` (\"all\", \"metrics\", \"alerts\", \"commits\"), `limit` (default: 10)\n".to_string());
        parts.push(
            "  - Returns: Ranked results with file paths, line numbers, and relevance scores\n"
                .to_string(),
        );
        parts.push("- `show_inline_diff`: Show a git diff inline in your response (PREFERRED for showing changes)\n".to_string());
        parts.push("  - Optional: `commit` (hash, \"HEAD\", \"HEAD~1\", etc. - defaults to HEAD/latest commit if omitted)\n".to_string());
        parts.push("  - Optional: `file` (specific file path to show diff for)\n".to_string());
        parts.push(
            "- `show_inline_table`: Show SQL query results as an inline table in your response\n"
                .to_string(),
        );
        parts.push("  - Optional: `query` (SQL query to match in history, uses latest result if omitted), `title`\n".to_string());
        parts.push("- `add_logs_pane`: Create a logs pane for viewing logs (useful for incident investigation)\n".to_string());
        parts.push("  - Optional: `query` (LogQL query), `loki_url` (Loki server URL, uses demo if omitted), `title`\n".to_string());
        parts.push(
            "- `add_tracing_pane`: Create a tracing pane for viewing distributed traces\n"
                .to_string(),
        );
        parts.push("  - Optional: `trace_id` (pre-load a specific trace), `title`\n".to_string());
        parts.push("- `add_terminal_pane`: Create a terminal pane for running shell commands (native app only)\n".to_string());
        parts.push("  - Optional: `title`\n".to_string());
        parts.push("- `set_visualization`: Change the visualization type for a pane\n".to_string());
        parts.push("  - Required: `viz_type` (\"time_series\", \"stat\", \"gauge\", \"bar_chart\", \"pie_chart\", \"sparkline\", \"heatmap\")\n".to_string());
        parts
            .push("  - Optional: `pane` (pane title/name, or omit for focused pane)\n".to_string());
        parts.push("- `set_absolute_time_range`: Set a specific time range (e.g., \"look at 2pm yesterday\")\n".to_string());
        parts.push("  - Required: `start` (Unix timestamp in seconds), `end` (Unix timestamp in seconds)\n".to_string());
        parts.push("- `refresh_pane`: Refresh panes to reload data\n".to_string());
        parts.push(
            "  - Optional: `pane` (pane title/name, or omit to refresh all panes)\n".to_string(),
        );
        parts.push("- `close_pane`: Close a pane\n".to_string());
        parts.push(
            "  - Required: `pane` (pane title/name or \"focused\" for current pane)\n".to_string(),
        );
        parts.push(
            "- `create_section`: Create a collapsible section (Grafana-style organization)\n"
                .to_string(),
        );
        parts.push("  - Required: `name` (section name)\n".to_string());
        parts.push("  - Optional: `collapsed` (start collapsed, default: false)\n".to_string());
        parts.push("- `maximize_pane`: Maximize a pane to fullscreen\n".to_string());
        parts.push(
            "  - Required: `pane` (pane title/name or \"focused\" for current pane)\n".to_string(),
        );
        parts.push(
            "- `load_workspace`: Load a saved workspace by name (for handoff from CLI to GUI)\n"
                .to_string(),
        );
        parts.push("  - Required: `workspace` (workspace name)\n".to_string());
        parts.push(
            "- `open_pr_review`: Open the PR review pane for the current repository\n".to_string(),
        );
        parts.push("- `review_pr`: Navigate to a specific PR in the review pane\n".to_string());
        parts.push("  - Required: `number` (PR number)\n".to_string());
        parts.push(
            "  - Optional: `focus` (focus area like \"security\", \"performance\")\n".to_string(),
        );
        parts
            .push("- `add_pr_comment`: Add a draft review comment on the current PR\n".to_string());
        parts.push(
            "  - Required: `path` (file path), `line` (line number), `body` (comment text)\n"
                .to_string(),
        );
        parts.push("- `submit_pr_review`: Submit the current PR review\n".to_string());
        parts.push(
            "  - Required: `event` (\"approve\", \"request_changes\", or \"comment\")\n"
                .to_string(),
        );
        parts.push("  - Optional: `body` (review summary)\n".to_string());
        parts.push("\n## REMINDER: No Shell Commands\n".to_string());
        parts.push(
            "You MUST output enya-command blocks instead of using bash/grep/find/cat.\n"
                .to_string(),
        );
        parts.push(
            "The user's codebase is already indexed. Shell access is NOT needed.\n".to_string(),
        );
        parts.push("If you catch yourself about to run a shell command, STOP and output the equivalent enya-command instead.\n".to_string());

        parts.join("")
    }
}

/// Commands that can be parsed from agent responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AgentCommand {
    /// Create a new query pane
    CreatePane {
        /// PromQL query
        query: String,
        /// Optional title
        #[serde(default)]
        title: Option<String>,
        /// If true, create a floating (detached) pane instead of a docked pane
        #[serde(default)]
        floating: Option<bool>,
        /// Position for floating panes as [x, y] pixels from top-left
        #[serde(default)]
        position: Option<[f32; 2]>,
    },
    /// Set the time range
    SetTimeRange {
        /// Time range preset (e.g., "15m", "1h", "6h", "24h", "7d")
        preset: String,
    },
    /// Search for metrics matching a pattern
    SearchMetrics {
        /// Search pattern
        pattern: String,
    },
    /// Show source code for a metric definition
    ShowMetricSource {
        /// Metric name to look up
        metric: String,
    },
    /// Show source code for an alert rule
    ShowAlertSource {
        /// Alert name to look up
        alert: String,
    },
    /// Show an inline time series chart in the agent response
    ShowInlineChart {
        /// PromQL query to execute
        query: String,
        /// Chart title
        #[serde(default)]
        title: Option<String>,
        /// Time range (e.g., "1h", "6h", "24h") - defaults to current dashboard range
        #[serde(default)]
        time_range: Option<String>,
        /// Chart height in pixels
        #[serde(default)]
        height: Option<f32>,
    },
    /// Show an inline source code preview in the agent response
    ShowInlineSource {
        /// Metric name to look up source for
        metric: String,
        /// Number of context lines to show (default: 5)
        #[serde(default)]
        context_lines: Option<usize>,
    },
    /// Search the indexed codebase for metrics, alerts, or commits
    SearchCodebase {
        /// Search query (full-text search)
        query: String,
        /// Filter by type: "all", "metrics", "alerts", or "commits"
        #[serde(default)]
        filter: Option<String>,
        /// Maximum results to return (default: 10)
        #[serde(default)]
        limit: Option<usize>,
    },
    /// Show a git diff inline in the response
    ShowInlineDiff {
        /// Commit reference (hash, "HEAD", "HEAD~1", etc.) or empty for working directory changes
        #[serde(default)]
        commit: Option<String>,
        /// Optional file path to show diff for specific file only
        #[serde(default)]
        file: Option<String>,
    },
    /// Show SQL query results as an inline table in the response
    ShowInlineTable {
        /// SQL query to match against recent SQL pane history (uses latest if omitted)
        #[serde(default)]
        query: Option<String>,
        /// Optional title override (defaults to the SQL query text)
        #[serde(default)]
        title: Option<String>,
    },
    /// Add a logs pane for viewing logs (demo or Loki backend)
    AddLogsPane {
        /// Optional LogQL query to pre-fill
        #[serde(default)]
        query: Option<String>,
        /// Optional Loki server URL (uses demo backend if not provided)
        #[serde(default)]
        loki_url: Option<String>,
        /// Optional title for the pane
        #[serde(default)]
        title: Option<String>,
    },
    /// Add a tracing pane for viewing distributed traces
    AddTracingPane {
        /// Optional trace ID to pre-load
        #[serde(default)]
        trace_id: Option<String>,
        /// Optional title for the pane
        #[serde(default)]
        title: Option<String>,
    },
    /// Add a terminal pane for running shell commands (native only)
    AddTerminalPane {
        /// Optional title for the pane
        #[serde(default)]
        title: Option<String>,
    },
    /// Set the visualization type for a pane
    SetVisualization {
        /// The pane to modify (by title/name or "focused" for current pane)
        #[serde(default)]
        pane: Option<String>,
        /// The visualization type: "time_series", "stat", "gauge", "bar_chart", "sparkline", "heatmap"
        viz_type: String,
    },
    /// Set an absolute time range (for looking at specific time periods)
    SetAbsoluteTimeRange {
        /// Start timestamp in Unix seconds (e.g., 1705593600 for 2024-01-18 12:00:00 UTC)
        start: f64,
        /// End timestamp in Unix seconds
        end: f64,
    },
    /// Refresh panes to reload data
    RefreshPane {
        /// Optional pane to refresh (by title/name), or omit to refresh all panes
        #[serde(default)]
        pane: Option<String>,
    },
    /// Close a pane
    ClosePane {
        /// The pane to close (by title/name or "focused" for current pane)
        pane: String,
    },
    /// Create a collapsible section (Grafana-style)
    CreateSection {
        /// Section name
        name: String,
        /// Whether the section starts collapsed (default: false)
        #[serde(default)]
        collapsed: Option<bool>,
    },
    /// Create a floating pane for investigation (detached from main layout)
    CreateFloatingPane {
        /// PromQL query for the pane
        query: String,
        /// Optional title for the pane
        #[serde(default)]
        title: Option<String>,
        /// Optional position as [x, y] pixels from top-left
        #[serde(default)]
        position: Option<[f32; 2]>,
    },
    /// Maximize a pane to fullscreen
    MaximizePane {
        /// The pane to maximize (by title/name or "focused" for current pane)
        pane: String,
    },
    /// Rename a pane
    RenamePane {
        /// The pane to rename (by current title/name or "focused" for current pane)
        pane: String,
        /// The new name for the pane
        new_name: String,
    },
    /// Duplicate a pane (clone with same query)
    DuplicatePane {
        /// The pane to duplicate (by title/name or "focused" for current pane)
        pane: String,
        /// Optional new name for the duplicated pane
        #[serde(default)]
        new_name: Option<String>,
    },
    /// Focus a specific pane
    FocusPane {
        /// The pane to focus (by title/name)
        pane: String,
    },
    /// Toggle zen mode (minimal UI)
    ToggleZenMode,
    /// Exit fullscreen mode
    ExitFullscreen,
    /// Sync repository (git fetch/pull and re-index codebase)
    Sync,
    /// Unified source lookup (metric or alert)
    ShowSource {
        /// Metric or alert name to look up
        name: String,
        /// Source type: "metric" (default) or "alert"
        #[serde(default)]
        source_type: Option<String>,
        /// Number of context lines to show (default: 5)
        #[serde(default)]
        context_lines: Option<usize>,
    },
    /// Load a saved workspace by name (for agent-to-human handoff)
    LoadWorkspace {
        /// Workspace name to load
        workspace: String,
    },
    /// Open the PR review pane
    OpenPrReview,
    /// Open a specific PR for review
    ReviewPr {
        /// PR number to review
        number: u32,
        /// Optional focus areas (e.g., "security", "performance")
        #[serde(default)]
        focus: Option<String>,
    },
    /// Add a draft review comment on the current PR
    AddPrComment {
        /// File path relative to repo root
        path: String,
        /// Line number in the new version
        line: usize,
        /// Comment body
        body: String,
    },
    /// Submit the current PR review
    SubmitPrReview {
        /// Review event: "approve", "request_changes", "comment"
        event: String,
        /// Optional review summary body
        #[serde(default)]
        body: Option<String>,
    },
}

impl AgentCommand {
    /// Returns a human-readable description of the command action.
    ///
    /// Used for displaying command execution status in the UI.
    pub fn description(&self) -> String {
        match self {
            AgentCommand::CreatePane {
                query,
                title,
                floating,
                ..
            } => {
                let prefix = if floating.unwrap_or(false) {
                    "Creating floating pane"
                } else {
                    "Creating pane"
                };
                if let Some(t) = title {
                    format!("{prefix} '{t}'")
                } else {
                    format!("{prefix} for query: {}", truncate_str(query, 40))
                }
            }
            AgentCommand::SetTimeRange { preset } => {
                format!("Setting time range to {preset}")
            }
            AgentCommand::SearchMetrics { pattern } => {
                format!("Searching metrics for '{pattern}'")
            }
            AgentCommand::ShowMetricSource { metric } => {
                format!("Opening source for metric '{metric}'")
            }
            AgentCommand::ShowAlertSource { alert } => {
                format!("Opening source for alert '{alert}'")
            }
            AgentCommand::ShowInlineChart { query, title, .. } => {
                if let Some(t) = title {
                    format!("Showing chart '{t}'")
                } else {
                    format!("Showing chart for: {}", truncate_str(query, 40))
                }
            }
            AgentCommand::ShowInlineSource { metric, .. } => {
                format!("Showing source for '{metric}'")
            }
            AgentCommand::SearchCodebase { query, filter, .. } => {
                if let Some(f) = filter {
                    format!("Searching {f} for '{query}'")
                } else {
                    format!("Searching codebase for '{query}'")
                }
            }
            AgentCommand::AddLogsPane { title, query, .. } => {
                if let Some(t) = title {
                    format!("Adding logs pane '{t}'")
                } else if let Some(q) = query {
                    format!("Adding logs pane with query: {}", truncate_str(q, 30))
                } else {
                    "Adding logs pane".to_string()
                }
            }
            AgentCommand::AddTracingPane { title, trace_id } => {
                if let Some(t) = title {
                    format!("Adding tracing pane '{t}'")
                } else if let Some(id) = trace_id {
                    format!("Adding tracing pane for trace {}", truncate_str(id, 20))
                } else {
                    "Adding tracing pane".to_string()
                }
            }
            AgentCommand::AddTerminalPane { title } => {
                if let Some(t) = title {
                    format!("Adding terminal pane '{t}'")
                } else {
                    "Adding terminal pane".to_string()
                }
            }
            AgentCommand::SetVisualization { pane, viz_type } => {
                if let Some(p) = pane {
                    format!("Setting '{p}' visualization to {viz_type}")
                } else {
                    format!("Setting visualization to {viz_type}")
                }
            }
            AgentCommand::SetAbsoluteTimeRange { start, end } => {
                // Format as duration for readability
                let duration_secs = (end - start) as i64;
                let duration = if duration_secs >= 86400 {
                    format!("{}d", duration_secs / 86400)
                } else if duration_secs >= 3600 {
                    format!("{}h", duration_secs / 3600)
                } else {
                    format!("{}m", duration_secs / 60)
                };
                format!("Setting time range ({duration} window)")
            }
            AgentCommand::RefreshPane { pane } => {
                if let Some(p) = pane {
                    format!("Refreshing pane '{p}'")
                } else {
                    "Refreshing all panes".to_string()
                }
            }
            AgentCommand::ClosePane { pane } => {
                if pane.to_lowercase() == "focused" {
                    "Closing focused pane".to_string()
                } else {
                    format!("Closing pane '{pane}'")
                }
            }
            AgentCommand::CreateSection { name, collapsed } => {
                if collapsed.unwrap_or(false) {
                    format!("Creating section '{name}' (collapsed)")
                } else {
                    format!("Creating section '{name}'")
                }
            }
            AgentCommand::CreateFloatingPane { title, query, .. } => {
                if let Some(t) = title {
                    format!("Creating floating pane '{t}'")
                } else {
                    format!("Creating floating pane for: {}", truncate_str(query, 30))
                }
            }
            AgentCommand::MaximizePane { pane } => {
                if pane.to_lowercase() == "focused" {
                    "Maximizing focused pane".to_string()
                } else {
                    format!("Maximizing pane '{pane}'")
                }
            }
            AgentCommand::RenamePane { pane, new_name } => {
                if pane.to_lowercase() == "focused" {
                    format!("Renaming focused pane to '{new_name}'")
                } else {
                    format!("Renaming '{pane}' to '{new_name}'")
                }
            }
            AgentCommand::DuplicatePane { pane, new_name } => {
                if let Some(name) = new_name {
                    format!("Duplicating '{pane}' as '{name}'")
                } else if pane.to_lowercase() == "focused" {
                    "Duplicating focused pane".to_string()
                } else {
                    format!("Duplicating pane '{pane}'")
                }
            }
            AgentCommand::FocusPane { pane } => {
                format!("Focusing pane '{pane}'")
            }
            AgentCommand::ToggleZenMode => "Toggling zen mode".to_string(),
            AgentCommand::ExitFullscreen => "Exiting fullscreen".to_string(),
            AgentCommand::Sync => "Syncing repository and re-indexing codebase".to_string(),
            AgentCommand::ShowInlineDiff { commit, file } => {
                let commit_str = commit.as_deref().unwrap_or("working directory");
                if let Some(f) = file {
                    format!("Showing diff for '{f}' at {commit_str}")
                } else {
                    format!("Showing diff for {commit_str}")
                }
            }
            AgentCommand::ShowSource {
                name, source_type, ..
            } => {
                let kind = source_type.as_deref().unwrap_or("metric");
                format!("Showing source for {kind} '{name}'")
            }
            AgentCommand::ShowInlineTable { query, title } => {
                if let Some(t) = title {
                    format!("Showing table '{t}'")
                } else if let Some(q) = query {
                    format!("Showing table for: {}", truncate_str(q, 40))
                } else {
                    "Showing latest SQL results".to_string()
                }
            }
            AgentCommand::LoadWorkspace { workspace } => {
                format!("Loading workspace '{workspace}'")
            }
            AgentCommand::OpenPrReview => "Opening PR review pane".to_string(),
            AgentCommand::ReviewPr { number, .. } => {
                format!("Opening PR #{number} for review")
            }
            AgentCommand::AddPrComment { path, line, .. } => {
                format!("Adding review comment on {path}:{line}")
            }
            AgentCommand::SubmitPrReview { event, .. } => {
                format!("Submitting PR review ({event})")
            }
        }
    }
}

/// Truncate a string to a maximum length, adding "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    crate::components::util::text_formatting::truncate_with_ellipsis(s, max_len)
}

/// Parse agent commands from a response text.
///
/// Looks for fenced code blocks with the `enya-command` language tag.
pub fn parse_commands(text: &str) -> Vec<AgentCommand> {
    let mut commands = Vec::new();

    // Look for ```enya-command blocks
    let mut in_block = false;
    let mut block_content = String::new();

    for line in text.lines() {
        if line.trim().starts_with("```enya-command") {
            in_block = true;
            block_content.clear();
        } else if in_block && line.trim().starts_with("```") {
            // End of block - try to parse
            if let Ok(cmd) = serde_json::from_str::<AgentCommand>(&block_content) {
                commands.push(cmd);
            } else {
                log::warn!("Failed to parse enya-command: {block_content}");
            }
            in_block = false;
        } else if in_block {
            block_content.push_str(line);
            block_content.push('\n');
        }
    }

    commands
}

/// Strip enya-command blocks from text for display purposes.
///
/// Removes the `enya-command` fenced code blocks so users see clean responses
/// without the internal command protocol.
pub fn strip_command_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut in_block = false;

    for line in text.lines() {
        if line.trim().starts_with("```enya-command") {
            in_block = true;
        } else if in_block && line.trim().starts_with("```") {
            in_block = false;
            // Skip the closing ``` too
        } else if !in_block {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
        }
    }

    // Trim trailing whitespace
    result.trim().to_string()
}

// ============================================================================
// Context Builder Helpers
// ============================================================================
//
// These helper functions consolidate context-building logic that was previously
// duplicated between `Workspace::update_agent_context` and
// `Workspace::build_editor_context`. By extracting these into reusable functions,
// we ensure consistent context generation across all AI agent integrations.
//
// Usage:
// - `build_connection_context`: Creates connection context from a QueryExecutor
// - `build_workspace_context`: Creates workspace context from current state
// - `build_codebase_context` (native only): Creates codebase context from CodebaseManager
//
// These are intentionally placed in the agent_context module (rather than a
// separate util module) because they are tightly coupled to the context types
// defined here and are only used for AI agent context building.
// ============================================================================

use crate::components::util::query_executor::{Backend, ConnectionHealth, QueryExecutor};

/// Build connection context from a query executor.
///
/// Extracts backend type, endpoint, connection status, and version information
/// from the query executor to create a `ConnectionContext` for AI agents.
///
/// # Arguments
/// * `executor` - Reference to the query executor
///
/// # Returns
/// A `ConnectionContext` populated with the current connection state.
pub fn build_connection_context(executor: &QueryExecutor) -> ConnectionContext {
    let backend_str = match executor.backend() {
        Backend::Demo => "demo".to_string(),
        Backend::Prometheus(url) => format!("prometheus:{url}"),
        Backend::Otlp => "otlp".to_string(),
    };

    let (is_online, version) = match executor.connection_health() {
        ConnectionHealth::Online { version } => (true, Some(version.clone())),
        _ => (false, None),
    };

    let endpoint = match executor.backend() {
        Backend::Prometheus(url) => Some(url.clone()),
        _ => None,
    };

    ConnectionContext {
        backend: backend_str,
        endpoint,
        is_online,
        version,
    }
}

/// Build workspace context from current workspace state.
///
/// Creates a `WorkspaceContext` containing the current time range, pane count,
/// and list of active queries. This gives AI agents awareness of what the user
/// is currently viewing.
///
/// # Arguments
/// * `time_range_label` - The display label for the current time range (e.g., "15 minutes")
/// * `pane_count` - Number of panes currently open in the viewport
/// * `queries` - List of PromQL queries from open query panes
///
/// # Returns
/// A `WorkspaceContext` populated with the workspace state.
pub fn build_workspace_context(
    time_range_label: String,
    pane_count: usize,
    queries: Vec<String>,
    filter: Option<String>,
) -> WorkspaceContext {
    WorkspaceContext {
        time_range: time_range_label,
        pane_count,
        queries,
        filter,
    }
}

/// Format a pane's data into a context block for the agent system prompt.
///
/// Produces a markdown section with the pane name, query, visualization type,
/// and a data summary (latest values, min/max for time series; current value for
/// stat/gauge; bar values for bar charts).
pub fn format_pane_context(
    name: &str,
    query: &str,
    info: &crate::components::pane::PaneInfo,
) -> String {
    use crate::components::pane::PaneVisualization;

    let mut out = format!("### Pane '{name}'\n");
    out.push_str(&format!("- Query: `{query}`\n"));
    out.push_str(&format!("- Visualization: {:?}\n", info.viz_type));

    match &info.visualization {
        PaneVisualization::TimeSeries { series } => {
            if series.is_empty() {
                out.push_str("- No data\n");
            } else {
                out.push_str(&format!("- Series ({}):\n", series.len()));
                // Limit to 10 series to avoid bloating the prompt
                for s in series.iter().take(10) {
                    if s.points.is_empty() {
                        out.push_str(&format!("  - {}: no data\n", s.name));
                        continue;
                    }
                    let latest = s.points.last().map(|p| p.value).unwrap_or(0.0);
                    let min = s
                        .points
                        .iter()
                        .map(|p| p.value)
                        .fold(f64::INFINITY, f64::min);
                    let max = s
                        .points
                        .iter()
                        .map(|p| p.value)
                        .fold(f64::NEG_INFINITY, f64::max);
                    out.push_str(&format!(
                        "  - {}: latest={latest:.4}, min={min:.4}, max={max:.4} ({} points)\n",
                        s.name,
                        s.points.len()
                    ));
                }
                if series.len() > 10 {
                    out.push_str(&format!("  - ... and {} more series\n", series.len() - 10));
                }
            }
        }
        PaneVisualization::Stat {
            value,
            unit,
            sparkline,
        } => {
            out.push_str(&format!("- Value: {value:.4}{unit}\n"));
            if !sparkline.is_empty() {
                let min = sparkline.iter().copied().fold(f64::INFINITY, f64::min);
                let max = sparkline.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                out.push_str(&format!(
                    "- Trend: {len} points, range [{min:.4}, {max:.4}]\n",
                    len = sparkline.len()
                ));
            }
        }
        PaneVisualization::Gauge {
            value,
            min,
            max,
            unit,
        } => {
            out.push_str(&format!("- Value: {value:.4}{unit} (range: {min}–{max})\n"));
        }
        PaneVisualization::BarChart { bars } => {
            out.push_str(&format!("- Bars ({}):\n", bars.len()));
            for (label, val) in bars.iter().take(20) {
                out.push_str(&format!("  - {label}: {val:.4}\n"));
            }
            if bars.len() > 20 {
                out.push_str(&format!("  - ... and {} more\n", bars.len() - 20));
            }
        }
        PaneVisualization::PieChart { segments } => {
            out.push_str(&format!("- Segments ({}):\n", segments.len()));
            let total: f64 = segments.iter().map(|(_, v)| v).sum();
            for (label, val) in segments.iter().take(20) {
                let pct = if total > 0.0 {
                    val / total * 100.0
                } else {
                    0.0
                };
                out.push_str(&format!("  - {label}: {val:.4} ({pct:.1}%)\n"));
            }
            if segments.len() > 20 {
                out.push_str(&format!("  - ... and {} more\n", segments.len() - 20));
            }
        }
        PaneVisualization::Sparkline { data } => {
            if !data.is_empty() {
                let min = data.iter().copied().fold(f64::INFINITY, f64::min);
                let max = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let latest = data.last().copied().unwrap_or(0.0);
                out.push_str(&format!(
                    "- Sparkline: {len} points, latest={latest:.4}, range [{min:.4}, {max:.4}]\n",
                    len = data.len()
                ));
            }
        }
        PaneVisualization::Heatmap => {
            out.push_str("- Heatmap (data summary not available)\n");
        }
    }

    out
}

/// Build codebase context from the codebase manager (native only).
///
/// Creates a `CodebaseContext` containing repository information, indexed metric
/// and file counts, and optionally recent commits. This helps AI agents understand
/// the codebase structure and recent changes.
///
/// # Arguments
/// * `repo_path` - Path to the repository root
/// * `metric_count` - Number of metrics discovered in the codebase
/// * `file_count` - Number of files scanned/indexed
/// * `recent_commits` - Optional list of recent commits (typically last 5)
///
/// # Returns
/// A `CodebaseContext` populated with codebase information.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_codebase_context(
    repo_path: String,
    metric_count: usize,
    file_count: usize,
    recent_commits: Vec<CommitSummary>,
) -> CodebaseContext {
    CodebaseContext {
        repo_url: repo_path,
        metric_count,
        file_count,
        recent_commits,
    }
}

/// Load project-specific context from ENYA.md or .enya/context.md (native only).
///
/// This function looks for a project context file in the repository root,
/// allowing users to provide custom instructions, conventions, and context
/// that will be injected into every AI agent prompt.
///
/// # Search Order
/// 1. `ENYA.md` in the repository root
/// 2. `.enya/context.md` in the repository root
///
/// # Arguments
/// * `repo_path` - Path to the repository root
///
/// # Returns
/// The file contents if found, `None` otherwise.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_project_context(repo_path: &std::path::Path) -> Option<String> {
    use std::fs;

    // Try ENYA.md first
    let enya_md = repo_path.join("ENYA.md");
    if enya_md.exists() {
        if let Ok(content) = fs::read_to_string(&enya_md) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                log::debug!("Loaded project context from {}", enya_md.display());
                return Some(trimmed.to_string());
            }
        }
    }

    // Try .enya/context.md as fallback
    let enya_context = repo_path.join(".enya").join("context.md");
    if enya_context.exists() {
        if let Ok(content) = fs::read_to_string(&enya_context) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                log::debug!("Loaded project context from {}", enya_context.display());
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_pane_command() {
        let text = r#"
Let me create a pane for you.

```enya-command
{"action": "create_pane", "query": "rate(http_requests_total[5m])", "title": "Request Rate"}
```

This will show the request rate.
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::CreatePane { query, title, .. } => {
                assert_eq!(query, "rate(http_requests_total[5m])");
                assert_eq!(title.as_deref(), Some("Request Rate"));
            }
            _ => panic!("Expected CreatePane command"),
        }
    }

    #[test]
    fn test_parse_set_time_range_command() {
        let text = r#"
```enya-command
{"action": "set_time_range", "preset": "1h"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::SetTimeRange { preset } => {
                assert_eq!(preset, "1h");
            }
            _ => panic!("Expected SetTimeRange command"),
        }
    }

    #[test]
    fn test_parse_multiple_commands() {
        let text = r#"
I'll set up your dashboard:

```enya-command
{"action": "set_time_range", "preset": "6h"}
```

And add some panes:

```enya-command
{"action": "create_pane", "query": "up", "title": "Service Health"}
```

```enya-command
{"action": "create_pane", "query": "rate(http_requests_total[5m])"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn test_context_to_prompt() {
        let context = EditorContext::new()
            .with_connection(ConnectionContext {
                backend: "prometheus".to_string(),
                endpoint: Some("http://localhost:9090".to_string()),
                is_online: true,
                version: Some("2.45.0".to_string()),
            })
            .with_metrics(vec![
                "http_requests_total".to_string(),
                "node_cpu_seconds_total".to_string(),
            ]);

        let prompt = context.to_prompt_block();
        assert!(prompt.contains("prometheus"));
        assert!(prompt.contains("http_requests_total"));
        assert!(prompt.contains("create_pane"));
    }

    #[test]
    fn test_strip_command_blocks() {
        let text = r#"
I'll create a pane for you.

```enya-command
{"action": "create_pane", "query": "rate(http_requests_total[5m])", "title": "Request Rate"}
```

This will show the request rate.
"#;

        let stripped = strip_command_blocks(text);
        assert!(!stripped.contains("enya-command"));
        assert!(!stripped.contains("create_pane"));
        assert!(stripped.contains("I'll create a pane for you."));
        assert!(stripped.contains("This will show the request rate."));
    }

    #[test]
    fn test_strip_command_blocks_multiple() {
        let text = r#"Setting up your dashboard:

```enya-command
{"action": "set_time_range", "preset": "6h"}
```

And adding panes:

```enya-command
{"action": "create_pane", "query": "up"}
```

Done!"#;

        let stripped = strip_command_blocks(text);
        assert!(!stripped.contains("enya-command"));
        assert!(stripped.contains("Setting up your dashboard:"));
        assert!(stripped.contains("And adding panes:"));
        assert!(stripped.contains("Done!"));
    }

    #[test]
    fn test_strip_command_blocks_no_commands() {
        let text = "Just a normal response with no commands.";
        let stripped = strip_command_blocks(text);
        assert_eq!(stripped, text);
    }

    #[test]
    fn test_parse_search_codebase_command() {
        let text = r#"
Let me search for that.

```enya-command
{"action": "search_codebase", "query": "http_requests", "filter": "metrics", "limit": 5}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::SearchCodebase {
                query,
                filter,
                limit,
            } => {
                assert_eq!(query, "http_requests");
                assert_eq!(filter.as_deref(), Some("metrics"));
                assert_eq!(*limit, Some(5));
            }
            _ => panic!("Expected SearchCodebase command"),
        }
    }

    #[test]
    fn test_parse_search_codebase_command_minimal() {
        let text = r#"
```enya-command
{"action": "search_codebase", "query": "error"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::SearchCodebase {
                query,
                filter,
                limit,
            } => {
                assert_eq!(query, "error");
                assert!(filter.is_none());
                assert!(limit.is_none());
            }
            _ => panic!("Expected SearchCodebase command"),
        }
    }

    #[test]
    fn test_parse_rename_pane_command() {
        let text = r#"
```enya-command
{"action": "rename_pane", "pane": "Query 1", "new_name": "Error Rate"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::RenamePane { pane, new_name } => {
                assert_eq!(pane, "Query 1");
                assert_eq!(new_name, "Error Rate");
            }
            _ => panic!("Expected RenamePane command"),
        }
    }

    #[test]
    fn test_parse_duplicate_pane_command() {
        let text = r#"
```enya-command
{"action": "duplicate_pane", "pane": "focused", "new_name": "Copy"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::DuplicatePane { pane, new_name } => {
                assert_eq!(pane, "focused");
                assert_eq!(new_name.as_deref(), Some("Copy"));
            }
            _ => panic!("Expected DuplicatePane command"),
        }
    }

    #[test]
    fn test_parse_duplicate_pane_command_minimal() {
        let text = r#"
```enya-command
{"action": "duplicate_pane", "pane": "CPU Usage"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::DuplicatePane { pane, new_name } => {
                assert_eq!(pane, "CPU Usage");
                assert!(new_name.is_none());
            }
            _ => panic!("Expected DuplicatePane command"),
        }
    }

    #[test]
    fn test_parse_focus_pane_command() {
        let text = r#"
```enya-command
{"action": "focus_pane", "pane": "Error Rate"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::FocusPane { pane } => {
                assert_eq!(pane, "Error Rate");
            }
            _ => panic!("Expected FocusPane command"),
        }
    }

    #[test]
    fn test_parse_toggle_zen_mode_command() {
        let text = r#"
```enya-command
{"action": "toggle_zen_mode"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], AgentCommand::ToggleZenMode));
    }

    #[test]
    fn test_parse_exit_fullscreen_command() {
        let text = r#"
```enya-command
{"action": "exit_fullscreen"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], AgentCommand::ExitFullscreen));
    }

    #[test]
    fn test_parse_set_visualization_command() {
        let text = r#"
```enya-command
{"action": "set_visualization", "viz_type": "gauge", "pane": "CPU Usage"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::SetVisualization { pane, viz_type } => {
                assert_eq!(pane.as_deref(), Some("CPU Usage"));
                assert_eq!(viz_type, "gauge");
            }
            _ => panic!("Expected SetVisualization command"),
        }
    }

    #[test]
    fn test_parse_set_visualization_command_minimal() {
        let text = r#"
```enya-command
{"action": "set_visualization", "viz_type": "stat"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::SetVisualization { pane, viz_type } => {
                assert!(pane.is_none());
                assert_eq!(viz_type, "stat");
            }
            _ => panic!("Expected SetVisualization command"),
        }
    }

    #[test]
    fn test_parse_set_absolute_time_range_command() {
        let text = r#"
```enya-command
{"action": "set_absolute_time_range", "start": 1705593600.0, "end": 1705597200.0}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::SetAbsoluteTimeRange { start, end } => {
                assert!((start - 1705593600.0).abs() < 0.001);
                assert!((end - 1705597200.0).abs() < 0.001);
            }
            _ => panic!("Expected SetAbsoluteTimeRange command"),
        }
    }

    #[test]
    fn test_parse_refresh_pane_command() {
        let text = r#"
```enya-command
{"action": "refresh_pane", "pane": "CPU Usage"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::RefreshPane { pane } => {
                assert_eq!(pane.as_deref(), Some("CPU Usage"));
            }
            _ => panic!("Expected RefreshPane command"),
        }
    }

    #[test]
    fn test_parse_refresh_pane_command_all() {
        let text = r#"
```enya-command
{"action": "refresh_pane"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::RefreshPane { pane } => {
                assert!(pane.is_none());
            }
            _ => panic!("Expected RefreshPane command"),
        }
    }

    #[test]
    fn test_parse_close_pane_command() {
        let text = r#"
```enya-command
{"action": "close_pane", "pane": "focused"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::ClosePane { pane } => {
                assert_eq!(pane, "focused");
            }
            _ => panic!("Expected ClosePane command"),
        }
    }

    #[test]
    fn test_parse_create_section_command() {
        let text = r#"
```enya-command
{"action": "create_section", "name": "Infrastructure", "collapsed": true}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::CreateSection { name, collapsed } => {
                assert_eq!(name, "Infrastructure");
                assert_eq!(*collapsed, Some(true));
            }
            _ => panic!("Expected CreateSection command"),
        }
    }

    #[test]
    fn test_parse_create_floating_pane_command() {
        let text = r#"
```enya-command
{"action": "create_floating_pane", "query": "up", "title": "Health", "position": [100.0, 200.0]}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::CreateFloatingPane {
                query,
                title,
                position,
            } => {
                assert_eq!(query, "up");
                assert_eq!(title.as_deref(), Some("Health"));
                assert_eq!(*position, Some([100.0, 200.0]));
            }
            _ => panic!("Expected CreateFloatingPane command"),
        }
    }

    #[test]
    fn test_parse_maximize_pane_command() {
        let text = r#"
```enya-command
{"action": "maximize_pane", "pane": "Error Rate"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::MaximizePane { pane } => {
                assert_eq!(pane, "Error Rate");
            }
            _ => panic!("Expected MaximizePane command"),
        }
    }

    #[test]
    fn test_parse_add_logs_pane_command() {
        let text = r#"
```enya-command
{"action": "add_logs_pane", "query": "{app=\"nginx\"}", "loki_url": "http://localhost:3100"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::AddLogsPane {
                query,
                loki_url,
                title,
            } => {
                assert_eq!(query.as_deref(), Some("{app=\"nginx\"}"));
                assert_eq!(loki_url.as_deref(), Some("http://localhost:3100"));
                assert!(title.is_none());
            }
            _ => panic!("Expected AddLogsPane command"),
        }
    }

    #[test]
    fn test_parse_add_tracing_pane_command() {
        let text = r#"
```enya-command
{"action": "add_tracing_pane", "trace_id": "abc123"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::AddTracingPane { trace_id, title } => {
                assert_eq!(trace_id.as_deref(), Some("abc123"));
                assert!(title.is_none());
            }
            _ => panic!("Expected AddTracingPane command"),
        }
    }

    #[test]
    fn test_parse_add_terminal_pane_command() {
        let text = r#"
```enya-command
{"action": "add_terminal_pane"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::AddTerminalPane { title } => {
                assert!(title.is_none());
            }
            _ => panic!("Expected AddTerminalPane command"),
        }
    }

    #[test]
    fn test_parse_load_workspace_command() {
        let text = r#"
```enya-command
{"action": "load_workspace", "workspace": "incident-42"}
```
"#;

        let commands = parse_commands(text);
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AgentCommand::LoadWorkspace { workspace } => {
                assert_eq!(workspace, "incident-42");
            }
            _ => panic!("Expected LoadWorkspace command"),
        }
    }
}
