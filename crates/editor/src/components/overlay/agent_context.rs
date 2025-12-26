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
    /// Current dashboard state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<DashboardContext>,
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
pub struct DashboardContext {
    /// Current time range description
    pub time_range: String,
    /// Number of panes
    pub pane_count: usize,
    /// List of open queries
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub queries: Vec<String>,
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

    /// Set the dashboard context.
    pub fn with_dashboard(mut self, dashboard: DashboardContext) -> Self {
        self.dashboard = Some(dashboard);
        self
    }

    /// Generate the context block to inject into the prompt.
    ///
    /// Returns a formatted string that will be prepended to user prompts.
    pub fn to_prompt_block(&self) -> String {
        let mut parts = Vec::new();

        parts.push("# Enya Editor Context\n".to_string());
        parts.push("You are integrated with Enya, a metrics visualization editor. Here is the current state:\n".to_string());

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
        if let Some(ref dashboard) = self.dashboard {
            parts.push("\n## Current Dashboard\n".to_string());
            parts.push(format!("- Time range: {}\n", dashboard.time_range));
            parts.push(format!("- Panes: {}\n", dashboard.pane_count));
            if !dashboard.queries.is_empty() {
                parts.push("- Active queries:\n".to_string());
                for query in &dashboard.queries {
                    parts.push(format!("  - {query}\n"));
                }
            }
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
        parts.push("  - Optional: `title` (pane title)\n".to_string());
        parts.push("- `set_time_range`: Change the dashboard time range\n".to_string());
        parts.push(
            "  - Required: `preset` (e.g., \"15m\", \"1h\", \"6h\", \"24h\", \"7d\")\n".to_string(),
        );
        parts.push(
            "- `search_metrics`: Open the metrics finder with a search pattern\n".to_string(),
        );
        parts.push("  - Required: `pattern` (search string)\n".to_string());
        parts.push(
            "- `show_metric_source`: Show source code where a metric is defined\n".to_string(),
        );
        parts.push("  - Required: `metric` (metric name)\n".to_string());
        parts.push("- `show_alert_source`: Show source code for an alert rule\n".to_string());
        parts.push("  - Required: `alert` (alert name)\n".to_string());

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
            AgentCommand::CreatePane { query, title } => {
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
}
