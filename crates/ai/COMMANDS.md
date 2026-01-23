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

Creates a new visualization pane with a PromQL query.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | PromQL expression to visualize |
| `title` | string | No | Display title for the pane |

**Example:**
```json
{"action": "create_pane", "query": "rate(http_requests_total[5m])", "title": "Request Rate"}
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

### `show_inline_source`

Displays source code preview inline within the agent's response. **Preferred** for showing metric definitions.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `metric` | string | Yes | Metric name to look up source for |
| `context_lines` | number | No | Lines of context to show (default: 5) |

**Example:**
```json
{"action": "show_inline_source", "metric": "http_requests_total", "context_lines": 10}
```

---

### `show_metric_source`

Opens a modal overlay showing the source code definition of a metric. Use when the user explicitly asks to "open", "go to", or "navigate to" the source.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `metric` | string | Yes | Metric name to look up |

**Example:**
```json
{"action": "show_metric_source", "metric": "http_requests_total"}
```

---

### `show_alert_source`

Opens a modal overlay showing the alert rule definition. Use when the user explicitly asks to "open", "go to", or "navigate to" the alert.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `alert` | string | Yes | Alert name to look up |

**Example:**
```json
{"action": "show_alert_source", "alert": "HighErrorRate"}
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

### `create_floating_pane`

Creates a floating pane that hovers above the main layout. Perfect for investigation workflows where you need to compare data without disrupting the dashboard layout.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | PromQL expression to visualize |
| `title` | string | No | Display title for the pane |
| `position` | [number, number] | No | Position as [x, y] pixels from top-left |

**Example:**
```json
{"action": "create_floating_pane", "query": "rate(http_errors_total[5m])", "title": "Error Investigation"}
```

**Example (positioned):**
```json
{"action": "create_floating_pane", "query": "up", "title": "Service Health", "position": [200, 150]}
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

### `rename_pane`

Renames a pane. Useful for giving panes meaningful names during investigation.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pane` | string | Yes | Current pane title/name, or `"focused"` for the currently focused pane |
| `new_name` | string | Yes | The new name for the pane |

**Example:**
```json
{"action": "rename_pane", "pane": "Query 1", "new_name": "Error Rate Analysis"}
```

---

### `duplicate_pane`

Duplicates a pane with the same query. Useful for creating comparison views or making variations.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pane` | string | Yes | Pane title/name to duplicate, or `"focused"` for the currently focused pane |
| `new_name` | string | No | Name for the duplicated pane (defaults to "original name (copy)") |

**Example:**
```json
{"action": "duplicate_pane", "pane": "Request Rate", "new_name": "Request Rate (yesterday)"}
```

---

### `focus_pane`

Focuses a specific pane. Useful for directing user attention to a particular metric.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pane` | string | Yes | Pane title/name to focus |

**Example:**
```json
{"action": "focus_pane", "pane": "Error Rate"}
```

---

### `toggle_zen_mode`

Toggles zen mode (minimal UI). Hides toolbars and other UI elements for distraction-free viewing.

**Example:**
```json
{"action": "toggle_zen_mode"}
```

---

### `exit_fullscreen`

Exits fullscreen/maximized mode. Returns to normal multi-pane view.

**Example:**
```json
{"action": "exit_fullscreen"}
```

---

### `sync`

Syncs the repository by fetching latest git commits and re-indexing the codebase (including Tantivy full-text search). Use this when the repository has been updated externally.

**Example:**
```json
{"action": "sync"}
```

---

## Command Preferences

When responding to users:

1. **Inline commands** (`show_inline_chart`, `show_inline_source`) are preferred for keeping content in the conversation flow
2. **Modal commands** (`show_metric_source`, `show_alert_source`) should only be used when the user explicitly asks to "open", "go to", or "navigate to" something
3. **Search**: Use `search_codebase` instead of `git log --grep` for faster full-text search with relevance ranking
4. **Pane commands** (`add_logs_pane`, `add_tracing_pane`, `add_terminal_pane`) are useful for incident investigation workflows - correlate metrics with logs and traces
5. **Visualization**: Use `set_visualization` to suggest better chart types based on the data (e.g., gauge for percentages, stat for single values)
6. **Time ranges**: Use `set_time_range` for relative ranges and `set_absolute_time_range` for specific incidents
7. **Pane lifecycle**: Use `refresh_pane` after time range changes, and `close_pane` to clean up after investigations
8. **Organization**: Use `create_section` for Grafana-style collapsible sections to organize related metrics
9. **Investigation**: Use `create_floating_pane` for temporary investigation panes that don't disrupt the layout
10. **Focus**: Use `maximize_pane` to fullscreen important metrics during incident response

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
