# Enya UI

A neovim-inspired UI for the Enya metrics dashboard, built with [egui](https://github.com/emilk/egui).

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
| `:float` | `fl`, `popup`, `detach` | Float focused chart into a draggable window |
| `:dock` | `d`, `attach`, `tile` | Dock all floating windows back to tiled layout |
| `:notify` | `n`, `toast` | Show a test notification (`info`/`success`/`warn`/`error`) |
| `:settings` | `set`, `options`, `config` | Open settings |
| `:help` | `h`, `?` | Open help/documentation |

### Fuzzy Finder (Telescope-style)

Press `Ctrl+P` to open the fuzzy finder for quick metric and query search.

- **Live preview**: See a chart preview of the selected metric
- **Fuzzy matching**: Type partial names to filter results
- **Keyboard navigation**:
  - `↑`/`↓` or `Ctrl+K`/`Ctrl+J` - Navigate results
  - `Enter` - Select item
  - `Ctrl+P` - Toggle preview pane
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

#### View Modes

| Key | Action |
|-----|--------|
| `Z` | Toggle zen mode (hide all panels) |
| `F` | Toggle fullscreen for focused pane |
| `W` | Float focused pane into a window |
| `D` | Dock all floating windows |

#### Global Shortcuts

| Key | Action |
|-----|--------|
| `:` | Open command palette |
| `Ctrl+P` | Open fuzzy finder |
| `Ctrl+,` | Open settings |

### Status Line (lualine-style)

A segmented status bar at the bottom of the screen showing:

- **Mode indicator**: Current mode (NORMAL, COMMAND, SEARCH, ZEN, FULLSCREEN, SETTINGS)
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

### Floating Windows

Pop out any chart into a draggable, resizable overlay window:

- Float with `W` key or `:float` command
- Dock back with `D` key or `:dock` command
- Each floating window has a dock button to return to tiled layout
- Windows can be closed with the `X` button

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
│   ├── time_range.rs        # Time range selection toolbar
│   └── time_series_chart.rs # Chart rendering component
└── theme.rs            # Light/dark theme definitions
```

## Building

```bash
# Native build
cargo build -p enya-ui

# WASM build (for web)
cargo build -p enya-ui --target wasm32-unknown-unknown
```

## Running CI Checks

```bash
just ci
```
