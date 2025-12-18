# Changelog

All notable changes to the Enya editor will be documented in this file.

## [Unreleased]

### Added

- **PromQL as default query language**: The editor now defaults to PromQL for query input, with full context-aware autocompletion for PromQL syntax including functions, aggregations, label selectors, duration literals, and modifiers.

- **Dual-language support**: Added `QueryLanguage` enum supporting both PromQL (default) and EnyaLang modes. Language can be toggled via the `set_language()` method on `QueryCompletion`.

- **PromQL validation for inline diagnostics**: `QueryValidator` now supports dual-language validation. PromQL queries are validated using `enya-promql::validate()` which wraps the `promql-parser` crate. Syntax errors are displayed as inline diagnostics in the query editor.

- **New enya-promql crate**: Created a dedicated crate for PromQL parsing and autocompletion with:
  - Context-aware completion analysis (`analyze()`)
  - Syntax suggestions for each context (`syntax_suggestions()`)
  - Query validation using the `promql-parser` crate
  - Lightweight character-based scanner for nesting depth tracking

- **Per-metric label fetching from Prometheus**: When connected to Prometheus, the editor now fetches label names and values for each metric individually via the `/api/v1/series` endpoint.

- **Dynamic autocompletion**: The query editor's inline autocompletion now uses real label data fetched from the backend instead of hardcoded demo values. Labels are fetched automatically when opening the buffer editor or metrics finder.

- **Backend-agnostic label interface**: Added `fetch_metric_labels()` to the `MetricsClient` trait, allowing any backend to provide per-metric label data for autocompletion.

- **Label caching**: Fetched labels are cached per metric to avoid redundant API calls. Cache is cleared on disconnect.

### Changed

- **Query pane naming**: Query panes now use sequential "Query N" naming (Query 1, Query 2, etc.) per workspace instead of using the initial metric name. This prevents confusion when users change the query to use different metrics. The counter resets to 1 when loading a new workspace.

- **Metrics finder preview**: Now shows actual label names and values for Prometheus metrics instead of placeholder dots. Labels are fetched on-demand when a metric is selected.

- **Buffer editor completions**: Completions are populated from cached metric labels when opening the editor. If connected but no labels are cached, hardcoded defaults are cleared and a fetch is triggered.

### Fixed

- **Command palette centering**: The command palette now always opens centered on screen.

- **Notifications positioning**: Added top padding to prevent notifications from overlapping with the title bar.

- **WASM time handling**: Fixed `TimeRange::now()` and Prometheus client to use `web_time::SystemTime` on WASM instead of `std::time::SystemTime`, which panics in browsers.

- **Empty chart message centering**: The "No data to display" message in charts is now centered.

- **Metric name completion on first open**: Fixed autocompletion not suggesting metric names on the first time entering the buffer editor. The issue was that typing partial queries (e.g., "rate(") would trigger label fetches for those partial strings, and empty responses would clear all completions. Now the original metric name is preserved and used for label fetching.

- **Completion popup width**: Increased completion popup width from 400px to 500-600px to accommodate long metric names. Added truncation for labels over 50 characters to prevent overlap with kind badges.
