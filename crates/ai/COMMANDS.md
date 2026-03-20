# Enya Commands

This document describes all commands available to AI agents integrated with Enya. Agents can execute these commands by outputting `enya-command` fenced code blocks in their responses.

## Command Format

Commands are JSON objects wrapped in fenced code blocks with the `enya-command` language tag:

```
```enya-command
{"action": "command_name", "param1": "value1", "param2": "value2"}
```
```

The editor automatically parses these blocks from agent responses and executes the corresponding actions.

## Available Commands

### `create_pane`

Creates a new visualization pane with a PromQL query. Optionally creates a floating (detached) pane for investigation.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | PromQL expression to visualize |
| `title` | string | No | Display title for the pane |
| `floating` | boolean | No | If true, create a floating pane (default: false) |
| `position` | [number, number] | No | Position for floating panes as [x, y] pixels from top-left |

**Example:**
```json
{"action": "create_pane", "query": "rate(http_requests_total[5m])", "title": "Request Rate"}
```

**Example (floating pane for investigation):**
```json
{"action": "create_pane", "query": "rate(http_errors_total[5m])", "title": "Error Investigation", "floating": true}
```

---

### `set_time_range`

Changes the global dashboard time range for all panes.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `preset` | string | Yes | Time range preset |

**Valid presets:** `"15m"`, `"1h"`, `"6h"`, `"24h"`, `"7d"`

**Example:**
```json
{"action": "set_time_range", "preset": "1h"}
```

---

### `search_metrics`

Opens the unified metrics finder with a search pattern pre-filled.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | Yes | Search pattern to filter metrics |

**Example:**
```json
{"action": "search_metrics", "pattern": "http_request"}
```

---

### `show_inline_chart`

Displays a time series chart inline within the agent's response. **Preferred** for conversational flow.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | PromQL expression to execute |
| `title` | string | No | Chart title |
| `time_range` | string | No | Time range (e.g., "1h", "6h") - defaults to dashboard range |
| `height` | number | No | Chart height in pixels |

**Example:**
```json
{"action": "show_inline_chart", "query": "rate(http_requests_total[5m])", "title": "Request Rate", "time_range": "1h"}
```

---

### `show_inline_table`

Displays SQL query results as an inline table within the agent's response. Matches against recent SQL pane query history or shows the latest result.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | No | SQL query to match in history (uses latest result if omitted) |
| `title` | string | No | Title override (defaults to the SQL query text) |

**Example (latest result):**
```json
{"action": "show_inline_table"}
```

**Example (specific query):**
```json
{"action": "show_inline_table", "query": "SELECT * FROM users LIMIT 10", "title": "User Table"}
```

---

### `show_source`

Shows source code for a metric or alert definition. The editor decides whether to show it inline or as a modal based on context. **Preferred** over the legacy `show_metric_source`, `show_inline_source`, and `show_alert_source` commands.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Metric or alert name to look up |
| `source_type` | string | No | `"metric"` (default) or `"alert"` |
| `context_lines` | number | No | Lines of context to show (default: 5) |

**Example (metric):**
```json
{"action": "show_source", "name": "http_requests_total"}
```

**Example (alert):**
```json
{"action": "show_source", "name": "HighErrorRate", "source_type": "alert"}
```

**Example (with more context):**
```json
{"action": "show_source", "name": "http_requests_total", "context_lines": 10}
```

---

### `search_codebase`

Performs full-text search over the indexed codebase using Tantivy. Returns ranked results with file paths, line numbers, and relevance scores. **Preferred** over `git log --grep` for searching.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search terms (full-text search) |
| `filter` | string | No | Filter by type: `"all"`, `"metrics"`, `"alerts"`, `"commits"` |
| `limit` | number | No | Maximum results to return (default: 10) |

**Example:**
```json
{"action": "search_codebase", "query": "error rate", "filter": "alerts", "limit": 5}
```

---

### `add_logs_pane`

Creates a logs pane for viewing logs. Useful for incident investigation and correlating metrics with log events. Supports demo mode or connecting to a Loki server.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | No | LogQL query to pre-fill |
| `loki_url` | string | No | Loki server URL (e.g., "http://localhost:3100"). Uses demo backend if omitted |
| `title` | string | No | Display title for the pane |

**Example (demo mode):**
```json
{"action": "add_logs_pane", "query": "{app=\"nginx\"} |= \"error\""}
```

**Example (Loki backend):**
```json
{"action": "add_logs_pane", "loki_url": "http://localhost:3100", "query": "{job=\"varlogs\"}"}
```

---

### `add_tracing_pane`

Creates a tracing pane for viewing distributed traces. Useful for investigating request latency and understanding service dependencies.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `trace_id` | string | No | Trace ID to pre-load |
| `title` | string | No | Display title for the pane |

**Example:**
```json
{"action": "add_tracing_pane", "trace_id": "abc123def456"}
```

**Example (empty pane for manual entry):**
```json
{"action": "add_tracing_pane"}
```

---

### `add_terminal_pane`

Creates a terminal pane for running shell commands. **Native app only** - not available in browser/WASM.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `title` | string | No | Display title for the pane |

**Example:**
```json
{"action": "add_terminal_pane"}
```

**Note:** This command will fail silently in the browser version of Enya.

---

### `set_visualization`

Changes the visualization type for a pane. Useful for suggesting better visualizations based on the data.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `viz_type` | string | Yes | Visualization type (see below) |
| `pane` | string | No | Pane title/name, or omit for currently focused pane |

**Valid visualization types:**
- `"time_series"` (or `"line"`, `"chart"`) - Line chart for time-based data
- `"stat"` (or `"big_number"`, `"single"`) - Big number display with optional sparkline
- `"gauge"` (or `"dial"`, `"meter"`) - Circular gauge for percentages/utilization
- `"bar_chart"` (or `"bar"`, `"bars"`) - Horizontal bar chart for comparisons
- `"pie_chart"` (or `"pie"`, `"donut"`) - Donut/pie chart for proportional data
- `"sparkline"` (or `"spark"`, `"mini"`) - Compact inline trend line
- `"heatmap"` (or `"heat"`, `"matrix"`) - Heat map for distributions

**Example:**
```json
{"action": "set_visualization", "viz_type": "gauge", "pane": "CPU Usage"}
```

**Example (focused pane):**
```json
{"action": "set_visualization", "viz_type": "stat"}
```

---

### `set_absolute_time_range`

Sets a specific time range using Unix timestamps. Essential for investigating specific incidents ("look at 2pm yesterday").

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `start` | number | Yes | Start timestamp in Unix seconds |
| `end` | number | Yes | End timestamp in Unix seconds |

**Example:**
```json
{"action": "set_absolute_time_range", "start": 1705593600, "end": 1705597200}
```

**Note:** Use Unix timestamps. For example, `1705593600` = 2024-01-18 12:00:00 UTC.

---

### `refresh_pane`

Refreshes panes to reload data with the current time range. Useful after changing time ranges or when data may have changed.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pane` | string | No | Pane title/name to refresh, or omit to refresh all panes |

**Example (refresh specific pane):**
```json
{"action": "refresh_pane", "pane": "Request Rate"}
```

**Example (refresh all panes):**
```json
{"action": "refresh_pane"}
```

---

### `close_pane`

Closes a pane. Useful for cleaning up the dashboard after investigation.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pane` | string | Yes | Pane title/name, or `"focused"` for the currently focused pane |

**Example:**
```json
{"action": "close_pane", "pane": "CPU Usage"}
```

**Example (close focused pane):**
```json
{"action": "close_pane", "pane": "focused"}
```

---

### `create_section`

Creates a collapsible section for organizing panes (Grafana-style). Useful for grouping related metrics.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Section name displayed in the header |
| `collapsed` | boolean | No | Whether section starts collapsed (default: false) |

**Example:**
```json
{"action": "create_section", "name": "API Performance"}
```

**Example (collapsed):**
```json
{"action": "create_section", "name": "Infrastructure", "collapsed": true}
```

---

### `maximize_pane`

Maximizes a pane to fullscreen view. Press the same keybinding again or use this command to exit.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pane` | string | Yes | Pane title/name, or `"focused"` for the currently focused pane |

**Example:**
```json
{"action": "maximize_pane", "pane": "Request Rate"}
```

**Example (focused pane):**
```json
{"action": "maximize_pane", "pane": "focused"}
```

---

### `load_workspace`

Loads a saved workspace by name. This is the key command for agent-to-human handoff: after building a workspace via the CLI (`enya init`, `enya add-section`, `enya add-pane`, etc.), the agent can load it in the GUI for the human to view.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `workspace` | string | Yes | Workspace name (as shown in `enya list`) |

**Example:**
```json
{"action": "load_workspace", "workspace": "incident-42"}
```

**Typical workflow:**
1. Agent creates workspace via CLI: `enya init incident-42 -e http://prometheus:9090`
2. Agent adds sections and panes via CLI
3. Agent loads it in the GUI: `{"action": "load_workspace", "workspace": "incident-42"}`

---

### `open_pr_review`

Opens the PR review pane for the current repository. Automatically detects the GitHub owner/repo from the configured git remote.

No parameters required.

**Example:**
```json
{"action": "open_pr_review"}
```

---

### `review_pr`

Navigates to a specific PR in the review pane. If no PR review pane is open, one is created automatically.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `number` | number | Yes | PR number to review |
| `focus` | string | No | Focus area (e.g., "security", "performance") |

**Example:**
```json
{"action": "review_pr", "number": 42, "focus": "security"}
```

---

### `add_pr_comment`

Adds a draft review comment on the currently open PR. Comments accumulate as drafts until submitted.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | File path relative to repo root |
| `line` | number | Yes | Line number in the new version |
| `body` | string | Yes | Comment text |

**Example:**
```json
{"action": "add_pr_comment", "path": "src/main.rs", "line": 42, "body": "Consider using a constant here instead of a magic number."}
```

---

### `submit_pr_review`

Submits the current PR review with all accumulated draft comments.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `event` | string | Yes | Review event: `"approve"`, `"request_changes"`, or `"comment"` |
| `body` | string | No | Review summary body |

**Example:**
```json
{"action": "submit_pr_review", "event": "approve", "body": "LGTM! Clean implementation."}
```

**Typical PR review workflow:**
1. Open the PR: `{"action": "review_pr", "number": 42}`
2. Add comments: `{"action": "add_pr_comment", "path": "src/lib.rs", "line": 15, "body": "..."}`
3. Submit: `{"action": "submit_pr_review", "event": "approve"}`

---

## Command Preferences

When responding to users:

1. **Inline commands** (`show_inline_chart`, `show_source`) are preferred for keeping content in the conversation flow
2. **Search**: Use `search_codebase` instead of `git log --grep` for faster full-text search with relevance ranking
3. **Pane commands** (`add_logs_pane`, `add_tracing_pane`, `add_terminal_pane`) are useful for incident investigation workflows - correlate metrics with logs and traces
4. **Visualization**: Use `set_visualization` to suggest better chart types based on the data (e.g., gauge for percentages, stat for single values)
5. **Time ranges**: Use `set_time_range` for relative ranges and `set_absolute_time_range` for specific incidents
6. **Pane lifecycle**: Use `refresh_pane` after time range changes, and `close_pane` to clean up after investigations
7. **Organization**: Use `create_section` for Grafana-style collapsible sections to organize related metrics
8. **Investigation**: Use `create_pane` with `"floating": true` for temporary investigation panes that don't disrupt the layout
9. **Focus**: Use `maximize_pane` to fullscreen important metrics during incident response
10. **Handoff**: Use `load_workspace` to load a workspace built via CLI into the GUI for the human
11. **PR Reviews**: Use `review_pr` → `add_pr_comment` → `submit_pr_review` for AI-assisted code review workflows

## Implementation

Commands are defined in:
- **Enum:** `crates/editor/src/components/overlay/agent_context.rs` → `AgentCommand`
- **Parser:** `crates/editor/src/components/overlay/agent_context.rs` → `parse_commands()`
- **Executor:** `crates/editor/src/workspace/panes.rs` → `handle_agent_commands()`

## Adding New Commands

1. Add variant to `AgentCommand` enum in `agent_context.rs`
2. Update `to_prompt_block()` to document the command for agents
3. Add handler in `Workspace::handle_agent_commands()`
4. **Update this file** (`crates/ai/COMMANDS.md`)
5. Update `crates/editor/CHANGELOG.md`

---

## Advanced Commands

These commands are supported but not advertised in the agent prompt. They still parse and execute if emitted. Legacy aliases for consolidated commands are also listed here.

### `create_floating_pane` (legacy)

Use `create_pane` with `"floating": true` instead.

### `show_inline_source` (legacy)

Use `show_source` instead. The editor decides inline vs modal display.

### `show_metric_source` (legacy)

Use `show_source` instead.

### `show_alert_source` (legacy)

Use `show_source` with `"source_type": "alert"` instead.

### `rename_pane`

Renames a pane. Parameters: `pane` (required), `new_name` (required).

### `duplicate_pane`

Duplicates a pane with the same query. Parameters: `pane` (required), `new_name` (optional).

### `focus_pane`

Focuses a specific pane. Parameters: `pane` (required).

### `toggle_zen_mode`

Toggles zen mode (minimal UI).

### `exit_fullscreen`

Exits fullscreen/maximized mode.

### `sync`

Syncs the repository by fetching latest git commits and re-indexing the codebase.
