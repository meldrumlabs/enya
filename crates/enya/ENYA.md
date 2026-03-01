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

# Open a workspace in the GUI editor
enya open <name>
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

Plugins live in `~/.enya/plugins/`.

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

## Metrics Discovery

Explore available Prometheus metrics, labels, and metadata. Useful for agents that need to understand what data is available before building queries or workspaces.

All commands use the same endpoint resolution as `enya query`: `--endpoint` flag, `--workspace` config, or `ENYA_PROMETHEUS_URL` env var.

```sh
# List all metric names
enya metrics list -e http://prometheus:9090
enya metrics list -w atlas
enya metrics list -e http://prometheus:9090 --match '{job="api"}'

# List all label names
enya metrics labels -e http://prometheus:9090
enya metrics labels -w atlas --match '{__name__="http_requests_total"}'

# List values for a specific label
enya metrics label-values job -e http://prometheus:9090
enya metrics label-values instance -w atlas

# Show metric type and help text
enya metrics info -e http://prometheus:9090                     # all metrics
enya metrics info http_requests_total -e http://prometheus:9090 # specific metric

# Find series matching a selector
enya metrics series '{job="api"}' -e http://prometheus:9090
enya metrics series 'http_requests_total' -w atlas
```

### JSON shapes

`enya --json metrics list`:
```json
{"metrics": ["cpu_usage", "http_requests_total", "memory_usage"], "count": 3}
```

`enya --json metrics labels`:
```json
{"labels": ["__name__", "env", "host", "job"], "count": 4}
```

`enya --json metrics label-values job`:
```json
{"values": ["api", "frontend", "worker"], "count": 3}
```

`enya --json metrics info http_requests_total`:
```json
{"metrics": [{"metric": "http_requests_total", "type": "counter", "help": "Total HTTP requests", "unit": ""}], "count": 1}
```

`enya --json metrics series '{job="api"}'`:
```json
{"series": [{"__name__": "http_requests_total", "job": "api", "method": "GET"}], "count": 1}
```

## Watch (Threshold Monitoring)

Poll a PromQL expression at regular intervals and alert when values cross a threshold. Useful for agents that need to monitor a metric and react when conditions are met.

```sh
# Alert if error rate exceeds 0.01 (polls every 30s by default)
enya watch atlas "rate(http_errors_total[5m])" --above 0.01

# Must stay above threshold for 5 continuous minutes before alerting
enya watch atlas "rate(http_errors_total[5m])" --above 0.01 --for 5m

# Alert if any "up" target drops below 1, polling every 15 seconds
enya watch atlas "up" --below 1 --every 15s

# Use a direct endpoint instead of a workspace
enya watch "up" --below 1 --endpoint http://prom:9090
```

### Behavior

- **Exit code 1**: Threshold condition triggered (or sustained for `--for` duration)
- **Exit code 130**: Clean shutdown via Ctrl-C (standard SIGINT)
- Uses Prometheus instant query (`/api/v1/query`) on each poll
- Checks ALL series returned by the expression

### Options

- `--above <value>` — Alert when any value exceeds this threshold
- `--below <value>` — Alert when any value drops below this threshold
- `--every <duration>` — Poll interval (default: 30s)
- `--for <duration>` — Condition must stay triggered for this duration
- `--endpoint <url>` — Prometheus endpoint URL
- `<workspace>` — Resolve endpoint from a workspace (optional first arg)

### JSON output

With `--json`, each poll prints a JSON line to stdout:

```json
{"timestamp":"2024-01-15 10:30:00","status":"ok","value":0.003,"threshold":"> 0.01","series_count":3}
{"timestamp":"2024-01-15 10:30:30","status":"alert","value":0.015,"threshold":"> 0.01","series_count":3,"triggered_for_secs":30}
```

Status values: `ok`, `warn` (triggered but `--for` not yet met), `alert` (triggered and exiting), `error`.

## Snapshots

Capture a workspace's current state including all query results at a point in time. Useful for incident reports, sharing state, and preserving data beyond Prometheus retention.

```sh
# Print snapshot to stdout (pretty-printed JSON)
enya snapshot atlas

# Compact JSON output
enya --json snapshot atlas

# Write to a file
enya snapshot atlas -o snapshot.json

# Override endpoint
enya snapshot atlas -e http://prometheus:9090
```

The snapshot captures:
- Full workspace configuration
- Timestamp and time range of capture
- Query results for every pane (executed as range queries over the workspace's time preset)
- Self-contained JSON — no external references needed

Individual pane query failures are captured as `{"error": "..."}` rather than failing the entire snapshot.

### JSON shape

```json
{
  "version": 1,
  "captured_at": 1704067200,
  "captured_at_human": "2024-01-01 00:00:00",
  "time_range": {
    "start": 1704063600,
    "end": 1704067200,
    "step": 60,
    "preset": "1h"
  },
  "workspace": { "...full WorkspaceConfig..." },
  "panes": [
    {
      "section": "API Performance",
      "name": "Request Rate",
      "query": "rate(http_requests_total[5m])",
      "result": {
        "resultType": "matrix",
        "result": [
          {
            "metric": {"job": "api", "method": "GET"},
            "values": [[1704063600, "42.5"], [1704063660, "43.1"]]
          }
        ]
      }
    }
  ]
}
```

## Serve (Remote / Headless Access)

Serve the WASM editor over HTTP with a built-in Prometheus proxy. Requires the `serve` feature (`--features serve`). The WASM assets are embedded in the binary at compile time.

```sh
# Serve a workspace on default port (3030)
enya serve <workspace>

# Custom port and bind address
enya serve <workspace> --port 8080 --bind 0.0.0.0

# Open browser automatically
enya serve <workspace> --open
```

Options:
- `--port` — Port to listen on (default: 3030)
- `--bind` — Address to bind to (default: 127.0.0.1)
- `--open` — Open the browser after starting

The server:
1. Embeds the WASM editor as static assets (single binary, no external files)
2. Rewrites the workspace endpoint to route through a local Prometheus proxy (`/proxy/*`)
3. Forwards API key as `Authorization: Bearer` header to Prometheus (if configured)
4. Serves the workspace via URL params (`?workspace=<base64>`) — no WASM changes needed

This is ideal for:
- **Remote/SSH environments** where you can't run a native GUI
- **Agent handoff over the network**: agent builds workspace, starts server, gives human a URL
- **Ephemeral investigation dashboards**: spin up, share URL, tear down with Ctrl-C

### Agent Handoff via Serve

```sh
# Agent builds workspace on a remote server
enya init incident-42 -e http://prometheus:9090
enya add-section incident-42 "Error Analysis"
enya add-pane incident-42 'rate(http_errors_total[5m])' --name "Error Rate" --tag Critical
enya set incident-42 time.preset 1h

# Agent starts the server
enya serve incident-42 --port 3030
# → Serving workspace 'incident-42' at http://localhost:3030
# → Proxying Prometheus at http://prometheus:9090

# Human connects via SSH tunnel or directly opens the URL
```

### Building with Serve Support

```sh
# Step 1: Build WASM editor with Trunk
cd crates/editor && trunk build --release

# Step 2: Build CLI with embedded WASM assets
cargo build -p enya --features serve --release

# Or use the just recipe:
just serve-build
```

## Shell Completions

Generate completions for your shell:

```sh
enya completions bash > ~/.local/share/bash-completion/completions/enya
enya completions zsh > ~/.zfunc/_enya
enya completions fish > ~/.config/fish/completions/enya.fish
```

Supported shells: bash, zsh, fish, powershell, elvish.

## Session (Agent Integration)

`enya session` starts a long-running JSON-RPC 2.0 process over stdin/stdout. Agents spawn it once and send requests over the bidirectional channel instead of forking a new process per command.

```sh
enya session
```

### Protocol

Newline-delimited JSON-RPC 2.0. Each line on stdin is a request; each line on stdout is a response or notification. Stderr is used for human-readable logs.

**Request:**
```json
{"jsonrpc":"2.0","id":1,"method":"workspace.show","params":{"name":"atlas"}}
```

**Response:**
```json
{"jsonrpc":"2.0","id":1,"result":{...}}
```

**Error:**
```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"method not found: bogus"}}
```

**Notification** (server→client, no `id`):
```json
{"jsonrpc":"2.0","method":"watch.status","params":{"watch_id":1,"status":"ok","value":0.003,...}}
```

### Available Methods

#### Workspace

| Method | Required Params | Optional Params |
|--------|----------------|-----------------|
| `workspace.list` | — | — |
| `workspace.show` | `name` | — |
| `workspace.init` | — | `name`, `endpoint`, `template` |
| `workspace.rm` | `name` | — |
| `workspace.get` | `name`, `key` | — |
| `workspace.set` | `name`, `key`, `value` | — |
| `workspace.add_section` | `name`, `section_name` | `layout`, `columns`, `collapsed` |
| `workspace.add_pane` | `name`, `query` | `pane_name`, `section`, `tag`, `unit`, `granularity`, `visualization`, `description` |
| `workspace.remove_section` | `name`, `section_name` | — |
| `workspace.remove_pane` | `name`, `pane` | `section` |
| `workspace.snapshot` | `name` | `endpoint` |

#### Query

| Method | Required Params | Optional Params |
|--------|----------------|-----------------|
| `query.instant` | `expression` | `endpoint`, `workspace` |
| `query.range` | `expression` | `endpoint`, `workspace`, `start`, `end`, `step` |

#### Metrics Discovery

| Method | Required Params | Optional Params |
|--------|----------------|-----------------|
| `metrics.list` | — | `endpoint`, `workspace`, `match` |
| `metrics.labels` | — | `endpoint`, `workspace`, `match` |
| `metrics.label_values` | `label` | `endpoint`, `workspace` |
| `metrics.info` | — | `metric`, `endpoint`, `workspace` |
| `metrics.series` | `selector` | `endpoint`, `workspace` |

#### Watch (background, managed by session)

| Method | Required Params | Optional Params |
|--------|----------------|-----------------|
| `watch.start` | `expression` + one of `above`/`below` | `endpoint`, `workspace`, `every`, `for` |
| `watch.stop` | `watch_id` | — |
| `watch.list` | — | — |

Watches send notifications to stdout:
- `watch.status` — periodic status: `{"watch_id":1, "status":"ok"|"warn"|"error", "value":0.003, ...}`
- `watch.triggered` — threshold crossed (watch auto-stops): `{"watch_id":1, "value":0.015, ...}`

#### Plugins

| Method | Required Params | Optional Params |
|--------|----------------|-----------------|
| `plugins.list` | — | — |
| `plugins.commands` | — | — |
| `plugins.install` | `source` | — |
| `plugins.remove` | `name` | — |

#### Exec

| Method | Required Params | Optional Params |
|--------|----------------|-----------------|
| `exec.run` | `command` | `args` |

Return shape varies by action type:
- **Shell**: `{command, shell, exit_code, success, stdout, stderr}`
- **URL**: `{command, url}`
- **Notify**: `{command, message}`
- **Lua**: `{command, plugin, success}`

#### Session

| Method | Params | Returns |
|--------|--------|---------|
| `session.info` | — | `{"version":"...", "capabilities":["workspace","query","metrics","watch","plugins"]}` |
| `session.shutdown` | — | `{"status":"shutting_down"}` (process exits) |

### Example: Agent Integration

```sh
# Start session (agent spawns this once)
enya session

# Agent sends requests on stdin:
{"jsonrpc":"2.0","id":1,"method":"session.info","params":{}}
{"jsonrpc":"2.0","id":2,"method":"workspace.list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"workspace.init","params":{"name":"incident-42","endpoint":"http://prometheus:9090"}}
{"jsonrpc":"2.0","id":4,"method":"workspace.add_section","params":{"name":"incident-42","section_name":"Error Analysis"}}
{"jsonrpc":"2.0","id":5,"method":"workspace.add_pane","params":{"name":"incident-42","query":"rate(http_errors_total[5m])","pane_name":"Error Rate","section":"Error Analysis"}}
{"jsonrpc":"2.0","id":6,"method":"query.instant","params":{"expression":"up","workspace":"incident-42"}}
{"jsonrpc":"2.0","id":7,"method":"watch.start","params":{"expression":"rate(http_errors_total[5m])","above":0.01,"workspace":"incident-42"}}
{"jsonrpc":"2.0","id":8,"method":"workspace.snapshot","params":{"name":"incident-42"}}
{"jsonrpc":"2.0","id":9,"method":"session.shutdown","params":{}}
```

### Error Codes

| Code | Meaning |
|------|---------|
| -32700 | Parse error (invalid JSON) |
| -32601 | Method not found |
| -32602 | Invalid params (missing required param, invalid value) |
| -32603 | Internal error (workspace not found, query failed, etc.) |

## Typical Agent Workflow

### Investigation and Handoff

The primary agent workflow: investigate a problem, build a workspace with findings, hand it off to a human.

```sh
# 1. Discover available metrics
enya --json metrics list -e http://prometheus:9090
enya --json metrics info http_errors_total -e http://prometheus:9090
enya --json metrics label-values job -e http://prometheus:9090

# 2. Create a workspace for the investigation
enya init incident-42 -e http://prometheus:9090

# 3. Query to understand the problem
enya --json query 'rate(http_errors_total[5m])' -w incident-42

# 4. Build a dashboard with the findings
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

# 5. Configure time range and settings
enya set incident-42 time.preset 1h
enya set incident-42 workspace.description "Elevated 5xx errors on API gateway since 14:30 UTC"

# 6. Snapshot the current state (preserves data for later review)
enya snapshot incident-42 -o incident-42-snapshot.json

# 7. Human opens the workspace in the GUI
enya open incident-42
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
