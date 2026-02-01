# Enya CLI — Agent Reference

Enya is an observability editor. This document describes the headless CLI interface for AI agents and automation.

All commands support `--json` for machine-consumable output. Errors return exit code 1 with `{"error": "..."}` in JSON mode.

## Workspaces

Workspaces are TOML files stored in `~/.enya/workspaces/`. Each defines a Prometheus endpoint, time range, and a set of query panes organized into sections.

```sh
# List all workspaces
enya list
enya --json list

# Show workspace details (name or path to .toml file)
enya show <name>
enya --json show <name>

# Create a workspace
enya init <name>                              # empty workspace
enya init <name> -e http://localhost:9090     # with Prometheus endpoint
enya init <name> -t demo                      # from template (default, demo, complex, atlas)
enya init <name> -o ./my-workspace.toml       # write to specific path

# Delete a workspace
enya rm <name>
```

### JSON shapes

`enya --json list`:
```json
{
  "dir": "/home/user/.enya/workspaces",
  "workspaces": [
    { "name": "demo", "description": "Interactive demo..." }
  ]
}
```

`enya --json show <name>` returns the full workspace config:
```json
{
  "workspace": { "name": "demo", "description": "..." },
  "time": { "preset": "1h", "refresh": "30s" },
  "sections": [
    {
      "name": "API Performance",
      "panes": [
        { "query": "sum(rate(http_requests_total[5m])) by (method)", "name": "HTTP Request Rate" }
      ]
    }
  ]
}
```

## Plugins

Plugins extend Enya with custom commands, keybindings, and pane types. Two formats exist:

- **TOML config plugins** (`.toml`) — static shell/url/notify actions
- **Lua plugins** (`.lua`) — dynamic commands with HTTP, logging, and scripting

Plugins live in `~/.config/enya/plugins/`.

```sh
# List installed plugins
enya plugins
enya --json plugins

# List all available commands across plugins
enya plugins commands
enya --json plugins commands

# Install a plugin
enya plugins install ./my-plugin.lua

# Remove a plugin
enya plugins remove <plugin-name>
```

### JSON shapes

`enya --json plugins commands`:
```json
{
  "commands": [
    {
      "name": "greet",
      "plugin": "hello-agent",
      "type": "lua",
      "description": "Print a greeting message",
      "accepts_args": true
    }
  ]
}
```

## Executing Commands

Run any plugin command headlessly:

```sh
enya exec <command> [args...]
enya --json exec <command> [args...]
```

Both TOML and Lua plugin commands work. Lua commands run with a headless host that provides real HTTP and logging but no-ops for UI operations (panes, clipboard, etc).

### JSON shapes

Config shell command:
```json
{ "command": "echo-args", "shell": "echo hello", "exit_code": 0, "success": true }
```

Lua command:
```json
{ "command": "greet", "plugin": "hello-agent", "success": true }
```

Notify/URL commands return `{ "command": "...", "message": "..." }` or `{ "command": "...", "url": "..." }`.

## Writing Lua Plugins for Headless Use

Lua plugins that avoid UI-only APIs work fully from the CLI. Available headless APIs:

| API | Behavior |
|-----|----------|
| `enya.notify(level, msg)` | Prints `[level] msg` to stdout |
| `enya.log(level, msg)` | Logs via standard log framework |
| `enya.http_get(url, headers)` | Real HTTP GET, returns `{status, body, error}` |
| `enya.http_post(url, body, headers)` | Real HTTP POST, returns `{status, body, error}` |
| `os.getenv(var)` | Read environment variables |

UI APIs (panes, clipboard, time range, custom visualizations) silently no-op in headless mode.

Example plugin:
```lua
plugin = {
    name = "my-agent-tool",
    version = "0.1.0",
    description = "Agent utility commands"
}

enya.register_command("check-api", {
    description = "Check API health",
    accepts_args = true,
}, function(args)
    local url = args ~= "" and args or "http://localhost:8080/health"
    local resp = enya.http_get(url, {})
    if resp.error then
        enya.notify("error", "Failed: " .. resp.error)
        return false
    end
    enya.notify("info", url .. " -> " .. tostring(resp.status))
    return true
end)
```

## Queries

Run PromQL queries against Prometheus or SQL queries via DataFusion.

### PromQL

```sh
enya query '<promql>' --endpoint <prometheus-url>
enya query '<promql>' --workspace <name>           # read endpoint from workspace
enya query '<promql>' --start 2h --end now --step 30s -e <url>
enya --json query '<promql>' -e <url>
```

Endpoint resolution order: `--endpoint` flag, `--workspace` config, `ENYA_PROMETHEUS_URL` env var.

Time range defaults: `--start 1h` (1 hour ago), `--end now`, `--step 60s`. Accepts relative durations (`1h`, `30m`, `2d`), Unix timestamps, and ISO 8601 (`2024-01-01T00:00:00Z`).

#### JSON shape

```json
{
  "result_type": "matrix",
  "series": [
    {
      "metric": { "method": "GET", "instance": "host1:9090" },
      "values": [
        { "timestamp": 1704067200.0, "value": "0.5" },
        { "timestamp": 1704067260.0, "value": "0.6" }
      ]
    }
  ],
  "series_count": 1
}
```

### SQL (requires `--features sql`)

```sh
enya query --sql 'SELECT * FROM events LIMIT 10' --file events.parquet
enya query --sql 'SELECT count(*) FROM logs' --file logs=/data/logs.csv
enya --json query --sql 'SELECT host, avg(latency) FROM data GROUP BY host' --file data.parquet
```

Use `--file` to register local files (Parquet, CSV, JSON). Format: `NAME=PATH` or just `PATH` (table name derived from filename).

#### JSON shape

```json
{
  "columns": [
    { "name": "host", "type": "Utf8" },
    { "name": "count", "type": "Int64" }
  ],
  "rows": [
    { "host": "server-1", "count": "42" }
  ],
  "row_count": 1
}
```

## Typical Agent Workflow

```sh
# 1. Discover what's available
enya --json list
enya --json plugins commands

# 2. Inspect a workspace
enya --json show production

# 3. Query metrics from a workspace
enya --json query 'rate(http_requests_total[5m])' -w production

# 4. Query with explicit endpoint
enya --json query 'up' -e http://prometheus:9090 --start 2h --step 30s

# 5. Run SQL against local data
enya --json query --sql 'SELECT * FROM events WHERE level = "error"' --file events.parquet

# 6. Create a workspace for investigation
enya init incident-42 -e http://prometheus:9090

# 7. Run plugin commands
enya --json exec check-api http://service:8080/health

# 8. Clean up
enya rm incident-42
```
