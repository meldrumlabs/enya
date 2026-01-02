# Profiling the Enya Editor

This document describes how to profile the Enya editor using the `profiling` crate with either Puffin or Tracy backends.

## Overview

The editor uses the [`profiling`](https://crates.io/crates/profiling) crate for zero-cost profiling instrumentation. When no profiling feature is enabled, all `#[profiling::function]` annotations compile to nothing.

Over 40 functions are instrumented across:
- **Widgets**: Buffer, StatusLine, TimeRange, LandingPage, AgentInputBar, Notifications
- **Panes**: QueryPane, AgentPane, TimeSeriesChart
- **Visualizations**: Sparkline, Stat, Gauge, Bar, Heatmap
- **Overlays**: CommandPalette, MetricsFinder, BufferEditor, and all other modals
- **Utilities**: Finder (fuzzy search), QueryCompletion, syntax highlighting
- **Workspace**: tile rendering, pane queries, filtered view

## Puffin (Recommended for Quick Profiling)

[Puffin](https://github.com/EmbarkStudios/puffin) is a simple instrumentation profiler with a standalone viewer.

### Setup

1. **Install the viewer** (one-time):
   ```bash
   cargo install puffin_viewer
   ```

2. **Run the editor with puffin enabled**:
   ```bash
   cargo run -p enya-editor --features puffin --release
   ```

   The editor will start a puffin HTTP server on `127.0.0.1:8585`.

3. **Connect the viewer**:
   ```bash
   puffin_viewer
   ```

   The viewer auto-connects and shows live flame graphs.

### What You'll See

The flame graph displays:
- **Horizontal bars** = function duration
- **Nested bars** = call hierarchy (parent calls child)
- **Width** = relative time spent

Example hierarchy:
```
EnyaApp::update
├── show_main_content
│   └── Workspace::show
│       ├── get_pane_tile_ids
│       ├── QueryPane::show
│       │   └── TimeSeriesChart::show
│       │       ├── format_timestamp
│       │       └── format_value_with_unit
│       └── paint_on_top_of_tile
└── StatusLine::show
```

### Key Metrics

| Metric | Target |
|--------|--------|
| Total frame time | <16ms for 60 FPS |
| Any single function | <2ms ideally |
| `get_pane_tile_ids` calls/frame | Should be 1-2, not 5-10 |

## Tracy (Advanced Profiling)

[Tracy](https://github.com/wolfpld/tracy) is a real-time, nanosecond resolution profiler with more advanced features.

### Setup

1. **Install Tracy**:
   - **macOS**: `brew install tracy`
   - **Linux**: Build from source or use package manager
   - **Windows**: Download from [releases](https://github.com/wolfpld/tracy/releases)

2. **Run the editor with tracy enabled**:
   ```bash
   cargo run -p enya-editor --features tracy --release
   ```

3. **Connect Tracy**:
   - Open the Tracy profiler application
   - Click "Connect" to connect to the running editor

### Tracy vs Puffin

| Feature | Puffin | Tracy |
|---------|--------|-------|
| Setup complexity | Simple | Moderate |
| Resolution | ~1ms | Nanosecond |
| Memory profiling | No | Yes |
| GPU profiling | No | Yes |
| Frame history | Limited | Extensive |
| Best for | Quick checks | Deep analysis |

## Interpreting Results

### Common Bottlenecks to Look For

1. **String allocations in hot paths**
   - `format_timestamp()` and `format_value_with_unit()` allocate strings
   - If these show significant time, consider caching

2. **Repeated tree traversals**
   - `get_pane_tile_ids()` walks the tile tree
   - Should be called once per frame, not multiple times

3. **Fuzzy matching overhead**
   - `Finder::refresh_results()` runs nucleo matching
   - Only runs on keystrokes, but can be slow with many items

4. **Syntax highlighting**
   - `highlight_line()` uses tree-sitter (native only)
   - Can be slow for large files

### Optimization Workflow

1. **Profile first**: Run with puffin/tracy to get baseline
2. **Identify hotspots**: Look for functions >1ms or called unexpectedly often
3. **Implement caching**: Add memoization for expensive pure functions
4. **Verify improvement**: Profile again to confirm gains

## Adding New Instrumentation

To add profiling to a new function:

```rust
#[profiling::function]
fn my_expensive_function() {
    // ...
}
```

For inline scopes within a function:

```rust
fn complex_function() {
    profiling::scope!("phase_1");
    // ... phase 1 work ...

    profiling::scope!("phase_2");
    // ... phase 2 work ...
}
```

## Troubleshooting

### Puffin viewer doesn't connect

- Ensure the editor is running with `--features puffin`
- Check that port 8585 is not blocked by firewall
- Look for "Puffin profiler server listening on 127.0.0.1:8585" in logs

### Tracy doesn't show data

- Ensure the editor is running with `--features tracy`
- Tracy requires running the profiler *before* starting the application
- On macOS, you may need to grant accessibility permissions

### Profiling overhead

- Release builds (`--release`) have minimal overhead
- Debug builds will show inflated times due to unoptimized code
- Always profile release builds for accurate measurements
