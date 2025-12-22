# Changelog

All notable changes to the Enya editor will be documented in this file.

## [Unreleased]

### Changed

- **Cleaner visualization headers**: Removed the gray metric name/query text that was displayed at the top of Gauge, Stat, Bar Chart, and Sparkline visualizations. Visualizations now only show a title when explicitly set (and not "Untitled"), using a stronger, more prominent text style. This eliminates visual clutter and prevents raw query text from appearing in chart displays.

- **Responsive visualization scaling**: All visualizations (Time Series, Gauge, Stat, Bar Chart, Sparkline) now scale dynamically based on available panel space. Text sizes, line widths, legend elements, and other dimensions scale proportionally with the panel size. This ensures visualizations look appropriate whether in a small tile or fullscreen.

### Added

- **Git commit timeline markers**: Time-series charts now display vertical markers for git commits that occurred during the visible time range. This helps correlate code changes with metric behavior (e.g., identify which deploy caused a spike). Features include:
  - Automatic commit fetching when codebase is configured and indexed
  - Commits displayed as dashed emerald vertical lines
  - Commit labels shown above the chart with truncated messages (up to 8 visible)
  - Hover over a commit marker to see the hash and full commit message
  - Navigate between commits with `]c` (next) and `[c` (previous)
  - Commits are cached per time range for performance
  - Native-only feature (requires `[codebase]` config with git repository)

- **Grafana dashboard JSON import**: Added `workspace::grafana` module for converting Grafana dashboard JSON exports to Enya's workspace TOML format. Supports timeseries, graph, stat, singlestat, gauge, barchart, bargauge, and heatmap panel types. See `examples/grafana-dashboard.json` for an example input.

- **Custom unit suffixes for values**: Added `unit` field to `PaneConfig` and all visualization types. Units like "ms", "req/s", "%", "MB/s" are now displayed on Y-axis labels and in chart legends. Grafana panel units are automatically converted during import.

- **Enhanced chart legend with values**: The time series chart legend is now displayed above the chart in a horizontal-wrapped layout showing the latest value for each series. Legends display up to 5 series by default, with a "+ N more" indicator that reveals all hidden series in a hover tooltip. Series labels are truncated intelligently (using tag values when available). Use a query containing "by_endpoint" or "by_method" to test with 12 demo series.

- **Cleaner query pane UI**: The query pane header bar (with mode indicator) is now hidden when not editing. An edit button appears as a subtle overlay in the top-right corner when hovering, and the buffer can be opened with 'e' key or by clicking the pencil icon.

- **Go to Metric Definition (`gd`)**: Press `gd` on a focused chart pane to view the source code where the metric is instrumented. Features include:
  - Source preview overlay showing ~20 lines of context around the metric definition
  - Proper Rust syntax highlighting using `tree-sitter-highlight` (keywords, types, strings, comments, functions, macros, etc.)
  - File path header with relative path and metric kind badge (counter/gauge/histogram)
  - Labels extracted from the metric macro displayed in the footer
  - Press `Escape` to dismiss the overlay
  - Demo shortcut: `gp` shows a preview with mock data for testing the UI
  - Native-only feature (requires codebase to be indexed via `[codebase]` config)

- **Codebase integration module**: Added a new `codebase` module for connecting the editor to git repositories and discovering metrics-rs instrumentation points. Features include:
  - `CodebaseManager` - Manages git repo lifecycle (clone, fetch, index) with async polling pattern
  - `CodebaseConfig` - Workspace config section (`[codebase]`) for specifying a git URL and optional branch
  - Tree-sitter parsing for Rust source files to find `counter!`, `gauge!`, and `histogram!` macros
  - `MetricInstrumentation` struct capturing metric name, kind, labels, file location, and line number
  - `CodebaseIndex` - In-memory index of all discovered metrics with search and lookup methods
  - Native-only feature (git/tree-sitter operations are `#[cfg(not(target_arch = "wasm32"))]`)
  - New dependencies: `gix` (pure Rust git), `tree-sitter`, `tree-sitter-rust`, `tree-sitter-highlight`, `walkdir`

- **Insta snapshot testing infrastructure**: Added the `insta` crate (v1.43) for snapshot testing. This enables easy-to-maintain tests for serialization formats and output stability. Snapshot tests are now used for:
  - Workspace TOML serialization (minimal, full, and with layout)
  - Pane config YAML serialization
  - Base64 URL encoding format stability
  - To update snapshots, run: `cargo insta test --accept` or `UPDATE_SNAPSHOTS=1 cargo test`

- **Rust 1.88 MSRV**: Updated minimum supported Rust version from 1.85 to 1.88.

- **Note on egui_kittest**: `egui_kittest` (UI snapshot testing) is ready to be enabled once egui 0.33.4 or later is released. Currently blocked by a compatibility bug between egui_kittest 0.33.3 and egui 0.33.2 (accesskit_update field mismatch in egui-winit).

- **Profiling instrumentation**: Added zero-cost profiling via the `profiling` crate. Instrumentation is always present but compiles to nothing without a backend. Two profiling backends are available:
  - `--features puffin` - Enables puffin profiler with HTTP server on port 8585 (use with `puffin_viewer`)
  - `--features tracy` - Enables tracy profiler backend (use with the Tracy profiler)
  - Instrumented locations:
    - Main render loop (`EnyaApp::update`, `show_main_content`, `draw_workspace`)
    - Workspace rendering (`Workspace::show`)
    - Query execution (`process_query_execution`)
    - Visualization rendering (`Visualization::show`, `TimeSeriesChart::show`)
    - Keyboard handling (`handle_viewport_keyboard`)
    - Overlay modals (`CommandPalette::show`, `MetricsFinder::show`)

- **Query-based visualization auto-selection**: The editor now automatically suggests an appropriate visualization type based on Prometheus query result characteristics:
  - `Scalar`/`String` results → Stat visualization
  - `Vector` results (single series) → Stat or Gauge (if percentage values)
  - `Vector` results (multiple series) → Bar Chart
  - `Matrix` results (single series, few points) → Stat/Sparkline
  - `Matrix` results (many points or series) → Time Series
  - The `cv` command continues to work for manual override, and once a user manually changes the visualization type, auto-selection is disabled for that pane

- **Comprehensive test coverage for command module**: Added 29 tests for `command.rs` covering `UICommand` variants (text, tooltips, keyboard shortcuts, icons, links), command channel (send/receive, FIFO ordering, clone, drop behavior), and the `UICommandSender` trait.

- **Extended test coverage for workspace config module**: Added 40+ new tests to `workspace/config/mod.rs` covering:
  - `LayoutConfig` (default tabs, share calculations, validation including nested containers)
  - `LayoutContainer` (share calculations, edge cases)
  - `LayoutType` (equality, serde for horizontal/vertical/tabs, nested layouts)
  - `ViewConfig` (defaults, `is_default()`, `app_theme()` with case insensitivity)
  - `TimeConfig` (defaults, `from_preset()`, `to_preset()` for all presets)
  - `ConnectionConfig` (defaults, `with_endpoint()`, `is_empty()`)
  - `PaneConfig` builder pattern (all setters, granularity/visualization parsing)
  - `WorkspaceConfig` (new, with_endpoint, add_pane, validate, error handling)
  - `WorkspaceError` display formatting for all variants
  - TOML serialization (skip_serializing_if behavior for default values)

- **Test coverage for tiles module (vim-style navigation context)**: Added 19 tests for `workspace/tiles.rs` covering:
  - `TreeBehavior` defaults, theme, and API key management
  - Focus management (set/get focused tile, focus changes)
  - Visual-multi state (active/inactive, selections, queries)
  - Filter state (active/inactive, filtered tiles, toggle)
  - Clone behavior preserves all state
  - Combined states (focus + visual-multi + filter + theme)
  - Edge cases (empty API key, long API key, unicode in queries, many tiles)

- **Keyboard-driven time range shortcuts**: Added vim-style time range presets using `t` as a leader key:
  - `t5` - Last 5 minutes
  - `t1` - Last 15 minutes (default)
  - `t3` - Last 30 minutes
  - `th` - Last 1 hour
  - `t6` - Last 6 hours
  - `td` - Last 24 hours (day)
  - `tw` - Last 7 days (week)

- **Time Range section in which-key overlay**: The `?` help overlay now includes a dedicated "Time Range" section documenting all time range keyboard shortcuts.

### Changed

- **Consolidated workspace module structure**: Reorganized the workspace-related code into a single `workspace/` module directory:
  - `Dashboard` → `Viewport` → `Workspace` (the runtime pane layout manager)
  - `Workspace` → `WorkspaceConfig` (the serialization/config struct)
  - `DashboardAction` → `ViewportAction` → `WorkspaceAction`
  - `dashboard.rs` → `viewport.rs` → `workspace/mod.rs`
  - `workspace.rs` → `workspace/config.rs`
  - This aligns internal naming with user-facing terminology where "Workspace" is the concept users interact with.

- **Centralized ID generation**: Replaced 8+ scattered `AtomicUsize`/`AtomicU64` static counters throughout the codebase with a single centralized `id_generator` module. This ensures unique IDs across all component types and eliminates duplicate ID generation patterns. The new module provides `next_id()` and `next_id_usize()` functions.

- **Reorganized components into categorized subdirectories**: Split the flat 27-file `components/` directory into four focused subdirectories:
  - `components/pane/` - Tile content types (query_pane, flamegraph, heatmap, time_series_chart, visualization)
  - `components/overlay/` - Modal UI (command_palette, metrics_finder, diagnostics, buffer_editor, info, multi_edit, tutorial, viewport_filter, which_key, workspace_finder)
  - `components/widget/` - Reusable UI elements (buffer, landing_page, notifications, status_line, time_range)
  - `components/util/` - Non-UI helpers (finder_utils, id_generator, multi_buffer, query_completion, query_executor, query_state, query_validation)
  - All types are re-exported from `components/mod.rs` for backwards compatibility.

- **Split workspace module into submodules**: Extracted independent types from `workspace/mod.rs` into focused submodules:
  - `workspace/input.rs` - Navigation direction enum (`NavDirection`) and visual multi-select state (`VisualMultiState`)
  - `workspace/tiles.rs` - `TreeBehavior` struct implementing `egui_tiles::Behavior` for pane rendering, focus borders, and filter overlays
  - `workspace/keyboard.rs` - Vim-style keyboard navigation handlers (`handle_viewport_keyboard`, `handle_visual_multi_keyboard`), navigation helpers, and visual-multi mode operations (~860 lines)
  - `workspace/serialization.rs` - Workspace save/load methods (`to_workspace_config`, `load_workspace_config`) and layout tree building/extraction (~350 lines)
  - `workspace/query.rs` - Query execution coordination (`process_query_execution`), polling for results, and triggering pane refreshes (~230 lines)
  - `workspace/overlays.rs` - Diagnostics overlay management methods (`toggle_diagnostics`, `show_diagnostics`, etc.) (~60 lines)
  - `workspace/panes.rs` - Pane management (add, close, split panes), tile tree queries, and activation (~290 lines)
  - `workspace/finders.rs` - Metrics finder and workspace finder modal methods, including demo/Prometheus metric item generation (~230 lines)
  - `workspace/rendering.rs` - Filtered view rendering, custom scrollbar, and scroll-to-focused-tile (~210 lines)
  - The main `Workspace` struct and core methods remain in `mod.rs` (~1190 lines, down from ~1940).

- **Split visualization module into submodules**: Reorganized the large `visualization.rs` file (1912 lines) into a focused `visualization/` module directory:
  - `visualization/mod.rs` - Core `VisualizationType` enum, `Visualization` wrapper enum, and common constants (~520 lines)
  - `visualization/stat.rs` - `StatChart` for big number display with sparkline and change indicators (~280 lines)
  - `visualization/gauge.rs` - `GaugeChart` for circular percentage/utilization gauges (~260 lines)
  - `visualization/bar.rs` - `Bar` and `BarChartViz` for horizontal bar charts (~240 lines)
  - `visualization/sparkline.rs` - `SparklineViz` for compact inline line charts (~200 lines)
  - `visualization/demo.rs` - Demo data population functions for all visualization types (~270 lines)
  - All types are re-exported from `pane/mod.rs` for backwards compatibility.

- **Split app module into submodules**: Reorganized the large `app.rs` file (~1485 lines) into a focused `app/` module directory:
  - `app/mod.rs` - Core `EnyaApp` struct, `eframe::App` implementation, UI command handling, and titlebar rendering (~510 lines)
  - `app/state.rs` - `AppState`, `UIState`, and `EditorMetrics` types for persisted state and frame time tracking (~95 lines)
  - `app/workspace_io.rs` - Workspace save/load/share/list operations with platform-specific implementations for native (TOML files) and WASM (base64 URL encoding) (~490 lines)

- **Split workspace config module into submodules**: Reorganized `workspace/config.rs` (~1,630 lines) into a focused `config/` module directory:
  - `config/mod.rs` - Core types (`WorkspaceConfig`, `WorkspaceMeta`, `ConnectionConfig`, `ViewConfig`, `TimeConfig`, `PaneConfig`), layout types (`LayoutConfig`, `LayoutType`, `LayoutNode`, `LayoutContainer`), `WorkspaceError` enum, and all tests (~640 lines)
  - `config/compact.rs` - Compact binary encoding for URL sharing using postcard + LZ4 compression (`CompactWorkspaceConfig`, `CompactLayout`, `CompactSinglePane`, `CompactPane`, `decode_workspace()`, `encode_workspace()`, `encode_pane()`) (~440 lines)
  - `config/templates.rs` - Default workspace TOML templates (`DEFAULT_WORKSPACE_TOML`, `COMPLEX_VIEWPORT_TOML`, `DEMO_WORKSPACE_TOML`) (~150 lines)

- **Shared overlay styling system**: Consolidated duplicate styling code across modal overlay components into shared utilities in `finder_utils.rs`:
  - `OverlayColors` - Theme-aware colors (text, muted_text, faint_text, accent, separator, elevated_bg, badge_bg)
  - `draw_separator()` / `draw_separator_colored()` - Horizontal separator lines at cursor position
  - `render_key_badge()` / `render_key_badge_large()` - Styled keyboard key badges (e.g., `Esc`, `⌘K`)
  - `draw_backdrop()` - Semi-transparent backdrop overlay for modals
  - Updated `which_key.rs`, `tutorial.rs`, `multi_edit.rs`, and `buffer_editor.rs` to use shared utilities, reducing code duplication.

- **Generic `Finder<T>` abstraction**: Created a reusable fuzzy finder component in `components/util/finder.rs` that extracts common patterns from finder modals:
  - `FinderItem` trait - Define how items are displayed and searched (`search_text()`, `icon()`, `secondary_text()`)
  - `FinderConfig` - Configuration for placeholder text, icons, preview pane, and empty state messages
  - `Finder<T>` - Generic finder with fuzzy matching via `nucleo`, keyboard navigation, match highlighting, and optional preview pane
  - `show_with_preview()` - Callback-based preview pane rendering for custom preview content
  - Refactored `MetricsFinder` and `WorkspaceFinder` to use the generic `Finder<T>`, reducing ~320 lines of duplicate code while maintaining full functionality including the metrics preview pane with tag display.

### Added

- **DemoMetricsClient for offline demo mode**: Added a new `DemoMetricsClient` in the `enya-client` crate that implements the `MetricsClient` trait with realistic mock data. The demo client provides:
  - A catalog of ~25 realistic Prometheus metrics (system, HTTP, Tokio runtime, application, database)
  - Proper label dimensions for each metric (host, env, method, status_code, pool, etc.)
  - Time-series data generation with appropriate patterns for counters, gauges, and histograms
  - Full metadata API support (metric names, label names, per-metric labels)

- **Viewport filter (`/` search)**: Added vim-style `/` search to filter visible panes by query content. Press `/` to open the filter input, type a search pattern, and press Enter to apply. Non-matching panes are dimmed with a "filtered" overlay. The filter status is shown in the status line. Press `/` again to edit the filter, or press Escape twice to clear it.

- **Interactive tutorial overlay**: Added a new `:tutorial` command that opens a step-by-step walkthrough of the editor's features. The tutorial covers navigation, editing, splits, visual multi-select, metrics finder, time range controls, workspaces, and more. Navigate with arrow keys or h/l, press number keys (1-9) to jump to specific steps.

- **PromQL as default query language**: The editor now defaults to PromQL for query input, with full context-aware autocompletion for PromQL syntax including functions, aggregations, label selectors, duration literals, and modifiers.

- **Dual-language support**: Added `QueryLanguage` enum supporting both PromQL (default) and EnyaLang modes. Language can be toggled via the `set_language()` method on `QueryCompletion`.

- **PromQL validation for inline diagnostics**: `QueryValidator` now supports dual-language validation. PromQL queries are validated using `enya-promql::validate()` which wraps the `promql-parser` crate. Syntax errors are displayed as inline diagnostics in the query editor.

- **New enya-promql crate**: Created a dedicated crate for PromQL parsing and autocompletion with:
  - Context-aware completion analysis (`analyze()`)
  - Syntax suggestions for each context (`syntax_suggestions()`)
  - Query validation using the `promql-parser` crate
  - Lightweight character-based scanner for nesting depth tracking

- **Per-metric label fetching from Prometheus**: When connected to Prometheus, the editor now fetches label names and values for each metric individually via the `/api/v1/series` endpoint.

- **Connection health check validation**: When connecting to a Prometheus endpoint via `:connect`, the editor now validates connectivity by calling `/api/v1/status/buildinfo`. The status line shows "ONLINE" only after successful health check, with the Prometheus version displayed in a diagnostic message. Connection failures show an error diagnostic.

- **Dynamic autocompletion**: The query editor's inline autocompletion now uses real label data fetched from the backend instead of hardcoded demo values. Labels are fetched automatically when opening the buffer editor or metrics finder.

- **Backend-agnostic label interface**: Added `fetch_metric_labels()` to the `MetricsClient` trait, allowing any backend to provide per-metric label data for autocompletion.

- **Label caching**: Fetched labels are cached per metric to avoid redundant API calls. Cache is cleared on disconnect.

### Changed

- **Demo workspace uses realistic PromQL queries**: Updated `DEMO_WORKSPACE_TOML` to use proper PromQL expressions that produce beautiful visualizations:
  - `sum(rate(http_requests_total[5m])) by (method)` - HTTP request rate grouped by method
  - `sum(db_connections_active) by (pool)` - Database connections aggregated by pool
  - `histogram_quantile(0.99, rate(http_request_duration_seconds[5m]))` - Request latency p99
  - `sum(app_queue_depth) by (queue)` - Queue depth aggregated by queue name

- **Demo mode uses async client pattern**: Demo mode now uses the same async query flow as Prometheus connections via `DemoMetricsClient`, enabling metadata fetching (metric names, labels) in offline mode.

- **Query pane naming**: Query panes now use sequential "Query N" naming (Query 1, Query 2, etc.) per workspace instead of using the initial metric name. This prevents confusion when users change the query to use different metrics. The counter resets to 1 when loading a new workspace.

- **Metrics finder preview**: Now shows actual label names and values for Prometheus metrics instead of placeholder dots. Labels are fetched on-demand when a metric is selected.

- **Buffer editor completions**: Completions are populated from cached metric labels when opening the editor. If connected but no labels are cached, hardcoded defaults are cleared and a fetch is triggered.

- **Time series chart default height**: Charts now use a sleek Grafana/PlanetScale-style default aspect ratio (0.35 height:width) with a minimum height of 180px, providing a polished default view while still allowing zoom.

- **Pane separators**: Added subtle visual separators (4px gap with 1px stroke) between panes in split layouts. The separator line changes color on hover/drag for better resize affordance.

- **Tab bar styling**: Improved pane header/tab bar appearance with theme-aware colors. Active tabs have elevated background, inactive tabs blend with the surface, and a subtle separator line divides the tab bar from content.

- **Active tab emerald border**: Active tabs now have an emerald border to match the "glass obsidian emerald" theme and improve visibility.

- **Chart Y-axis formatting**: Large values on the Y-axis now display with K/M/B suffixes (e.g., 1.5K, 2.3M) for improved readability.

- **Softer grid lines**: Chart grid lines are now more transparent (40% opacity) for a cleaner, less cluttered appearance.

- **Improved empty state styling**: The "No data to display" state now features a branded design with a subtle circular background, dimmed icon, and helpful "Run a query to see results" hint.

- **Loading state animation**: Query panes now show an animated emerald loading bar while queries are in flight, providing visual feedback during data fetching.

- **Consistent visualization spacing**: Standardized padding (16px top/bottom) across all visualization types (StatChart, GaugeChart, BarChartViz, SparklineViz) for a more uniform appearance.

### Fixed

- **Workspace visualization type loading**: Fixed `load_workspace()` to apply the visualization type from pane config. Previously, all panes would default to time series regardless of the `visualization` field in the workspace file.

- **Command palette centering**: The command palette now always opens centered on screen.

- **Notifications positioning**: Added top padding to prevent notifications from overlapping with the title bar.

- **WASM time handling**: Fixed `TimeRange::now()` and Prometheus client to use `web_time::SystemTime` on WASM instead of `std::time::SystemTime`, which panics in browsers.

- **Empty chart message centering**: The "No data to display" message in charts is now centered.

- **Metric name completion on first open**: Fixed autocompletion not suggesting metric names on the first time entering the buffer editor. The issue was that typing partial queries (e.g., "rate(") would trigger label fetches for those partial strings, and empty responses would clear all completions. Now the original metric name is preserved and used for label fetching.

- **Completion popup width**: Increased completion popup width from 400px to 500-600px to accommodate long metric names. Added truncation for labels over 50 characters to prevent overlap with kind badges.

- **Command palette Tab completion focus**: Fixed Tab completion in the command palette losing focus. Pressing Tab to complete a command like `:c` → `:connect ` now keeps the cursor in the input field so you can continue typing the endpoint.

- **Landing page "Recent Queries"**: Renamed "Recent Plots" to "Recent Queries" on the landing page. Now shows the query pane name (e.g., "Query 1") instead of just the metric name. Long names are automatically truncated with ellipsis to prevent overflow into the workspaces column.
