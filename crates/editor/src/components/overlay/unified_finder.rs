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
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;

#[cfg(not(target_arch = "wasm32"))]
use crate::ui::semantic_icons;

#[cfg(not(target_arch = "wasm32"))]
use super::preview::{render_diff_line_preview, render_source_preview};
#[cfg(not(target_arch = "wasm32"))]
use super::syntax_highlight::HighlightCache;
#[cfg(not(target_arch = "wasm32"))]
use crate::codebase::search::{SearchResult, SearchResultKind};

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

    /// Returns whether this mode requires native (non-WASM) support for codebase search.
    /// Note: Metrics mode can still show live Prometheus metrics on WASM.
    #[must_use]
    pub fn requires_native(&self) -> bool {
        matches!(self, Self::All | Self::Alerts | Self::Commits)
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
    /// A metric from codebase search.
    #[cfg(not(target_arch = "wasm32"))]
    CodebaseResult(SearchResult),
}

impl UnifiedResult {
    /// Returns the display name for this result.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::LiveMetric { name, .. } => name,
            #[cfg(not(target_arch = "wasm32"))]
            Self::CodebaseResult(result) => &result.name,
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
    #[cfg(not(target_arch = "wasm32"))]
    OpenDiffViewer {
        /// Commit hash.
        hash: String,
        /// Commit message (for title).
        message: String,
        /// Full diff content.
        diff: String,
    },
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
            #[cfg(target_arch = "wasm32")]
            _ => {
                // Native-only modes show empty on WASM
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

        // Check if debounce period has elapsed and we need to refresh results
        // Only for Metrics mode - codebase modes (All, Alerts, Commits) are handled
        // externally via set_codebase_results() in the workspace
        let should_refresh = if let Some(last_change) = self.last_query_change {
            let elapsed = last_change.elapsed().as_millis() as u64;
            let is_metrics_mode = matches!(self.mode, FinderMode::Metrics);
            elapsed >= SEARCH_DEBOUNCE_MS
                && self.query != self.last_searched_query
                && is_metrics_mode
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

        // Request repaint if debounce is pending for Metrics mode
        // (codebase modes don't use the internal debounce)
        if self.last_query_change.is_some() && matches!(self.mode, FinderMode::Metrics) {
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

        let colors = FinderColors::new(self.theme);
        let overlay_style = OverlayStyle::frosted_glass(self.theme);

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
                    self.render_header(ui, &colors, total_width);

                    ui.add_space(8.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );
                    ui.add_space(4.0);

                    // Calculate footer height for proper layout
                    let footer_height = 30.0; // Fixed footer height

                    // Content area - takes all remaining space minus footer
                    let content_height = ui.available_height() - footer_height - 16.0; // 16 = spacing + margins
                    clicked_index =
                        self.render_content(ui, &colors, list_width, preview_width, content_height);

                    // Use add_space with remaining available height to push footer to bottom
                    let remaining = ui.available_height() - footer_height - 10.0;
                    if remaining > 0.0 {
                        ui.add_space(remaining);
                    }

                    // Footer separator - now at the bottom
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );

                    // Use bottom-aligned layout to push footer content to the bottom of available space
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.add_space(8.0); // Bottom padding
                        self.render_footer(ui);
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

        if should_close {
            self.close();
        }

        action
    }

    /// Renders the header with search input and mode badge.
    fn render_header(&mut self, ui: &mut egui::Ui, _colors: &FinderColors, total_width: f32) {
        let accent = self.theme.accent_primary();
        let mode_color = self.mode.color(self.theme);
        let text_col = text_color(self.theme);
        let badge_width = 100.0; // Fixed badge width for consistent positioning

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Search icon with accent color
            ui.label(
                RichText::new(egui_nerdfonts::regular::MAGNIFY)
                    .color(accent)
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
                        .color(text_col.gamma_multiply(0.4))
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
                            .color(text_col.gamma_multiply(0.4))
                            .size(typography::XS),
                    );
                }
            });
        });
    }

    /// Renders the main content area. Returns clicked index if any.
    fn render_content(
        &mut self,
        ui: &mut egui::Ui,
        colors: &FinderColors,
        list_width: f32,
        preview_width: f32,
        content_height: f32,
    ) -> Option<usize> {
        // Check for native-only modes on WASM
        #[cfg(target_arch = "wasm32")]
        if self.mode.requires_native() {
            self.render_empty_state(
                ui,
                content_height,
                egui_nerdfonts::regular::DESKTOP,
                "Native app required",
                Some("Codebase search is only available in the native app"),
                colors,
            );
            return None;
        }

        if self.results.is_empty() && !self.query_text().is_empty() {
            // No results
            self.render_empty_state(
                ui,
                content_height,
                egui_nerdfonts::regular::MAGNIFY_CLOSE,
                "No results found",
                None,
                colors,
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
                colors,
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
                        clicked_index = self.render_results_list(ui, colors, content_height);
                    },
                );

                // Separator
                ui.painter().vline(
                    ui.cursor().left(),
                    ui.available_rect_before_wrap().y_range(),
                    egui::Stroke::new(1.0, colors.separator),
                );

                // Preview
                ui.allocate_ui_with_layout(
                    egui::vec2(preview_width, content_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.render_preview(ui, colors);
                    },
                );
            },
        );

        clicked_index
    }

    /// Renders the results list. Returns the index of a clicked item if any.
    fn render_results_list(
        &mut self,
        ui: &mut egui::Ui,
        _colors: &FinderColors,
        max_height: f32,
    ) -> Option<usize> {
        let mut clicked_index: Option<usize> = None;
        let text_col = text_color(self.theme);
        let accent_col = self.theme.accent_primary();
        let highlight_col = self.theme.highlight_match_text();

        // Get the clip rect for the list area to prevent text overflow
        let list_clip_rect = ui.available_rect_before_wrap();

        egui::ScrollArea::vertical()
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
                    let is_commit = false;

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

                    // Scroll into view
                    if is_selected {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });

        clicked_index
    }

    /// Renders the preview pane.
    fn render_preview(&self, ui: &mut egui::Ui, colors: &FinderColors) {
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
                            .color(text_color(self.theme).gamma_multiply(0.4))
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

                let text_col = text_color(self.theme);

                // Header - truncate title to fit available width
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(result.icon())
                            .color(self.mode.color(self.theme))
                            .size(20.0),
                    );
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
                                .color(text_col.gamma_multiply(0.6))
                                .size(typography::SM),
                        );

                        ui.add_space(12.0);

                        // Separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, colors.separator),
                        );
                        ui.add_space(12.0);

                        // Tags section
                        let tag_key_color = self.theme.syntax_key();
                        let tag_value_color = self.theme.syntax_value();

                        ui.label(
                            RichText::new("Available Tags")
                                .color(text_col.gamma_multiply(0.6))
                                .size(typography::XS),
                        );
                        ui.add_space(8.0);

                        if tags.is_empty() {
                            ui.label(
                                RichText::new("No tags available")
                                    .color(text_col.gamma_multiply(0.4))
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
                                                        .color(text_col.gamma_multiply(0.4))
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
                            ui.horizontal(|ui| {
                                // Use language-specific file icon
                                let file_icon = semantic_icons::file_icon(&search_result.file);
                                ui.label(
                                    RichText::new(file_icon)
                                        .color(self.theme.accent_muted())
                                        .size(14.0),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{}:{}",
                                        search_result.file.display(),
                                        search_result.line
                                    ))
                                    .color(text_col.gamma_multiply(0.6))
                                    .size(typography::SM),
                                );
                            });
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
                                colors,
                                self.theme,
                                self.highlight_cache.as_ref(),
                            );
                        }
                    }
                }
            });
    }

    /// Renders the footer with keyboard hints.
    fn render_footer(&self, ui: &mut egui::Ui) {
        let accent = self.theme.accent_hover();
        let hint_color = text_color(self.theme).gamma_multiply(0.4);

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
            ui.add_space(16.0);
            ui.label(RichText::new("esc").color(accent).size(typography::XS));
            ui.label(
                RichText::new("close")
                    .color(hint_color)
                    .size(typography::XS),
            );
        });
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
        }
    }

    /// Renders a premium empty state with centered icon, message, and optional hint.
    fn render_empty_state(
        &self,
        ui: &mut egui::Ui,
        content_height: f32,
        icon: &str,
        message: &str,
        hint: Option<&str>,
        _colors: &FinderColors,
    ) {
        let accent = self.theme.accent_primary();
        let text_col = text_color(self.theme);

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
                    accent.gamma_multiply(0.08),
                );

                // Inner circle slightly brighter
                ui.painter().circle_filled(
                    circle_center,
                    circle_radius * 0.85,
                    accent.gamma_multiply(0.05),
                );

                // Icon centered in the circle
                let icon_galley = ui.painter().layout_no_wrap(
                    icon.to_string(),
                    typography::proportional(icon_height),
                    accent.gamma_multiply(0.5),
                );
                let icon_pos = egui::pos2(
                    circle_center.x - icon_galley.size().x / 2.0,
                    circle_center.y - icon_galley.size().y / 2.0,
                );
                ui.painter().galley(icon_pos, icon_galley, accent);

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
                            .color(text_col.gamma_multiply(0.4))
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
