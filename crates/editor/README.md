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
| `Shift+N` | Go to next workspace tab |
| `Shift+P` | Go to previous workspace tab |

#### Global Shortcuts

| Key | Action |
|-----|--------|
| `:` | Open command palette |
| `Ctrl+K` | Open fuzzy finder |
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
| `Escape` | Exit visual-block mode |

### URL Sharing

- **Full workspace**: `:share` command
- **Single pane**: `yy` on focused pane

## Building

```bash
cargo build -p enya-editor
```

## Running CI Checks

```bash
just ci
```
