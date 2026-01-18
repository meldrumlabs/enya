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

## Command Preferences

When responding to users:

1. **Inline commands** (`show_inline_chart`, `show_inline_source`) are preferred for keeping content in the conversation flow
2. **Modal commands** (`show_metric_source`, `show_alert_source`) should only be used when the user explicitly asks to "open", "go to", or "navigate to" something
3. **Search**: Use `search_codebase` instead of `git log --grep` for faster full-text search with relevance ranking

## Implementation

Commands are defined in:
- **Enum:** `crates/editor/src/components/overlay/agent_context.rs` → `AgentCommand`
- **Parser:** `crates/editor/src/components/overlay/agent_context.rs` → `parse_commands()`
- **Executor:** `crates/editor/src/workspace/mod.rs` → `handle_agent_commands()`

## Adding New Commands

1. Add variant to `AgentCommand` enum in `agent_context.rs`
2. Update `to_prompt_block()` to document the command for agents
3. Add handler in `Workspace::handle_agent_commands()`
4. **Update this file** (`crates/ai/COMMANDS.md`)
5. Update `crates/editor/CHANGELOG.md`
