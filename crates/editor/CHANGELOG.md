# Changelog

All notable changes to the Enya editor will be documented in this file.

## [Unreleased]

### Added

- **Floating comment gutter**: PR review comments now render in a floating gutter to the right of the diff instead of breaking the code flow inline. Comments are anchored to their source line with subtle connector lines, and displaced cards (when comments are on adjacent lines) push downward with visual anchor lines back to the source. The anchor line in the diff gets a subtle accent highlight. Falls back to the original inline layout when the pane is narrower than 700px or in split-view mode.

### Fixed

- **`gg` scroll-to-top broken in PR review pane**: The workspace's `g` leader key handler (for `gd`/`ga`/`gp`) was consuming the `g` key press before the PR review pane's diff renderer could use it for `gg`. Now skipped when the focused pane handles its own navigation.
- **Mouse wheel scrolling broken in PR diff view**: The `ScrollArea` scroll offset was set each frame but never read back from egui's output, so mouse wheel input was overwritten on the next frame. Both unified and split views now sync the scroll offset back.
- **PR file tree squashed with long filenames**: The file panel had a fixed 220px width, leaving little room for filenames when combined with indent and stats. Now uses 28% of pane width, clamped between 180–320px.
- **`n`/`p` file navigation out of order across folders**: `file_diffs` followed the API's diff output order while the file tree was sorted alphabetically by path, so `n`/`p` would jump to unexpected files rather than following the visible tree order. Now sorts `file_diffs` by path to match the tree.

## [0.1.3] - 2026-04-01

### Fixed

- **File tree stats overlapping filenames**: In the PR review pane file tree, the comment icon and +/- stats no longer overlap long filenames — the filename now truncates based on the actual width of the right-side stats.
- **`ct` theme shortcut blocked by PR review pane**: Workspace leader key sequences (like `ct` to cycle theme) now run before pane-level keyboard handlers, so the diff renderer's `c` shortcut no longer steals the key press.
- **PR review pane keyboard navigation stolen by workspace**: The workspace's h/j/k/l tile-navigation handler consumed these keys before the PR pane could use them for list scrolling, diff navigation, and back navigation. Added `handles_own_navigation()` to the `Component` trait so panes can opt out of workspace-level hjkl consumption.

### Added

- **PR description banner**: When opening a PR, a collapsible description banner appears below the tab bar (visible on all tabs). Shows the markdown-rendered PR body truncated to a few lines with a fade-out gradient, expandable to the full description. Click the header to collapse entirely. Gives immediate context without switching to the Conversation tab.
- **Unread comment indicators**: File tree rows show an accent-colored dot when a file has unseen review comments. Comments are marked as "seen" when the user views the file.
- **Collapsible file tree panel**: The file panel in the Files tab can be collapsed to a thin chevron strip, giving the diff view full width. Click the chevron to expand again.
- **Copy hint on selection**: When diff lines are selected, a floating "⌘C copy N lines" hint badge appears to make the copy interaction discoverable.
- **Hunk flash animation**: Jumping to a hunk via `{`/`}` now briefly flashes the target hunk header with the accent color, making it easy to spot the landing position.
- **Enhanced word-level diff highlights**: Boosted contrast of inline word-change backgrounds across all themes (especially dark themes) and added a thin underline to changed words for a stronger visual cue when scanning modified lines.

### Changed

- **Light theme diff palettes overhauled**: Reworked diff colors for all four light themes (Parchment, Stockholm, Copenhagen, Light). Line backgrounds are now softer and more muted (GitHub-style washes instead of highlighter-pen saturation). Parchment gets a fully warm-shifted palette — cream-green additions, cream-rose deletions, warm brown hunk headers — matching its paper aesthetic. Light gets its own distinct neutral-cool palette instead of sharing Parchment's colors. Gutter stripes are muted across all light themes for less visual noise.
- **Dark theme diff identity**: Void now uses violet-tinted diff colors (lavender-teal additions, lavender-pink deletions) matching its purple aesthetic. Neon uses magenta-tinted colors (cyan-green additions, hot pink deletions). Graphite gets warm earthy tones (olive-green, orange-red). Ayu shifts to amber/teal. Aurora gets pink-shifted reds and cyan-shifted greens. Each dark theme now has a distinct diff personality.
- **Theme-aware search highlights**: Replaced hardcoded orange/gold search highlight colors with per-theme values. Void uses purple highlights, Neon uses magenta, and all themes get properly matched search colors instead of a one-size-fits-all gold.

## [0.1.2] - 2026-03-24

### Fixed

- **Stale review status when switching PRs**: Opening a different PR in the review pane no longer carries over the "Review submitted successfully" message, draft comments, comment input, or other per-review state from the previous PR.
- **PR list selection preserved on refresh**: Refreshing the PR list now preserves the selected PR by number instead of resetting to the top.

### Changed

- **Approve with optional message**: The Approve button now opens a dropdown popup with an optional message field, allowing reviewers to include a comment when approving a PR.
- **Shared DiffRenderer**: Unified diff rendering between the overlay and PR review pane into a shared `DiffRenderer` struct. The PR pane now gains search (`/`/`⌘F`), hunk jumping (`{`/`}`), line selection (click line numbers), context expansion (click hunk headers), and `⌘C` copy — features previously only available in the commit diff overlay. The overlay shed ~1200 lines of code.
- **Threaded inline review comments**: Review comments now appear as threaded conversations directly in the Files tab diff, similar to GitHub. Comments on the same line are grouped into threads with avatar placeholders, reply buttons, and collapsible "show N more replies" for long threads. The Conversation tab now shows only PR-level discussion.
- **"+" comment button on hover**: Hovering over a diff line shows a "+" icon in the gutter. Clicking it opens the comment input inline at that line.
- **Per-file comment count badges**: The file sidebar shows comment counts next to each file with review or draft comments.

### Added

- **PR list refresh button**: The PR list header now includes a clickable refresh button with a spinner during loading, and a styled Retry button on error states.
- **"Ready to merge" badge**: PRs in the list view show a green "Ready to merge" badge when all checks pass, the PR is approved, and it's mergeable with no conflicts.
- **Enhanced PR description display**: The Conversation tab now renders the PR description as a distinct "Description" section with an icon header, separate from regular comment bubbles.
- **Open PR in GitHub**: Added an external-link button next to the PR number in the detail view header that opens the pull request in the browser.
- **Copy file contents**: Added a copy button in the diff file header that copies the full new-side file contents to the clipboard.
- **GitHub avatar images**: Comment threads now display real GitHub profile avatars (fetched asynchronously) instead of letter-in-circle placeholders. Falls back to the letter placeholder while loading or if the fetch fails.
- **PR review state badges**: The PR list now shows "Approved" (green) or "Changes requested" (red) badges on PRs that have been reviewed, fetched during preloading.
- **Syntax-highlighted diffs**: Diff viewer now shows language-aware syntax colors (keywords, strings, types, etc.) layered under diff backgrounds using tree-sitter, with WASM fallback to flat colors.
- **Collapsible hunk separators**: Hunk headers replaced with styled separators showing hidden line count and function context (e.g. "··· 42 lines hidden ··· fn foo()").
- **Hunk-to-hunk navigation**: `{` / `}` keys jump directly between changed hunks instead of scrolling line by line.
- **Line selection and copy**: Click line numbers to select lines (shift+click for range), `⌘C` to copy selected content. Selection cleared with `Esc`.
- **Search within diff**: Press `/` or `⌘F` to open an inline search bar. Case-insensitive matching across all files with `Enter`/`Shift+Enter` to cycle through matches. Current match highlighted in orange, other matches in yellow. Automatically scrolls to match and switches files.
- **Expand context on demand**: Click hunk separators to reveal up to 20 additional lines of surrounding context from the full file. Expands incrementally with each click.
- **TOML syntax highlighting**: Added tree-sitter TOML grammar for syntax-highlighted TOML diffs.
- **All language grammars enabled by default**: Syntax highlighting for Rust, Go, Python, JavaScript/TypeScript, and TOML now ships out of the box.
- **`:review` command**: Open the PR review pane from the command palette with `:review` (aliases: `:pr`, `:pulls`). Accepts an optional `owner/repo` argument, falls back to the current workspace repo.
- **Git credential auth for PR review**: PR review pane now reads GitHub tokens from `git credential fill` (picks up `gh` CLI, macOS Keychain, Git Credential Manager), enabling access to org repos without extra OAuth grants. Falls back to the existing OAuth token.
- **PR Review pane**: New `PrReviewPane` component for reviewing GitHub pull requests directly in Enya. Features include:
  - List open PRs with status dots, author, draft badges, and relative timestamps
  - Detail view with Files, Conversation, and Checks tabs
  - Per-file unified and split diff rendering with word-level highlights
  - Inline commenting with draft accumulation and batch submission
  - Review bar with Approve, Request Changes, and Comment actions
  - Full AI agent integration via `open_pr_review`, `review_pr`, `add_pr_comment`, and `submit_pr_review` commands
  - Workspace serialization/deserialization support (`visualization: "pr_review"`)
  - WASM-compatible GitHub API client with proxy support
  - Vim-style keyboard navigation: j/k to move, Enter/l to open, Escape/h to go back, 1/2/3 to switch tabs, r to refresh, g/G to jump
  - Focus-aware key handling — pane only captures keys when focused, Escape drills out naturally
  - Preloads data for top 10 PRs in the background for instant navigation

### Fixed

- **Keyboard commands after closing diff viewer**: Fixed a bug where vim-style navigation (space+f, ?, etc.) stopped working after closing the diff viewer because the search TextEdit's focus lingered in egui memory.

## [0.1.1] - 2026-03-18

### Changed

- **Project-based workspace storage**: Workspaces are now stored under project directories (`~/.enya/projects/{project}/workspaces/`) instead of a flat `~/.enya/workspaces/` directory. Every workspace belongs to a project. Conversations are similarly scoped to `~/.enya/projects/{project}/conversations/{workspace}/`. Projects are discovered from the filesystem — no separate config needed.

### Added

- **Project deletion**: Hover a project header in the sidebar to reveal a delete button. Confirms with a y/n dialog before removing the project directory and all its workspaces and conversations.

### Fixed

- **Sidebar workspace ordering**: Workspaces within a project are now sorted alphabetically so loading a workspace no longer shuffles the sidebar list.
- **WASM tutorial visibility**: Tutorial workspaces now correctly appear under the "Tutorial" project header on WASM.
- **Project collapse toggle lag**: Toggling a project's collapsed state in the sidebar is now instant instead of delayed by the 2-second filesystem scan cooldown.

### Added

- **Pie chart visualization**: New donut-style pie chart for proportional data with interactive hover, segment explosion, center total/detail display, and a scrollable legend. Available via the `cv` keybinding cycle or `set_visualization` command with `"pie_chart"` / `"pie"` / `"donut"`.

### Fixed

- **macOS GUI app PATH resolution**: Fixed `npx`, `git`, and other developer commands failing when Enya.app is launched from Finder, Dock, or Spotlight. The app now resolves the user's login shell PATH at startup.
- **Gauge/Stat/Bar/Sparkline visualizations showing demo data**: Non-time-series visualizations now display real query data instead of demo placeholder values. Previously, `populate_from_response` only handled time series; gauge/stat/bar/sparkline types were stuck showing demo data even with a real backend connected.
- **OTLP JSON string-encoded integers**: Fixed deserialization of OTLP JSON payloads where 64-bit integers are string-encoded per the protobuf JSON spec (e.g. `"42"` instead of `42`).
- **OTLP query routing using wrong field**: Fixed queries using pane display name instead of the actual metric name, causing "No data" results for OTLP metrics.
- **Keybindings stuck after agent panel interaction**: Fixed keyboard shortcuts (Space+h, Space+f, aa, etc.) becoming unresponsive after clicking on a viewport pane while the agent panel had focus. Clicking the viewport now correctly transfers focus back from the agent panel.
- **Codebase indexing per project**: Switching between projects now correctly resets the codebase manager, preventing the wrong repository from being indexed. Previously, loading a workspace from a different project could show or re-index the previous project's repo.

### Added

- **OTLP Settings UI**: Added OTLP Receiver section to Settings → Connections showing the receiver endpoint (e.g. `localhost:4318`) with configurable port that takes effect on next launch.
- **OTLP metrics ingestion**: The Enya agent now accepts OpenTelemetry metrics via `POST /v1/metrics` (OTLP HTTP JSON and protobuf), in addition to existing traces and logs support. Gauge, Sum (counter), and Histogram metrics are stored in-memory and queryable through the editor's existing query panes.
- **Embedded OTLP receiver**: The native editor now starts an embedded OTLP HTTP receiver on `localhost:4318` (the standard OTLP port). Developers can point their OTel SDK at Enya (`OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318`) to view metrics, logs, and traces without running separate Prometheus/Loki/Tempo infrastructure or the Enya agent.
- **OTLP as supplementary data source**: OTLP metrics are now available alongside Prometheus as an additional data source. When both are active, OTLP metric names appear in autocomplete and queries for OTLP-sourced metrics are automatically routed to the embedded receiver.
- **OTLP protobuf support**: All three OTLP signal types (traces, logs, metrics) now accept both JSON (`application/json`) and protobuf (`application/x-protobuf`) wire formats, auto-detected from the Content-Type header.
- **SQL pane Flight SQL benchmarking**: The `/bench` command now works over Flight SQL connections with per-phase timing breakdown matching dft's Flight SQL benchmarks: Get Flight Info, Time to First Byte (TTFB), Do Get, and Total. Phase labels adapt automatically for Flight vs local backends.

### Changed

- **Unified tutorial workspaces**: Native tutorial workspaces now use the same names as WASM (`quick-start`, `infra`, `logs-and-traces`). Old tutorial files (`golden-signals`, `infrastructure`, `multi-service`) are automatically cleaned up.
- **SQL pane premium header bar**: Added an accent-tinted header bar matching the LogsPane design, with database icon, "SQL" title, inline mode badges (DIFF/EXPLAIN), connection status indicator, and result count badge. Includes a 1px accent line separator.
- **SQL pane loading shimmer**: Query execution now shows a table-shaped skeleton loading animation with sweeping shimmer gradient instead of just a spinner.
- **SQL pane scroll shadows**: Added VS Code-style fade gradients at the top/bottom of the result scroll area.
- **SQL pane table hover rows**: Data rows highlight with a subtle accent tint on hover for better visual feedback.
- **SQL pane numeric column alignment**: Numeric columns (integers, floats, decimals) are now right-aligned in result tables.
- **SQL pane sort indicators**: Column sort arrows are now rendered separately in accent color at the right edge. Unsorted columns show a ghost arrow on hover to hint at clickability.
- **SQL pane cell click-to-copy**: Click any cell value in the result table to copy it to the clipboard.
- **SQL pane truncation tooltips**: Hover over truncated cell values to see the full content in a tooltip.
- **SQL pane row gutter border**: Added a subtle right border to the row number gutter for visual separation.
- **SQL pane card header visibility**: Close and collapse icons are now more visible (increased opacity from 0.3/0.6 to 0.5/0.7).
- **SQL pane empty state polish**: Improved empty state with icon glow frame, larger icon, and better text hierarchy.
- **SQL pane layout constants**: Extracted magic numbers into named constants for consistent styling.
- **SQL pane fullscreen overlay feature parity**: The fullscreen table overlay now has hover row highlighting, cell click-to-copy, truncation tooltips, numeric column right-alignment, accent-colored sort indicators with ghost arrows on hover, and a gutter right border — matching the inline table interactions.
- **SQL pane suggestions popup polish**: The autocomplete suggestions popup now has a drop shadow, hover row highlighting, and a pointing hand cursor on suggestion rows.
- **SQL pane run button visual weight**: The run button (↵) now shows an accent-tinted fill and border when a query is ready to execute, making it visually prominent instead of nearly invisible.
- **SQL pane connection popup cursor feedback**: Connection rows, table names, and "Manage in Settings" links in the connection popup and sidebar tree now show a pointing hand cursor on hover.

### Added

- **Snapshot support for TracingPane and LogsPane**: Trace and log panes are now included in snapshots. Previously they were silently skipped when sharing snapshots.
- **Snapshot trait methods on Component**: Added `extract_snapshot_data()`, `load_snapshot_data()`, and `to_pane_config()` as default methods on the `Component` trait, making snapshot support an explicit contract for new pane types.
- **macOS window vibrancy**: The custom titlebar now has a subtle translucent vibrancy effect on macOS, allowing the desktop to gently show through. Uses `NSVisualEffectView` with the colorless Selection material (same approach as Zed) for a neutral blur that works with all themes.

### Changed

- **Connection errors in diagnostics overlay**: Prometheus and agent endpoint connection failures now appear in the diagnostics overlay instead of as toast notifications. This makes errors persistent and easier to find rather than auto-dismissing after 4 seconds.
- **Centralized data directory**: All Enya data now lives under `~/.enya/`. User plugins moved from `~/.config/enya/plugins/` to `~/.enya/plugins/`. Search indexes moved from `{repo}/.enya/tantivy/` to `~/.enya/indexes/`. AI conversations moved from `{cwd}/.enya/conversations/` to `~/.enya/conversations/`.
- **Reduced logging verbosity**: Downgraded ~120 `log::info!` calls to `log::debug!` across the editor crate. Internal details like agent command execution, keyboard handler traces, pane operations, finder selections, query executor internals, and indexing progress are now debug-level. User-facing events (connections, plugin loads, workspace save/load, updates, screenshots) remain at info level.
- **Skip landing page for returning users**: When the user has any projects, the app now auto-restores the last workspace on startup instead of showing the landing page. The landing page only appears for first-time users with no projects.
- **Streamlined tutorial**: Reduced from 24 to 15 steps (web). Removed advanced pane management steps (Move Panes, Merge Into Tabs, Floating Panes, Visual Multi-Select, Workspace Undo). Merged time navigation into one step and AI Agent Setup into Ask the AI Agent. Added a "Project Sidebar" step (`Space+b`) and expanded "Find Anything" with prefix-based fuzzy search (`@` metrics, `!` alerts, `#` commits). Colorscheme step now uses `ct` (cycle themes) and `:settings` instead of the style picker.

### Removed

- **Style picker overlay**: Removed the standalone style picker overlay (`:style` command) since the Settings page already provides the same theme and font selection UI. Use `:settings` or `ct` to change themes.
- **GitHub sign-in is desktop only**: Disabled GitHub OAuth sign-in on WASM since the OAuth App only supports one callback URL (`127.0.0.1` for native). The Connect button now shows "Desktop only" on the web editor.

### Fixed

- **WASM keybindings getting stuck during tutorial**: Fixed keybindings (Space+h, vim navigation, etc.) becoming unresponsive when navigating the tutorial on WASM. Three issues contributed: (1) browser canvas focus loss left leader key state permanently active (Space has no timeout by design), (2) the `/` and `?` handlers could open overlays with text inputs behind the tutorial, stealing egui focus and blocking all vim keybindings, (3) leader key state was never cleared when modal overlays blocked keyboard handling. Now leader keys are cleared on window focus loss, when any modal blocks keyboard handling, and the `/`/`?` handlers are guarded by `tutorial_overlay.is_open()`.
- **Responsive overlay sizing**: All modal overlays now use sidebar-aware sizing utilities (`overlay_width`, `overlay_height`, `overlay_max_height`) that automatically reduce minimum dimensions on smaller screens. Fixes overlays overflowing on WASM at 1.5× zoom on laptop-sized browser viewports. Also removes deprecated `ctx.screen_rect()` usage in buffer editor, multi-edit, and codebase finder overlays. Extended responsive sizing to the generic finder, SQL result overlay, settings page (sidebar width, content max-width, theme/font panel height), floating pane defaults, and logs query history popup. Redesigned the which-key overlay (`?`) with a tabbed category layout (Navigate, Edit, Go To, Agent) so it stays compact on smaller screens instead of showing all groups at once.
- **macOS auto-update**: The "Restart" button in the update banner now downloads the signed DMG, mounts it, copies the full `.app` bundle using `ditto` (preserving code signatures and notarization), atomically swaps it into place, and restarts. Previously the asset matcher searched for architecture substrings absent from the `Enya.dmg` filename, so macOS users always fell back to "Download" (opening the browser). Stale `.app.old` bundles from previous updates are cleaned up on startup.
- **Tracing pane no longer auto-focuses input**: The trace ID input field no longer grabs focus when the tracing pane is first activated, matching the behavior of other panes.
- **SQL pane spinner theming**: Loading spinners in the SQL pane now use the active theme's accent color instead of the default black.
- **SQL pane premature "no results" message**: Fixed "Query returned no results" appearing while a query was still running; the message now only shows after the query completes.
- **SQL pane keyboard conflicts with input bar**: The result card no longer consumes keyboard events (tab, arrow keys, etc.) when the SQL input bar has focus.
- **SQL table overlay keyboard navigation**: The h/l keys and Escape now work correctly in the fullscreen table overlay instead of being consumed by the result card underneath.

### Added

- **SQL pane auto-connect**: When a SQL pane is opened and Flight SQL connections are configured in Settings, the pane automatically connects to the first endpoint in the list instead of requiring manual connection.
- **SQL pane expand results button**: Added an expand button in the query result stats bar that opens the fullscreen table overlay for easier browsing of large result sets.
- **SQL pane `/bench` command**: Benchmark query execution by running it N times (default 10) and displaying a styled phase timing table with min/median/mean/max breakdown for logical planning, physical planning, and execution phases. Benchmark results are fully snapshotable (serialized with microsecond precision). Uses datafusion-dft's benchmarking engine via a vendored copy. Local DataFusion sessions only. Usage: `/bench [iterations] <query>`.
- **SQL pane `/describe` command**: Show per-column statistics for a table including count, null count, distinct count, min, max, and mean (numeric columns only). Generates a single dynamic aggregate SQL query for efficient computation. Results displayed in a styled table with stats bar. Fully snapshotable. Local DataFusion sessions only. Usage: `/describe <table>`.
- **SQL pane query cancellation**: Running queries can now be cancelled via a cancel button in the card header or by pressing Escape. Works for both local DataFusion and Flight SQL backends. Local queries are cancelled at the next batch boundary; Flight SQL tasks are aborted immediately.
- **SQL pane adaptive table height**: Result tables now use available vertical space (up to 600px) instead of a fixed 400px height, showing more rows without scrolling.
- **SQL pane row count indicator**: Query result footers now show "Rows 1–50 of 1,234" with clickable prev/next page buttons alongside existing keyboard navigation.
- **Recent commits in fuzzy finder**: Typing `#` in the fuzzy finder now shows a list of the most recent commits sorted by timestamp (newest first), without requiring a search query. Typing further filters the commits as before.
- **Theme cycle keybinding**: Added `ct` keybinding to cycle through app themes.
- **Mobile browser detection**: On WASM, mobile browsers see the landing page but are blocked from entering the editor with a notification saying "Enya is designed for desktop".
- **OTLP as a datasource protocol**: Added support for OpenTelemetry Protocol (OTLP) as a datasource backend, letting Enya receive telemetry data directly from OTel SDKs without requiring Grafana stack infrastructure. The agent daemon accepts OTLP JSON payloads at `/v1/traces` and `/v1/logs`, stores them in memory, and serves them via HTTP query endpoints (`/api/otlp/traces/search`, `/api/otlp/traces/{id}`, `/api/otlp/logs/query`, `/api/otlp/labels`, `/api/otlp/health`). The editor can query these endpoints by setting `backend = "otlp"` with an `endpoint` URL in `[logs]` and `[tracing]` workspace config sections. Tracing panes now load traces from configured backends (Tempo or OTLP) when a trace ID is entered.
- **Tempo endpoint in Settings**: Added a Tempo trace endpoint configuration field to both the Settings overlay and full-page Settings, alongside the existing Prometheus and Loki fields. Also added `TracingConfig` to workspace TOML configuration.
- **Series filter dropdown for time series charts**: When a chart has 6+ series, a filter icon appears in the legend bar. Clicking it (or pressing `gs`) opens a searchable dropdown popup where users can toggle individual series on/off. Features include fuzzy search filtering, All/None quick toggles, keyboard navigation (arrows + Tab to toggle), and stable color assignment. Hidden series are excluded from both the chart and legend, and filter state persists across data refreshes.

### Changed

- **SQL pane cell abstraction**: Replaced the monolithic `QueryCell` struct with a proper enum-based `Cell` type system. Cells now use a `CellKind` enum (`Query`, `Info`, `Diff`, `Explain`) carrying only variant-specific data, eliminating meaningless `Option` fields and making the type system enforce valid states at compile time.
- **Snapshot-friendly SQL cell kinds**: The snapshot format is now cell-kind-aware, so all SQL notebook content — queries, info messages, diff comparisons, and explain plans — round-trips through save/restore. Shared immutable snapshots now display the full notebook workflow in read-only mode, even without a SQL connection. This is a breaking change to the snapshot binary format; previously saved snapshots are not compatible.
- **Single-result-cell SQL pane**: Replaced the scrolling multi-cell notebook with a single result cell that updates in place when a new query runs. Info and error messages now appear as transient status banners between the result and input bar rather than accumulating as cells. Multi-cell vim navigation (j/k/gg/G) removed; users who need multiple simultaneous results can open additional SQL panes.
- **Streamlined WASM tutorial**: Removed the on-call and deep-dive tutorial workspaces to simplify the demo. Replaced the Latency time series in the quick-start workspace with a Latency Heatmap.
- **Hide workspace name in agent mode**: The workspace name segment in the status line is now hidden when agent mode is active, freeing up space for the inline agent input bar.
- **Neovim tilde markers flush left**: Removed 16px left margin on `~` markers in the empty workspace view so they sit at the very left edge.
- **Leader popup only for Space**: Removed the leader key popup overlay for `g` commands; the which-key popup now only appears for Space leader actions.
- **Sidebar empty state text**: Changed "No workspaces yet" to "No projects yet" in the project sidebar empty state.
- **Show Tutorial project on WASM**: The Tutorial project is now always visible in the sidebar on WASM builds, since tutorials are the primary content for web users.
- **Simplified tutorial workspaces**: Reduced each tutorial workspace from 7-10 panes to 4 panes and added explicit 2x2 grid layouts so all panes are visible in the viewport instead of hidden behind tabs.
- **WASM sidebar shows only tutorial workspaces**: On WASM, the project sidebar now hides ungrouped workspaces, the "Add project" button, the "+" workspace creation button on project headers, and the archive/delete button on workspace rows. Only the pre-defined Tutorial project workspaces are shown for a clean demo experience.
- **Friendlier tutorial workspace names**: Renamed tutorial workspaces to more approachable names: quick-start, on-call, deep-dive, infra, and logs-and-traces.
- **Logs & Traces tutorial workspace**: Replaced the multi-service comparison workspace with a new logs-and-traces workspace that showcases the logs pane (demo SQL query logs) and tracing pane (demo distributed trace) alongside metric panes.

### Fixed

- **Tutorial workspace demo data**: Workspaces loaded without a backend connection (e.g. tutorials) now show realistic demo data in all pane types. Previously, `from_config_numbered` never called `populate_demo_data`, and `set_visualization_type` only populated demo data when the type differed from the default (TimeSeries), leaving TimeSeries panes empty.
- **Gauge percentage text clipping**: Fixed the percentage value (e.g. "54%") being cut off at the bottom of gauge panes by correcting the height overhead constant from 120 to 166 in the gauge sizing calculation.

- **Responsive time range toolbar**: Preset buttons, custom range label, and range description now progressively hide as the toolbar narrows to prevent overlapping text.
- **Agent panel copy button**: Copy button now appears inline to the right of the message header instead of on a separate row below it.

### Removed

- **Workspace finder overlay**: Removed the workspace finder overlay (`Space+W`) and its entry from the Space leader popup. Workspace switching is available through the project sidebar.
- **Command palette cleanup**: Removed `:provider`/`:ai`, `:share-live`, `:open-snapshot`, `:refresh`, `:source`, and `:loki` commands. Renamed `:info` to `:version`.
- **Sections feature**: Removed the collapsible sections system (section headers, section-based navigation, section rendering). Workspaces now always use the flat `egui_tiles` layout. Users can use separate workspaces instead of sections. Removed `SectionConfig`, `SectionLayout`, `SectionState`, `FocusTarget`, `SectionRenderer`, and all related methods.
- **Health indicator icon from status line**: Removed the online/offline health icon from the far right of the status bar for a cleaner look.

### Added

- **Model selection for AI agents**: Selecting a model in settings (e.g., Sonnet 4.5, Haiku 4.5, Opus 4.6) now correctly routes to that model via `session/set_model` ACP call. Previously, the agent's own default always overrode the selection.

- **Provider switching**: Switching between Claude Code and Codex in settings now correctly spawns the right agent subprocess. The persistent client is recreated on provider change, and the correct model defaults are resolved per provider.

- **System theme**: New "System" theme option that automatically follows the OS light/dark preference in real-time. Appears as the first option in the style picker. On first launch, the editor now defaults to System instead of Dark.

- **Project sidebar**: New always-visible left sidebar panel showing all workspaces. Click to switch workspaces, toggle visibility with `[`. Replaces the modal workspace finder as the primary workspace navigation. The active workspace name is also shown in the status bar.

- **Project grouping in sidebar**: Create projects to group related workspaces into collapsible sections. Click a project header to collapse/expand, use the "+" button on a project header to create a workspace inside it. Ungrouped workspaces appear after project sections. Projects and their collapsed state persist across sessions.

- **Tutorial workspaces**: Five built-in tutorial workspaces showcasing different observability use cases — Golden Signals (latency, traffic, errors, saturation), Incident Response (cross-signal investigation), Service Overview (all visualization types), Infrastructure (CPU, memory, disk, network), and Multi-Service (side-by-side comparison). Grouped under a "Tutorial" project that is auto-created for new users.

- **Workspace deletion from sidebar**: Hover over a workspace to reveal a trash icon that deletes the workspace file and removes it from all projects.

- **Landing page redesign**: Streamlined landing page menu — removed "Find workspace" and "Create workspace" (now handled by the sidebar), added "Create project" option. Tutorial project is hidden from the sidebar unless a tutorial workspace is actively loaded.

- **Project creation wizard**: "Create project" now opens a multi-step wizard (project name, workspace name, connection endpoint, git repository) to set up a new project with its first workspace. The sidebar "+" button on project headers creates empty workspaces directly without a wizard.

- **Snapshot naming**: The `:snapshot` command now accepts an optional name (e.g. `:snapshot P99 latency spike`) which becomes the workspace name embedded in the blob. When a recipient opens the snapshot, the name is displayed in the status bar next to the SNAPSHOT badge.
- **Geist Mono font**: Added Geist Mono as a new font option in the editor settings (SIL Open Font License v1.1). Set as the new default editor font.
- **Stockholm theme**: New light theme with cool Nordic-inspired tones — crisp white backgrounds, blue-gray borders, and steel blue (#4A6FA5) accent. Designed to pair with Geist Mono and the Stockholm design identity.
- **Copenhagen theme**: New light theme with warm Danish hygge tones — warm white backgrounds, natural warm-gray borders, and muted sage green (#6B8F71) accent. Designed for premium observability with cozy readability.
- **Void theme**: New OLED-black dark theme with electric violet (#7C3AED) accent. True black backgrounds for OLED displays with a cyberpunk aesthetic.
- **Neon theme**: New dark theme with hot magenta (#E040A0) accent. Deep black backgrounds with vibrant magenta highlights for a bold cyberpunk look.
- **Onyx theme**: New dark theme with gold (#D4AF37) accent. True dark neutral backgrounds with warm gold tones for a refined luxury aesthetic.
- **Light theme**: New default light theme — the white counterpart of Dark. Cool neutral white backgrounds with the same Enya Emerald (#10B981) accent, like white obsidian glass.
- **Parchment theme**: The original Light theme renamed to Parchment — warm cream paper backgrounds with rich black ink and sepia tones.

### Changed

- **Theme aliases removed**: Themes are now selectable only from the Settings page; command-line aliases have been removed from the theme parser.

- **Default font**: Changed default editor font from Departure Mono to Geist Mono for improved readability.
- **Website font**: Switched website from Departure Mono to Geist Mono to match the editor default.

- **Automatic git sync**: `CodebaseManager` now periodically fetches remote commits (default every 5 minutes). When new commits are detected, incremental Tantivy re-indexing is triggered automatically. Manual `:git` syncs reset the timer to avoid double-fetching. Configurable in Settings → Codebases → Sync interval (Off, 1m, 5m, 15m, 30m).

- **SNAPSHOT status mode**: When opening an immutable shared snapshot, the status bar now shows "SNAPSHOT" mode (blue badge) instead of "NORMAL", making it immediately clear the workspace is read-only.

- **Storage settings category**: New "Storage" section in settings sidebar (native only) showing data locations for configuration (`~/.enya/`), workspaces, cloned repositories, and plugins. Each card displays the directory path with a "Reveal" button to open it in Finder. Full j/k keyboard navigation and Enter/l to reveal the focused item.

### Fixed

- **SQL pane connection popup**: Three fixes: (1) popup rows used `egui::Frame` which only senses hover — added `ui.interact()` with `Sense::click()` for proper click handling. (2) Popup was positioned using pane width instead of screen coordinates via `Area::fixed_pos`, causing it to render off-screen — now uses `ui.max_rect()` to compute correct screen position. (3) `connect_saved` didn't mark the connection as active during connecting, so the pill stayed on "Not connected" — now sets active immediately with state-aware dot colors (accent=connecting, green=connected, red=failed).

- **Landing page cutoff on WASM**: Fixed text cutting off below keyboard hints at default 100% zoom on WASM. Increased `UNSCALED_CONTENT_HEIGHT` to account for the memorial text line and WASM-only "Download Native App" link, using platform-specific values (720 for WASM, 690 for native).

- **Overlay centering with sidebar**: Fixed all overlays (tutorial, fuzzy finder, plugins, settings, command palette, etc.) centering on the full screen instead of the content area when the project sidebar is open. Overlays now center within the area to the right of the sidebar.

- **Logo tinting for light themes**: Builtin light themes (Light, Parchment, Stockholm, Copenhagen) now use the original branded logo instead of a tinted version. Accent-color tinting is only applied for non-builtin themes (Midnight, Ayu, Aurora, etc. and custom themes).

- **Snapshot layout preservation**: Fixed snapshots collapsing split/tiled pane layouts into a single tab. The layout extraction now uses only QueryPane tile IDs for pane index mapping, preventing index mismatches when non-QueryPane components (LogsPane, PluginPanes) are in the viewport tree. Also normalizes redundant single-pane Tabs wrappers added by `all_panes_must_have_tabs` so the saved layout is clean and compact.

- **Profile j/k navigation**: Fixed keyboard navigation in the Profile settings section being stuck on the GitHub sign-in button. All 5 items (GitHub auth, default workspace, timezone, default time range, startup page) are now navigable with j/k, and Enter/l toggles the focused dropdown. Focus highlight borders now appear on all profile cards.

- **GitHub authentication in Settings**: New "Auth" section in the settings sidebar under an "ACCOUNT" group. Uses the Authorization Code flow on both platforms — on native, opens the browser and captures the callback via a local server; on WASM, redirects the page to GitHub and detects the callback on reload. Displays the connected GitHub username when signed in. Auth state persists across sessions. Token exchange goes through the API worker (client secret stays server-side).

- **Full snapshot format with conversation data**: New binary snapshot format (`crates/config/src/workspace/snapshot.rs`) that encodes workspace config, pane visualization data, and optional agent conversation (messages with inline charts, source code, diffs, and search results). Designed for R2 blob storage with postcard + LZ4 compression. Includes `extract_snapshot_conversation()` on `AgentPanel` to capture live conversations into snapshot-friendly types.

- **`:snapshot` command**: Upload workspace snapshot (with pane data and agent conversation) to a blob server via `:snapshot` command. Encodes the full snapshot, POSTs to the snapshot server, and copies the returned URL to clipboard. Uses the async upload pattern with progress and error notifications.

- **`:open-snapshot <id>` command**: Load a snapshot from the blob server by ID. Also supports `?snapshot=<id>` URL parameter for WASM browser loading — open a snapshot link and the workspace, pane data, and conversation are restored automatically.

- **Production R2 snapshot storage**: Cloudflare Worker at `api.enya.build` with R2 bucket for production snapshot blob storage. WASM editor uploads/downloads via the R2 API; native uses the local dev server on port 3001.

- **WASM demo data for unified finder**: All search modes (All, Alerts, Commits) now show demo codebase results in the WASM build, showcasing what Tantivy search provides on native. Includes sample metrics, alert rules, and git commits with diff previews and fuzzy matching. Selecting a commit opens the full diff viewer. Live Prometheus metrics continue to work as before.

- **Diff viewer available on WASM**: The diff viewer overlay now works in the WASM build, enabling commit diff viewing from the unified finder and agent panel.

- **Landing page memorial**: Added "In memory of Enya — the family dog" below "Crafted in Stockholm" on the landing page, in a smaller, more subtle style with typewriter animation.

- **Auto-update notification banner**: Checks GitHub Releases for new versions on startup and every 30 minutes. Shows a non-intrusive frosted glass banner in the bottom-right corner with "See changes" (opens release page) and "Restart" (downloads and replaces binary) buttons. Dismissed versions are persisted in settings. Supports native auto-update and WASM page reload.

- **Token usage display in agent panel**: After each AI response, a subtle footer shows the model name, token counts (total, input, output), and request duration. Re-exported `TokenUsage` from `enya-ai` crate.
- **AI tutorial steps**: Added three new tutorial steps for AI features: "AI Agent Setup" (API key and prerequisites), "Ask the AI Agent" (expanded with @metric mention tip), and "Agent Quick Actions" (single-key agent operators aw/ae/ay/ac/ar/af).
- **Settings overlay**: New settings overlay (`:settings` command or landing page) with AI tab (provider, model, API key), Styling tab (side-by-side theme/font panels with color swatches and code previews), Connection tab (Prometheus and Loki endpoint/API key), and Codebase tab (git repo URL). Premium card-style input UX with labels above inputs, bordered input boxes, section grouping, focus glow, and dropdown chevrons. Auto-saves on close. Vim-style navigation (j/k, Tab for tabs, Enter to edit/cycle, h/l switch panels). Per-provider API key storage. Settings persist via eframe.
- **Profile preferences in Settings**: Compact card-row UI for Default workspace, Timezone (Local/UTC), Default time range, and Startup page (Landing page/Last workspace) preferences. All persisted in settings with backward-compatible serde defaults.
- **Auth-gated snapshot uploads**: Snapshot uploads now require GitHub sign-in. The editor sends the access token as a Bearer header; the Worker validates it against GitHub's API before accepting the upload. Unsigned users see a friendly error notification directing them to Settings → Profile.

- **SQL pane snapshot support**: SQL query history (query text, results, execution stats, and execution plans) is now included in workspace snapshots. Shared snapshots preserve the full SQL investigation — results are serialized as string rows and reconstituted as Arrow `StringArray` batches on load so all rendering code works unchanged. Inline SQL tables in agent chat are also preserved in snapshots (previously silently dropped). Backward-compatible with existing snapshots via V1 fallback decoding.
- **WASM read-only SQL snapshot rendering**: SQL snapshots now render in the browser (WASM build) as a read-only notebook. Query text, string-formatted results with paginated tables, execution stats, and plan trees are all displayed from snapshot data — no Arrow or DataFusion dependency required. Collapsed cards show a compact 3-row preview; expanded cards offer Table and Plan tabs with sticky column headers, row number gutters, NULL styling, and operator-colored plan trees with connection lines and metrics.

- **Flight SQL connection management in Settings**: Named Arrow Flight SQL connections (e.g., "prod", "staging", "local") can now be configured in Settings → Connections. Connections persist between sessions and are synced to all open SQL panes. Add, edit, and delete connections with inline forms. The SQL pane sidebar popup now shows Settings-defined connections with a "Manage in Settings" link instead of an in-pane add dialog. Legacy single `default_flight_sql_endpoint` is auto-migrated to the new connection list on first launch.

- **SQL autocomplete improvements**: Column name completion after SELECT, WHERE, GROUP BY, ORDER BY, HAVING, SET, and ON (using columns from tables referenced in FROM/JOIN clauses). SQL keyword and function completion with fuzzy matching (2+ characters). DataFusion function list expanded with array, hashing, string, and encoding functions. Matched characters are now highlighted in the suggestion popup with the accent color. New Keyword and Function suggestion icons.
- **SQL syntax highlighting improvements**: Slash commands (`/explain`, `/diff`, etc.) are now highlighted like dot commands. PostgreSQL-style type casts (`::integer`, `::text`) are highlighted with the type color. Known table names from the active connection are highlighted in the input bar. Function list expanded with DataFusion-specific functions (unnest, generate_series, make_array, array functions, hash functions, and more).
- **SQL pane copy to clipboard**: Press `⌘C`/`Ctrl+C` in the table result overlay to copy all visible data as TSV (tab-separated values with headers). In the plan overlay, copies the execution plan as indented text. Shows a brief "Copied!" badge for visual feedback.
- **Execution time in query history cells**: Query cell headers now show execution duration alongside row count (e.g., "100 rows · 45.23ms") for completed queries.
- **Human-readable row counts in plan tree**: Plan tree nodes now display row counts using compact formatting (e.g., "1.5K rows", "2.3M rows") instead of raw numbers.

- **Inline SQL tables in agent panel**: SQL query results can now be displayed inline in agent chat messages. Press `S` in the table overlay to share results to the agent panel. AI agents can also use the `show_inline_table` command to display SQL results inline. Tables show column headers with data types, up to 10 rows with alternating backgrounds, and NULL values in italic faint style.
- **SQL input history navigation**: Press `Up`/`Down` arrows in the SQL input bar to cycle through previously executed queries. Consecutive duplicate entries are deduplicated. Current input is preserved when entering history mode.
- **NULL value styling in table overlay**: NULL values now render with italic text and a subtle background tint for clear visual distinction from regular values.
- **Column sort in table overlay**: Click column headers to sort table results. Cycles through ascending (▲), descending (▼), and original order. Active sort column is highlighted with accent color. Sorting is numeric-aware (numbers sort correctly) and NULL values sort last.

### Fixed

- **Profile j/k navigation**: Fixed keyboard navigation in the Profile settings section being stuck on the GitHub sign-in button. All 5 items (GitHub auth, default workspace, timezone, default time range, startup page) are now navigable with j/k, and Enter/l toggles the focused dropdown. Focus highlight borders now appear on all profile cards.
- **`load_workspace` agent command**: AI agents can now programmatically load a saved workspace in the GUI using `{"action": "load_workspace", "workspace": "name"}`. This enables the agent-to-human handoff workflow: an agent builds a workspace via the CLI (`enya init`, `enya add-section`, `enya add-pane`), then loads it in the editor for the human to view.
- **Close agent panel with `x`**: When the agent panel is focused, pressing `x` closes it, matching the behavior of workspace panes.

### Changed

- **Full-page settings**: Replaced the settings modal overlay with a full-page settings experience featuring sidebar category navigation (Connections, AI, Editor), spacious content area with themed cards, and premium Linear/Conductor-style design. Settings page is now owned by `EnyaApp` via `UIState::Settings`, eliminating duplicated overlay handling code. Added Arrow Flight SQL endpoint configuration under Connections.
- **SQL connection management moved to Settings**: Removed `/connect` command and `.open <endpoint>` dot-command from the SQL pane. Connections are now exclusively managed in Settings → Connections. The SQL pane sidebar popup populates from Settings-defined connections. The in-pane "Add Connection" dialog has been replaced with a "Manage in Settings" link.
- **Simplified tutorial layout**: Reduced tutorial from 4 panes (3 rows) to 2 vertically stacked panes so charts aren't squished on smaller laptop screens.
- **Reduced theme count**: Removed Nord, Catppuccin, Bergman, Stockholm, Midsommar, and Skärgård themes. Kept 7 focused themes: Dark, Light, Midnight, Ayu, Aurora, Graphite, and Ink.
- **Improved Ink chart palette**: Replaced monochrome gray chart colors with distinct muted hues (dusty blue, rose, sage, ochre, lavender, umber, verdigris) for better series legibility.
- **SQL pane notebook-cell layout**: Refactored the SQL pane from a centered REPL (showing only the latest result with modal overlays) to a notebook-cell layout where all query history is visible as scrollable cards. Collapsed cards show a status icon, SQL preview, row count, and execution time. Click a card or press Enter to expand it inline with full table data (sortable columns, pagination, vim scroll), execution plan view, and tab switching between Table/Plan views. Press Escape to collapse. New query results auto-expand. Keyboard shortcuts (hjkl scroll, [/] page, Cmd+C copy, S share) work in expanded cards. Info/system messages are no longer shown inline.
- **Vim-style notebook cell navigation**: Full vim navigation for the notebook cell list. Press `Esc` from input to enter cell navigation mode. `j`/`k`/`↑`/`↓`/`Tab`/`Shift+Tab` to move between cells. `Enter` to expand. `Esc`/`i` to return to input. `G` to jump to last cell, `gg` to jump to first. `Ctrl+d`/`Ctrl+u` for half-page jumps (5 cells). `y` to yank (copy SQL to clipboard). `d`/`x` to delete a cell from history. Selected cell is highlighted with an accent border. Auto-scrolls to keep the selected cell in view. Context-sensitive keyboard hints show available keys for the current mode (INPUT / NAV / EXPAND).
- **Click to select, double-click to expand**: Single-clicking a collapsed card selects it (entering NAV mode with accent border), double-clicking expands it. Previously single-click expanded immediately.
- **Cell execution numbers**: Query cards now show execution numbers (`[1]`, `[2]`, `[3]`, ...) in the card header for orientation in long histories.
- **Empty state placeholder**: When no queries have been run, the SQL pane shows a welcoming placeholder with an icon and hint text ("Run a query to get started").
- **Sticky column headers**: Column headers in expanded table results now stay visible when scrolling vertically, using a synced dual-scroll-area approach.
- **Vim page motions in expanded table**: Press `G` to jump to the last page and `gg` to jump to the first page of table results.
- **Multi-line SQL input**: The SQL input bar now supports multi-line queries. Press `Enter` to execute, `Shift+Enter` to insert a newline. The input auto-expands in height as lines are added (up to ~7 lines). `Ctrl/Cmd+Enter` also executes as a legacy shortcut.
- **Auto-open results overlay**: Query results now automatically open in the full table overlay when execution completes. Press `Escape` to close and return focus to the input bar. Removed SQL query preview from table overlay footer.

### Fixed

- **Header text overlap when agent panel is open**: Keyboard hints in the workspace toolbar are now hidden when the toolbar is too narrow (< 700px), preventing them from overlapping with the time range controls.
- **WASM UI too small at default browser zoom**: Applied a 1.5x zoom factor to the WASM build so content (text, buttons, landing page) is readable at 100% browser zoom without requiring manual zoom.

- **Unit labels preserved in shared/snapshot URLs**: The unit suffix (e.g. "req/s", "ms", "%") is now encoded in compact URL sharing formats. Previously, shared snapshots would show raw numbers without their unit labels.

- **Snapshot sharing**: Users can share immutable snapshots of workspaces that include the actual plot data, viewable with no backend connection. Snapshot URLs use the compact binary encoding (`postcard + LZ4 + base64`) with `s`/`t` format prefixes. Snapshot panes are read-only — they never refresh and hide the edit button. `:share` and `yy` are context-aware — they produce snapshot URLs when panes have data loaded, or config-only URLs otherwise. `:share-live` forces config-only sharing. Supports all visualization types: time series, stat, gauge, bar chart, heatmap, and sparkline. Snapshot URL size is optimized via LTTB downsampling (cap 100 points/series), delta-encoded timestamps (regular interval detection), f32 precision, and string deduplication (shared string table with u16 indices).

- **Multi-pane snapshot sharing**: In visual-multi mode (`Ctrl+V`), select multiple panes and press `yy` to share a snapshot URL containing only those selected panes. The shared URL opens with just the selected panes in a default layout.

### Changed

- **Plugins overlay shows native-only notice on WASM**: The plugins overlay now displays a "Native app required" message when running in the browser, matching the pattern used by the unified finder for codebase search.

- **Share links use `enya.build/editor` base path**: Consolidated share URL construction with an `EDITOR_BASE_URL` constant. Native builds use `https://enya.build/editor`, WASM builds derive from the current page URL (supporting self-hosted `enya serve` deployments) with `enya.build/editor` as fallback.
- **Agent panel opens at 50% width**: The agent panel now opens at 50% of the available workspace width (instead of a fixed 400px), so the panel and viewport share space equally. The width resets to 50% each time the panel is opened. The panel remains resizable (min 300px, max 80% of available width).

- **Renamed `enya-workspace` crate to `enya-config`**: The workspace configuration crate has been renamed from `enya-workspace` to `enya-config` to better reflect its broader scope. All imports updated from `enya_workspace` to `enya_config`.

### Added

- **`Config` type for daemon configuration**: New `Config` struct in `enya-config` for infrastructure/daemon settings (`~/.enya/config.toml`). Covers datasource endpoints (Prometheus, Loki, Tempo) and server bind settings, separate from workspace view configuration.

### Changed

- **Extract workspace config into `enya-config` crate**: Workspace configuration types (`WorkspaceConfig`, `PaneConfig`, `TimeConfig`, `ViewConfig`, `LayoutConfig`, etc.), compact binary encoding, and workspace templates have been moved to the new standalone `enya-config` crate. The editor now depends on `enya-config` and re-exports all types, so downstream code is unaffected. Editor-specific conversion methods (using `Granularity`, `VisualizationType`, `AppTheme`, `TimeRangePreset`, `QueryState`) are provided via extension traits (`PaneConfigExt`, `TimeConfigExt`, `ViewConfigExt`) and free functions. This decouples the serializable workspace format from the editor UI, enabling a future CLI tool and other consumers to create/read workspace files without pulling in egui.

### Removed

- **Team collaboration features**: Removed all team collaboration code (chat, channels, presence, team menu, team status). Removed `crates/team-api` and `crates/cloud` crates. Removed `teams` feature flag from the editor.


### Added

- **Go-to leader popup (g key)**: A which-key style popup now appears when pressing `g`, showing available go-to commands: `d` (go to definition), `a` (go to alert), `f` (float pane). Uses the same unified design as the Space leader popup with frosted glass styling and key badges.
- **Agent panel scroll UX**: Auto-scrolls to bottom during streaming when user is at the bottom. When the user scrolls up during an active stream, a floating "Jump to latest" pill button appears at the bottom of the chat area. Clicking it snaps back to the latest content.
- **Streaming fade-in animation**: During AI response streaming, newly received text chunks fade in from 60% to 100% opacity over 150ms, creating a smooth word-by-word appearance effect.
- **Multi-line agent input**: The agent input bar (standalone mode) and agent panel input now support multi-line queries via `Shift+Enter`. The input auto-expands in height as lines are added. Bare `Enter` still submits the query. The inline status bar input remains single-line for compact display.
- **Conversation thread management**: Named conversation threads with save/load and pinning. The agent panel header now shows a thread picker: click the conversation name to open a dropdown listing all threads (pinned first, then by recency). Threads auto-name from the first user message. Supports: new conversation (trash icon or picker), rename (pencil icon with inline editor), pin/unpin (pin icon), delete (X in picker). Threads persist as JSON files in `.enya/conversations/` on native; memory-only on WASM.
- **Code block copy buttons**: Each fenced code block in agent panel markdown now has a per-block copy button (top-right). Clicking it copies the code content to clipboard and shows a "Copied!" confirmation with checkmark icon for 1.5 seconds.
- **Agent panel keyboard navigation**: Vim-style message navigation when panel has focus. `j`/`k` or arrow keys to select messages (with accent border highlight and auto-scroll), `y` to yank selected message to clipboard, `/` to search conversation (case-insensitive), `n`/`N` to cycle through search matches, `g`/`G` to jump to first/last message.
- **Markdown rendering in agent panel**: Assistant messages are now rendered with full markdown formatting instead of plain text. Supports headings, bold, italic, strikethrough, inline code (accent-colored with elevated background), fenced code blocks with language labels (monospace in themed Frame), ordered/unordered lists with nested indentation, blockquotes with accent left bar, horizontal rules, and links (accent-colored). Uses `pulldown-cmark` for parsing with a custom egui renderer built on `LayoutJob` for mixed inline styles, matching the existing Obsidian Glass design system.
- **Which-key style leader popup**: Inspired by neovim's which-key.nvim plugin (included in LazyVim), a dynamic popup now appears when pressing Space (leader key) showing available Space+X commands with nerd font icons. Key badges are displayed on the right side for clean visual hierarchy. The popup appears after 150ms delay (so power users typing fast sequences won't see it) and stays visible until a command is executed, Escape is pressed, or an invalid key dismisses it (no auto-timeout, matching neovim's behavior). Commands: `f` (find), `w` (workspace), `h` (home), `d` (diagnostics), `a` (agent), `t` (time picker), `p` (plugins). The leader popup and Space+X shortcuts are only available in workspace view (not on landing page, which has its own UI for navigation).
- **Custom time range picker**: New smart overlay for selecting custom time ranges. Access via `tc` keybinding or click "Custom" button. Features:
  - **Duration input**: "2h", "30m", "1d", "last 2 hours"
  - **Named dates**: "today", "yesterday", "this week", "last week", "this month"
  - **Date ranges**: "jan 15 to jan 20", "2024-01-15 to 2024-01-20"
  - **Date with time**: "2024-01-15 09:00 to 2024-01-15 18:00"
  - Fuzzy autocomplete suggestions, keyboard navigation with `↑`/`↓` or `Ctrl+N`/`Ctrl+P`
- **Tutorial: Colorscheme step**: New "Colorscheme" step at the beginning of the tutorial lets users select their preferred theme and font. Press `s` to open the style picker, or `→` to skip.
- **Tutorial: Step picker overlay**: Press `g` during the tutorial to open a two-column overview of all steps organized by category (Navigation, Editing, Time, Git, Workspace, Advanced, Help). Navigate with `j`/`k`, select with `Enter`, or use number keys `1-9` for quick jumps.
- **Tutorial: Commit annotations step**: New tutorial step teaching the `gc` keybinding to toggle git commit markers on charts, with navigation hints (`]c`/`[c` for next/prev commit)
- **Tutorial: Tab merging step**: New dedicated step teaching `Ctrl+W t h/j/k/l` to merge panes into tabbed groups. The "Move Panes" step now focuses solely on pane rearrangement.
- **Tutorial: Cycle visualization step**: New step teaching the `cv` keybinding to cycle through visualization types (line chart, bar chart, table, etc.).
- **Agent input bar: Command result badge**: When the AI response contains enya-commands, the Response state shows "✓ N actions applied" in accent color instead of raw text preview. The overlay variant also shows a truncated text summary alongside the badge. Makes it clear when the agent performed actions.
- **Agent input bar: Character/context indicator**: While typing, a subtle character count appears on the right side. When context panes are attached, shows pane count alongside (e.g., "2 panes  47").
- **Agent input bar: State transition animations**: State changes (Ready→Typing→Processing→Response) now fade in with a 150ms ease-out cubic animation instead of instant swaps, applied in both overlay and inline modes.
- **Data-aware agent context**: The agent system prompt now includes rich dashboard state. Selected context panes include visualization type and data summaries (latest/min/max values for time series, current value for stat/gauge, bar values for bar charts). When no panes are explicitly selected, the focused pane is automatically included, enabling natural "explain this spike" workflows. The active viewport filter pattern is also injected into the dashboard context.
- **Inline git diff rendering**: The agent can now display GitHub-style git diffs inline within conversation messages using the `show_inline_diff` command. Shows commit info header, file stats with +/- badges, and syntax-highlighted diff lines with addition/deletion highlighting. Supports showing diffs for specific commits or working directory changes, with optional file filtering. Inline content (diffs, charts, source) generated while using the quick input bar mode is preserved when handing off to the full agent panel. Click the commit hash or "Open" link to open the full diff viewer, or press `o` when the message is selected via j/k navigation.
- **Agent panel slash commands and @mentions**: The agent panel's input now supports `/` slash commands and `@` metric mentions, matching the quick input bar functionality. Type `/` for command autocomplete (diff, explain, etc.) or `@` for metric suggestions. Navigate with arrow keys or Ctrl+J/K, select with Enter/Tab, cancel with Escape.

### Changed

- **Inline charts use real query data**: `show_inline_chart` now executes the PromQL query through the QueryExecutor and displays real data from Prometheus. Falls back to demo sine wave data when in offline/demo mode.
- **Unified `show_source` command**: Consolidated `show_metric_source`, `show_inline_source`, and `show_alert_source` into a single `show_source` command. Accepts `source_type` ("metric" or "alert") and `context_lines` parameters. The editor decides inline vs modal display. Legacy command names still parse and execute.
- **`create_pane` supports floating mode**: Added optional `floating` and `position` parameters to `create_pane`. When `floating: true`, creates a detached investigation pane. The legacy `create_floating_pane` command still works as an alias.
- **Trimmed agent prompt**: Removed UI-chrome commands from the agent system prompt to reduce cognitive overhead: `exit_fullscreen`, `toggle_zen_mode`, `focus_pane`, `rename_pane`, `duplicate_pane`, `sync`, and `create_floating_pane`. All commands still parse and execute if emitted; they are just no longer advertised.
- **Commit annotations hidden by default**: Git commit markers are now hidden by default when loaded. Use `gc` to toggle visibility. Previously, commits were auto-shown when a git repository was configured.

### Removed

- **Demo team annotations**: Removed demo annotations from tutorial charts. Team collaboration features will be added in a future release.

### Fixed

- **WASM build uuid compatibility**: Added `js` feature to uuid dependency for WASM builds. This fixes the `wasm32-unknown-unknown` target compilation which was failing due to uuid requiring a randomness source.
- **Demo data timestamps**: Demo/tutorial charts now generate data relative to current time instead of a hardcoded timestamp from Nov 2023. All time presets (5m, 15m, 1h, 24h, 7d) now show consistent demo data.
- **Demo panes feel like real setup**: Demo/tutorial panes now show loading animation and visual feedback when time range changes or refresh is triggered, making the tutorial experience identical to real data usage.
- **Multi-select refresh (r key)**: Fixed `refresh()` to properly handle demo vs real panes with loading animation. Demo panes show loading briefly then regenerate demo data; real panes mark for re-query through the query executor.
- **Dark theme logo**: Use original branded logo (not tinted) for Dark theme. Detects Dark theme by its Enya emerald accent color (#10B981) to handle resolved Custom theme variants.
- **Agent panel keyboard conflict with diff viewer**: Fixed keybindings (j/k, o) in the agent panel being captured even when the diff viewer is open. The agent panel now disables its keyboard handling when the diff viewer is active.
- **Agent panel selection border alignment**: Fixed the vim navigation selection border extending outside the scroll area. Now properly clips to the visible content area.
- **Agent input bar Esc stop styling**: The "Esc stop" hint during processing now matches the "Esc clear" hint style in the response state, using consistent typography and alignment across both inline and panel modes.

### Added

- **Plugin focused pane API**: Lua plugins can now access information about the currently focused pane for sharing context to external services:
  - `enya.get_focused_pane()` - Returns `{pane_type, title, query, metric_name}` or nil if no pane is focused
  - Supports all pane types: query, logs, tracing, sql, custom_table, custom_chart, custom_stat, custom_gauge
  - Enables sharing workflows to Slack, Discord, or other collaboration tools
- **Share to Slack/Discord example plugin**: New example plugin (`share-to-slack.lua`) demonstrating how to share pane context:
  - `:share-slack [message]` - Share focused pane context to Slack
  - `:share-discord [message]` - Share to Discord
  - `:share-clipboard` - Copy context to clipboard
  - Includes time range, pane type, query, and metric name
  - Keybindings: `<leader>ss` (Slack), `<leader>sd` (Discord), `<leader>sy` (clipboard)
- **Community plugin marketplace**: Plugin overlay now has an "Available" tab to browse and install community plugins:
  - Press `Tab` or `1`/`2` to switch between Installed and Available tabs
  - Press `r` to refresh the list of available plugins from the remote registry
  - Press `i` on an available plugin to install it to `~/.config/enya/plugins/`
  - Press `x` on an installed plugin to remove it
  - Uses `plugins/index.toml` in the repo as the plugin registry
  - **Hot-reload**: Plugins are activated immediately after installation or update (no restart required)
  - **Braille spinner**: Shows animated progress indicator during plugin installation
  - **Auto-refresh**: Available plugins list refreshes automatically when opening the overlay
- **Plugin overlay keybinding**: Press `Space+p` anywhere in the editor to open the plugins overlay
- **Plugin overlay premium UX**: Enhanced plugin overlay with polished visual design:
  - **Tab underline indicator**: Active tab highlighted with colored accent underline
  - **Selected row accent bar**: Vertical accent bar on left side of selected plugin row
  - **Keyboard hint badges**: Pill-styled keyboard shortcuts with subtle backgrounds
  - **Confirmation dialog**: Pressing `x` to remove a plugin shows a modal confirmation dialog (`y` to confirm, `n`/`Esc` to cancel)
  - **Search filter**: Press `/` to search/filter plugins by name or description (works in both Installed and Available tabs)
  - **Vim navigation**: `G` jumps to last item, `gg` jumps to first item
  - **Scroll-to-selected**: List automatically scrolls to keep the selected plugin visible when navigating with `j/k`/`G`/`gg`
  - **Update indicator**: Shows "UPDATE" badge on installed plugins when a newer version is available in the registry

- **Landing page typewriter animation**: Terminal-style typewriter entrance effect when the landing page loads. Logo appears instantly, then text elements type out character by character at 60 cps with a blinking cursor (▌) - tagline, menu items (staggered), and footer hints.
- **Landing page monospace shortcuts**: Menu item shortcuts now render in a monospace font for a clean, terminal-native look.
- **About overlay**: New "About" option on landing page opens a dedicated overlay describing Enya as a keyboard-first observability editor that connects metrics, logs, traces, SQL, and git with AI.
- **Plugin system**: Neovim-style Lua plugin architecture for customizing the editor:
  - **Lua plugins**: Full scripting support with conditional logic, input validation, and HTTP requests
  - **Plugin registry**: Central `PluginRegistry` for managing plugin lifecycle (register, init, activate, deactivate)
  - **Plugin context**: `PluginContext` provides access to command sender, async runtime, theme, and notifications
  - **Hook system**: Lifecycle hooks (`on_workspace_loaded`, `on_pane_added`, etc.), command hooks, keyboard hooks, theme hooks, and pane hooks
  - **Custom themes**: Lua plugins can define custom color themes with inheritance from base themes
  - **Plugin loader**: Automatic discovery from `~/.config/enya/plugins/` and workspace `.enya/plugins/`
  - **Documentation**: Comprehensive [PLUGINS.md](./PLUGINS.md) guide for plugin authors
- **Plugin pane management API**: Lua plugins can now programmatically manage workspace panes:
  - `enya.add_query_pane(query, [title])` - Add a query pane with PromQL query
  - `enya.add_logs_pane()` - Add a logs pane with current time range
  - `enya.add_tracing_pane([trace_id])` - Add a tracing pane
  - `enya.add_terminal_pane()` - Add a terminal pane (native only)
  - `enya.add_sql_pane()` - Add a SQL pane
  - `enya.close_pane()` - Close the focused pane
  - `enya.focus_pane(direction)` - Navigate to adjacent panes
- **Plugin time range API**: Lua plugins can control the global time range:
  - `enya.set_time_range(preset)` - Set time range preset ("5m", "1h", "24h", etc.)
  - `enya.set_time_range_absolute(start, end)` - Set absolute time range
  - `enya.get_time_range()` - Get current time range

### Fixed

- **WASM compatibility in plugin context**: Fixed `get_time_range()` in `EditorPluginHost` which used `std::time::SystemTime` directly, causing panics in WASM browsers. Now uses the WASM-safe `now_unix_secs_f64()` utility.
- **Tutorial command**: Added missing `:tutorial` (or `:tut`) command to the command palette to open the interactive tutorial. The command now properly appears in the command palette and restarts the tutorial from the beginning.
- **Vim navigation after overlay close**: Fixed an issue where vim keys (h/j/k/l) wouldn't work immediately after closing overlays. All overlays now properly clear egui focus on close so keyboard navigation resumes instantly. Affected overlays: command palette, buffer editor, multi-edit, which-key, workspace creator, tutorial, info, about, source preview, diagnostics, diff viewer, codebase finder, and unified finder.
- **Consistent key consumption in overlays**: Standardized overlays to use `consume_key()` instead of `key_pressed()` to prevent keys from being processed multiple times in the same frame. Affected overlays: about, agent panel, annotation editor, buffer editor, codebase finder, command palette, diff viewer, info, multi-edit, source preview, and viewport filter.
- **Stale focus validation after pane close**: Focus is now validated after closing a pane to ensure it references an existing tile. If the focus target was removed (e.g., container collapsed), focus falls back to the first available pane.
- **Visual-multi cursor validation**: The visual-multi selection cursor is now validated against existing panes. If the cursor references a deleted pane, it resets to the first available pane.
- **Recursion depth guards**: Added depth limits (100 levels) to recursive tree traversal functions to prevent stack overflow on pathological tree structures.


### Changed

- **Tutorial overlay refresh**: Updated the interactive tutorial (`:tutorial`) with new sections and platform-aware content:
  - Added **Quick Time Presets** step for `t1/th/td` shortcuts
  - Added **Floating Panes** step for `gf` and `:dock` commands
  - Added **Move Panes** step for `Ctrl+W h/j/k/l` and tab merging
  - Added **Workspace Undo** step for `u` keybinding
  - Added **Ask the AI Agent** step for `aa` and `Space+a` keybindings
  - Added **Terminal & SQL** step (native-only) for `:terminal` and `:sql` commands
  - Updated **Metrics Finder** to **Find Anything** with `Space+f` keybinding
  - Tutorial now detects WASM vs native and hides native-only features on web
  - Replaced progress dots with a simple progress bar and "X of Y" step counter for better visual clarity

- **Landing page footer**: Updated credit text from "Developed by Meldrum Labs" to "Crafted in Stockholm"
- **Landing page header**: Removed large "ENYA" title text for a cleaner, more minimal design - the brand name now appears subtly in the version badge (e.g., "Enya [ v0.1.0 ]")

- **Status line minimalist redesign**: Simplified the right section of the status line for a premium, cleaner look:
  - Replaced tabs count, viewport info, last refresh time, and connection status with a single health indicator
  - Health indicator shows green checkmark when all good, warning symbol for warnings, error symbol for errors or connection issues
  - Hover tooltip provides details about the current status with keyboard shortcut hint (Space+d)
  - Shows repo name with short commit hash (e.g., "my-repo · abc1234") instead of truncated commit message
  - Git branch icon displayed next to repo name for semantic clarity
  - Full commit message shown on hover
  - Kept team collaboration status
  - Mode badge on left remains unchanged

### Added

- **Workspace undo system**: Vim-style `u` keybinding to undo workspace operations:
  - **Close pane**: Restores closed panes to their exact position in the tile tree (tabs, splits)
  - **Float pane**: Undoes floating a pane, restoring it to its original tile tree position
  - **Dock pane**: Undoes docking a floating pane, restoring it to floating with original position/size
  - Focus is restored if the pane was focused when the action occurred
  - Undo stack holds up to 50 actions
  - Uses command pattern with inverse operations for efficiency

- **Sync command**: New `:sync git` command in the command palette and `sync` agent command to fetch latest git commits and re-index the codebase (including Tantivy full-text search). Useful when the repository has been updated externally.

- **Keyboard navigation test infrastructure**: Comprehensive testing for vim-style keyboard navigation:
  - Extended `LeaderKeyState` tests: timeout edge cases, boundary behavior, multiple key independence
  - Extended `VisualMultiState` with `selected_tiles()` and `validate_selections()` methods and tests
  - New `keyboard_logic.rs` module with pure decision logic testable without egui::Context
  - Tests for all leader key sequences: Space+*, t* (time ranges), g* (go-to), a* (agent operators)
  - Tests for Ctrl+W and Ctrl+W t window management sequences
  - Tests for modal blocking logic (11 overlay types)
  - `KeyboardDecision` enum representing all keyboard-triggered actions
  - `KeyboardContext` struct for minimal state needed for keyboard decisions
  - Enabled `egui_kittest` 0.33.3 for UI testing with snapshot support
  - New `tests/ui_integration.rs` with egui_kittest harness tests for WhichKey overlay

- **TESTING.md documentation**: Comprehensive testing guide covering:
  - Quick start commands for running tests
  - Three-layer test architecture (unit tests, integration tests, WASM checks)
  - Detailed egui_kittest tutorial with examples
  - Snapshot testing setup and usage
  - Guidelines for writing new keyboard shortcut tests
  - Troubleshooting common issues (zig toolchain, ghostty build)

- **ENYA.md project context**: AI agents now automatically load project-specific context from `ENYA.md` or `.enya/context.md` in the repository root:
  - Custom instructions, conventions, and context are injected into every agent prompt
  - Supports documenting metric naming conventions, important SLOs, common queries, and team workflows
  - Search order: `ENYA.md` in repo root, then `.enya/context.md` as fallback

- **Streaming action indicators**: Agent commands now show visual feedback during execution:
  - Each command displays as an activity item (e.g., "Creating section 'Infrastructure'")
  - Success/failure indicated with checkmark or error icons
  - Activities appear in both the agent input bar and full agent panel
  - Provides real-time visibility into what the agent is doing

- **New AI agent commands** for incident investigation workflows:
  - `add_logs_pane`: Create a logs pane with optional LogQL query and Loki backend support
  - `add_tracing_pane`: Create a tracing pane with optional trace ID to pre-load
  - `add_terminal_pane`: Create a terminal pane for running shell commands (native only)
  - `set_visualization`: Change pane visualization type (time_series, stat, gauge, bar_chart, sparkline, heatmap)
  - `set_absolute_time_range`: Set specific time range with Unix timestamps for incident investigation
  - `refresh_pane`: Refresh specific or all panes to reload data
  - `close_pane`: Close a pane by name or the focused pane
  - `create_section`: Create Grafana-style collapsible sections for organizing panes
  - `create_floating_pane`: Create floating panes for investigation workflows
  - `maximize_pane`: Maximize a pane to fullscreen
  - `rename_pane`: Rename a pane dynamically
  - `duplicate_pane`: Clone a pane with same query for comparison workflows
  - `focus_pane`: Programmatically focus a specific pane
  - `toggle_zen_mode`: Toggle minimal UI mode
  - `exit_fullscreen`: Exit maximized/fullscreen mode

- **Amp-style thinking indicator**: Premium visual feedback during AI requests:
  - Animated pulsing dots with wave effect in both inline input bar (`aa`) and full agent panel (`Space+a`)
  - Stage-based status messages (Connecting, Reading context, Thinking, Using tools, Generating)
  - Elapsed time display for long-running requests
  - Real-time activity updates showing current actions (e.g., "Creating section", "Fetching metrics")

- **Neovim-inspired visual polish**:
  - **Yank flash**: Brief highlight effect when sharing/yanking panes (triggered on `yy`)
  - **Dim inactive panes**: Subtle overlay on unfocused panes for visual hierarchy
  - **Focus pulse**: Glow effect when a pane receives focus, drawing attention to the active pane

- **Layout transitions**: Smooth animated transitions when panes split:
  - New panes smoothly grow from small to target size using ease-out-cubic easing
  - Sibling panes animate their share changes during splits
  - 150ms animation duration for fluid, responsive feel

### Changed

- **Improved pane name matching**: Agent commands now prefer exact matches when finding panes by name, falling back to substring matches. This prevents ambiguous matches (e.g., "CPU" matching both "CPU" and "CPU Usage")
- **Refactored pane resolution**: Extracted common `resolve_pane_target()` helper for consistent "focused" keyword handling across all pane-targeting commands
- **Terminal feature is now optional**: The `terminal` feature (which includes the embedded terminal emulator) is enabled by default but can be disabled with `--no-default-features --features all-languages`. This allows building and testing without the zig toolchain required by ghostty
- **Upgraded egui ecosystem**: Updated egui, eframe, and egui_extras from 0.33.2 to 0.33.3 for bug fixes and improved compatibility

- **SQL Diff Viewer**: Compare query results, execution plans, schemas, or execution profiles between two different connections (e.g., staging vs production):
  - **Data comparison**: `/diff staging prod SELECT * FROM users LIMIT 10` - compare query results in one command
  - **Plan comparison**: `/diff analyze staging prod SELECT * FROM users` - compare EXPLAIN ANALYZE plans
  - **Schema comparison**: `/diff schema staging prod users` - compare table schemas between connections
    - Unified table view showing column name, left type, right type, and status
    - Status highlighting: matching (✓), changed (yellow), removed (red), added (green)
    - Statistics: matching, changed, removed, added column counts
  - **Profile comparison**: `/diff profile staging prod SELECT * FROM orders` - compare EXPLAIN ANALYZE with premium visual design
    - Hero summary card with big timing numbers, visual time bars, and verdict badge (Faster/Slower)
    - Unified operator tree with visual time bars showing relative execution time
    - Delta chips with color-coded timing changes (+/-ms)
    - Row count and memory chips for significant metric differences
    - Bottleneck warning indicators for operators with >50ms regression
    - Tree guide lines with proper visual hierarchy
    - Strikethrough styling on slower timings with color-coded highlights
  - **Demo modes**:
    - `/diff demo` - preview data diff with sample data
    - `/diff schema demo` - preview schema diff with sample column differences
    - `/diff profile demo` - preview profile diff with sample timing differences
  - **Side-by-side data view**: Tables rendered in split columns with row counts and schema validation
  - **Row-level highlighting**: Rows are highlighted based on diff status using theme diff colors:
    - Matching rows: neutral styling
    - Left-only rows: red/removed background (rows only in source connection)
    - Right-only rows: green/added background (rows only in target connection)
  - **Side-by-side plan view**: Execution plan trees for both connections with operator metrics
  - **Diff statistics**: Shows matching rows, left-only rows, right-only rows at a glance
  - **Schema mismatch detection**: Warns when schemas don't match between connections
  - **Error handling**: Gracefully shows errors from either connection while displaying successful results
  - **Concurrent execution**: Both queries run in parallel via `tokio::join!` for faster comparisons
  - New types: `DiffType`, `DiffQueryResult`, `DiffStats`, `DiffRow`, `DiffRowPair`, `TableDiff`, `RowDiffStatus`, `ColumnDiffStatus`, `SchemaDiffColumn`, `SchemaDiffResult` for structured diff data
  - New module: `diff.rs` with `compute_table_diff()`, `compute_detailed_diff()`, `compute_schema_diff()`, and `schemas_compatible()` utilities

- **SQL Plan View**: Query execution plan visualization with three view modes:
  - **Tree View**: Vim-navigable hierarchical tree with expand/collapse, bottleneck highlighting
  - **Stats View**: Aggregate dashboard showing total time, operator count, rows, memory, bottleneck warning, category breakdown, and top slowest operators
  - **Waterfall View**: Gantt-style visualization showing parallel execution timing
  - Tab key cycles between view modes, Shift+Tab cycles backward
  - Navigation keys (j/k, g/G, b for bottleneck) work in Tree/Waterfall views without propagating to underlying viewport
  - Proper metrics parsing for real execution output (`output_rows=`, `elapsed_compute=`, `output_bytes=`)
  - **Dedicated 12-color plan palette**: Each theme has a unique 12-color palette optimized for execution plan visualization with maximally distinct colors for: Scan/Read (blue), Filter/Limit (green), Join (orange), Aggregate/Group (purple), Sort/Order (red), Project (teal), Hash (yellow), Remote/Exchange (cyan), Union/Interleave (pink), Cooperative/Yield (lime), and other Exec operators (amber)

- **Floating panes** (zellij-inspired): Detachable panes that hover above the tile layout for quick investigations:
  - Float any focused pane using `gf` (go-float) keyboard shortcut or `:float` command
  - Floating panes render above tile layout but below modal overlays
  - Custom title bar with pin, minimize, dock, and close buttons
  - Drag title bar to reposition, resize from edges and corners
  - **Double-click title bar to maximize/restore** (fills viewport with margin)
  - **Screen edge snapping** when dragging within 20px of viewport edges
  - **Smooth animations**: Fade/scale on appear, animated minimize/expand transitions
  - **Auto-arrange**: `:float arrange` (or `:fl a`) tiles all floating panes in a grid
  - **Glass effect**: Semi-transparent backdrop for modern appearance
  - Dock back to tile layout via dock button or `:dock` command
  - Pin toggle to keep floating pane on top
  - Minimize toggle to collapse to title bar only (now animated)
  - Scratch panes auto-close on Escape
  - Tab cycling between floating panes (not yet implemented)
  - Full theme support with focus indicators
  - **Native breakout windows** (desktop only): Pop floating panes out to separate OS windows
    - Click the pop-out button (arrow icon) in the title bar to detach to a native window
    - Native windows can be moved outside the main app, onto different monitors
    - Custom themed title bar matching the app's dark/light theme (no native chrome)
    - Draggable title bar for window movement
    - Click "pop in" button or close button (X) to return the pane to the main app

- **Agent panel as first-class layout participant**: The agent panel now participates in the layout flow like the channels panel:
  - Viewport shrinks to accommodate the agent panel when open (instead of floating overlay)
  - Left-edge highlight provides visual anchoring (symmetric with channels panel's right-edge highlight)
  - Focus state tracking with 2px accent border when panel has vim focus
  - `show_inside` method for rendering within the UI hierarchy (layout-aware)
  - Matches the premium feel of the team channels panel on the left side
  - **Vim navigation**: Press `l` at the rightmost pane edge to focus the agent panel, `h` to return to viewport
  - Press `i` or `Enter` when panel has vim focus to enter the chat input, `Escape` to return to vim navigation
  - Works in both section-based and tile-based workspace layouts

- **Tab-to-panel handoff for agent input bar**: Seamlessly continue a conversation from the quick input bar in the persistent agent panel:
  - Press `Tab` when viewing a response in the agent input bar to open it in the side panel
  - Opens the three-panel layout: channels on left, viewport center, agent panel on right
  - Conversation context (query, response, activities) is preserved during handoff
  - Visual hint shows "Tab: open in panel" in the response state
  - New `ConversationHandoff` type for transferring state between components
  - `export_for_handoff()` and `import_from_handoff()` methods for clean state transfer
  - Non-intrusive workflow that doesn't disrupt the existing pane layout
  - Full inline content support: charts, source code previews, and search results render in the panel
  - Agent commands (ShowInlineChart, ShowInlineSource, SearchCodebase) inject content into the panel when open
  - `Space+a` now toggles the agent panel open/closed (previously created a new agent pane)

- **Vim-style navigation for channels panel**: Added vim keybindings to navigate to and within the team channels panel:
  - Press `h` at the leftmost pane edge to transfer focus to the channels panel
  - Works in both section-based and tile-based workspace layouts
  - Use `j`/`k` to navigate up/down within the panel, including across sections (threads → channels → team)
  - Press `Enter` to select the highlighted thread, channel, or team member
  - In split view (chat open): press `l` to focus the chat input, `Escape` to return to sidebar
  - In sidebar-only: press `l` to return focus to the viewport
  - Accent border indicates when the panel has vim focus
  - Focus state properly resets when panel is hidden via `Space+g` or team disconnect
  - Keyboard input is blocked when overlays are open (style picker, command palette, finders), including chat input handling

- **Tracing pane for distributed trace visualization**: New pane type for visualizing distributed traces from Grafana Tempo with a waterfall/timeline view:
  - **TracingClient trait** in `enya-client` crate with `TempoClient` implementation for Grafana Tempo HTTP API
  - **Waterfall chart** showing spans as horizontal bars on a timeline with hierarchy indentation
  - **Service-based span coloring** using theme chart palettes for consistent visual identification
  - **Error span highlighting** with semantic error colors
  - **Span detail panel** showing service, operation, duration, status, tags, and logs
  - **Hover tooltips** with quick span information
  - **Demo mode** with sample trace data for testing without a backend
  - **Command palette integration**: Use `:trace` (or `:tr`, `:tracing`) to open a tracing pane
  - **Optional trace ID argument**: `:trace abc123def456` pre-fills and loads a specific trace
  - Theme-aware styling that adapts to all 13 AppTheme variants
  - WASM compatible (uses web_time for timestamps)
  - Trace data models: `Trace`, `Span`, `SpanStatus`, `SpanLog`, `TraceSummary`, `TraceSearchParams`
  - `TraceManager` for managing in-flight trace fetch requests

- **Collapsible sections**: Added Grafana-style collapsible sections for grouping panes with expandable/collapsible headers:
  - New `SectionConfig` and `SectionLayout` types for TOML configuration
  - Section layouts: horizontal, vertical, grid (with columns), and tabs
  - New `FocusTarget` enum to track focus on section headers vs panes
  - New `SectionState` for runtime collapsed state management
  - `SectionRenderer` component for rendering section headers (with collapse indicator ▼/▶, name, pane count badge)
  - Section content rendering with layout-specific pane arrangement
  - Click-to-toggle collapse behavior on section headers
  - Navigate between sections and panes using standard vim motions (hjkl)
  - Helper methods: `migrate_to_sections()`, `all_panes()`, `uses_sections()`
  - Demo workspace (`DEMO_WORKSPACE_TOML`) uses sections format to showcase the feature
  - Example TOML format:
    ```toml
    [[sections]]
    name = "API Performance"
    layout = "horizontal"

    [[sections.panes]]
    query = "rate(http_requests_total[5m])"
    name = "Request Rate"
    ```

- **SQL pane with Arrow Flight SQL (native-only)**: REPL-style SQL client for connecting to Flight SQL servers (DataFusion, DuckDB, InfluxDB IOx, etc.):
  - New `SqlPane` component implementing the `Component` trait
  - **OpenCode-style command interface**: Centered, minimal input with suggestions:
    - `SQL >` prompt with mode indicator (SQL, DIFF, EXPLAIN, PROFILE)
    - Inline connection pill showing active database
    - Suggestions popup above input (command palette style)
    - Centered content with max-width for readability
  - **Command system** (type `/` to see commands):
    - `/diff` - Compare query results across environments
    - `/explain` - Show query execution plan
    - `/profile` - Profile with detailed timing
    - `/schema` - Show table structure
    - `/connect` - Switch active connection
    - `/export` - Export results
    - `/history` - Query history
    - `/help` - Show available commands
  - **Smart suggestions**: Context-aware completions appearing above input:
    - Commands when typing `/`
    - Tables after FROM, JOIN, etc.
    - Fuzzy matching on partial names
    - Keyboard navigation (↑↓) and Tab to insert
  - **Connection management** via inline pill or `/connect`:
    - Status dot (green=connected, gray=disconnected)
    - Click pill to see connection dropdown
    - Add, remove, switch connections
  - **Multi-connection management**: Save and manage multiple database connections
    - Add connections via "Add Connection" dialog
    - Click connected item to set as active
    - Click disconnected item to connect
    - Context menu for disconnect, remove
    - Connections show status indicator (●) - green=connected, gray=disconnected
  - REPL interface with query history displayed as cells
  - Results rendered as tables with schema-aware column formatting
  - **SQL syntax highlighting** in both input area and query history using theme-aware colors:
    - Keywords (SELECT, FROM, WHERE, etc.) highlighted in keyword color
    - Functions (COUNT, SUM, AVG, etc.) highlighted in function color
    - Strings, numbers, comments highlighted appropriately
    - Dot-commands (`.help`, `.open`, etc.) highlighted in accent color
  - DuckDB/SQLite-style dot-commands: `.open`, `.close`, `.tables`, `.explain`, `.analyze`, `.plan`, `.demo`, `.help`
  - New `enya-datafusion` crate providing:
    - `FlightClient` for Arrow Flight SQL connections with auth support
    - Async query execution with streaming results
    - Metadata queries (catalogs, schemas, tables, columns)
    - Local DataFusion session for file-based queries (Parquet, CSV, JSON)
    - Query plan extraction and visualization utilities
  - Open with `:sql` or `:datafusion` command
  - Execute queries with `Ctrl+Enter` or click the play button

- **Query Plan Visualization (native-only)**: Interactive query plan analysis with three visualization modes:
  - **Tree View**: Vim-navigable hierarchical plan tree with:
    - `j/k` for up/down navigation between operators
    - `h/l` for collapse/expand nodes
    - `b` to jump to bottleneck operator
    - `Space` to toggle expand/collapse
    - `G` to jump to bottom, `g` to jump to top
    - Color-coded operators by type (scans=blue, filters=green, joins=orange, etc.)
    - Bottleneck highlighting with warning icon for slowest operators
    - Inline metrics showing execution time, row counts, and memory usage
  - **Timeline View**: Horizontal bar chart (egui_plot) showing:
    - Operator execution times as horizontal bars
    - Color-coded by operator type
    - Sorted by execution time descending
    - Legend with timing breakdown
  - **Diff View**: Side-by-side plan comparison for:
    - Comparing logical vs physical plans
    - Analyzing optimization effects
    - Each side has independent vim navigation
  - Commands: `.explain <query>` for logical plan, `.analyze <query>` for EXPLAIN ANALYZE
  - `.plan [tree|timeline|diff|hide]` to switch between views or toggle visibility
  - Plan viewer toggle button in SQL pane header

- **SQL pane overlay system**: Minimal result display with expandable overlay views for detailed inspection:
  - **Compact preview**: Most recent result shown inline with dynamic column fitting based on available width
  - **Table overlay** (press `t` or `Enter`): Full paginated table view with diff-viewer-inspired UX:
    - Header with table icon, accent-colored title, and stat badges (row count, column count, execution time)
    - Row number gutter with darker background matching diff viewer line numbers
    - Fixed-width columns ensuring proper header/body alignment during horizontal scroll
    - Vim-style keyboard navigation: `h/l` horizontal scroll, `j/k` vertical scroll, `[/]` page navigation
    - Keyboard events consumed to prevent propagation to underlying panes
    - Consistent overlay sizing with other modals (85% screen width/height, clamped 700-1400px × 500-900px)
    - Frosted glass styling matching unified finder and diff viewer
    - Long values truncated with ellipsis to fit column width
    - NULL values shown in muted text, alternating row backgrounds
  - **Plan overlay** (press `p`): Query execution plan visualization
  - Press `Esc` to close any overlay and return to compact view
  - Press `c` to clear results from the compact preview
  - Overlay renders centered on screen with dimmed backdrop

- **Neovim-style intro message**: When a workspace has no panes, a centered intro screen displays "Enya" with tagline "A Neovim-inspired observability editor for builders", version number, and aligned command hints. Includes `~` tilde markers on every line (left margin) just like Neovim:
  - `type  Space+f    fuzzy finder`
  - `type  aa         ask AI agent` (native only)
  - `type  ?          help`
  - `type  :          commands`

- **Filter match highlighting**: When using the viewport filter (`/`), matched text in pane names is now highlighted with the accent color and underlined for better visibility.

- **LogsPane query history**: Added query history feature to the LogsPane component. Users can now recall previous LogQL queries from a dropdown menu in the header. The history stores up to 20 most recent unique queries, with the most recent appearing first. Duplicate queries are automatically deduplicated and moved to the front.

- **Full git history indexing with incremental updates**: Improved git integration to index the complete repository history instead of just the last 1000 commits:
  - Batch diff fetching using `git log -p` instead of individual `git show` commands (significantly faster)
  - Parallel semantic extraction using rayon for CPU-bound parsing operations
  - Incremental indexing on restarts: only fetches and indexes new commits since the last indexed commit
  - Skip merge commits and follow only main branch history (`--no-merges --first-parent`)
  - Progress tracking via atomic counters for accurate status updates during parallel operations
  - Large diffs (>100KB) are truncated after semantic extraction to prevent memory bloat
  - Semantic analysis preserved even for truncated diffs (functions added/removed, metrics changed)
  - Regression tests ensure diff content is preserved (not just file names)
  - Premium status line UX: animated spinner in accent color with simple "Indexing" text

- **Separate metrics and logs config**: Workspace config now has distinct `[metrics]` and `[logs]` sections for Prometheus and Loki connections respectively. The old `[connection]` section is still supported via serde alias for backward compatibility. New `LogsConfig` includes `endpoint`, `api_key`, and `default_query` fields.

- **Premium LogsPane theme styling**: The LogsPane component now fully adapts to the current AppTheme with premium visual enhancements:
  - Header with subtle accent-tinted background (dark themes) and accent line separator
  - Table header text blended with theme accent color for visual cohesion
  - Row selection with accent glow effect and enhanced hover states
  - Level badges with theme-aware backgrounds and subtle borders for light themes
  - Dropdown popups with accent-tinted borders and improved shadows
  - Loading skeleton shimmer intensity adapts to light/dark themes
  - All 13 themes (Dark, Light, Midnight, Nord, Catppuccin, Ayu, etc.) now provide unique accent tints

- **Terminal pane (native-only)**: Embedded terminal emulator backed by ghostty's VT library for running shell commands while debugging incidents:
  - New `TerminalPane` component implementing the `Component` trait
  - Run commands like `kubectl logs`, `k9s`, or any shell command directly in the editor
  - Theme-aware terminal colors that adapt to the current editor theme with live updates
  - Semantic ANSI palette colors (Red is red, Green is green, etc.) via `terminal_palette()`
  - Dynamic palette updates for running TUI apps (k9s, htop, vim update colors immediately on theme change)
  - Full keyboard input support including special keys (arrows, function keys, etc.)
  - Mouse support for applications that use mouse reporting
  - PTY integration via `portable-pty` for cross-platform shell spawning
  - Three new workspace crates: `ghostty_vt_sys` (FFI bindings), `ghostty_vt` (safe Rust API), `egui_ghostty` (egui widget)
  - Native-only feature (requires Zig 0.14.1 toolchain for building ghostty)

### Changed

- **Full git history indexing**: The codebase search index now indexes the complete git history instead of only the last 1000 commits. This ensures all historical commits are searchable, improving codebase search accuracy for repositories with long histories. Added new `fetch_all_commits()` and `count_commits()` functions in `enya-analyzer` for efficient full-history operations. **Parallelized diff fetching** using rayon for 4-8x faster indexing on multi-core systems - diff extraction and semantic analysis now run concurrently across all CPU cores.

- **Arrow/DataFusion utilities consolidated**: Moved `format_array_value()` and plan text parsing functions (`parse_plan_text`, `parse_metrics`, `parse_metric_usize`, `parse_metric_duration`, `parse_metric_bytes`) to the `enya_datafusion` crate. The editor now imports these from the shared crate instead of having local copies. The `plan_parsing.rs` module is now focused on demo data generation.

- **Plan view functions moved to shared crate**: The `format_duration`, `format_bytes`, and `format_rows` functions, along with `total_time()`, `bottleneck_time()`, and `operator_count()` methods on `PlanNode`, are now in `enya_datafusion` instead of being duplicated across plan view types. This consolidates plan analysis logic in the shared crate alongside related types like `OperatorMetrics` and `OperatorCategory`.

- **SQL pane module reorganization**: Split the 6000-line `pane.rs` into focused modules following idiomatic Rust practices:
  - `command.rs` - SQL pane commands (`SqlCommand` enum and parsing)
  - `connections.rs` - Connection management types and UI rendering (`ConnectionId`, `SavedConnection`, `ConnectionAction`, `ConnectionSnapshot`, plus `render_connection_popup`, `render_connection_tree`, `render_add_connection_dialog`)
  - `suggestions.rs` - Autocomplete types (`Suggestion`, `SuggestionIcon`, `SuggestionState`)
  - `types.rs` - Core types (`SqlMode`, `ResultOverlay`, `QueryCell`, `QueryStatus`)
  - `pane.rs` - Main `SqlPane` struct (reduced to ~5700 lines)
  - Native-only modules properly gated with `#[cfg(not(target_arch = "wasm32"))]` for WASM compatibility

- **SQL commands streamlined**: Removed unused/stub commands (`/help`, `/plan`, `/profile`, `/export`, `/watch`, `/sample`) - kept only working commands: `/explain`, `/analyze`, `/diff`, `/schema`, `/connect`, `/history`, `/demo`. The `/` trigger now shows all available commands in the fuzzy finder, making `/help` redundant.

- **Connection saving with names**: The `/connect` command now supports saving connections with custom names: `/connect <endpoint> <name>`. For example, `/connect localhost:50051 local` saves and connects with the name "local". Previous behavior (switching to existing or connecting to endpoint directly) still works.

- **Plan viewer UX overhaul**: Premium visual refresh for the execution plan viewer:
  - **Pill-style tab bar**: Replaced plain selectable labels with styled pill tabs in a subtle container
  - **Key badge hints**: Mode-specific keybindings now shown with premium key badges (like keyboard keys) instead of plain text
  - **Tree connection lines**: Vertical and horizontal guide lines showing parent-child relationships in the tree view
  - **Mini progress bars**: Inline percentage bars next to execution time showing relative cost at a glance
  - **Waterfall grid lines**: Vertical grid lines at time markers (0%, 25%, 50%, 75%, 100%) for easier time reading
  - **Premium stat cards**: Cards now have accent-colored icons and a left edge accent stripe
  - Removed redundant sub-headers from each view since the main header and tabs establish context

- **Premium agent panel styling**: Improved the AgentPanel overlay UX to match the refined look of the team channels panel:
  - Switched from `OverlayColors` to `ChatColors` for better theme integration and consistent styling across chat components
  - Added premium dividers between header, chat area, and input sections
  - Message bubbles now use `ChatColors` backgrounds (`own_message_bg`, `agent_message_bg`) with rounded corners
  - Assistant messages feature a left accent bar indicator (matching channel selection style)
  - Role labels now include icons (user account, robot, info) for visual clarity
  - Inline content blocks (charts, source code, search results) use premium frame styling with accent-tinted borders
  - Activity rows use allocate_exact_size rendering with proper typography positioning (matching channels panel rows)
  - Input area has refined styling with larger corner radius and premium elevated background
  - Consistent use of theme typography constants and proper spacing throughout

- **Tracing module restructure in `enya-client`**: Reorganized the `tempo` module under a new `tracing` parent module to support future tracing backends. The module structure is now `enya_client::tracing` with `tempo` as a submodule (`enya_client::tracing::tempo`). Common types (`Trace`, `Span`, `SpanStatus`, etc.) are re-exported from the `tracing` module root for convenience.

- **Viewport pane alignment**: Improved pane layout consistency to prevent bottom panes from overlapping the status line. The viewport tree now always renders at the exact viewport height, with `TreeBehavior::min_size()` (200px) preventing panes from becoming too small. Scrolling only activates when panes would be smaller than this absolute minimum. Added explicit clip rectangles and constrained child UI rendering to ensure content never overflows into the status bar area.

- **Tutorial layout**: Updated the tutorial layout to show "HTTP Requests" and "Requests by Endpoint" side by side in the top row, with "CPU Usage" and "Memory Used" stacked below. This demonstrates both horizontal and vertical pane arrangements.

- **WASM native app promo**: Changed from an intrusive auto-popup overlay to a subtle clickable link below the version badge in the landing page header. Users can click "Download Native App for full experience" to see detailed information about native-only features (git integration, AI agents, persistent workspaces). Less invasive while still informing users about the full desktop experience.

- **Provider-as-mode UX for agent input bar**: Redesigned the agent mode status line for a premium, uncluttered experience:
  - Provider (Claude/OpenAI logo + name) now appears directly in the mode badge position - the provider IS the mode
  - Removed redundant "AGENT" label and duplicate provider badge
  - Response state now shows a preview of the AI's response (first line, truncated) instead of generic "Response ready"
  - Processing state now shows contextual activity with appropriate icons: Sending (→), Thinking (spinner), tool use (file/search/edit icons), Responding
  - Tool use shows the tool name and a preview of what it's doing (e.g., "Read" + "main.rs", "Grep" + "error handling")
  - Thinking state shows a preview of the AI's thought process
  - Response state shows `Tab expand` and `Esc clear` hints side by side (both with accent key styling)
  - Wider input field (500px max) with cleaner placeholder: "Ask anything... / commands @ metrics"

- **Toolbar UX: filter left, time right**: Redesigned the top toolbar for better visual balance:
  - Pane filter input now appears on the left side (always visible, expands when focused)
  - Time range controls pushed to the right side (Grafana-style placement)
  - Filter shows match count (e.g., "2/4") when active
  - Click filter icon or start typing to activate, Enter to apply, Esc to clear
  - Fills the previously empty toolbar space with useful functionality

### Fixed

- **Agent panel vim navigation**: Fixed vim navigation not working in the agent panel and viewport after opening/returning from the panel. Multiple issues were addressed: (1) Removed the sync loop that was syncing `agent_panel_focused` with the panel's internal `has_focus` state every frame - this was causing focus to be lost unexpectedly. (2) Added `ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL))` when toggling the agent panel open or transferring focus to it, ensuring no stale egui widget focus blocks vim keys. (3) Changed keyboard handling to use `ui.ctx().input()` with `key_pressed()` (matching the channels panel pattern). (4) Fixed text input focus detection to only release vim focus when the user actually clicks on the input, not when egui auto-restores focus. (5) Fixed `focus_input` flag to always clear regardless of vim focus state. (6) Added `skip_vim_keys_once` flag to prevent lingering keypresses from being detected as vim navigation immediately after panel gains focus (e.g., the 'a' from `Space+a` was being detected as 'h' navigation). (7) Fixed `ReturnFocusToViewport` to properly set `section_focus` (not just `behavior.focused_tile()`), which controls the visual focus indicator on viewport panes. (8) Removed `agent_panel.is_open()` from the keyboard handler's early-return condition - the agent panel can be open while viewport has navigation focus (only `agent_panel_focused` should block viewport keyboard handling). (9) Fixed `is_at_section_right_edge()` to return true for the rightmost pane of ANY section (not just the last section), allowing `l` to transfer focus to the agent panel from any section.

- **Dynamic pane addition in sections mode**: Fixed `:terminal`, `:trace`, and other dynamic pane commands not showing panes when collapsible sections were active. The `add_tile_to_viewport` method now automatically clears sections mode when adding dynamic panes, since they aren't part of any configured section. This ensures newly created panes are always visible.

- **Visual-multi selection in sections**: Fixed visual-multi mode (Ctrl+V) not displaying selection indicators when using collapsible sections. Section render methods (horizontal, vertical, grid, tabs) now properly draw visual-multi selection highlights on selected panes. Also fixed navigation in visual-multi mode to work with flat pane lists in sections.

- **Focus border alignment**: Fixed the focus border around selected panes not aligning with actual content. Changed from using `available_rect_before_wrap()` to `min_rect()` to get the actual content rectangle.

- **Single series legend**: Fixed time series charts not showing legends when only a single series is present. Changed condition from `self.series.len() > 1` to `!self.series.is_empty()`.

- **Unified finder WASM freeze**: Fixed the unified fuzzy finder freezing on WASM by using `crate::util::Instant` (which uses `web_time::Instant` on WASM) instead of `std::time::Instant` which doesn't work properly in browsers.

- **Chat input Escape focus release**: Fixed vim navigation not working after pressing Escape to exit the chat input. Four issues were fixed: (1) The text input now properly surrenders both widget-level and global egui focus when returning to sidebar navigation. (2) The channels panel Escape handler no longer closes the split view when the chat input is focused, allowing the chat view to properly handle Escape and return vim focus to the sidebar. (3) Added `ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL))` to clear global focus state, ensuring the keyboard handler's focus check doesn't block vim keys. (4) When closing split view via Escape (e.g., when chat input lost focus due to clicking elsewhere), now properly restores vim focus to sidebar and clears egui focus.

- **Style picker focus restoration**: Fixed vim navigation not working after closing the style picker (theme/font selector). The picker now clears egui framework focus when it closes, ensuring keyboard events are properly handled by vim navigation. Also fixed returning focus to viewport from channels panel to properly restore focus to the first pane if no pane was previously focused.

- **SQL pane runtime crash**: Fixed a crash when opening the SQL pane (`:sql` command) with "there is no reactor running" error. The DataFusion session's `init_executor()` now accepts a tokio runtime handle, matching the pattern used by `AgentPane`.

- **Command palette always accessible**: Fixed `:` not opening the command palette after navigating in certain views (e.g., SQL execution plan viewer, diff viewer). The `:` handler is now processed globally at the start of `handle_viewport_keyboard()` before any overlay blocking checks, allowing commands like `:style` to be opened on top of any overlay (except when a text field has focus or the command palette is already open).

- **Style picker z-order over diff viewer**: Fixed the style picker appearing behind the diff viewer when opened on top of it, and keyboard navigation (j/k) in the style picker causing the diff viewer to scroll and come to the foreground. The diff viewer now renders earlier in the z-order (before style_picker and command_palette) and disables keyboard handling when another overlay is on top.

### Removed

- **Dead theme code in design.rs**: Removed ~580 lines of unused theme code including `white_theme()`, standalone `gruvbox_theme()`, `black_theme()` legacy wrapper, and 4 commented-out theme implementations. The theming system now uses only `dark_theme()` and `light_theme()` which are driven by the `AppTheme` enum.

- **Dead code cleanup across panes**: Removed unused fields and functions:
  - `SqlPane::get_tables()` - unused method (10 lines)
  - `TracingPane::api_key` and `staging_api_key` - unused fields
  - `WaterfallChart::zoom_level` - unused field (zoom feature not implemented)

- **Component trait cleanup**: Removed `set_api_key()` and `set_staging_api_key()` from the `Component` trait - they were stored but never read by any component. Removed the `api_key` field from `QueryPane`, `TimeSeriesChart`, `Buffer`, and the propagation code from workspace rendering.

- **AgentPane component**: Removed the `AgentPane` viewport pane in favor of the `AgentPanel` overlay. AI agent conversations now use the right-side panel exclusively, providing a cleaner separation between observability content (viewport panes) and AI assistance (overlay panel). The inline content types (`InlineChart`, `InlineSource`, `InlineSearchResults`) have been moved to a new `inline_content.rs` module.

- **Workspace tab bar**: Removed the workspace tab bar (barbar.nvim-style) at the top of the editor. The editor now manages a single workspace directly instead of multiple tabs. Related keyboard shortcuts (`Shift+N`, `Shift+P`, `Shift+T`, `Shift+X`) have been removed. The `:q` command now quits the application instead of closing the current tab.

- **Theme-based chart palettes**: Each of the 13 themes now has its own unique 8-color palette for time series visualization:
  - **Dark themes** (9 themes): Vibrant saturated colors optimized for dark backgrounds
    - Dark: Emerald accent with vibrant complements
    - Midnight: Electric neon cyberpunk
    - Nord: Aurora borealis colors from the Nord spec
    - Catppuccin: Pastel macchiato palette
    - Ayu: Warm sunset gradients
    - Bergman: Silver cinema with muted accents
    - Aurora: Northern lights teal/green
    - Graphite: Industrial orange/steel
    - Ink: Silver monochrome editorial
  - **Light themes** (4 themes): Muted/desaturated colors for elegance on light backgrounds
    - Light: Classic muted professional
    - Stockholm: Nordic clarity blues
    - Midsommar: Swedish summer brights
    - Skärgård: Baltic sea blues
  - Commit markers now also adapt per-theme for consistent styling
  - Charts now feel native to each theme's aesthetic
  - Colors update dynamically when switching themes (no app restart needed)

### Changed

- **Theme is now a user preference**: Theme is stored in user settings, not per-workspace. Loading a workspace no longer overrides your theme preference. This means your preferred theme stays consistent across all workspaces.

- **Streamlined theme selection**: Reduced from 24 to 13 curated themes for a more focused experience

- **Improved Style Picker UX**:
  - Chart palette preview dots showing all 8 series colors for each theme
  - Cleaner two-row layout: UI palette bar + theme name on top, chart dots below

- **Unified Style Picker overlay** (`:style` command): A combined theme and font selector:
  - Side-by-side layout: Theme panel on the left, Font panel on the right
  - **Live preview**: Both theme and font change in real-time as you navigate
  - See the immediate impact of font changes while browsing themes
  - Theme panel with color palette bars (bg/elevated/accent/text preview)
  - Font panel with Rust code sample rendered in each font's actual typeface
  - Syntax highlighting in font preview: keywords in accent color
  - Key cap styled keyboard hints in footer
  - Panel switch animation with subtle glow effect
  - Prominent active panel header with dot indicator and larger text
  - Keyboard navigation: Tab/h/l to switch panels, ↑↓/jk to navigate, Enter to select, Esc to cancel
  - Vim-style controls: Ctrl+N/P for navigation
  - Focus indicator bar shows active panel
  - "●" indicator marks current theme/font
  - Cancel restores both original theme and font
  - Aliases: `:st`, `:theme`, `:t`

### Removed

- **Old Theme Picker**: Removed separate `:theme` command overlay in favor of unified Style Picker
- **Font command**: Removed `:font` command - use `:style` for both theme and font selection

- **Sumi theme** - Japanese ink calligraphy-inspired monochrome light theme:
  - Warm washi paper base (#F5F3EF) with brush black accent (#1A1A1A)
  - Elegant, minimal, zen-like aesthetic inspired by traditional Japanese calligraphy
  - Traditional Japanese color palette: Sumi black, Beni vermillion (#B43C32), Matcha green (#466E46), Ai indigo (#465A78), Yamabuki gold (#B48232)
  - Aliases: `su`, `calligraphy`, `brush`, `japanese`, `washi`

- **Five new Swedish-inspired light themes**: Added five distinctive light themes celebrating Swedish culture and nature:
  - **Midsommar** - Swedish summer celebration with flag blue accent (#2563EB). Bright summer white base (#FEFEF5) capturing the endless daylight of Swedish midsummer. Aliases: `mid`, `summer`, `swedish-summer`, `flagblue`
  - **Falu** - Swedish countryside with iconic Falu red accent (#802418). Weathered wood white base (#FAF8F5) inspired by the distinctive rödfärg seen on traditional Swedish barns and houses. Aliases: `fa`, `red`, `rodfarg`, `countryside`, `barn`
  - **Birch** - Swedish forest (björkskog) with birch leaf green accent (#4A5D23). Birch bark white base (#FAFBF8) evoking the light, airy feel of Swedish birch forests. Aliases: `bi`, `bjork`, `bjorkskog`, `forest-light`
  - **Fika** - Swedish coffee culture with coffee brown accent (#6F4E37). Cream/milk white base (#FBF8F4) celebrating the ritual of Swedish fika (coffee break). Aliases: `fi`, `coffee`, `kaffe`
  - **Skärgård** - Stockholm archipelago with deep Baltic blue accent (#1E4D6B). Sea mist white base (#F8FBFC) inspired by the Stockholm island chain's maritime aesthetic. Aliases: `sk`, `archipelago`, `baltic`, `coastal`
  - All themes feature complete color palettes optimized for code and time series visualization

- **Four new original premium themes**: Added four distinctive themes with original color palettes:
  - **Graphite** (dark) - Industrial precision with molten orange accent (#E85D04). Deep warm charcoal base (#121214) inspired by foundry aesthetics, warm off-white text for excellent readability. Aliases: `graph`, `industrial`, `foundry`, `molten`
  - **Ink** (dark) - Monochrome editorial with pure silver accent (#C0C0C8). Blue-black base (#0A0A0F) for data-focused precision, cool typography-inspired palette. Aliases: `i`, `editorial`, `monochrome`, `silver`
  - **Bone** (light) - Museum ivory with charcoal accent (#374151). Pure ivory base (#FEFDFB) with gallery-white aesthetics, elegant museum-like minimalism. Aliases: `bo`, `museum`, `ivory`, `gallery`
  - **Sand** (light) - Desert warmth with terracotta brown accent (#9A6B4C). Warm sand base (#FAF7F2) with earthy tones, inspired by desert landscapes and natural materials. Aliases: `sa`, `desert`, `terracotta`, `earthy`
  - All themes feature complete color palettes for backgrounds, borders, text, accents, syntax highlighting, charts, heatmaps, and diff visualization

- **Five new Scandinavian light themes**: Added five premium light themes inspired by Nordic minimalism and design:
  - **Stockholm** - Clean Nordic white with slate blue accent (#5C7A99). IKEA-inspired clarity with warm off-white backgrounds and maximum readability. Aliases: `sthlm`, `sto`, `ikea`, `nordic-white`
  - **Copenhagen** - Danish hygge light with terracotta rose accent (#C4847A). Cozy warmth with creamy linen backgrounds and natural ceramic tones. Aliases: `cph`, `hygge`, `danish`, `linen`
  - **Helsinki** - Finnish functionalism with forest green accent (#4A7C6F). Alvar Aalto-inspired with cool paper backgrounds and nature-meets-tech aesthetic. Aliases: `hel`, `finnish`, `aalto`, `forest`
  - **Oslo** - Norwegian fjord light with deep fjord blue accent (#3D6B8C). Crisp glacier white backgrounds optimized for data visualization. Aliases: `osl`, `fjord`, `norwegian`
  - **Reykjavik** - Icelandic minimal with volcanic gray accent (#2D3436). Extreme minimalism with pure snow backgrounds and stark contrasts. Aliases: `rvk`, `iceland`, `volcanic`, `snow`
  - All themes feature carefully tuned contrast ratios for excellent readability and data visualization

- **Two new Nordic-inspired dark themes**: Added two dark theme presets with Scandinavian inspiration:
  - **Bergman** - Swedish foggy noir with steel silver accent (#A8B0C0). Inspired by Ingmar Bergman's cinematic style (The Seventh Seal), featuring muted foggy charcoal backgrounds with cool, understated silver tones for a contemplative, moody aesthetic.
  - **Aurora** - Northern Lights with aurora teal accent (#7EE8B8). Deep night sky backgrounds with vibrant teal-green accents reminiscent of the aurora borealis, designed for excellent data visualization contrast.
  - Both themes include complete color palettes for all UI elements, syntax highlighting, diffs, and chart visualizations
  - Use `:theme bergman` (or aliases: `b`, `noir`, `fog`, `seventh-seal`) or `:theme aurora` (or aliases: `ar`, `northern`, `lights`, `borealis`) to switch

- **Five new premium themes**: Added five new theme presets designed for premium UX feel, optimized for both code viewing and time series/chart visualization:
  - **Midnight** - Deep space blue with electric blue accent (#3B82F6). Ultra-dark with navy undertones, inspired by Figma/Linear dark modes.
  - **Rosé Pine** - Soft muted elegance with rose pink accent (#EBBCBA). Popular aesthetic theme with calming, low-contrast colors.
  - **Catppuccin (Mocha)** - Warm pastel dark with mauve accent (#CBA6F7). Modern theme with warm, pastel accents that's easy on the eyes.
  - **Ayu** - Soft amber warmth with orange accent (#FFB454). Refined dark theme with warm golden tones for a sophisticated feel.
  - **Vesper** - Ultra-premium dark with warm amber accent (#FFC799). Near-black with subtle warm accents, inspired by fintech/trading platforms.
  - All themes include complete color palettes for: backgrounds, borders, text, accents, syntax highlighting, visualization/chart colors, diff colors, and UI elements
  - Theme-specific heatmap gradients for data visualization
  - Use `:theme <name>` or aliases (`:theme cat`, `:theme rose`, `:theme midnight`, etc.) to switch

- **LogQL parser and autocomplete crate** (`enya-logql`): Lightweight LogQL parser for context-aware autocomplete, mirroring the architecture of `enya-promql`:
  - **Lexer** (`lexer.rs`): `ScanState` for tracking nesting depth, `TokenHint` enum, `partial_at_cursor()`, `last_token_before()` for cursor context detection
  - **Completion** (`completion.rs`): `Context` enum for 15 autocomplete states (stream selectors, pipe stages, line filters, grouping, etc.), `analyze()` for context detection, `syntax_suggestions()` for static suggestions
  - **Validation** (`validation.rs`): Basic structural validation (balanced brackets, stream selector presence, function argument validation)
  - **LogQL syntax support**: Stream selectors `{}`, line filters (`|=`, `!=`, `|~`, `!~`), parsers (`json`, `logfmt`, `pattern`, `regexp`, `unpack`), filter expressions (`line_format`, `label_format`), range aggregations (`rate`, `count_over_time`, `bytes_rate`), aggregations (`sum`, `avg`, `topk`)

- **LogQL autocomplete in BufferEditor**: `QueryCompletion` now supports both PromQL and LogQL via `QueryLanguage` enum:
  - `QueryLanguage::PromQL` (default) for metric queries
  - `QueryLanguage::LogQL` for log queries
  - `set_language()` method to switch completion mode
  - Context-aware suggestions for LogQL pipe stages, parsers, and line filters
  - Language-aware hint text: PromQL shows `rate(http_requests_total[5m])`, LogQL shows `{app="nginx"} |= "error" | json`

- **Logs pane component**: New `LogsPane` component for metric→log correlation, enabling drill-down from metric spikes to see actual SQL queries (or other logs) during that period:
  - **Editable LogQL query** via modal BufferEditor (same UX as QueryPane - press `e` to edit, `:w` to save)
  - Edit button overlay in top-right corner for quick access
  - Scrollable table view with color-coded log levels (Error=red, Warn=yellow, Info=blue, Debug=gray)
  - Level filter dropdown (All, Error, Warn, Info, Debug, Trace)
  - Text search filter for searching within log messages
  - Timestamp formatting (HH:MM:SS.mmm)
  - Loading skeleton animation during data fetch
  - Configurable backend via `LogsBackend` enum (Demo or Loki)
  - Implements `Component` trait for integration with workspace tile system
  - `add_logs_pane()` for demo mode, `add_loki_pane()` for real Loki servers

- **Loki logs backend**: Full `LokiClient` implementation in `enya-client` for querying Grafana Loki:
  - HTTP API integration via `/loki/api/v1/query_range`, `/loki/api/v1/labels`, `/loki/api/v1/status/buildinfo`
  - LogQL query building from labels and text filters
  - Response parsing with log level detection from labels (`level`, `severity`) or message patterns
  - Health check support for connection validation
  - Works on both native (tokio) and WASM (wasm-bindgen-futures)

- **Logs command palette commands**:
  - `:logs` - Open demo logs pane with synthetic SQL query data
  - `:loki <url>` - Connect to a Loki server (e.g., `:loki localhost:3100`)

- **Chart drilldown to logs**: Double-click on any time series chart to open a logs pane centered on that timestamp:
  - Creates a 5-minute window around the clicked point
  - Enables quick correlation from metric spikes to logs
  - Works with both demo and Loki backends
  - `ChartInteraction::DrilldownLogs` event propagates from chart → visualization → query pane → workspace

### Changed

- **Refactored query completion helpers**: Consolidated duplicate code between PromQL and LogQL completion handlers:
  - Unified `find_word_start()` function with language-specific delimiters parameter
  - Added `COMMON_DELIMITERS` constant for shared delimiter set
  - Extracted helper methods: `push_item()`, `push_operators()`, `push_durations()`, `push_tag_keys()`, `push_tag_values()`, `push_metrics()`
  - Reduced ~200 lines of duplicated code

- **Team chat channels panel with Split View**: A Zed-inspired left sidebar for team collaboration with channels, threads, and inline chat:
  - `Channel` - Hierarchical channels with kinds (General, Incidents, Deployments, Alerts, Custom)
  - `Thread` - Conversation threads with priority levels (Normal, High, Critical) and status tracking
  - `ChatMessage` - Messages with support for @mentions (users, agents, charts)
  - `ChatState` - State management for channels, threads, and messages with demo data
  - `ChannelsPanel` - Threads-first sidebar component showing:
    - Active threads at the top (pinned, critical, or with unread) for quick incident access
    - Collapsible channel tree with unread badges
    - Slack/Discord-style team presence section with member names
  - **Split View Chat** (Option A): When a channel or thread is selected, the panel expands to show:
    - Left: Compact sidebar with threads, channels, and team members
    - Right: Chat message view with conversation history
    - Message input with @mention support and send button
    - Chart embed button for inline plots in chat
    - Dynamic panel sizing (220-400px sidebar only, 400-800px with chat open)
    - Escape key to close split view
  - `ChatView` component with:
    - Header showing channel/thread name with back and close buttons
    - Message bubbles with author avatars and timestamps
    - Different styling for own messages, other users, and AI agents
    - Agent badge for AI-generated messages
    - System messages (centered, italic)
    - Message reactions display
    - Inline chart embed placeholders (click to navigate)
  - **Inline time series charts in messages**: Share metrics data directly in chat with snapshot embedding:
    - `InlineChart` struct holds `Vec<Series>` data captured at share time
    - Charts render as full `TimeSeriesChart` components with axes and values
    - Compact mode (legend hidden) fits naturally in message flow
    - Data is frozen at share time - all team members see identical metrics
    - Themed frame with chart icon header matching message bubble style
    - Demo data shows P99 latency spike scenario with realistic data points
  - **@pane autocomplete for chart sharing**: Type `@` in chat to mention and embed pane data:
    - `PaneInfo` struct provides pane names and series data for autocomplete
    - Autocomplete popup appears above input when typing `@` followed by any characters
    - Fuzzy filtering matches pane names as you type
    - Arrow keys, Tab, and Enter for navigation and selection
    - Escape to dismiss the autocomplete popup
    - Selected pane creates an `InlineChart` snapshot attached to the message
    - Visual indicator shows when a chart is pending attachment
    - Series count shown in autocomplete dropdown for each pane
  - **#commit reference autocomplete in chat**: Type `#` in chat to reference commits and open diff viewer:
    - `CommitInfo` struct provides commit hash, message, timestamp, and diff data
    - Autocomplete popup shows commit history as you type `#` followed by query
    - Commit hash displayed in monospace with full message and relative timestamp
    - Fuzzy search filters by hash and commit message
    - Arrow keys, Tab, and Enter for navigation and selection
    - Selected commit inserts clickable `[#hash]` reference in message
    - Clickable commit references in rendered messages open the diff viewer
    - Git commit icon with premium accent styling for commit links
    - Codebase search integration via `SearchChatCommits` action
  - **Extended inline visualizations in chat**: Beyond time series charts, team members can share various data visualizations:
    - `InlineVisualization` enum supporting multiple visualization types
    - `InlineStat` - Single stat cards showing key metrics with trend indicators:
      - Large value display with label and optional subtitle
      - Trend arrows (up/down/neutral) with semantic colors (red for up = bad, green for down = good)
      - Previous value comparison display
    - `InlineTable` - Tabular data for structured information:
      - Header row with column names
      - Auto-sized columns with truncation for long values
      - "... and N more rows" indicator when rows exceed max display
    - `InlineBarChart` - Horizontal bar charts for categorical comparisons:
      - Labels, bars, and value annotations
      - Background track with filled bar overlay
      - Cycling color palette (accent, blue, purple, yellow)
      - Max value scaling for proportional bar widths
    - Theme-aware colors in `ChatColors` for trends and chart palettes
    - Demo data showcases all visualization types in the incident response thread
  - Premium UX styling:
    - Theme-aware colors (Dark, Nord, Gruvbox, Light themes)
    - Smooth hover states with subtle backgrounds
    - Left accent bars for selected items
    - Pill-style unread badges with theme accent colors
    - Subtle section dividers
    - Premium tooltips for team members with presence status
  - Keyboard navigation (Tab to switch sections, arrows to navigate - j/k reserved for viewport)
  - AI agent integration through @agent mentions in chat messages
  - **Workspace integration**: Channels panel shows as a resizable left sidebar when team mode is active
    - Auto-opens when `:team demo` command activates team mode
    - Toggle with `Space+g` keyboard shortcut
    - Dynamic width based on split view state
  - **Full-screen chat takeover**: When a channel or thread is selected, the chat takes over the entire workspace
    - Hides the plots viewport to provide a full-screen chat experience
    - Escape key or back button restores the normal viewport with plots
    - `Space+g` shortcut works to close chat and return to viewport
    - `:` command palette works when chat input is not focused
    - Proper layout with fixed header (48px), scrollable messages, and fixed input (64px)
    - Input area correctly respects the app status bar at the bottom

- **Chart annotations for team collaboration**: Pin comments to specific points or time ranges on charts for team communication:
  - `Annotation` - Data structure with message, author, priority (Normal/Important/Critical), and target (Point/Range/DataPoint)
  - Annotations render as vertical lines for all target types (matching commit marker style)
  - Hover over annotations to see author, message, and resolved status in a tooltip
  - Priority colors: blue (Normal), orange (Important), red (Critical), gray (Resolved)
  - "Add Annotation" action in team menu opens annotation editor overlay
  - Keyboard navigation: `]a` / `[a` to jump between annotations, `gn` to toggle visibility
  - Demo charts include sample annotations with varying priorities
  - Annotation editor modal with message input, priority selector, and save/cancel actions

- **Team collaboration state management**: Added a decoupled `TeamState` module for optional team collaboration features. The editor continues to work normally without team features enabled:
  - `TeamConfig` - Configuration struct with optional `server_url` and `auth_token`
  - `TeamState` - Wraps `TeamManager` from `enya-team-api` with a clean polling interface
  - Optional connectivity - `TeamState::default()` returns a disconnected state
  - Presence tracking - tracks online/idle/offline status for team members
  - Unread notification count for mentions
  - `status_info()` returns `None` when not connected, hiding team UI automatically
  - Integrated into `EnyaApp`: polls for team events each frame, passes status to status line and workspace
  - Space+t keyboard shortcut to toggle team menu (when connected)
  - Demo mode (`:team demo`) for testing the team UI without a backend
  - Sleek centered overlay design matching the unified finder UX:
    - Frosted glass styling with premium shadow
    - Keyboard navigation (↑/↓/j/k for items, Tab to switch sections, Enter to select, Esc to close)
    - Two sections: MEMBERS (with presence dots and viewing status) and ACTIONS
    - Hover and selection highlighting with accent colors

- **Pane descriptions**: Panes now support an optional `description` field for providing context:
  ```toml
  [[panes]]
  query = "histogram_quantile(0.99, ...)"
  name = "P99 Latency"
  description = "99th percentile latency - alert threshold is 500ms"
  ```
  - An info icon (ℹ) appears in the pane toolbar when a description is set
  - Hover over the icon to view the description
  - Tab title shows ℹ indicator for panes with descriptions

- **Auto-refresh interval support**: Workspaces can now configure automatic query refresh via the `[time]` section:
  ```toml
  [time]
  preset = "1h"
  refresh = "30s"  # Options: off, 10s, 30s, 1m, 5m, 15m
  ```
  - Use `:refresh <interval>` (or `:r`) command to change at runtime
  - A countdown timer appears in the toolbar when refresh is active
  - Refresh interval is saved when exporting workspaces

### Changed

- **Refactored chat module colors**: Extracted shared color helpers into `chat/theme_helpers.rs`
  - New `ChatColors` struct provides theme-aware colors for chat UI elements
  - Consolidates duplicate color logic from `chat_view.rs` and `channels_panel.rs`
  - Semantic color methods: `selection_bg()`, `hover_bg()`, `own_message_bg()`, `agent_message_bg()`, etc.
  - All themes (Dark, Light, Nord, Gruvbox) have consistent color semantics

- **Team API runtime fix**: Fixed crash when using `:team connect` command on native platforms:
  - `TeamClient` now requires a `tokio::runtime::Handle` for spawning async HTTP requests
  - `TeamManager` accepts and passes runtime handle to client on connect
  - `TeamState` sets runtime via `set_async_runtime()` before connecting
  - Error message shown if connect attempted without runtime handle set

- **Fixed WebSocket 403 error**: WebSocket now connects with the correct team ID from the API:
  - Added `list_teams()` method to `TeamClient` to fetch user's teams
  - Two-phase authentication: first fetches user, then fetches teams
  - WebSocket connects using the actual team ID from the server instead of a random UUID
  - `pending_teams` and `pending_user` fields track auth state during connection

- **WebSocket status indicator in team status**: The status line now shows real-time WebSocket connection state:
  - `WsState` enum: Connected, Connecting, Reconnecting, Failed, Disconnected
  - Visual indicators: no symbol when connected, `...` connecting, `~` reconnecting, `!` failed, `-` disconnected
  - Hover tooltip shows detailed WebSocket status (e.g., "Real-time: connected")
  - `TeamStatusInfo` now includes `ws_state` field populated from `TeamManager`
  - Demo mode simulates connected state for testing

- **Cloud backend channels API**: Full REST API and WebSocket support for team chat channels:
  - New `enya-team-api` types: `Channel`, `ChannelKind`, `ChatThread`, `NewChannel`, `NewThread`, `InlineChartData`
  - New `TeamEvent` variants: `ChannelCreated`, `ThreadCreated`, `ThreadResolved`
  - Cloud API endpoints (`enya-cloud`):
    - `GET/POST /teams/{team_id}/channels` - List and create channels
    - `GET /teams/{team_id}/channels/{channel_id}` - Get channel details
    - `GET/POST /teams/{team_id}/channels/{channel_id}/threads` - List and create threads
    - `GET/POST /teams/{team_id}/channels/{channel_id}/threads/{thread_id}/messages` - Messages in threads
    - `POST /teams/{team_id}/channels/{channel_id}/threads/{thread_id}/resolve` - Mark thread resolved
  - Database models: `DbChannel`, `DbChannelThread` with conversions to API types
  - Real-time broadcasting of channel/thread events via WebSocket
  - Client methods: `list_channels()`, `create_channel()`, `list_channel_threads()`, `create_thread()`, `list_channel_messages()`, `send_channel_message()`
  - `TeamManager` caching: Automatic caching for channels, threads, and messages with cache invalidation on create operations
  - Real-time event handling in `TeamState::poll()`: Updates caches when `ChannelCreated`, `ThreadCreated`, and `ThreadResolved` events arrive via WebSocket

- **Chat module types use UUID**: Editor chat types (`ChannelId`, `ThreadId`, `MessageId`) now use `Uuid` to match API types, enabling seamless synchronization with the cloud backend:
  - `Channel::from_api()`, `Thread::from_api()`, `ChatMessage::from_api()` - Convert API types to editor types
  - `ChannelKind::from_api()` / `ChannelKind::to_api()` - Convert between editor and API channel kinds

### Fixed

- **Chat messages now appear after sending**: Fixed bug where messages added through the chat input would not appear in the conversation. The root cause was that `workspace.chat_state` was being cloned from `team_state.chat_state` on every frame, overwriting any locally added messages. Messages are now added directly to `team_state.chat_state` via a new `WorkspaceAction::SendChatMessage` action, ensuring they persist across frames.

- **Channel and thread creation now works**: Fixed clicking the "+" button to create channels and threads. Added `WorkspaceAction::CreateChannel` and `WorkspaceAction::CreateThread` actions that route through `EnyaApp` to `TeamState`. In demo mode, channels/threads are created locally; in live mode, API calls are made to the cloud backend. A notification appears to confirm the action.
  - Added `pending_channel_creates` to track API create operations and poll for completion
  - When API responds, the channel is added to cache and a `ChannelCreated` event is emitted
  - Channels now have unique names with timestamps (e.g., "channel-1234")
  - Fixed sync logic to update chat state when manager has more channels (not just on initial load)

- **ChatState passed by reference instead of cloning**: Refactored to pass `&ChatState` to `workspace.show()` instead of cloning every frame. This improves performance and clarifies ownership - `TeamState` owns the `ChatState`, workspace only borrows it for rendering.

### Changed

- **Renamed `[codebase]` to `[git]` in workspace config**: The workspace TOML section for git repository integration has been renamed from `[codebase]` to `[git]` to better reflect its purpose. The internal `CodebaseConfig` struct is now `GitConfig`. The fields (`url`, `branch`, `language`) remain the same.

- **Inline endpoint support in workspace config**: You can now specify the Prometheus endpoint directly in the `[workspace]` section for simpler workspaces:
  ```toml
  [workspace]
  name = "my-dashboard"
  endpoint = "http://localhost:9090"
  ```
  The separate `[connection]` section is still supported for advanced options like `api_key`, and takes precedence if both are specified.

### Changed

- **Beautiful diff viewer styling (GitHub/delta-inspired)**: Completely redesigned the diff viewer overlay and preview pane with modern, professional styling:
  - **Side panel file list**: Replaced horizontal file tabs with a vertical "Changed Files" panel on the right side, similar to VS Code and Conductor. Shows file icons, names, and +/- stats for each file with click-to-select and hover effects.
  - **Split view toggle**: Press `s` to switch between unified diff and side-by-side split view. Split view shows old code on the left and new code on the right with aligned lines, making it easy to compare changes. Paired deletions and additions are shown on the same row.
  - **Word-level diff highlighting**: Uses the `similar` crate to compute character-level differences between paired add/delete lines. Changed characters are highlighted with brighter background colors, making it easy to see exactly what changed within a line.
  - **Dual line numbers**: Shows both old and new line numbers in a dark gutter area, making it easy to reference specific lines.
  - **Colored gutter stripes**: Green stripe for additions, red stripe for deletions - a subtle but effective visual cue on the left edge of each changed line.
  - **Theme-aware diff colors**: Diff colors now respect the active theme (Dark, Light, Nord, Gruvbox). Each theme has appropriate colors for additions, deletions, context lines, hunk headers, and gutter stripes. Light theme uses readable dark green/red text on light tinted backgrounds.
  - **Stat badges**: Separate green (+N) and red (-N) badges in the header showing additions and deletions per file.
  - **Larger, more readable overlay**: Increased popup dimensions (85% width/height) for better code review experience.
  - **Consistent preview styling**: The unified finder's diff preview now uses the same beautiful styling with gutter stripes and proper colors.

### Added

- **Unified fuzzy finder (Telescope-style)**: Added a single unified finder that consolidates all search functionality into one modal with prefix-based mode switching. Features include:
  - Single entry point with `Space f` keybinding
  - **Prefix-based modes** for quick mode switching:
    - (no prefix) - Search live metrics from Prometheus
    - `@` - Search indexed metrics from source code
    - `!` - Search alert rules from codebase
    - `#` - Search git commits
    - `:` - Execute editor commands
    - `>` - Open/switch workspaces
    - `/` - Search everything in codebase
  - Tab key cycles through modes
  - **Preview pane** shows context-aware details for each result type:
    - **Metrics/Alerts**: Source code preview with tree-sitter syntax highlighting (matching the full Source Preview Overlay styling) and target line highlighting
    - **Live metrics**: Tags preview showing available label keys and values
    - **Commits**: Diff preview with +/- line highlighting
  - Nucleo-based fuzzy matching for fast, typo-tolerant search
  - Full Tantivy integration for codebase search modes
  - WASM-compatible: codebase search modes disabled on WASM, metrics/commands/workspaces work on both platforms

- **Tantivy full-text search index** (native-only): Added a Tantivy-based full-text search index for the codebase. Features include:
  - Indexes metrics, alerts, and git commits in a persistent on-disk index stored in `{repo}/.enya/tantivy/`
  - Schema supports metric name, kind, labels, function context, alert expressions, severity, commit messages, and file locations
  - Full-text search with relevance scoring via Tantivy's BM25 algorithm
  - Filter searches by type (metrics, alerts, commits, or all)
  - `TantivyCodebaseIndex` API: `open_or_create()`, `rebuild()`, `rebuild_with_commits()`, `search()`, `search_metrics()`, `search_alerts()`, `search_commits()`
  - Automatically indexes up to 1000 recent git commits when building the index
  - Automatic reader reload after index updates for immediate search visibility
  - Metadata persistence tracking indexed commit, timestamp, and document counts
  - AI agent tool (`SearchCodebaseTool`) for codebase search via the Agent mode
  - WASM-compatible: All Tantivy code is behind `#[cfg(not(target_arch = "wasm32"))]`

- **Agent `search_codebase` command**: AI agents can now use the `search_codebase` command to search the Tantivy index for metrics, alerts, and commits. This provides faster, ranked full-text search compared to using `git log --grep` directly. Results are displayed inline in the agent conversation with relevance scores, file paths, and line numbers.

- **Status line Tantivy progress**: The status line now shows detailed progress while the Tantivy full-text search index is being built in the background. Progress includes:
  - Current phase (Fetching commits, Indexing metrics, Indexing alerts, Indexing commits, Finalizing)
  - Progress count (e.g., `[42/100]`)
  - Current item name (metric name, alert name, or commit hash with message)

- **Improved cloning status**: The status line now shows the repository name being cloned (e.g., "Cloning enya...") instead of the generic "Cloning repo..." message.

### Fixed

- **Unified finder search modes not working**: Fixed an issue where search results weren't appearing or would briefly appear then disappear when using mode prefixes (`@` for metrics, `#` for commits, `!` for alerts). Three bugs were fixed:
  1. The mode was being read from a stale cache instead of parsing from the current query prefix
  2. The codebase search was incorrectly running for Metrics mode, interfering with live Prometheus metric search
  3. The internal debounce `refresh_results()` was clearing codebase results that were already set by the workspace

  Now the mode is parsed from the query in real-time, each mode uses the correct search backend, and the internal debounce only runs for Metrics mode (codebase modes are handled immediately by the workspace).

- **Match highlight color visibility**: Added a new `highlight_match_text()` theme method that provides bright, visible colors for fuzzy match highlighting in search results. The previous `highlight_match()` color was too dark for text foreground use. Match highlights now use bright gold/amber colors that stand out clearly against the background.

- **Diff viewer showing full commit diff**: Fixed an issue where the diff viewer overlay only displayed the first file of a commit. The diff viewer now correctly shows all files in a commit, with n/p navigation between files.

### Added

- **Codebase finder overlay (Space+c)**: Added a new telescope-style fuzzy finder for searching the codebase. Features include:
  - Full-text search across metrics, alerts, and git commits using the Tantivy index
  - Filter buttons to narrow results by type (All, Metrics, Alerts, Commits)
  - Tab key cycles through filter modes
  - **Tabbed preview pane** with Code, Diff, and Blame views:
    - **Code tab**: Shows result details (type, severity, file location, code snippets)
    - **Diff tab**: Delta/GitHub-style diff rendering with green background for additions, red for deletions, blue for hunk headers
    - **Blame tab**: Placeholder for upcoming git blame integration
  - Arrow keys (←/→) or Ctrl+Tab/Shift+Tab to switch preview tabs
  - j/k or arrow keys (↑/↓) for navigation, Enter to select, Escape to close
  - Selecting a metric or alert opens the source preview overlay at the definition location
  - Native-only (not available on WASM)

### Changed

- **Theme-aware egui visuals**: The `dark_theme()` function in `design.rs` now uses theme-aware colors instead of hardcoded Obsidian Glass palette constants. This ensures that egui widgets (including egui_plot backgrounds, panels, and all widget visuals) correctly reflect the selected theme's colors. Previously, switching to Nord or Gruvbox themes would leave the plot background as black; now each theme uses its own appropriate background color.

- **Theme-adaptive logo**: The Enya logo now adapts to the current theme:
  - **Dark** (default): Uses the original branded logo with full color
  - **Light**: Uses the grayscale logo directly (ink on paper aesthetic)
  - **Other themes** (Nord, Gruvbox): Uses a grayscale version with overlay blend tinting that preserves the logo's depth and shading while applying the theme's accent color. Dark areas stay dark, mid-tones and highlights receive the theme color. Textures are cached per theme for efficient rendering.

### Added

- **Extensible theme presets**: The editor now supports multiple theme presets:
  - **Dark** (default) - Obsidian Glass with signature Enya emerald (#10B981)
  - **Nord** - Arctic blue (#88C0D0)
  - **Gruvbox** - Warm orange (#D65D0E)
  - **Light** - Paper/Ink aesthetic with warm cream backgrounds and grayscale syntax
  - Use `:theme <name>` to switch themes (e.g., `:theme gruvbox`, `:theme nord`, `:theme light`)
  - Use `:theme` (no argument) to cycle to the next theme
  - Theme can be configured in workspace TOML files with `theme = "dark"` etc.

### Changed

- **Platform-specific GPU backends**: wgpu now only enables the Metal backend on macOS and Vulkan on Linux/Windows, instead of enabling both everywhere. Combined with X11/Wayland being Linux-only, this reduces compile time and binary size on each platform.

- **Disable egui default fonts**: Since we bundle our own fonts (Departure Mono, Maple Mono, JetBrains Mono, Iosevka), we no longer include egui's embedded default fonts (`epaint_default_fonts`). This reduces binary size.

- **Use ring instead of aws-lc for TLS**: Switched from aws-lc-sys (heavy C dependency) to ring for rustls crypto, reducing dependency count by ~35 crates and significantly improving compile times.

- **Additional language grammars are now optional**: Go, Python, and JavaScript/TypeScript syntax highlighting are controlled by the `all-languages` feature flag (enabled by default). Rust highlighting and codebase integration (git, metrics discovery) are always available on native builds. Build with `--no-default-features` to exclude the extra language grammars.

### Changed

- **Parallel query execution (Grafana-style refresh)**: Query execution now runs in parallel instead of sequentially. When refreshing the time range or triggering a manual refresh:
  - All panes fire their queries simultaneously using async promises
  - All panes show the loading skeleton animation at once
  - Each pane completes and displays data as soon as its query returns
  - Time range changes (via toolbar, keyboard shortcuts, or agent commands) now trigger automatic refresh of all panes
  - This significantly improves perceived performance with multiple panels

### Fixed

- **Landing page j/k navigation with mouse hover**: Fixed an issue where pressing j/k to navigate the landing page menu would be immediately overridden by a stationary mouse cursor hovering over a different item. Now mouse hover only updates the selection when the mouse actually moves, allowing keyboard and mouse navigation to coexist without conflict.

### Changed

- **Compact landing page layout**: Reduced logo size, text size, and spacing to fit better on smaller viewports (especially WASM). Removed the tagline to save vertical space.

- **Workspace creator on WASM**: The "Create workspace" option on the landing page now shows a two-step workspace creator overlay on WASM (name, then endpoint). On native, the full three-step wizard (name, endpoint, git repo) is still shown.

### Added

- **Native app promo overlay (WASM only)**: When using the web version, a frosted glass overlay appears on the landing page highlighting features only available in the native desktop app:
  - Git integration for cloning repos and viewing diffs
  - AI agents for intelligent metric analysis and query suggestions
  - Local workspace persistence with full filesystem access
  - Includes a "Download for macOS" link to the native app
  - Press Enter or Escape to dismiss and continue to the web version
  - Overlay only shows once per session (remembers dismissal)

- **Vim-style window movement (Ctrl+W h/j/k/l)**: Move the focused pane to the edge of the viewport in the specified direction, matching Neovim's window movement behavior:
  - `Ctrl+W h` - Move pane to far left (becomes leftmost vertical split)
  - `Ctrl+W j` - Move pane to bottom (becomes bottom horizontal split)
  - `Ctrl+W k` - Move pane to top (becomes top horizontal split)
  - `Ctrl+W l` - Move pane to far right (becomes rightmost vertical split)
  - New "Window Movement" section added to the which-key overlay (`?`)

- **Merge panes into tabs (Ctrl+W t h/j/k/l)**: Merge the focused pane into a tab container with the pane in the specified direction:
  - `Ctrl+W t h` - Merge with pane to the left into a tab group
  - `Ctrl+W t j` - Merge with pane below into a tab group
  - `Ctrl+W t k` - Merge with pane above into a tab group
  - `Ctrl+W t l` - Merge with pane to the right into a tab group
  - If the target pane is already in a tab container, the focused pane is added to that container
  - Otherwise, a new tab container is created with both panes

### Changed

- **Premium agent input bar styling**: Enhanced the agent input bar to match the Obsidian Glass emerald theme:
  - Agent mode badge now uses signature emerald accent instead of amber for visual consistency
  - Added subtle emerald-tinted inner glow on the top edge for glass reflection effect
  - Added soft emerald bottom edge glow in dark mode for depth
  - Increased corner radius to 14px for a more premium feel
  - Enhanced shadow depth for better elevation
  - Suggestion pills are now clickable with emerald-highlighted commands and hover effects
  - Pills insert the command prefix when clicked for faster command entry

### Added

- **Slash commands for Agent mode**: Type `/` in the agent input bar to trigger command suggestions, similar to how `@` works for metric mentions. Core commands:
  - `/investigate` - Deep-dive analysis with correlations and anomalies
  - `/diff` - Compare metric states between two time ranges
  - `/query` - Generate PromQL from natural language
  - `/explain` - Explain what the current query or chart shows
  - Fuzzy search through commands with highlighted matches
  - Keyboard navigation (↑/↓ or Ctrl+J/K) and Tab/Enter to select
  - Commands are inserted into the input (e.g., `/investigate `) so you can continue typing
  - Combine with `@` mentions: `/investigate @http_requests_total why is it spiking?`

- **Configurable editor font**: Use `:font <name>` to switch between fonts. Available options: `maple` (Maple Mono), `departure` (Departure Mono), `jetbrains` (JetBrains Mono), `iosevka` (Iosevka). The preference is persisted across sessions. Departure Mono is the default.

### Changed

- **Minimal vim-like command palette**: Reduced from 24 to 11 commands. Added `:q`/`:quit` to close workspace and `:w`/`:write` to save workspace. Removed commands with keyboard shortcuts (`:zen` → `Z`, `:fullscreen` → `F`, `:home` → `Space+h`, `:diagnostics` → `Space+d`, `:help` → `?`) and non-vim-like commands (`:search`, `:connect`, `:prometheus`, `:close`, `:exit`, `:commits`, `:tabnew`, `:tabclose`, `:workspaces`, `:tutorial`, `:mksession`).
- **Unified hover styling across UI components**: All interactive list/menu components now use the same subtle hover styling as the landing page - a light 5% text color background with emerald accent color for icons on hover/select. Updated components include:
  - Workspace finder
  - Command palette
  - Query completion popup
  - Agent input bar mentions popup
  - Workspace tabs
  - Time range widget buttons
- **Premium layered pane focus border**: Pane focus now features a premium glass effect with three layered emerald borders - an outer subtle glow, mid glow, and crisp inner border. This creates depth and matches the Obsidian Glass theme. Uses brighter emerald in visual-multi mode to distinguish the cursor pane from selected panes.
- **Distinct tab vs pane focus colors**: Active tabs now use sky blue (#6EBEF8) for their outline, while pane focus uses emerald. This creates a clear visual hierarchy between tab selection and pane focus.
- **Consolidated chart colors**: Time series charts and demo visualizations now use the centralized `palette::chart::PALETTE` instead of hardcoded colors, ensuring consistent theming across all visualizations.
- **Premium query overlay styling**: The query overlay shown at the bottom of selected panes in visual-multi mode now features Obsidian Glass styling with an emerald accent bar on the left edge and a subtle top border line. Uses palette colors for consistent theming.
- **Viewport filter as bottom bar**: The `/` search filter now renders as a vim-style command line bar above the status line (like Agent mode) instead of a centered overlay. This is less intrusive and more consistent with vim's search behavior. Only available in Normal mode.
- **Zen mode hides workspace tabs**: The workspace tab bar is now hidden when in Zen mode for a fully distraction-free experience.
- **Simplified status modes**: Removed `StatusMode::Zen` and `StatusMode::Fullscreen` since these are display preferences, not modal states. The status line now stays in Normal mode with distinct secondary badges - purple "ZEN" and cyan "FULLSCREEN" - when these display preferences are active.

### Fixed

- **Popup positioned above cursor**: Both `/` slash command and `@` mention popups now appear directly above the trigger character position instead of centered, matching code editor autocomplete behavior. Popups are clamped to stay on screen.
- **Language icons now render correctly**: Fixed language icons (Rust, Go, Python, etc.) in the status bar not rendering. Updated the bundled Nerd Font (Symbols Nerd Font) to the latest version which includes the MDI `LANGUAGE_*` icons with actual language logos.
- **Time series x-axis visible in split panes**: Fixed an issue where the x-axis (time labels) was clipped when splitting panes horizontally (stacked). The chart now uses the actual remaining height after legend rendering to ensure the x-axis labels are always visible. Also improved height calculation for portrait-oriented panes (vsplit) to use a compact 20% max height.

### Changed

- **Enhanced indexing status**: The status line now shows the current file being indexed with a Zed-like format (e.g., "Indexing main.rs + 42 more") instead of just "Indexing [5/42]...". This provides better visibility into what files are being processed during codebase indexing.
- **Language configuration for codebase scanning**: Workspaces can now specify a `language` field in the `[codebase]` config section to limit metric scanning to a specific language. Supported values: `rust`, `go`, `python`, `javascript`, `typescript`. If not specified, all language scanners are used. This avoids indexing irrelevant files (e.g., Python `__init__.py` in a Rust codebase).
- **Improved file filtering**: The indexer now excludes more common static asset directories (`dist`, `build`, `public`, `assets`) and skips minified files (`*.min.*`).
- **Enhanced codebase status display**: The status bar now shows richer information when a codebase is configured:
  - During indexing: Shows language icon (Rust gear, Go gopher, Python logo, etc.) with the current file being indexed
  - When ready: Shows repo name with language icon and metrics count (e.g., " my-app | 42 metrics")

### Added

- **Vim-style Agent mode**: New modal agent mode for AI-assisted interactions, inspired by Neovim's modal editing:
  - Press `a` from Normal or Visual mode to enter Agent mode
  - Agent Input Bar appears above the status line for lightweight interaction
  - Status line shows "AGENT" mode indicator in amber
  - Quick command keys: `w` (what's wrong?), `y` (why?), `c` (compare), `r` (related), `e` (explain), `f` (fix), `s` (summarize), `h` (history)
  - **Agent operator pattern**: Vim-style operators like `aw`, `ae`, `ay`, `ac`, `ar`, `af`, `as`, `ah` for quick agent commands directly from Normal mode
    - `aw` - What's wrong? (triage current pane)
    - `ae` - Explain (describe the focused metric)
    - `ay` - Why? (root cause analysis)
    - `ac` - Compare (to baseline)
    - `ar` - Related (show correlated metrics)
    - `af` - Fix (remediation suggestions)
    - `as` - Summarize (incident summary)
    - `ah` - History (past similar incidents)
    - `aa` - Enter agent mode without sending a command
  - Natural language input for custom queries
  - Visual mode integration: selected panes automatically become context for the agent
  - Press `+`/`-` to add/remove focused pane from context, `Ctrl+C` to clear context
  - Press `Escape` to exit Agent mode
- **AgentInputBar component**: Standalone AI input component for Agent mode:
  - Four states: Ready, Typing, Processing, Response
  - Premium Obsidian Glass styling with frosted glass background and subtle inner highlight
  - Shows current AI provider (Claude/Codex) in an amber-accented badge
  - Context panes displayed in a subtle badge with pane icon
  - Direct AI integration via Claude Code CLI (no side panel dependency)
  - Streaming response support with live activity display
  - Processing state shows status message with tool use tracking
  - Response state can expand for longer responses
  - Activity display shows only the most recent activity for a compact UI
  - **Enya command support**: AI responses can now execute Enya commands (create_pane, set_time_range, search_metrics, etc.) just like the Agent Panel
  - **Immediate query execution for agent-created panes**: Panes created by AI commands now automatically load data from Prometheus without requiring a manual refresh
  - **Auto-exit agent mode**: Agent mode automatically closes after successful command execution, returning to Normal mode for seamless vim-style navigation

- **@ mention support for metrics**: Type `@` in the input to trigger a fuzzy finder popup for metrics:
    - Premium Obsidian Glass styling with frosted glass background, emerald accents, and inner highlight
    - Fuzzy search through all available metrics with emerald-highlighted match characters
    - Keyboard navigation with arrow keys (↑/↓) or Ctrl+K/J/N/P
    - Select with Enter or Tab to insert the metric name
    - Press Escape to dismiss the popup
    - Metrics are sourced from the connected Prometheus instance
    - Wide popup (520px) to accommodate long metric names

- **Workspace creation overlay** (native only): New three-step wizard for creating workspaces, matching the Tutorial overlay's frosted glass styling. Features include:
  - Step 1: Enter workspace name (prefilled with "my-workspace")
  - Step 2: Enter connection endpoint (prefilled with "http://localhost:9090")
  - Step 3: Optional git repository path for commit annotations
  - Progress dots showing current step
  - Keyboard navigation: `Enter` to proceed, `Escape` to cancel
  - Workspace tab is automatically renamed to the entered name
  - Workspace is automatically saved to disk after creation, making it discoverable in the workspace finder
  - On WASM, clicking "Create workspace" or the + button creates a workspace directly (overlay not available)

### Fixed

- **Agent-created panes now execute queries reliably**: Fixed a bug where panes created by AI would fail to load data because the query tracking used volatile TileIds that could change when egui_tiles restructured the tree during `ui()` calls. Now uses stable pane component IDs for tracking pending queries.

### Changed

- **Premium Obsidian Glass theme refinements**: Enhanced the dark theme with a more luxurious, high-end feel:
  - Refined background colors with subtle cool undertones for better depth perception
  - Richer emerald accent colors with added glow effects on interactive elements
  - Improved text hierarchy with warmer whites and refined secondary/tertiary tones
  - Enhanced syntax highlighting with more vibrant, harmonious colors
  - Premium shadow system with layered depth for popups and floating elements
  - Increased corner radius (4px → 6px) for a more refined look
  - Thicker cursor (2.5px) with slower, more elegant blink animation
  - Updated HTML background with subtle emerald radial gradient overlay
  - Improved query completion popup with triple-layer shadows and refined styling
  - **Glass overlay system enhancements**:
    - New `PremiumGlass` overlay variant with deeper shadows and inner glow
    - Frosted glass overlays now feature inner top-edge highlight for glass reflection effect
    - Enhanced backdrop with subtle vignette effect at screen edges
    - New `draw_premium_backdrop()` with centered emerald glow for branded modals
  - **Premium keyboard badges**: Key hints now feature subtle drop shadow and 3D top-edge highlight

- **Notification styling**: Updated notifications to use the obsidian glass emerald theme:
  - Frosted glass background matching other overlays
  - Uses semantic colors from the palette (emerald for success)
  - Improved shadow and border styling
  - Consistent with the overall design system

- **Moved heatmap into visualization module**: The `heatmap.rs` module is now located at `components/pane/visualization/heatmap.rs` alongside other visualization types for consistency.
- **Moved theme into ui module**: The `theme.rs` module is now located at `ui/theme.rs` alongside other UI primitives (colors, typography, icons, etc.).
- **Moved workspace_tabs into workspace module**: The `workspace_tabs.rs` module is now located at `workspace/tabs.rs` alongside other workspace-related code.
- **Alpha-nvim inspired landing page**: Redesigned the landing page with a minimal, centered layout inspired by alpha-nvim. Changes include:
  - Clean vertical menu with six actions: Find workspace (`w`), Create workspace (`n`), Tutorial (`t`), Docs (`d`), Shortcuts (`?`), and About (`i`)
  - Docs option opens the documentation website at enya.build/docs
  - Shortcuts option opens the which-key overlay with all keyboard shortcuts
  - About option opens the info overlay with version and build information
  - Menu items display icon, label, and keyboard shortcut in a single row
  - Vim-style navigation with `j`/`k` (or arrows) to move through menu items
  - Press `Enter` to activate the selected item, or use direct shortcuts
  - Keyboard hints footer showing available navigation keys
  - Content is vertically centered in the viewport
  - Large centered logo and Enya branding
  - Status line is hidden on the landing page (only shows in workspaces)

### Removed

- **Flamegraph visualization**: Removed the `FlamegraphViz` visualization type which was used for CPU/memory profiling visualization. This simplifies the visualization options to focus on time-series metrics.
- **wgpu GPU rendering module**: Removed the GPU-accelerated rendering module (`crate::wgpu`) that was used for heatmap rendering. Heatmaps now use CPU rendering exclusively.

### Added

- **Inline content in Agent Pane**: Agent responses can now include rich inline content:
  - Inline time series charts using the `TimeSeriesChart` component for consistent styling with dashboard charts
  - Inline source code previews with full tree-sitter syntax highlighting (Rust, Go, Python, JavaScript/TypeScript)
  - New agent commands: `show_inline_chart` and `show_inline_source`
  - Compact chart rendering with series colors matching the main dashboard palette
  - Source previews show file path, language badge, and highlight the target line

- **Agent Pane - first-class AI chat in viewport**: The AI agent is now a first-class pane in the viewport (not a side panel). Features include:
  - Press `Space+a` to create or focus an Agent pane
  - Runs in parallel with query/chart panes in the tile layout
  - Supports multiple concurrent agent conversations
  - Agent can execute editor commands (create panes, set time range, search metrics)
  - Implements the Component trait for full integration with the tile system

- **Agent Panel tool integration**: The AI agent can now execute editor commands to help build dashboards. Features include:
  - Agent receives context about the current editor state (connection, metrics, codebase, dashboard)
  - Agent can output `enya-command` blocks to create visualization panes with PromQL queries
  - Agent can set the time range (e.g., "1h", "6h", "24h", "7d")
  - Agent can open the metrics search with a pattern
  - Agent can show source code for metric definitions (`show_metric_source`)
  - Agent can show source code for alert rules (`show_alert_source`)
  - Commands are automatically parsed from agent responses and executed in the workspace

### Fixed

- **Keyboard shortcuts not firing in Agent mode**: Fixed an issue where typing `/` or `?` in the Agent Input Bar would incorrectly trigger the viewport filter or which-key overlay. These overlay handlers now check for `agent_mode_active` before consuming key events.
- **@ mention popup loses focus**: Fixed an issue where after selecting a metric from the @ mention popup, focus would not return to the text input. The input field now receives focus automatically after a selection is made, with the cursor positioned at the end of the text.
- **Agent-created panes not loading data**: Fixed an issue where panes created by the AI agent (via `create_pane` command) would not automatically load data from Prometheus. The `handle_agent_commands` function now requests a repaint after creating panes, ensuring query execution runs on the next frame.
- **Read tool file path in Agent Panel**: Fixed the Agent Panel not showing file paths for Read tool activities. Added `path` field lookup in addition to `file_path` for tool summary extraction.

### Changed

- **Agent Panel uses ACP protocol**: The Agent Panel now uses the Agent Client Protocol (ACP) via the `@zed-industries/claude-code-acp` npm package instead of the legacy CLI output format. This change:
  - Uses JSON-RPC 2.0 over stdio for agent communication
  - Implements the standard ACP session lifecycle (initialize → session/new → session/prompt)
  - Enables future support for other ACP-compatible agents
  - Streaming responses now use `session/update` notifications
  - Authentication is inherited from Claude CLI - Claude Max subscription works if you've run `claude /login`

### Added

- **Agent Panel (Claude Code integration)**: Press `Space+a` to toggle the agent panel, a side panel for chatting with Claude Code. Features include:
  - Real-time streaming responses from Claude Code CLI
  - Chat history with user/assistant messages
  - Enter to send, Escape to close
  - Native-only feature (CLI not available in WASM)

- **Query timeout handling**: Panes no longer get stuck in a perpetual loading state when the Prometheus backend is unreachable. Features include:
  - Default 30-second timeout for query requests
  - Automatic timeout detection with clear error messages ("query timed out after 30s")
  - Loading animation stops and error diagnostic is shown when timeout occurs
  - Queries are not started until the connection health check completes
  - If the connection fails, panes don't show loading state (no query is attempted)
  - Orphaned loading states are now cleaned up if a pane is removed during query execution

- **Workspace connection config**: The `[connection]` section in workspace TOML files is now applied when loading a workspace. Previously, the endpoint was logged but not used. Now:
  - Connection is automatically established to the specified Prometheus endpoint
  - Health check is initiated and metric/label metadata is fetched
  - If the connection fails (e.g., Prometheus is not running), panes show an error rather than staying in loading state indefinitely

- **Go to Alert (`ga`)**: Press `ga` on a focused chart pane to view alert rules that reference the metric. Features include:
  - Source preview overlay showing ~20 lines of context around the alert definition in YAML files
  - Alert severity badge (critical/warning) displayed in the header
  - Alert name and message shown in the footer
  - Press `Escape` to dismiss the overlay
  - Native-only feature (requires codebase to be indexed via `[codebase]` config)

- **Alert rule indexing**: The codebase indexer now scans YAML files for Prometheus alerting rules. Features include:
  - Parses standard Prometheus alert rule format (`groups.rules.alert`)
  - Extracts alert name, PromQL expression, severity, message, and runbook URL
  - Uses `enya-promql::extract_metric_name()` to identify which metric an alert references
  - New `AlertRule` struct capturing alert metadata and file location
  - `CodebaseIndex.find_alerts_by_metric()` to look up alerts by metric name
  - New dependency: `tree-sitter-yaml` for YAML parsing (consistent with the Rust scanner)

- **Go To section in which-key overlay**: The `?` help overlay now includes a "Go To" section documenting `gd` (go to metric definition) and `ga` (go to alert) shortcuts.

- **Function context in metric definitions**: The go-to-definition feature (`gd`) now shows the containing function name when viewing metric source code. For metrics inside impl blocks, the display shows `Type::function_name` format. This helps quickly understand which code path records a metric.

- **Metric prefix matching**: Go-to-definition (`gd`) now handles runtime metric prefixes. When metrics-rs adds a prefix at runtime (e.g., `myapp_`), the lookup now falls back to suffix matching to find the source definition. For example, querying `myapp_http_requests_total` will find `counter!("http_requests_total")` in the source code.

- **Multi-location navigation**: When a metric is defined at multiple locations in the codebase, the source preview now supports cycling through all of them. Features include:
  - Location indicator `[1/3]` shown in the footer when multiple locations exist
  - Press `N` to go to the next location, `P` (or `Shift+N`) for previous
  - Footer hint updates to show `N/P to cycle • Esc to close` when applicable
  - Wraps around at the ends (pressing `N` on the last goes to the first)

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
  - `components/pane/` - Tile content types (query_pane, heatmap, time_series_chart, visualization)
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

- **PromQL as the query language**: The editor uses PromQL for query input, with full context-aware autocompletion for PromQL syntax including functions, aggregations, label selectors, duration literals, and modifiers.

- **PromQL validation for inline diagnostics**: PromQL queries are validated using `enya-promql::validate()` which wraps the `promql-parser` crate. Syntax errors are displayed as inline diagnostics in the query editor.

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
