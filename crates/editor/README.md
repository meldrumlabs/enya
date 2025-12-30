# Enya Editor

A neovim-inspired editor for the Enya metrics dashboard, built with [egui](https://github.com/emilk/egui).

## Neovim-Inspired Features

### Command Palette

Press `:` to open the command palette (similar to neovim's command mode).

| Command | Alias | Description |
|---------|-------|-------------|
| `:theme` | `t` | Toggle or set theme (`dark`/`light`) |
| `:search` | `s` | Open fuzzy finder search |
| `:split` | `sp` | Split pane (`h`/`v`) |
| `:vsplit` | `vs` | Vertical split |
| `:close` | `q` | Close current tab |
| `:exit` | | Quit application |
| `:zen` | `z` | Toggle zen mode |
| `:fullscreen` | `full` | Toggle fullscreen for focused chart |
| `:home` | | Show the landing page |
| `:screenshot` | `ss` | Take a screenshot |
| `:mksession` | `mks` | Save workspace |
| `:source` | `so` | Load workspace |
| `:workspaces` | `ws` | List available workspaces |
| `:share` | | Share workspace as URL |
| `:commits` | `git` | Toggle git commit markers |
| `:connect` | | Connect to agent |
| `:diagnostics` | `diag` | Toggle/show/hide/clear diagnostics |
| `:tabnew` | | Create new workspace tab |
| `:tabclose` | | Close current workspace tab |
| `:info` | `version` | Show version and build info |
| `:help` | `h` | Show help |

### Fuzzy Finder (Telescope-style)

Press `Space+m` to open the metrics finder for quick metric search.

- **Live preview**: See a chart preview of the selected metric
- **Fuzzy matching**: Type partial names to filter results
- **Keyboard navigation**:
  - `↑`/`↓` or `Ctrl+K`/`Ctrl+J` - Navigate results
  - `Enter` - Select item
  - `Esc` - Close

### Keyboard Shortcuts

#### Viewport Navigation (vim-style)

| Key | Action |
|-----|--------|
| `H` / `←` | Move focus left |
| `J` / `↓` | Move focus down |
| `K` / `↑` | Move focus up |
| `L` / `→` | Move focus right |
| `Esc` | Clear focus |
| `X` | Close focused pane |
| `yy` | Yank (share) focused pane as URL |

#### View Modes

| Key | Action |
|-----|--------|
| `Z` | Toggle zen mode |
| `F` | Toggle fullscreen for focused pane |

#### Chart Zoom Controls

| Key | Action |
|-----|--------|
| `+` / `=` | Zoom in on Y-axis |
| `-` | Zoom out on Y-axis |
| `.` / `>` | Zoom in on X-axis |
| `,` / `<` | Zoom out on X-axis |
| `0` | Reset zoom |
| `gg` | Go to start of data |
| `G` | Go to end of data |

#### Workspace Tabs

| Key | Action |
|-----|--------|
| `Shift+T` | Create new workspace tab |
| `Shift+X` | Close current workspace tab |
| `Shift+N` | Go to next workspace tab |
| `Shift+P` | Go to previous workspace tab |

#### Global Shortcuts

| Key | Action |
|-----|--------|
| `:` | Open command palette |
| `Space+m` | Open metrics finder |
| `Space+w` | Open workspace finder |
| `Space+h` | Go to home |
| `?` | Show which-key help overlay |

### Status Line (lualine-style)

A segmented status bar at the bottom showing:

- **Mode indicator**: NORMAL, COMMAND, SEARCH, ZEN, FULLSCREEN, V-MULTI
- **Connection status**: Server connection state
- **Viewport info**: Current layout information
- **Open tabs**: Number of open chart tabs

### Visual-Block Mode (Multi-Pane Selection)

Select and edit multiple panes simultaneously (`Ctrl+V`).

| Key | Action |
|-----|--------|
| `j`/`k`/`h`/`l` | Navigate and select panes |
| `Space` | Toggle selection |
| `a` | Select all panes |
| `n` | Deselect all |
| `x` | Close selected panes |
| `e` | Edit selected panes |
| `r` | Refresh selected panes |
| `Escape` | Exit visual-block mode |

### Agent Mode

Press `aa` to enter Agent mode for AI-powered observability assistance. The agent can analyze metrics, generate PromQL queries, and help investigate incidents.

#### Quick Commands (single key, when input is empty)

| Key | Action |
|-----|--------|
| `w` | What's wrong? (triage) |
| `y` | Why? (root cause) |
| `c` | Compare (to baseline) |
| `e` | Explain (focused element) |
| `f` | Fix (remediation) |
| `s` | Summarize (incident) |
| `h` | History (past incidents) |

#### Slash Commands

Type `/` to trigger command suggestions:

| Command | Aliases | Description |
|---------|---------|-------------|
| `/investigate` | `inv`, `dig` | Deep-dive analysis with correlations and anomalies |
| `/diff` | `compare`, `cmp` | Compare metric states between two time ranges |
| `/query` | `q`, `promql` | Generate PromQL from natural language |
| `/explain` | `exp`, `what` | Explain what the current query or chart shows |

#### Metric Mentions

Type `@` to autocomplete metric names. Combine with slash commands:

```
/investigate @http_requests_total why is it spiking?
/query show me error rate for @api_latency_seconds
/diff @cpu_usage compare last hour to yesterday
```

#### Agent Mode Shortcuts

| Key | Action |
|-----|--------|
| `↑`/`↓` | Navigate popup suggestions |
| `Tab`/`Enter` | Select suggestion |
| `Esc` | Close popup or exit agent mode |
| `+` | Add focused pane to context |
| `-` | Remove focused pane from context |

### URL Sharing

Share your workspace or individual panes via URL:

- **Full workspace**: Use `:share` command to copy a URL containing your entire workspace layout
- **Single pane**: Use `yy` (vim-style yank) on a focused pane to share just that query

URLs are compact using binary encoding (postcard + LZ4 compression + base64) and can be shared at `enya.build/editor`:

- `?workspace=...` - Full workspace with all panes and layout
- `?pane=...` - Single pane with just one query

## Workspaces

Workspaces are TOML configuration files that define a collection of panes with queries and an optional i3-style tiling layout. Workspaces are stored in `~/.enya/workspaces/`.

### Workspace Commands

| Command | Aliases | Description |
|---------|---------|-------------|
| `:mksession [name]` | `mks` | Save current workspace |
| `:source [name]` | `so` | Load a workspace |
| `:workspaces` | `ws` | Open workspace finder (fuzzy search) |

### Basic Workspace Structure

```toml
[workspace]
name = "my-dashboard"
description = "Production monitoring dashboard"

[view]
theme = "dark"

[time]
preset = "1h"  # Options: 5m, 15m, 30m, 1h, 3h, 6h, 12h, 24h, 7d

[[panes]]
query = "env:prod AND service:api"
name = "API Latency"
aggregation = "p95"
granularity = "1m"

[[panes]]
query = "env:prod AND name:error_rate"
name = "Error Rate"
aggregation = "sum"
granularity = "5m"
```

### i3-Style Layout Configuration

Workspaces support i3-style tiling layouts with horizontal/vertical splits and custom proportions.

#### Layout Types

- `horizontal` - Split panes side by side
- `vertical` - Stack panes top to bottom
- `tabs` - Group panes as tabs

#### Pane References

Panes are referenced by their 0-indexed position in the `[[panes]]` array:

```toml
[[panes]]          # Index 0
query = "service:api"

[[panes]]          # Index 1
query = "service:db"

[[panes]]          # Index 2
query = "service:cache"
```

#### Simple Example

```toml
# Creates: Pane0 | Pane1 with equal widths
[layout]
type = "horizontal"
children = [0, 1]
```

#### Nested Layout Example

```toml
# Creates: API (2/3 width) | (DB / Cache stacked, 1/3 width)
# +---------------------+-----------+
# |                     | Database  |
# |        API          +-----------+
# |                     |   Cache   |
# +---------------------+-----------+
[layout]
type = "horizontal"
shares = [2.0, 1.0]
children = [
    0,
    { type = "vertical", children = [1, 2] }
]
```

#### Layout Properties

| Property | Type | Description |
|----------|------|-------------|
| `type` | string | Container type: `horizontal`, `vertical`, or `tabs` |
| `children` | array | Pane indices (numbers) or nested container objects |
| `shares` | array | Optional proportional sizing (defaults to equal `1.0` for each child) |

### Connection Configuration

```toml
[connection]
endpoint = "http://localhost:9797"
api_key = "optional-api-key"
```

## Building

```bash
cargo build -p enya-editor
```

## Running CI Checks

```bash
just ci
```
