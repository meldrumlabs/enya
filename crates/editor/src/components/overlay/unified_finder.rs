//! UnifiedFinder - A single Telescope-style fuzzy finder for all search modes.
//!
//! This module provides a unified finder that consolidates metrics and codebase search
//! into a single modal with prefix-based mode switching.
//!
//! # Prefix Modes
//!
//! | Prefix | Mode | Description |
//! |--------|------|-------------|
//! | (none) | All | Default: search everything (metrics, alerts, commits) |
//! | `@` | Metrics | Search metrics (both live Prometheus and codebase) |
//! | `!` | Alerts | Search alert rules from codebase |
//! | `#` | Commits | Search git commits |
//!
//! # Keyboard Shortcuts
//!
//! | Key | Action |
//! |-----|--------|
//! | `Space f` | Open unified finder |
//! | `↑` / `k` / `Ctrl+K` | Navigate up |
//! | `↓` / `j` / `Ctrl+J` | Navigate down |
//! | `Enter` | Select item |
//! | `Escape` | Close finder |
//! | `@` `!` `#` | Switch modes (prefix characters) |

use std::path::PathBuf;

use rustc_hash::{FxHashMap, FxHashSet};

use egui::{Color32, RichText, text::LayoutJob};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::components::util::finder_utils::{FinderColors, FinderKeyboardInput, OverlayStyle};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{FileOpenerAction, FileOpenerPopup, FileOpenerResult};
use crate::components::util::{ScrollShadowConfig, ScrollState, render_scroll_shadows};
use crate::ui::palette;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;

#[cfg(not(target_arch = "wasm32"))]
use crate::ui::semantic_icons;

use super::preview::render_diff_line_preview;
#[cfg(not(target_arch = "wasm32"))]
use super::preview::render_source_preview;
#[cfg(not(target_arch = "wasm32"))]
use crate::codebase::search::{SearchResult, SearchResultKind};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::HighlightCache;

// =============================================================================
// FinderMode
// =============================================================================

/// Search mode determines what type of results to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinderMode {
    /// Search everything in codebase (default).
    #[default]
    All,
    /// Search metrics (both live Prometheus and codebase).
    Metrics,
    /// Search alert rules.
    Alerts,
    /// Search git commits.
    Commits,
}

impl FinderMode {
    /// Returns the prefix character for this mode.
    #[must_use]
    pub fn prefix(&self) -> Option<char> {
        match self {
            Self::All => None,
            Self::Metrics => Some('@'),
            Self::Alerts => Some('!'),
            Self::Commits => Some('#'),
        }
    }

    /// Returns the display label for this mode.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Metrics => "Metrics",
            Self::Alerts => "Alerts",
            Self::Commits => "Commits",
        }
    }

    /// Returns the icon for this mode.
    #[must_use]
    pub fn icon(&self) -> &'static str {
        use egui_nerdfonts::regular;
        match self {
            Self::All => regular::MAGNIFY,
            Self::Metrics => regular::CHART_LINE,
            Self::Alerts => regular::BELL_ALERT,
            Self::Commits => regular::GIT_COMMIT,
        }
    }

    /// Returns the accent color for this mode's badge.
    #[must_use]
    pub fn color(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::All => theme.accent_muted(),
            Self::Metrics => theme.accent_primary(),
            Self::Alerts => palette::semantic::WARNING,
            Self::Commits => theme.chart_commit_marker(),
        }
    }

    /// Parse mode from a query prefix.
    #[must_use]
    pub fn from_prefix(query: &str) -> (Self, &str) {
        let query = query.trim();
        if let Some(rest) = query.strip_prefix('@') {
            (Self::Metrics, rest)
        } else if let Some(rest) = query.strip_prefix('!') {
            (Self::Alerts, rest)
        } else if let Some(rest) = query.strip_prefix('#') {
            (Self::Commits, rest)
        } else {
            (Self::All, query)
        }
    }

    /// Returns the next mode in the cycle order.
    /// Order: All -> Metrics -> Alerts -> Commits -> All
    #[must_use]
    pub fn cycle_next(self) -> Self {
        match self {
            Self::All => Self::Metrics,
            Self::Metrics => Self::Alerts,
            Self::Alerts => Self::Commits,
            Self::Commits => Self::All,
        }
    }
}

// =============================================================================
// WASM Demo Types
// =============================================================================

/// The type of a demo codebase search result (WASM only).
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq)]
pub enum DemoResultKind {
    /// A metric instrumentation point found in source code.
    Metric,
    /// An alert rule found in source code.
    Alert {
        /// Alert severity (critical, warning, info).
        severity: String,
    },
    /// A git commit.
    Commit {
        /// Commit SHA.
        hash: String,
        /// Unified diff content.
        diff: String,
    },
}

/// A demo codebase search result for WASM (mirrors `SearchResult` from `enya-search`).
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct DemoSearchResult {
    /// The type of result.
    pub kind: DemoResultKind,
    /// The name (metric name, alert name, or commit message).
    pub name: String,
    /// File path (relative to repo root).
    pub file: String,
    /// Line number (1-indexed).
    pub line: usize,
    /// Optional snippet or additional context.
    pub snippet: Option<String>,
}

// =============================================================================
// UnifiedResult
// =============================================================================

/// A unified search result that can represent any searchable item.
#[derive(Debug, Clone)]
pub enum UnifiedResult {
    /// A live metric from Prometheus.
    LiveMetric {
        /// Metric name.
        name: String,
        /// Metric category.
        category: String,
        /// Tags/labels associated with this metric (key -> set of values).
        tags: FxHashMap<String, FxHashSet<String>>,
    },
    /// A codebase search result from Tantivy (native only).
    #[cfg(not(target_arch = "wasm32"))]
    CodebaseResult(SearchResult),
    /// A demo codebase search result (WASM only).
    #[cfg(target_arch = "wasm32")]
    DemoResult(DemoSearchResult),
}

impl UnifiedResult {
    /// Returns the display name for this result.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::LiveMetric { name, .. } => name,
            #[cfg(not(target_arch = "wasm32"))]
            Self::CodebaseResult(result) => &result.name,
            #[cfg(target_arch = "wasm32")]
            Self::DemoResult(demo) => &demo.name,
        }
    }

    /// Returns the icon for this result.
    #[must_use]
    pub fn icon(&self) -> &'static str {
        use egui_nerdfonts::regular;
        match self {
            Self::LiveMetric { .. } => regular::CHART_LINE,
            #[cfg(not(target_arch = "wasm32"))]
            Self::CodebaseResult(result) => match &result.kind {
                SearchResultKind::Metric(_) => regular::CHART_LINE,
                SearchResultKind::Alert { .. } => regular::BELL_ALERT,
                SearchResultKind::Commit { .. } => regular::GIT_COMMIT,
            },
            #[cfg(target_arch = "wasm32")]
            Self::DemoResult(demo) => match &demo.kind {
                DemoResultKind::Metric => regular::CHART_LINE,
                DemoResultKind::Alert { .. } => regular::BELL_ALERT,
                DemoResultKind::Commit { .. } => regular::GIT_COMMIT,
            },
        }
    }

    /// Returns the secondary text (subtitle) for this result.
    #[must_use]
    pub fn secondary_text(&self) -> Option<String> {
        match self {
            Self::LiveMetric { category, .. } => Some(format!("[{category}]")),
            #[cfg(not(target_arch = "wasm32"))]
            Self::CodebaseResult(result) => {
                if !result.file.as_os_str().is_empty() {
                    Some(format!("{}:{}", result.file.display(), result.line))
                } else {
                    None
                }
            }
            #[cfg(target_arch = "wasm32")]
            Self::DemoResult(demo) => {
                if !demo.file.is_empty() {
                    Some(format!("{}:{}", demo.file, demo.line))
                } else {
                    None
                }
            }
        }
    }
}

// =============================================================================
// UnifiedFinderAction
// =============================================================================

/// Actions that can result from the unified finder.
#[derive(Debug, Clone)]
pub enum UnifiedFinderAction {
    /// Create a pane for a metric.
    CreateMetricPane(String),
    /// Navigate to source location.
    NavigateToSource {
        /// File path.
        file: PathBuf,
        /// Line number.
        line: usize,
    },
    /// Open diff viewer for a commit.
    OpenDiffViewer {
        /// Commit hash.
        hash: String,
        /// Commit message (for title).
        message: String,
        /// Full diff content.
        diff: String,
    },
    /// An error occurred (e.g., file not found).
    Error(String),
}

// =============================================================================
// UnifiedFinder
// =============================================================================

/// Debounce duration in milliseconds for search input.
const SEARCH_DEBOUNCE_MS: u64 = 50;

/// A unified Telescope-style fuzzy finder.
pub struct UnifiedFinder {
    /// Current search query (may include prefix).
    query: String,
    /// Current search mode (derived from prefix or set explicitly).
    mode: FinderMode,
    /// Whether the finder is open.
    is_open: bool,
    /// Search results.
    results: Vec<UnifiedResult>,
    /// Match positions for highlighting (parallel to results).
    match_positions: Vec<Vec<usize>>,
    /// Selected index.
    selected_index: usize,
    /// Theme.
    theme: AppTheme,
    /// Nucleo fuzzy matcher.
    matcher: Matcher,
    /// Whether to request focus on next frame.
    request_focus: bool,
    /// Available live metrics for metrics mode (name, category, tags).
    live_metrics: Vec<(String, String, FxHashMap<String, FxHashSet<String>>)>,
    /// Timestamp of last query change (for debouncing).
    last_query_change: Option<Instant>,
    /// Last query that was actually searched (for debounce tracking).
    last_searched_query: String,
    /// Repository root path for constructing full file paths (native only).
    #[cfg(not(target_arch = "wasm32"))]
    repo_path: Option<PathBuf>,
    /// Last query+mode that triggered a codebase search (for change detection).
    #[cfg(not(target_arch = "wasm32"))]
    last_codebase_search: Option<(String, FinderMode)>,
    /// Cached syntax highlights for source preview (file path -> highlights).
    #[cfg(not(target_arch = "wasm32"))]
    highlight_cache: Option<HighlightCache>,
    /// File opener popup for opening files in external apps (native only).
    #[cfg(not(target_arch = "wasm32"))]
    file_opener: FileOpenerPopup,
    /// Flag to open file opener on next frame (for keyboard shortcut).
    #[cfg(not(target_arch = "wasm32"))]
    pending_open_file_opener: bool,
}

impl Default for UnifiedFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl UnifiedFinder {
    /// Creates a new unified finder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: String::new(),
            mode: FinderMode::default(),
            is_open: false,
            results: Vec::new(),
            match_positions: Vec::new(),
            selected_index: 0,
            theme: AppTheme::default(),
            matcher: Matcher::new(Config::DEFAULT),
            request_focus: false,
            live_metrics: Vec::new(),
            last_query_change: None,
            last_searched_query: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            repo_path: None,
            #[cfg(not(target_arch = "wasm32"))]
            last_codebase_search: None,
            #[cfg(not(target_arch = "wasm32"))]
            highlight_cache: None,
            #[cfg(not(target_arch = "wasm32"))]
            file_opener: FileOpenerPopup::new(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_open_file_opener: false,
        }
    }

    /// Sets the repository root path for constructing full file paths.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_repo_path(&mut self, path: Option<PathBuf>) {
        self.repo_path = path;
    }

    /// Sets the UI theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Returns `true` if the finder is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Opens the finder with the default mode.
    pub fn open(&mut self) {
        self.open_with_mode(FinderMode::default());
    }

    /// Opens the finder with a specific mode.
    pub fn open_with_mode(&mut self, mode: FinderMode) {
        self.is_open = true;
        self.mode = mode;
        self.query.clear();
        if let Some(prefix) = mode.prefix() {
            self.query.push(prefix);
        }
        self.results.clear();
        self.match_positions.clear();
        self.selected_index = 0;
        self.request_focus = true;
    }

    /// Closes the finder.
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.results.clear();
        self.match_positions.clear();
        self.selected_index = 0;
        self.last_query_change = None;
        self.last_searched_query.clear();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.last_codebase_search = None;
            self.highlight_cache = None;
        }
    }

    /// Cycles to the next mode, preserving the search text.
    pub fn cycle_mode(&mut self) {
        // Get current query text without the prefix
        let query_text = self.query_text().to_string();

        // Cycle to next mode
        let next_mode = self.mode.cycle_next();
        self.mode = next_mode;

        // Rebuild query with new prefix
        self.query.clear();
        if let Some(prefix) = next_mode.prefix() {
            self.query.push(prefix);
        }
        self.query.push_str(&query_text);

        // Reset search state to trigger a fresh search
        self.results.clear();
        self.match_positions.clear();
        self.selected_index = 0;
        self.last_query_change = Some(Instant::now());
        self.last_searched_query.clear();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.last_codebase_search = None;
        }
    }

    /// Sets the available live metrics.
    pub fn set_live_metrics(
        &mut self,
        metrics: Vec<(String, String, FxHashMap<String, FxHashSet<String>>)>,
    ) {
        self.live_metrics = metrics;
    }

    /// Gets the current query without the prefix.
    #[must_use]
    pub fn query_text(&self) -> &str {
        let (_, text) = FinderMode::from_prefix(&self.query);
        text
    }

    /// Sets the query text, preserving the current mode prefix.
    pub fn set_query(&mut self, query: &str) {
        if let Some(prefix) = self.mode.prefix() {
            self.query = format!("{prefix}{query}");
        } else {
            self.query = query.to_string();
        }
        self.selected_index = 0;
        self.refresh_results();
    }

    /// Gets the current mode based on query prefix.
    ///
    /// This parses the mode from the query prefix (e.g., `#` for commits)
    /// to ensure the mode is always in sync with the current query.
    #[must_use]
    pub fn mode(&self) -> FinderMode {
        let (mode, _) = FinderMode::from_prefix(&self.query);
        mode
    }

    /// Refreshes the search results based on current query and mode.
    fn refresh_results(&mut self) {
        self.results.clear();
        self.match_positions.clear();

        // Clone query to avoid borrow issues
        let query = self.query.clone();
        let (mode, query_text) = FinderMode::from_prefix(&query);
        let query_text = query_text.to_string();
        self.mode = mode;

        match mode {
            // Metrics mode: search live Prometheus metrics first
            // Codebase metrics will be added externally via set_codebase_results
            FinderMode::Metrics => self.search_live_metrics(&query_text),
            // All, Alerts, Commits modes are handled externally via set_codebase_results
            #[cfg(not(target_arch = "wasm32"))]
            FinderMode::All | FinderMode::Alerts | FinderMode::Commits => {}
            // On WASM, populate with demo codebase results
            #[cfg(target_arch = "wasm32")]
            FinderMode::All | FinderMode::Alerts | FinderMode::Commits => {
                self.search_demo_results(&query_text, mode);
            }
        }

        // Reset selection if out of bounds
        if self.selected_index >= self.results.len() && !self.results.is_empty() {
            self.selected_index = self.results.len() - 1;
        } else if self.results.is_empty() {
            self.selected_index = 0;
        }
    }

    /// Searches live metrics.
    fn search_live_metrics(&mut self, query: &str) {
        if query.is_empty() {
            // Show all metrics
            for (name, category, tags) in &self.live_metrics {
                self.results.push(UnifiedResult::LiveMetric {
                    name: name.clone(),
                    category: category.clone(),
                    tags: tags.clone(),
                });
                self.match_positions.push(Vec::new());
            }
            return;
        }

        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();

        for (name, category, tags) in &self.live_metrics {
            indices.clear();
            let haystack = Utf32Str::new(name, &mut buf);

            if pattern
                .indices(haystack, &mut self.matcher, &mut indices)
                .is_some()
            {
                self.results.push(UnifiedResult::LiveMetric {
                    name: name.clone(),
                    category: category.clone(),
                    tags: tags.clone(),
                });
                self.match_positions
                    .push(indices.iter().map(|&i| i as usize).collect());
            }
        }
    }

    /// Returns demo codebase search results for WASM.
    #[cfg(target_arch = "wasm32")]
    fn demo_results() -> Vec<DemoSearchResult> {
        vec![
            // Metrics
            DemoSearchResult {
                kind: DemoResultKind::Metric,
                name: "http_requests_total".into(),
                file: "src/server/metrics.rs".into(),
                line: 42,
                snippet: Some("counter!(\"http_requests_total\", \"method\" => method)".into()),
            },
            DemoSearchResult {
                kind: DemoResultKind::Metric,
                name: "cpu_usage_percent".into(),
                file: "src/monitoring/system.rs".into(),
                line: 87,
                snippet: Some("gauge!(\"cpu_usage_percent\", value)".into()),
            },
            DemoSearchResult {
                kind: DemoResultKind::Metric,
                name: "memory_heap_bytes".into(),
                file: "src/monitoring/system.rs".into(),
                line: 103,
                snippet: Some("gauge!(\"memory_heap_bytes\", heap_size)".into()),
            },
            DemoSearchResult {
                kind: DemoResultKind::Metric,
                name: "db_query_duration_seconds".into(),
                file: "src/db/pool.rs".into(),
                line: 156,
                snippet: Some(
                    "histogram!(\"db_query_duration_seconds\", elapsed.as_secs_f64())".into(),
                ),
            },
            DemoSearchResult {
                kind: DemoResultKind::Metric,
                name: "api_error_rate".into(),
                file: "src/server/middleware.rs".into(),
                line: 29,
                snippet: Some("counter!(\"api_error_rate\", \"status\" => status_code)".into()),
            },
            // Alerts
            DemoSearchResult {
                kind: DemoResultKind::Alert {
                    severity: "critical".into(),
                },
                name: "HighErrorRate".into(),
                file: "alerts/server.yml".into(),
                line: 12,
                snippet: Some("rate(http_requests_total{status=~\"5..\"}[5m]) > 0.05".into()),
            },
            DemoSearchResult {
                kind: DemoResultKind::Alert {
                    severity: "warning".into(),
                },
                name: "MemoryPressure".into(),
                file: "alerts/system.yml".into(),
                line: 28,
                snippet: Some("memory_heap_bytes / memory_limit_bytes > 0.85".into()),
            },
            DemoSearchResult {
                kind: DemoResultKind::Alert {
                    severity: "warning".into(),
                },
                name: "DiskSpaceLow".into(),
                file: "alerts/system.yml".into(),
                line: 45,
                snippet: Some("disk_free_bytes / disk_total_bytes < 0.10".into()),
            },
            DemoSearchResult {
                kind: DemoResultKind::Alert {
                    severity: "info".into(),
                },
                name: "CertExpiringSoon".into(),
                file: "alerts/tls.yml".into(),
                line: 8,
                snippet: Some("cert_expiry_seconds < 86400 * 30".into()),
            },
            DemoSearchResult {
                kind: DemoResultKind::Alert {
                    severity: "critical".into(),
                },
                name: "LatencySpike".into(),
                file: "alerts/server.yml".into(),
                line: 34,
                snippet: Some(
                    "histogram_quantile(0.99, rate(http_duration_seconds_bucket[5m])) > 2.0".into(),
                ),
            },
            // Commits
            DemoSearchResult {
                kind: DemoResultKind::Commit {
                    hash: "a1b2c3d".into(),
                    diff: "diff --git a/src/db/pool.rs b/src/db/pool.rs\n\
                           --- a/src/db/pool.rs\n\
                           +++ b/src/db/pool.rs\n\
                           @@ -45,7 +45,9 @@ impl ConnectionPool {\n\
                           -    let pool = Pool::new(config);\n\
                           +    let pool = Pool::builder()\n\
                           +        .max_connections(32)\n\
                           +        .idle_timeout(Duration::from_secs(300))\n\
                           +        .build(config);"
                        .into(),
                },
                name: "Fix connection pooling".into(),
                file: "src/db/pool.rs".into(),
                line: 45,
                snippet: None,
            },
            DemoSearchResult {
                kind: DemoResultKind::Commit {
                    hash: "e4f5g6h".into(),
                    diff: "diff --git a/src/server/middleware.rs b/src/server/middleware.rs\n\
                           --- a/src/server/middleware.rs\n\
                           +++ b/src/server/middleware.rs\n\
                           @@ -18,4 +18,12 @@ async fn handle_request(req: Request) {\n\
                           +    for attempt in 0..3 {\n\
                           +        match upstream.send(&req).await {\n\
                           +            Ok(resp) => return resp,\n\
                           +            Err(e) if attempt < 2 => {\n\
                           +                tracing::warn!(\"retry {}: {}\", attempt + 1, e);\n\
                           +                tokio::time::sleep(backoff(attempt)).await;\n\
                           +            }\n\
                           +            Err(e) => return Err(e),\n\
                           +        }\n\
                           +    }"
                        .into(),
                },
                name: "Add retry logic".into(),
                file: "src/server/middleware.rs".into(),
                line: 18,
                snippet: None,
            },
            DemoSearchResult {
                kind: DemoResultKind::Commit {
                    hash: "i7j8k9l".into(),
                    diff: "diff --git a/Cargo.toml b/Cargo.toml\n\
                           --- a/Cargo.toml\n\
                           +++ b/Cargo.toml\n\
                           @@ -12,3 +12,3 @@\n\
                           -tokio = \"1.35\"\n\
                           +tokio = \"1.36\"\n\
                           -serde = \"1.0.193\"\n\
                           +serde = \"1.0.196\""
                        .into(),
                },
                name: "Update dependencies".into(),
                file: "Cargo.toml".into(),
                line: 12,
                snippet: None,
            },
            DemoSearchResult {
                kind: DemoResultKind::Commit {
                    hash: "m0n1o2p".into(),
                    diff: "diff --git a/src/auth/mod.rs b/src/auth/mod.rs\n\
                           --- a/src/auth/mod.rs\n\
                           +++ b/src/auth/mod.rs\n\
                           @@ -1,8 +1,10 @@\n\
                           -pub fn authenticate(token: &str) -> bool {\n\
                           -    validate_jwt(token).is_ok()\n\
                           -}\n\
                           +pub struct AuthResult {\n\
                           +    pub user_id: String,\n\
                           +    pub roles: Vec<Role>,\n\
                           +}\n\
                           +\n\
                           +pub fn authenticate(token: &str) -> Result<AuthResult, AuthError> {\n\
                           +    let claims = validate_jwt(token)?;\n\
                           +    Ok(AuthResult::from(claims))\n\
                           +}"
                    .into(),
                },
                name: "Refactor auth module".into(),
                file: "src/auth/mod.rs".into(),
                line: 1,
                snippet: None,
            },
            DemoSearchResult {
                kind: DemoResultKind::Commit {
                    hash: "q3r4s5t".into(),
                    diff: "diff --git a/src/server/handler.rs b/src/server/handler.rs\n\
                           --- a/src/server/handler.rs\n\
                           +++ b/src/server/handler.rs\n\
                           @@ -67,5 +67,7 @@ fn process_batch(items: &[Item]) {\n\
                           -    for item in items {\n\
                           -        process_single(item);\n\
                           -    }\n\
                           +    items.par_iter().for_each(|item| {\n\
                           +        process_single(item);\n\
                           +    });"
                        .into(),
                },
                name: "Performance improvements".into(),
                file: "src/server/handler.rs".into(),
                line: 67,
                snippet: None,
            },
        ]
    }

    /// Searches demo codebase results with fuzzy matching (WASM only).
    #[cfg(target_arch = "wasm32")]
    fn search_demo_results(&mut self, query: &str, mode: FinderMode) {
        let demos = Self::demo_results();

        // Filter by mode
        let filtered: Vec<&DemoSearchResult> = demos
            .iter()
            .filter(|d| match mode {
                FinderMode::All => true,
                FinderMode::Alerts => matches!(d.kind, DemoResultKind::Alert { .. }),
                FinderMode::Commits => matches!(d.kind, DemoResultKind::Commit { .. }),
                FinderMode::Metrics => matches!(d.kind, DemoResultKind::Metric),
            })
            .collect();

        if query.is_empty() {
            // Show all matching results
            for demo in filtered {
                self.results.push(UnifiedResult::DemoResult(demo.clone()));
                self.match_positions.push(Vec::new());
            }
            return;
        }

        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut indices: Vec<u32> = Vec::new();
        let mut buf = Vec::new();

        for demo in filtered {
            indices.clear();
            let haystack = Utf32Str::new(&demo.name, &mut buf);

            if pattern
                .indices(haystack, &mut self.matcher, &mut indices)
                .is_some()
            {
                self.results.push(UnifiedResult::DemoResult(demo.clone()));
                self.match_positions
                    .push(indices.iter().map(|&i| i as usize).collect());
            }
        }
    }

    /// Returns true if a codebase search is needed (query or mode changed).
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn needs_codebase_search(&self) -> bool {
        let query_text = self.query_text().to_string();
        let current_mode = self.mode();
        match &self.last_codebase_search {
            Some((last_query, last_mode)) => {
                &query_text != last_query || current_mode != *last_mode
            }
            None => true,
        }
    }

    /// Sets codebase search results (called externally by workspace).
    /// This clears existing results first - use for All, Alerts, Commits modes.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_codebase_results(&mut self, results: Vec<SearchResult>) {
        // Update the last search tracker
        self.last_codebase_search = Some((self.query_text().to_string(), self.mode()));

        self.results.clear();
        self.match_positions.clear();

        for result in results {
            self.results.push(UnifiedResult::CodebaseResult(result));
            self.match_positions.push(Vec::new()); // Tantivy handles highlighting
        }

        if self.selected_index >= self.results.len() && !self.results.is_empty() {
            self.selected_index = self.results.len() - 1;
        }
    }

    /// Appends codebase search results to existing results (called externally by workspace).
    /// This preserves existing results (e.g., live metrics) - use for Metrics mode.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn append_codebase_results(&mut self, results: Vec<SearchResult>) {
        // Update the last search tracker
        self.last_codebase_search = Some((self.query_text().to_string(), self.mode()));

        for result in results {
            self.results.push(UnifiedResult::CodebaseResult(result));
            self.match_positions.push(Vec::new()); // Tantivy handles highlighting
        }
    }

    /// Update the highlight cache for the currently selected item's source file.
    /// Call this before rendering to pre-compute highlights.
    #[cfg(not(target_arch = "wasm32"))]
    fn update_highlight_cache(&mut self) {
        // Get the selected result's file path
        let file_path = match self.results.get(self.selected_index) {
            Some(UnifiedResult::CodebaseResult(search_result)) => {
                if matches!(search_result.kind, SearchResultKind::Commit { .. }) {
                    // Commits don't need source highlighting
                    return;
                }
                // Construct full path
                if let Some(repo) = &self.repo_path {
                    repo.join(&search_result.file)
                } else {
                    search_result.file.clone()
                }
            }
            _ => return,
        };

        // Check if cache is still valid for this file
        if let Some(cache) = &self.highlight_cache {
            if cache.file_path == file_path {
                // Cache is still valid
                return;
            }
        }

        // Create new cache using the constructor (handles file read and highlighting)
        self.highlight_cache = HighlightCache::new(file_path);
    }

    /// Shows the unified finder and returns an action if one was triggered.
    #[must_use]
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> Option<UnifiedFinderAction> {
        if !self.is_open {
            return None;
        }

        // Sync mode from query prefix at the start of each frame
        // This ensures mode is always consistent with the current query
        let (parsed_mode, _) = FinderMode::from_prefix(&self.query);
        self.mode = parsed_mode;

        // Update highlight cache for current selection (native only)
        #[cfg(not(target_arch = "wasm32"))]
        self.update_highlight_cache();

        let mut action: Option<UnifiedFinderAction> = None;
        let mut should_close = false;
        let mut clicked_index: Option<usize> = None;

        // Handle keyboard input
        let input = FinderKeyboardInput::read(ctx);

        if input.escape {
            should_close = true;
        }

        if input.navigate_up && self.selected_index > 0 {
            self.selected_index -= 1;
        }

        if input.navigate_down && self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }

        if input.confirm && !self.results.is_empty() {
            if let Some(result) = self.results.get(self.selected_index) {
                action = self.handle_selection(result);
                should_close = true;
            }
        }

        // Tab cycles through modes (All -> Metrics -> Alerts -> Commits -> All)
        if input.cycle_mode {
            self.cycle_mode();
        }

        // 'o' opens file in external app (native only, for codebase results)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let o_pressed = ctx.input(|i| i.key_pressed(egui::Key::O) && !i.modifiers.ctrl);
            if o_pressed && !self.file_opener.is_open() {
                // Check if selected result is a codebase result with a file path
                if let Some(UnifiedResult::CodebaseResult(search_result)) =
                    self.results.get(self.selected_index)
                {
                    if !search_result.file.as_os_str().is_empty() {
                        self.pending_open_file_opener = true;
                    }
                }
            }
        }

        // Check if debounce period has elapsed and we need to refresh results
        // On native: only Metrics mode uses internal debounce (codebase modes are external)
        // On WASM: all modes use internal debounce (demo data is populated internally)
        #[cfg(target_arch = "wasm32")]
        let is_internal_mode = true;
        #[cfg(not(target_arch = "wasm32"))]
        let is_internal_mode = matches!(self.mode, FinderMode::Metrics);

        let should_refresh = if let Some(last_change) = self.last_query_change {
            let elapsed = last_change.elapsed().as_millis() as u64;
            elapsed >= SEARCH_DEBOUNCE_MS
                && self.query != self.last_searched_query
                && is_internal_mode
        } else {
            false
        };

        if should_refresh {
            self.refresh_results();
            self.last_searched_query.clone_from(&self.query);
            self.last_query_change = None;
            // Reset codebase search tracker so workspace will re-append codebase results
            // (refresh_results only adds live metrics, codebase metrics need to be re-appended)
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.last_codebase_search = None;
            }
        }

        // Request repaint if debounce is pending for internally-populated modes
        if self.last_query_change.is_some() && is_internal_mode {
            ctx.request_repaint_after(std::time::Duration::from_millis(SEARCH_DEBOUNCE_MS));
        }

        // Calculate dimensions - fixed large size for consistent appearance
        let screen_rect = ctx.available_rect();
        // Large fixed width (80% of screen, clamped)
        let total_width = (screen_rect.width() * 0.80).clamp(800.0, 1200.0);
        let base_column_width = total_width / 2.0;
        let list_width = base_column_width;
        let preview_width = base_column_width;
        // Large fixed height (70% of screen, clamped)
        let popup_max_height = (screen_rect.height() * 0.70).clamp(500.0, 700.0);

        // Extract colors from theme (Custom variant handles plugin colors internally)
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let text_col = self.theme.text_primary();
        let text_muted = self.theme.text_primary().gamma_multiply(0.5);
        let accent_col = self.theme.accent_primary();
        let accent_hover = self.theme.accent_hover();
        let border_col = self.theme.border_subtle();
        let bg_elevated = self.theme.bg_elevated();
        let highlight_col = self.theme.highlight_match_text();
        let colors = FinderColors::new(self.theme);

        egui::Area::new(egui::Id::new("unified_finder"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -30.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Allocate a fixed-size rect to constrain the entire overlay
                // This is the key - by allocating the exact size we want,
                // nothing inside can expand beyond it
                let (area_rect, _response) = ui.allocate_exact_size(
                    egui::vec2(total_width, popup_max_height + 24.0),
                    egui::Sense::hover(),
                );

                // Set clip rect to the allocated area to prevent visual overflow
                ui.set_clip_rect(area_rect);

                // Create a child UI that's constrained to our allocated rect
                let mut child_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(area_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );

                // Premium glass frame with refined styling
                let frame = overlay_style
                    .frame()
                    .inner_margin(egui::Margin::symmetric(0, 12))
                    .corner_radius(14.0) // Slightly more rounded for premium feel
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 32,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    });

                let frame_response = frame.show(&mut child_ui, |ui| {
                    // Set both min and max to ensure consistent size
                    ui.set_width(total_width);
                    ui.set_min_height(popup_max_height);
                    ui.set_max_height(popup_max_height);

                    // Header with search input and mode badge
                    self.render_header(ui, total_width, text_col, text_muted, accent_col);

                    ui.add_space(8.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, border_col),
                    );
                    ui.add_space(4.0);

                    // Calculate footer height for proper layout
                    let footer_height = 30.0; // Fixed footer height

                    // Content area - takes all remaining space minus footer
                    let content_height = ui.available_height() - footer_height - 16.0; // 16 = spacing + margins
                    clicked_index = self.render_content(
                        ui,
                        &colors,
                        list_width,
                        preview_width,
                        content_height,
                        text_col,
                        text_muted,
                        accent_col,
                        highlight_col,
                        border_col,
                        bg_elevated,
                    );

                    // Use add_space with remaining available height to push footer to bottom
                    let remaining = ui.available_height() - footer_height - 10.0;
                    if remaining > 0.0 {
                        ui.add_space(remaining);
                    }

                    // Footer separator - now at the bottom
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, border_col),
                    );

                    // Use bottom-aligned layout to push footer content to the bottom of available space
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.add_space(8.0); // Bottom padding
                        self.render_footer(ui, text_muted, accent_hover);
                    });
                });

                // Draw premium glass effects - top edge highlight
                let rect = frame_response.response.rect;
                if let Some(inner_highlight) = overlay_style.inner_highlight() {
                    let highlight_rect = egui::Rect::from_min_size(
                        rect.left_top() + egui::vec2(1.0, 1.0),
                        egui::vec2(rect.width() - 2.0, 1.5),
                    );
                    ui.painter()
                        .rect_filled(highlight_rect, 12.0, inner_highlight);
                }
            });

        // Handle click selection (after UI rendering)
        if let Some(idx) = clicked_index {
            self.selected_index = idx;
            if let Some(result) = self.results.get(idx) {
                action = self.handle_selection(result);
                should_close = true;
            }
        }

        // Show file opener popup and handle result (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.file_opener.set_theme(self.theme);
            match self.file_opener.show(ctx, self.theme) {
                FileOpenerResult::Selected(file_action) => {
                    if let Some(path) = self.file_opener.file_path() {
                        match &file_action {
                            FileOpenerAction::OpenIn(app) => {
                                if let Err(e) = app.execute(path) {
                                    log::warn!("Failed to open file: {e}");
                                }
                            }
                            FileOpenerAction::CopyPath => {
                                ctx.copy_text(path.display().to_string());
                            }
                            FileOpenerAction::CopyRelativePath => {
                                if let Some(rel) = self.file_opener.relative_path() {
                                    ctx.copy_text(rel.display().to_string());
                                }
                            }
                        }
                    }
                }
                FileOpenerResult::Closed | FileOpenerResult::None => {}
            }
        }

        if should_close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
        }

        action
    }

    /// Renders the header with search input and mode badge.
    fn render_header(
        &mut self,
        ui: &mut egui::Ui,
        total_width: f32,
        text_col: Color32,
        text_muted: Color32,
        accent_col: Color32,
    ) {
        let mode_color = self.mode.color(self.theme);
        let badge_width = 100.0; // Fixed badge width for consistent positioning

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Search icon with accent color
            ui.label(
                RichText::new(egui_nerdfonts::regular::MAGNIFY)
                    .color(accent_col)
                    .size(18.0),
            );

            ui.add_space(12.0);

            // Search input - fixed width to leave room for badge
            let input_width = total_width - badge_width - 80.0; // 80 = margins + icons
            let response = ui.add_sized(
                egui::vec2(input_width, 28.0),
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text(
                        RichText::new(format!(
                            "Search {}...  @ metrics  ! alerts  # commits",
                            self.mode.label().to_lowercase()
                        ))
                        .color(text_muted.gamma_multiply(0.8))
                        .size(typography::MD),
                    )
                    .text_color(text_col)
                    .frame(false)
                    .font(typography::proportional(typography::MD)),
            );

            if self.request_focus {
                response.request_focus();
                self.request_focus = false;
            }

            if response.changed() {
                // Record timestamp for debounce - actual refresh happens in show() after delay
                self.last_query_change = Some(Instant::now());
            }

            // Use remaining space to push badges to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Mode badge with premium styling like agent input bar
                let badge_bg = mode_color.gamma_multiply(0.18);
                egui::Frame::new()
                    .fill(badge_bg)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.set_min_width(badge_width - 24.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            // Mode icon
                            ui.label(
                                RichText::new(self.mode.icon())
                                    .color(mode_color)
                                    .size(typography::SM),
                            );
                            // Mode label with prefix
                            if let Some(prefix) = self.mode.prefix() {
                                ui.label(
                                    RichText::new(format!("[{prefix}] {}", self.mode.label()))
                                        .color(mode_color)
                                        .size(typography::SM)
                                        .strong(),
                                );
                            } else {
                                ui.label(
                                    RichText::new(self.mode.label())
                                        .color(mode_color)
                                        .size(typography::SM)
                                        .strong(),
                                );
                            }
                        });
                    });

                // Result count badge (only show when there are results)
                if !self.results.is_empty() {
                    ui.add_space(8.0);
                    let count_text = if self.results.len() >= 50 {
                        "50+".to_string()
                    } else {
                        self.results.len().to_string()
                    };
                    ui.label(
                        RichText::new(format!("{count_text} results"))
                            .color(text_muted.gamma_multiply(0.8))
                            .size(typography::XS),
                    );
                }
            });
        });
    }

    /// Renders the main content area. Returns clicked index if any.
    #[allow(clippy::too_many_arguments)]
    fn render_content(
        &mut self,
        ui: &mut egui::Ui,
        colors: &FinderColors,
        list_width: f32,
        preview_width: f32,
        content_height: f32,
        text_col: Color32,
        text_muted: Color32,
        accent_col: Color32,
        highlight_col: Color32,
        border_col: Color32,
        bg_elevated: Color32,
    ) -> Option<usize> {
        if self.results.is_empty() && !self.query_text().is_empty() {
            // No results
            self.render_empty_state(
                ui,
                content_height,
                egui_nerdfonts::regular::MAGNIFY_CLOSE,
                "No results found",
                None,
                text_col,
                text_muted,
                accent_col,
            );
            return None;
        }

        if self.results.is_empty() {
            // Empty state - prompt to search
            self.render_empty_state(
                ui,
                content_height,
                self.mode.icon(),
                &format!("Type to search {}", self.mode.label().to_lowercase()),
                Some("Use prefixes: @ metrics  ! alerts  # commits"),
                text_col,
                text_muted,
                accent_col,
            );
            return None;
        }

        // Track clicked index from list
        let mut clicked_index: Option<usize> = None;

        // Allocate the full content area to ensure consistent sizing
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), content_height),
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
                // Two-column layout: results list + preview
                ui.allocate_ui_with_layout(
                    egui::vec2(list_width, content_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        clicked_index = self.render_results_list(
                            ui,
                            content_height,
                            text_col,
                            text_muted,
                            accent_col,
                            highlight_col,
                            bg_elevated,
                        );
                    },
                );

                // Separator
                ui.painter().vline(
                    ui.cursor().left(),
                    ui.available_rect_before_wrap().y_range(),
                    egui::Stroke::new(1.0, border_col),
                );

                // Preview
                ui.allocate_ui_with_layout(
                    egui::vec2(preview_width, content_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.render_preview(
                            ui, colors, text_col, text_muted, accent_col, border_col,
                        );
                    },
                );
            },
        );

        clicked_index
    }

    /// Renders the results list. Returns the index of a clicked item if any.
    #[allow(clippy::too_many_arguments)]
    fn render_results_list(
        &mut self,
        ui: &mut egui::Ui,
        max_height: f32,
        text_col: Color32,
        text_muted: Color32,
        accent_col: Color32,
        highlight_col: Color32,
        bg_elevated: Color32,
    ) -> Option<usize> {
        let _ = text_muted; // Used in secondary text rendering
        let mut clicked_index: Option<usize> = None;

        // Get the clip rect for the list area to prevent text overflow
        let list_clip_rect = ui.available_rect_before_wrap();

        // Scroll handling
        let row_height = 38.0;
        let scroll_id = egui::Id::new("unified_finder_scroll");

        let scroll_output = egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .max_height(max_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Create a clipped painter to prevent text from spilling into preview pane
                let clipped_painter = ui.painter().with_clip_rect(list_clip_rect);

                for (i, (result, positions)) in self
                    .results
                    .iter()
                    .zip(self.match_positions.iter())
                    .enumerate()
                {
                    let is_selected = i == self.selected_index;

                    let row_height = 38.0;
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    let is_hovered = response.hovered();

                    // Premium row styling with subtle gradients and glow
                    if is_selected {
                        // Selected: accent-tinted background with subtle inner glow
                        let bg_color = accent_col.gamma_multiply(0.15);
                        clipped_painter.rect_filled(rect, 6.0, bg_color);

                        // Subtle glow border around the selected row
                        let glow_rect = rect.expand(1.0);
                        clipped_painter.rect_stroke(
                            glow_rect,
                            6.0,
                            egui::Stroke::new(1.0, accent_col.gamma_multiply(0.3)),
                            egui::StrokeKind::Outside,
                        );

                        // Left accent bar with rounded caps
                        let indicator_rect =
                            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
                        clipped_painter.rect_filled(indicator_rect, 2.0, accent_col);
                    } else if is_hovered {
                        // Hovered: subtle highlight with light border
                        let bg_color = text_col.gamma_multiply(0.06);
                        clipped_painter.rect_filled(rect, 6.0, bg_color);

                        // Very subtle border on hover
                        clipped_painter.rect_stroke(
                            rect,
                            6.0,
                            egui::Stroke::new(0.5, text_col.gamma_multiply(0.1)),
                            egui::StrokeKind::Inside,
                        );
                    }

                    // Content - use content_rect for clipping and layout
                    let content_rect = rect.shrink2(egui::vec2(16.0, 0.0));
                    let mut cursor_x = content_rect.left();

                    // Calculate max width for the name (leave space for secondary text)
                    // Commits get more space (90%) since they have longer messages and less
                    // useful secondary text. Other results use 65% for name.
                    #[cfg(not(target_arch = "wasm32"))]
                    let is_commit = matches!(
                        result,
                        UnifiedResult::CodebaseResult(r) if matches!(r.kind, SearchResultKind::Commit { .. })
                    );
                    #[cfg(target_arch = "wasm32")]
                    let is_commit = matches!(
                        result,
                        UnifiedResult::DemoResult(r) if matches!(r.kind, DemoResultKind::Commit { .. })
                    );

                    let max_name_width = if is_commit {
                        content_rect.width() * 0.90
                    } else {
                        content_rect.width() * 0.65
                    };

                    // Icon
                    let icon_color = if is_selected || is_hovered {
                        accent_col
                    } else {
                        text_col.gamma_multiply(0.6)
                    };
                    let icon_galley = clipped_painter.layout_no_wrap(
                        result.icon().to_string(),
                        typography::proportional(typography::LG),
                        icon_color,
                    );
                    clipped_painter.galley(
                        egui::pos2(
                            cursor_x,
                            content_rect.center().y - icon_galley.size().y / 2.0,
                        ),
                        icon_galley.clone(),
                        icon_color,
                    );
                    cursor_x += icon_galley.size().x + 10.0;

                    // Name - with match highlighting
                    let name_str = result.name();
                    let available_for_name = max_name_width - (cursor_x - content_rect.left());

                    // Check if we need to truncate
                    let font = typography::proportional(typography::MD);
                    let full_galley = clipped_painter.layout_no_wrap(
                        name_str.to_string(),
                        font.clone(),
                        text_col,
                    );
                    let needs_truncation = full_galley.size().x > available_for_name;

                    let name_galley = if !positions.is_empty() && !needs_truncation {
                        // Use highlighted galley when we have match positions and don't need truncation
                        create_highlighted_galley(
                            ui,
                            name_str,
                            positions,
                            font,
                            text_col,
                            highlight_col,
                        )
                    } else if needs_truncation {
                        // Fall back to plain truncated text when truncation is needed
                        // (highlight positions would be wrong after truncation)
                        let truncated_name =
                            truncate_to_width(name_str, available_for_name, font.clone(), ui);
                        clipped_painter.layout_no_wrap(truncated_name, font, text_col)
                    } else {
                        // No highlights, no truncation - just use the full galley
                        full_galley
                    };

                    clipped_painter.galley(
                        egui::pos2(
                            cursor_x,
                            content_rect.center().y - name_galley.size().y / 2.0,
                        ),
                        name_galley.clone(),
                        text_col,
                    );
                    cursor_x += name_galley.size().x + 12.0;

                    // Secondary text (right-aligned) - also truncate if needed
                    if let Some(secondary) = result.secondary_text() {
                        let remaining = content_rect.right() - cursor_x - 8.0;
                        if remaining > 50.0 {
                            let truncated_secondary = truncate_to_width(
                                &secondary,
                                remaining,
                                typography::proportional(typography::SM),
                                ui,
                            );
                            let secondary_galley = clipped_painter.layout_no_wrap(
                                truncated_secondary,
                                typography::proportional(typography::SM),
                                text_col.gamma_multiply(0.5),
                            );

                            // Right-align secondary text
                            let secondary_x =
                                content_rect.right() - secondary_galley.size().x - 8.0;
                            clipped_painter.galley(
                                egui::pos2(
                                    secondary_x.max(cursor_x),
                                    content_rect.center().y - secondary_galley.size().y / 2.0,
                                ),
                                secondary_galley,
                                text_col.gamma_multiply(0.5),
                            );
                        }
                    }

                    // Handle click selection
                    if response.clicked() {
                        clicked_index = Some(i);
                    }

                    // Use egui's built-in scroll_to_me for selected items
                    if is_selected {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                }

                // Bottom padding to prevent last item from being obscured by scroll shadow
                ui.add_space(row_height);
            });

        // Render scroll shadows for the results list
        let scroll_state = ScrollState::from_scroll_output(
            scroll_output.content_size,
            scroll_output.inner_rect,
            scroll_output.state.offset,
        );
        let shadow_config = ScrollShadowConfig::default()
            .with_color(bg_elevated)
            .with_opacity(0.5);
        render_scroll_shadows(ui, scroll_output.inner_rect, scroll_state, shadow_config);

        clicked_index
    }

    /// Renders the preview pane.
    #[allow(clippy::too_many_arguments)]
    fn render_preview(
        &mut self,
        ui: &mut egui::Ui,
        _colors: &FinderColors,
        text_col: Color32,
        text_muted: Color32,
        accent_col: Color32,
        border_col: Color32,
    ) {
        // No background fill - uses the same frosted glass as the rest of the overlay
        // This matches the Source Preview Overlay styling

        let available_height = ui.available_height();
        let preview_width = ui.available_width();

        // Hard lock the width to prevent ANY content from expanding the overlay
        // Using set_width forces both min and max to this exact value
        ui.set_width(preview_width);

        // Set clip rect to visually clip any overflow
        let clip_rect = ui.available_rect_before_wrap();
        ui.set_clip_rect(clip_rect);

        let Some(result) = self.results.get(self.selected_index) else {
            // Center the "Select an item" message both horizontally and vertically
            ui.allocate_ui_with_layout(
                egui::vec2(preview_width, available_height),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.label(
                        RichText::new("Select an item to preview")
                            .color(text_muted.gamma_multiply(0.8))
                            .italics(),
                    );
                },
            );
            return;
        };

        // Use the full available space for preview content
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                // Hard lock content width to prevent expansion (accounting for margins)
                let content_width = preview_width - 32.0;
                ui.set_width(content_width);
                ui.set_min_height(available_height - 24.0); // Fill vertical space

                // Set clip rect for inner content too
                let inner_clip = ui.available_rect_before_wrap();
                ui.set_clip_rect(inner_clip);

                // Header - truncate title to fit available width
                ui.horizontal(|ui| {
                    ui.label(RichText::new(result.icon()).color(accent_col).size(20.0));
                    ui.add_space(8.0);

                    // Truncate title to fit available space (reserve space for icon + margins)
                    let max_title_width = preview_width - 50.0;
                    let truncated_title = truncate_to_width(
                        result.name(),
                        max_title_width.max(100.0),
                        typography::proportional(typography::LG),
                        ui,
                    );
                    ui.label(
                        RichText::new(truncated_title)
                            .color(text_col)
                            .size(typography::LG)
                            .strong(),
                    );
                });

                ui.add_space(12.0);

                // Details based on result type
                match result {
                    UnifiedResult::LiveMetric { category, tags, .. } => {
                        // Category
                        ui.label(
                            RichText::new(format!("Category: {category}"))
                                .color(text_muted)
                                .size(typography::SM),
                        );

                        ui.add_space(12.0);

                        // Separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, border_col),
                        );
                        ui.add_space(12.0);

                        // Tags section - use accent colors for syntax highlighting
                        let tag_key_color = accent_col;
                        let tag_value_color = text_col;

                        ui.label(
                            RichText::new("Available Tags")
                                .color(text_muted)
                                .size(typography::XS),
                        );
                        ui.add_space(8.0);

                        if tags.is_empty() {
                            ui.label(
                                RichText::new("No tags available")
                                    .color(text_muted.gamma_multiply(0.8))
                                    .italics()
                                    .size(typography::SM),
                            );
                        } else {
                            // Show tags in a scrollable area
                            let remaining_height = ui.available_height();
                            egui::ScrollArea::vertical()
                                .max_height(remaining_height)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    // Sort tag keys for consistent display
                                    let mut tag_keys: Vec<_> = tags.keys().collect();
                                    tag_keys.sort();

                                    for (idx, key) in tag_keys.iter().enumerate() {
                                        if let Some(values) = tags.get(*key) {
                                            // Tag key
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(format!("{key}:"))
                                                        .color(tag_key_color)
                                                        .size(typography::MD)
                                                        .strong(),
                                                );
                                            });

                                            // Tag values (show up to 5, with ellipsis if more)
                                            let mut sorted_values: Vec<_> = values.iter().collect();
                                            sorted_values.sort();
                                            let display_count = sorted_values.len().min(5);
                                            let has_more = sorted_values.len() > 5;

                                            ui.indent(egui::Id::new(("tag_values", idx)), |ui| {
                                                for value in
                                                    sorted_values.iter().take(display_count)
                                                {
                                                    ui.label(
                                                        RichText::new(format!("• {value}"))
                                                            .color(tag_value_color)
                                                            .size(typography::SM),
                                                    );
                                                }
                                                if has_more {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "  ... and {} more",
                                                            sorted_values.len() - 5
                                                        ))
                                                        .color(text_muted.gamma_multiply(0.8))
                                                        .italics()
                                                        .size(typography::XS),
                                                    );
                                                }
                                            });

                                            ui.add_space(6.0);
                                        }
                                    }
                                });
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    UnifiedResult::CodebaseResult(search_result) => {
                        // File location with language-specific icon
                        if !search_result.file.as_os_str().is_empty() {
                            let file_path = search_result.file.clone();
                            let file_label_response = ui.horizontal(|ui| {
                                // Use language-specific file icon
                                let file_icon = semantic_icons::file_icon(&file_path);
                                ui.label(
                                    RichText::new(file_icon)
                                        .color(self.theme.accent_muted())
                                        .size(14.0),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{}:{}",
                                        file_path.display(),
                                        search_result.line
                                    ))
                                    .color(text_col.gamma_multiply(0.6))
                                    .size(typography::SM),
                                )
                            });

                            // Handle pending file opener from 'o' key press
                            if self.pending_open_file_opener {
                                self.pending_open_file_opener = false;
                                let popup_pos = file_label_response.response.rect.left_bottom();
                                // Construct full path
                                let full_path = if let Some(repo) = &self.repo_path {
                                    repo.join(&file_path)
                                } else {
                                    file_path.clone()
                                };
                                self.file_opener.open_with_base(
                                    popup_pos,
                                    full_path,
                                    self.repo_path.clone(),
                                );
                            }
                        }

                        // Score badge (more subtle)
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:.0}%", search_result.score * 10.0))
                                    .color(text_col.gamma_multiply(0.35))
                                    .size(typography::XS),
                            );
                            ui.label(
                                RichText::new("relevance")
                                    .color(text_col.gamma_multiply(0.25))
                                    .size(typography::XS),
                            );
                        });

                        // Content preview based on result type
                        let is_commit =
                            matches!(search_result.kind, SearchResultKind::Commit { .. });

                        if is_commit {
                            // Commits: show diff with highlighting
                            if let Some(snippet) = &search_result.snippet {
                                ui.add_space(8.0);
                                let remaining_height = ui.available_height();
                                egui::ScrollArea::both()
                                    .max_height(remaining_height)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        for line in snippet.lines() {
                                            render_diff_line_preview(
                                                ui, line, text_col, self.theme,
                                            );
                                        }
                                    });
                            }
                        } else {
                            // Metrics/Alerts: show source code preview
                            ui.add_space(8.0);
                            let remaining_height = ui.available_height();
                            // Construct full path by joining repo_path with relative file path
                            let full_path = if let Some(repo) = &self.repo_path {
                                repo.join(&search_result.file)
                            } else {
                                search_result.file.clone()
                            };
                            render_source_preview(
                                ui,
                                &full_path,
                                search_result.line,
                                remaining_height,
                                text_col,
                                _colors,
                                self.theme,
                                self.highlight_cache.as_ref(),
                            );
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    UnifiedResult::DemoResult(demo) => {
                        // File location
                        if !demo.file.is_empty() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(egui_nerdfonts::regular::FILE_DOCUMENT_OUTLINE)
                                        .color(self.theme.accent_muted())
                                        .size(14.0),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("{}:{}", demo.file, demo.line))
                                        .color(text_col.gamma_multiply(0.6))
                                        .size(typography::SM),
                                );
                            });
                        }

                        // Type-specific preview
                        match &demo.kind {
                            DemoResultKind::Commit { diff, .. } => {
                                ui.add_space(8.0);
                                let remaining_height = ui.available_height();
                                egui::ScrollArea::both()
                                    .max_height(remaining_height)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        for line in diff.lines() {
                                            render_diff_line_preview(
                                                ui, line, text_col, self.theme,
                                            );
                                        }
                                    });
                            }
                            DemoResultKind::Alert { severity } => {
                                ui.add_space(8.0);
                                // Severity badge
                                let severity_color = match severity.as_str() {
                                    "critical" => palette::semantic::ERROR,
                                    "warning" => palette::semantic::WARNING,
                                    _ => palette::semantic::INFO,
                                };
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(severity)
                                            .color(severity_color)
                                            .size(typography::SM)
                                            .strong(),
                                    );
                                });

                                // Expression snippet
                                if let Some(snippet) = &demo.snippet {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("Expression")
                                            .color(text_muted)
                                            .size(typography::XS),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(snippet)
                                            .color(text_col)
                                            .font(typography::monospace(typography::SM)),
                                    );
                                }
                            }
                            DemoResultKind::Metric => {
                                // Snippet for metrics
                                if let Some(snippet) = &demo.snippet {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("Source")
                                            .color(text_muted)
                                            .size(typography::XS),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(snippet)
                                            .color(text_col)
                                            .font(typography::monospace(typography::SM)),
                                    );
                                }
                            }
                        }
                    }
                }
            });
    }

    /// Renders the footer with keyboard hints.
    fn render_footer(&self, ui: &mut egui::Ui, text_muted: Color32, accent_hover: Color32) {
        let accent = accent_hover;
        let hint_color = text_muted.gamma_multiply(0.8);

        ui.horizontal(|ui| {
            ui.add_space(16.0);
            // Key hints with accent-colored keys
            ui.label(RichText::new("↑↓").color(accent).size(typography::XS));
            ui.label(
                RichText::new("navigate")
                    .color(hint_color)
                    .size(typography::XS),
            );
            ui.add_space(16.0);
            ui.label(RichText::new("⏎").color(accent).size(typography::XS));
            ui.label(
                RichText::new("select")
                    .color(hint_color)
                    .size(typography::XS),
            );
            ui.add_space(16.0);
            // Tab to cycle modes
            ui.label(RichText::new("tab").color(accent).size(typography::XS));
            ui.label(
                RichText::new("cycle")
                    .color(hint_color)
                    .size(typography::XS),
            );
            ui.add_space(16.0);
            // Mode prefix hints
            ui.label(RichText::new("@!#").color(accent).size(typography::XS));
            ui.label(
                RichText::new("modes")
                    .color(hint_color)
                    .size(typography::XS),
            );
            // 'o' to open file in external app (native only, only for codebase results with file paths)
            #[cfg(not(target_arch = "wasm32"))]
            if self.selected_has_file_path() {
                ui.add_space(16.0);
                ui.label(RichText::new("o").color(accent).size(typography::XS));
                ui.label(RichText::new("open").color(hint_color).size(typography::XS));
            }
            ui.add_space(16.0);
            ui.label(RichText::new("esc").color(accent).size(typography::XS));
            ui.label(
                RichText::new("close")
                    .color(hint_color)
                    .size(typography::XS),
            );
        });
    }

    /// Returns true if the currently selected result is a codebase result with a file path.
    #[cfg(not(target_arch = "wasm32"))]
    fn selected_has_file_path(&self) -> bool {
        match self.results.get(self.selected_index) {
            Some(UnifiedResult::CodebaseResult(search_result)) => {
                !search_result.file.as_os_str().is_empty()
            }
            _ => false,
        }
    }

    /// Handles selection of a result and returns the appropriate action.
    fn handle_selection(&self, result: &UnifiedResult) -> Option<UnifiedFinderAction> {
        match result {
            UnifiedResult::LiveMetric { name, .. } => {
                Some(UnifiedFinderAction::CreateMetricPane(name.clone()))
            }
            #[cfg(not(target_arch = "wasm32"))]
            UnifiedResult::CodebaseResult(search_result) => match &search_result.kind {
                SearchResultKind::Metric(_) | SearchResultKind::Alert { .. } => {
                    if !search_result.file.as_os_str().is_empty() {
                        Some(UnifiedFinderAction::NavigateToSource {
                            file: search_result.file.clone(),
                            line: search_result.line,
                        })
                    } else {
                        None
                    }
                }
                SearchResultKind::Commit { hash, diff, .. } => {
                    Some(UnifiedFinderAction::OpenDiffViewer {
                        hash: hash.clone(),
                        message: search_result.name.clone(),
                        diff: diff.clone(),
                    })
                }
            },
            // On WASM, demo results open diff viewer for commits or create metric panes
            #[cfg(target_arch = "wasm32")]
            UnifiedResult::DemoResult(demo) => match &demo.kind {
                DemoResultKind::Commit { hash, diff } => {
                    Some(UnifiedFinderAction::OpenDiffViewer {
                        hash: hash.clone(),
                        message: demo.name.clone(),
                        diff: diff.clone(),
                    })
                }
                _ => Some(UnifiedFinderAction::CreateMetricPane(demo.name.clone())),
            },
        }
    }

    /// Renders a premium empty state with centered icon, message, and optional hint.
    #[allow(clippy::too_many_arguments)]
    fn render_empty_state(
        &self,
        ui: &mut egui::Ui,
        content_height: f32,
        icon: &str,
        message: &str,
        hint: Option<&str>,
        text_col: Color32,
        text_muted: Color32,
        accent_col: Color32,
    ) {
        // Use allocate_ui_with_layout to ensure consistent height
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), content_height),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                // Center the content vertically
                let icon_height = 42.0;
                let message_height = 20.0;
                let hint_height = if hint.is_some() { 20.0 } else { 0.0 };
                let total_content = icon_height + 20.0 + message_height + 8.0 + hint_height;
                let top_padding = (content_height - total_content) / 2.0;

                ui.add_space(top_padding.max(40.0));

                // Icon inside a subtle circular background (premium feel)
                let icon_area_size = 72.0;
                let (icon_rect, _) = ui.allocate_exact_size(
                    egui::vec2(icon_area_size, icon_area_size),
                    egui::Sense::hover(),
                );

                // Circular background with subtle gradient feel
                let circle_center = icon_rect.center();
                let circle_radius = icon_area_size / 2.0;

                // Outer subtle glow
                ui.painter().circle_filled(
                    circle_center,
                    circle_radius,
                    accent_col.gamma_multiply(0.08),
                );

                // Inner circle slightly brighter
                ui.painter().circle_filled(
                    circle_center,
                    circle_radius * 0.85,
                    accent_col.gamma_multiply(0.05),
                );

                // Icon centered in the circle
                let icon_galley = ui.painter().layout_no_wrap(
                    icon.to_string(),
                    typography::proportional(icon_height),
                    accent_col.gamma_multiply(0.5),
                );
                let icon_pos = egui::pos2(
                    circle_center.x - icon_galley.size().x / 2.0,
                    circle_center.y - icon_galley.size().y / 2.0,
                );
                ui.painter().galley(icon_pos, icon_galley, accent_col);

                ui.add_space(20.0);

                // Message text
                ui.label(
                    RichText::new(message)
                        .color(text_col.gamma_multiply(0.7))
                        .font(typography::proportional(typography::MD)),
                );

                // Hint text (smaller, more muted)
                if let Some(hint_text) = hint {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(hint_text)
                            .color(text_muted)
                            .font(typography::proportional(typography::SM)),
                    );
                }
            },
        );
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Truncates a string to fit within a given pixel width, adding "..." if truncated.
fn truncate_to_width(text: &str, max_width: f32, font: egui::FontId, ui: &egui::Ui) -> String {
    // Quick check - if the text is short, it probably fits
    if text.len() < 20 {
        return text.to_string();
    }

    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        font.clone(),
        Color32::WHITE, // Color doesn't matter for width calculation
    );

    if galley.size().x <= max_width {
        return text.to_string();
    }

    // Binary search for the right length
    let mut low = 0;
    let mut high = text.chars().count();
    let chars: Vec<char> = text.chars().collect();

    while low < high {
        let mid = (low + high).div_ceil(2);
        let truncated: String = chars[..mid].iter().collect();
        let test_str = format!("{truncated}...");

        let test_galley = ui
            .painter()
            .layout_no_wrap(test_str, font.clone(), Color32::WHITE);

        if test_galley.size().x <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    if low == 0 {
        "...".to_string()
    } else {
        let truncated: String = chars[..low].iter().collect();
        format!("{truncated}...")
    }
}

/// Creates a galley with highlighted match positions for fuzzy search results.
///
/// Characters at positions in `match_positions` are rendered with `highlight_color`,
/// all other characters use `normal_color`.
fn create_highlighted_galley(
    ui: &egui::Ui,
    text: &str,
    match_positions: &[usize],
    font: egui::FontId,
    normal_color: Color32,
    highlight_color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::default();

    for (i, ch) in text.chars().enumerate() {
        let color = if match_positions.contains(&i) {
            highlight_color
        } else {
            normal_color
        };

        job.append(
            &ch.to_string(),
            0.0,
            egui::TextFormat {
                font_id: font.clone(),
                color,
                ..Default::default()
            },
        );
    }

    ui.fonts_mut(|f| f.layout_job(job))
}
