# Enya Editor

A neovim-inspired observability editor built on [egui](https://github.com/emilk/egui). Tile-based workspace for PromQL/LogQL queries and time-series visualization. Runs natively and in the browser via WASM.

## Architecture

```
src/
  app/              EnyaApp entry point and AppState
  workspace/        Tile-based layout (egui_tiles)
  command/          UICommand enum + mpsc channel
  plugin/           Editor-side plugin bridge (re-exports enya-plugin)
  connection/       Prometheus and demo data backends
  components/
    pane/           Query, logs, tracing, terminal, sql, plugin panes
    overlay/        Command palette, finder, tutorial, diagnostics, ...
    widget/         Status line, sparkline, time controls, ...
    util/           Shared helpers
  ui/               Theme, colors, typography, icons, settings
  codebase/         Git repo + metrics-rs discovery (native only)
  util/             WASM-compatible time helpers
```

## Building

```bash
just install   # first time — dev tools + submodules
just run       # build and run
just ci        # fmt, clippy, machete, tests, WASM check
```

### Feature Flags

| Feature | Description |
|---------|-------------|
| `terminal` | Ghostty terminal pane (requires zig) |
| `sql` | DataFusion SQL pane |
| `all-languages` | Tree-sitter for Go, Python, JS/TS |

`puffin` and `tracy` profiling backends are mutually exclusive.

## WASM

- Use `crate::util::Instant` (not `std::time::Instant`)
- Use `crate::util::now_unix_secs()` (not `SystemTime`)
- Gate native-only code with `#[cfg(not(target_arch = "wasm32"))]`

## Plugins

See the [Plugin Authoring Guide](../../docs/plugins.md).
