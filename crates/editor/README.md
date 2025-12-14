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
