# Enya Editor

A neovim-inspired editor for the Enya metrics dashboard, built with [egui](https://github.com/emilk/egui).

## Neovim-Inspired Features

### Command Palette

Press `:` to open the command palette (similar to neovim's command mode).

| Command | Aliases | Description |
|---------|---------|-------------|
| `:theme` | `t` | Set theme (`dark` or `light`) |
| `:metrics` | `m`, `tree`, `sidebar` | Toggle metrics panel visibility |
| `:inspector` | `i`, `info`, `details` | Toggle inspector panel visibility |
| `:split` | `sp` | Split pane (`h` for horizontal, `v` for vertical) |
| `:vsplit` | `vs`, `vsp` | Vertical split |
| `:close` | `q`, `quit`, `bd` | Close current tab |
| `:zen` | `z`, `focus`, `distraction-free` | Toggle zen mode |
| `:fullscreen` | `full`, `maximize`, `max` | Toggle fullscreen for focused chart |
| `:notify` | `n`, `toast` | Show a test notification (`info`/`success`/`warn`/`error`) |
| `:settings` | `set`, `options`, `config` | Open settings |
| `:help` | `h`, `?` | Open help/documentation |
| `:share` | | Share workspace as URL (copies to clipboard) |
| `:screenshot` | `ss` | Capture window screenshot |
| `:tag` | `#` | Manage tags (see below) |
| `:tags` | `taglist`, `tl` | Show all tags |

### Fuzzy Finder (Telescope-style)

Press `Ctrl+P` to open the fuzzy finder for quick metric and query search.

- **Live preview**: See a chart preview of the selected metric
- **Fuzzy matching**: Type partial names to filter results
- **Tag search**: Type `#` to search and open tagged queries
- **Keyboard navigation**:
  - `↑`/`↓` or `Ctrl+K`/`Ctrl+J` - Navigate results
  - `Enter` - Select item (for tags, opens all queries with that tag)
  - `Ctrl+P` - Toggle preview pane
  - `Esc` - Close

### Hierarchical Tags

Organize queries with hierarchical tags (e.g., `production/api/latency`).

#### Tag Commands

| Command | Description |
|---------|-------------|
| `:tag +production` | Add tag to focused chart |
| `:tag -production` | Remove tag from focused chart |
| `:tag production` | Filter queries by tag |
| `:tag` | Clear tag filter |
| `:tags` | Show all defined tags |

#### Usage

1. **Add tags**: Focus a chart and run `:tag +mytag` (auto-saves raw metrics as queries)
2. **Search by tag**: In fuzzy finder, type `#` then tag name (e.g., `#prod`)
3. **Open tagged queries**: Select a tag in fuzzy finder to open all queries with that tag
4. **Hierarchical paths**: Use `/` for hierarchy (e.g., `production/api` matches `production/api/latency`)

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
| `Z` | Toggle zen mode (hide all panels) |
| `F` | Toggle fullscreen for focused pane |

#### Chart Zoom Controls

| Key | Action |
|-----|--------|
| `+` / `=` | Zoom in on Y-axis (values) |
| `-` | Zoom out on Y-axis (values) |
| `.` / `>` | Zoom in on X-axis (time) |
| `,` / `<` | Zoom out on X-axis (time) |
| `0` | Reset zoom to fit all data |
| `gg` | Go to start of data |
| `G` | Go to end of data |

#### Workspace Tabs (barbar.nvim-style)

| Key | Action |
|-----|--------|
| `Shift+T` | Create new workspace tab |
| `Shift+N` | Go to next workspace tab |
| `Shift+P` | Go to previous workspace tab |

Commands: `:tabnew`, `:tabnext`/`:tabn`, `:tabprev`/`:tabp`, `:tabclose`/`:tabc`

#### Global Shortcuts

| Key | Action |
|-----|--------|
| `:` | Open command palette |
| `Ctrl+P` | Open fuzzy finder |
| `Ctrl+,` | Open settings |

### Status Line (lualine-style)

A segmented status bar at the bottom of the screen showing:

- **Mode indicator**: Current mode (NORMAL, COMMAND, SEARCH, ZEN, FULLSCREEN, V-MULTI)
- **Connection status**: Server connection state
- **Selected metric**: Currently selected metric name
- **Viewport info**: Current layout information
- **Open tabs**: Number of open chart tabs

### Notifications (nvim-notify-style)

Toast-style notifications in the top-right corner with:

- Four severity levels: Info, Success, Warning, Error
- Auto-dismiss with progress bar
- Fade-out animation
- Manual dismiss with close button

### Zen Mode

Distraction-free viewing mode that hides:

- Metrics panel (left sidebar)
- Inspector panel (right sidebar)
- Time range toolbar

Toggle with `Z` key or `:zen` command.

### Fullscreen Mode

Maximize a single chart pane to fill the entire viewport:

- Toggle with `F` key or `:fullscreen` command
- All other panes are temporarily hidden
- Status line shows "FULLSCREEN" mode indicator

### Visual-Block Mode (Multi-Pane Selection)

Select and edit multiple panes simultaneously, inspired by Vim's visual-block mode (`Ctrl+V`) and Zed's multibuffer concept.

#### Entering Visual-Block Mode

Press `Ctrl+V` with a focused pane to enter visual-block mode. The status line shows "V-MULTI".

#### Selection

| Key | Action |
|-----|--------|
| `j` / `↓` | Move cursor down and select pane |
| `k` / `↑` | Move cursor up and select pane |
| `h` / `←` | Move cursor left and select pane |
| `l` / `→` | Move cursor right and select pane |
| `Space` | Toggle selection on current pane |
| `a` | Select all panes |
| `n` | Deselect all panes |
| `Escape` | Exit visual-block mode |

Selected panes are highlighted with a purple tint and border.

#### Multi-Edit Overlay

Press `e` after selecting panes to open the multi-edit overlay:

- **Stacked excerpts**: Edit each pane's query in a labeled text field
- **Find/Replace**: Search and replace across all selected panes at once
- **Match count**: Shows number of matches found

| Key | Action |
|-----|--------|
| `Tab` | Cycle through excerpts |
| `Shift+Tab` | Cycle backwards |
| `⌘⇧R` | Replace all matches |
| `⌘↵` | Apply changes and close |
| `Escape` | Cancel and close |

#### Use Case Example

To change `env:staging` to `env:production` across multiple query panes:

1. `Ctrl+V` - Enter visual-block mode
2. `j`/`k` - Navigate to select the panes you want to edit
3. `e` - Open multi-edit overlay
4. Type `env:staging` in Find field
5. Type `env:production` in Replace field
6. `⌘⇧R` - Replace all
7. `⌘↵` - Apply changes

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
| `:w [name]` | `write`, `save` | Save current workspace |
| `:e [name]` | `edit`, `open`, `load` | Load a workspace |
| `w` | | Open workspace finder (fuzzy search) |

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
tag = "Critical"
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

Panes are referenced by their 0-indexed position in the `[[panes]]` array. The order in which panes are defined determines their index:

```toml
[[panes]]          # Index 0
query = "service:api"

[[panes]]          # Index 1
query = "service:db"

[[panes]]          # Index 2
query = "service:cache"
```

You can then use these indices in the layout's `children` array to arrange panes in any order.

#### Simple Example

```toml
# Creates: Pane0 | Pane1 with equal widths
[layout]
type = "horizontal"
children = [0, 1]
```

#### Nested Layout Example

```toml
[[panes]]
query = "service:api"
name = "API"

[[panes]]
query = "service:db"
name = "Database"

[[panes]]
query = "service:cache"
name = "Cache"

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

#### Complex Dashboard Layout

```toml
# 8-pane production dashboard
# +-------------------+-------------------+
# |                   |   API p99 (1)     |
# |   Overview (0)    +-------------------+
# |                   |   API p50 (2)     |
# +-------------------+-------------------+
# | DB (3) | Cache(4) | Errors | Mem | CPU|
# +--------+----------+--------+-----+----+

[layout]
type = "vertical"
shares = [2.0, 1.0]
children = [
    { type = "horizontal", shares = [1.0, 1.0], children = [
        0,
        { type = "vertical", children = [1, 2] }
    ]},
    { type = "horizontal", shares = [1.5, 1.5, 1.0, 1.0, 1.0], children = [3, 4, 5, 6, 7] }
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

### Default Workspaces

Two example workspaces are created automatically on first run:

- `example.toml` - Simple 3-pane layout demonstrating basic features
- `dashboard.toml` - Complex 8-pane production dashboard with nested layout

## Architecture

```
crates/ui/src/
├── app.rs              # Main application state and rendering
├── dashboard.rs        # Dashboard with tiled layout (egui_tiles)
├── components/
│   ├── command_palette.rs   # Neovim-style command input
│   ├── fuzzy_finder.rs      # Telescope-style search with preview
│   ├── status_line.rs       # Lualine-style status bar
│   ├── notifications.rs     # nvim-notify-style toasts
│   ├── metrics_tree.rs      # Metrics browser sidebar
│   ├── inspector.rs         # Metric details panel
│   ├── tags.rs              # Hierarchical tagging system
│   ├── custom_queries.rs    # Saved queries with tags
│   ├── multi_edit.rs        # Multi-pane editing overlay
│   ├── time_range.rs        # Time range selection toolbar
│   └── time_series_chart.rs # Chart rendering component
└── theme.rs            # Light/dark theme definitions
```

## Building

```bash
# Native build
cargo build -p enya-editor

# WASM build (for web)
cargo build -p enya-editor --target wasm32-unknown-unknown
```

## Running CI Checks

```bash
just ci
```
