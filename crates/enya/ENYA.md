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

### Workspace Properties

Read and write workspace configuration using dot-notation keys:

```sh
# Read a property
enya get <name> time.preset
enya --json get <name> metrics.endpoint

# Set a property
enya set <name> time.preset 1h
enya set <name> metrics.endpoint http://prometheus:9090
enya set <name> view.zen_mode true
```

Available keys:

| Key | Description |
|-----|-------------|
| `workspace.name` | Workspace display name |
| `workspace.description` | Description text |
| `workspace.endpoint` | Inline Prometheus endpoint |
| `metrics.endpoint` | Prometheus endpoint (takes precedence over workspace.endpoint) |
| `metrics.api_key` | API key for metrics endpoint |
| `logs.endpoint` | Loki endpoint |
| `logs.api_key` | API key for logs endpoint |
| `logs.default_query` | Default LogQL query |
| `git.url` | Repository URL for go-to-definition |
| `git.branch` | Git branch |
| `git.language` | Language hint |
| `view.theme` | Theme name (e.g. "dark", "light") |
| `view.zen_mode` | Boolean — "true" or "false" |
| `time.preset` | Time range (e.g. "15m", "1h", "6h", "24h") |
| `time.refresh` | Auto-refresh interval (e.g. "30s", "1m") or empty for off |

### Building Workspaces

Add and remove sections and panes to programmatically construct dashboards:

```sh
# Add a section
enya add-section <name> "API Performance"
enya add-section <name> "Infrastructure" --layout grid --columns 2 --collapsed

# Add panes to a section
enya add-pane <name> 'rate(http_requests_total[5m])' --name "Request Rate" --section "API Performance"
enya add-pane <name> 'histogram_quantile(0.99, latency)' --name "Latency p99" --section "API Performance" --tag Critical --unit ms
enya add-pane <name> 'avg(cpu_usage)' --name "CPU" --visualization stat

# Remove a pane or section
enya remove-pane <name> "Latency p99"
enya remove-pane <name> "CPU" --section "Infrastructure"   # disambiguate if name appears in multiple sections
enya remove-section <name> "Infrastructure"
```

Options for `add-section`:
- `--layout` — horizontal (default), vertical, grid, tabs
- `--columns` — column count for grid layout
- `--collapsed` — start section collapsed

Options for `add-pane`:
- `--name` — display name (defaults to query expression)
- `--section` — target section (defaults to last section)
- `--tag` — label tag (e.g. "Critical", "Warning")
- `--unit` — unit suffix (e.g. "ms", "req/s", "%")
- `--granularity` — query step (e.g. "1m", "5m")
- `--visualization` — display type (e.g. "time_series", "stat")
- `--description` — description text

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

`enya --json get <name> <key>`:
```json
{ "workspace": "prod-api", "key": "time.preset", "value": "1h" }
```

`enya --json set <name> <key> <value>`:
```json
{ "workspace": "prod-api", "key": "time.preset", "value": "1h" }
```

`enya --json add-section <name> <section>`:
```json
{ "workspace": "prod-api", "section": "API Performance", "layout": "horizontal" }
```

`enya --json add-pane <name> <query>`:
```json
{ "workspace": "prod-api", "section": "API Performance", "pane": "Request Rate", "query": "rate(http_requests_total[5m])" }
```

`enya --json remove-pane <name> <pane>`:
```json
{ "workspace": "prod-api", "removed_pane": "Request Rate", "section": "API Performance" }
```

`enya --json remove-section <name> <section>`:
```json
{ "workspace": "prod-api", "removed_section": "Infrastructure", "panes_removed": 4 }
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

## Shell Completions

Generate completions for your shell:

```sh
enya completions bash > ~/.local/share/bash-completion/completions/enya
enya completions zsh > ~/.zfunc/_enya
enya completions fish > ~/.config/fish/completions/enya.fish
```

Supported shells: bash, zsh, fish, powershell, elvish.

## Typical Agent Workflow

### Investigation and Handoff

The primary agent workflow: investigate a problem, build a workspace with findings, hand it off to a human.

```sh
# 1. Create a workspace for the investigation
enya init incident-42 -e http://prometheus:9090

# 2. Query to understand the problem
enya --json query 'rate(http_errors_total[5m])' -w incident-42

# 3. Build a dashboard with the findings
enya add-section incident-42 "Error Analysis" --layout horizontal
enya add-pane incident-42 'rate(http_errors_total[5m])' \
  --name "Error Rate" --section "Error Analysis" --tag Critical
enya add-pane incident-42 'histogram_quantile(0.99, http_request_duration_seconds_bucket[5m])' \
  --name "Latency p99" --section "Error Analysis" --unit ms

enya add-section incident-42 "Infrastructure" --layout grid --columns 2
enya add-pane incident-42 'avg(node_cpu_seconds_total{mode="idle"})' \
  --name "CPU Idle" --section "Infrastructure"
enya add-pane incident-42 'node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes' \
  --name "Memory Available" --section "Infrastructure" --unit "%"

# 4. Configure time range and settings
enya set incident-42 time.preset 1h
enya set incident-42 workspace.description "Elevated 5xx errors on API gateway since 14:30 UTC"

# 5. Human opens the workspace in the GUI
enya --workspace incident-42
```

### Quick Exploration

```sh
# Discover workspaces and query metrics
enya --json list
enya --json show production
enya --json query 'up' -e http://prometheus:9090 --start 2h --step 30s

# Run SQL against local data
enya --json query --sql 'SELECT * FROM events WHERE level = "error"' --file events.parquet

# Run plugin commands
enya --json exec check-api http://service:8080/health
```
