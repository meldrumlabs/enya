# Enya Editor

A neovim-inspired editor for the Enya metrics dashboard, built with [egui](https://github.com/emilk/egui).

## Neovim-Inspired Features

### Command Palette

Press `:` to open the command palette (similar to neovim's command mode).

| Command | Aliases | Description |
|---------|---------|-------------|
| `:theme` | `t` | Set theme (`dark` or `light`) |
| `:search` | `s`, `find`, `f` | Open fuzzy finder search |
| `:split` | `sp` | Split pane (`h` for horizontal, `v` for vertical) |
| `:vsplit` | `vs` | Vertical split |
| `:close` | `q`, `quit` | Close current tab |
| `:exit` | | Quit application |
| `:write` | `w`, `save` | Save buffer |
| `:edit` | `e` | Edit buffer (enter insert mode) |
| `:new` | `enew`, `buffer` | Create a new buffer |
| `:zen` | `z`, `focus` | Toggle zen mode |
| `:fullscreen` | `full`, `max` | Toggle fullscreen for focused chart |
| `:home` | `landing`, `start` | Show the landing page |
| `:screenshot` | `ss`, `snap` | Take a screenshot |
| `:mksession` | `mks`, `savews` | Save workspace |
| `:source` | `so`, `loadws` | Load workspace |
| `:workspaces` | `ws`, `sessions` | List available workspaces |
| `:share` | `export`, `url` | Share workspace as URL |
| `:commits` | `git`, `markers` | Toggle git commit markers |
| `:connect` | `conn` | Connect to agent |
| `:diagnostics` | `diag`, `d` | Toggle/show/hide/clear diagnostics |
| `:info` | `version`, `about` | Show version and build info |
| `:help` | `h`, `?` | Show help |

### Fuzzy Finder (Telescope-style)

Press `Ctrl+K` to open the fuzzy finder for quick metric search.

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
| `Ctrl+K` | Open fuzzy finder |
| `?` | Show which-key help overlay |

### Status Line (lualine-style)

A segmented status bar at the bottom of the screen showing:

- **Mode indicator**: Current mode (NORMAL, COMMAND, SEARCH, ZEN, FULLSCREEN, V-MULTI)
- **Connection status**: Server connection state
- **Viewport info**: Current layout information
- **Open tabs**: Number of open chart tabs

### Notifications (nvim-notify-style)

Toast-style notifications in the top-right corner with:

- Four severity levels: Info, Success, Warning, Error
- Auto-dismiss with progress bar
- Fade-out animation
- Manual dismiss with close button

### Zen Mode

Distraction-free viewing mode that hides the time range toolbar.

Toggle with `Z` key or `:zen` command.

### Fullscreen Mode

Maximize a single chart pane to fill the entire viewport:

- Toggle with `F` key or `:fullscreen` command
- All other panes are temporarily hidden
- Status line shows "FULLSCREEN" mode indicator

### Visual-Block Mode (Multi-Pane Selection)

Select and edit multiple panes simultaneously, inspired by Vim's visual-block mode (`Ctrl+V`).

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

### URL Sharing

Share your workspace or individual panes via URL:

- **Full workspace**: Use `:share` command to copy a URL containing your entire workspace layout
- **Single pane**: Use `yy` (vim-style yank) on a focused pane to share just that query

## Architecture

```
crates/editor/src/
├── app.rs              # Main application state and rendering
├── dashboard.rs        # Dashboard with tiled layout (egui_tiles)
├── components/
│   ├── command_palette.rs   # Neovim-style command input
│   ├── metrics_finder.rs    # Telescope-style search with preview
│   ├── status_line.rs       # Lualine-style status bar
│   ├── notifications.rs     # nvim-notify-style toasts
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
