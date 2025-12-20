# Changelog

All notable changes to the Enya editor will be documented in this file.

## [Unreleased]

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
  - `app/screenshot.rs` - Screenshot capture and saving, with browser download support on WASM (~150 lines)
  - `app/fonts.rs` - Font configuration for DepartureMono and Nerd Fonts icons (~35 lines)
  - All types remain re-exported from `app/mod.rs` for backwards compatibility.

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
