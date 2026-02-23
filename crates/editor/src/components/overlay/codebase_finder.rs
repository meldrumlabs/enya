//! CodebaseFinder - A telescope/fzf-style finder for codebase search.
//!
//! This module provides a finder modal for searching the codebase using
//! Tantivy full-text search. It can search metrics, alerts, and commits
//! with relevance ranking.
//!
//! # Features
//!
//! - Full-text search across metrics, alerts, and git commits
//! - Filter by type (all, metrics, alerts, commits)
//! - Preview pane showing result details
//! - Relevance scoring with BM25
//!
//! # Keyboard Shortcuts
//!
//! - `Space+c` - Open codebase finder
//! - `Tab` - Cycle through filter modes
//! - `Enter` - Select result (navigate to source, or open diff viewer for commits)
//! - `Escape` - Close finder
//!
//! For commits, pressing Enter opens the dedicated DiffViewerOverlay for
//! navigating between changed files with syntax highlighting.

use egui::{Color32, RichText};

use crate::codebase::search::{SearchFilter, SearchResult, SearchResultKind};
use crate::components::OverlayColors;
use crate::ui::palette;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Result from the codebase finder.
#[derive(Debug, Clone)]
pub struct CodebaseFinderResult {
    /// The selected search result.
    pub result: SearchResult,
}

/// Status of the codebase for display in the finder.
#[derive(Debug, Clone, Default)]
pub enum CodebaseFinderStatus {
    /// No codebase configured.
    #[default]
    NoCodebase,
    /// Codebase is being indexed.
    Indexing,
    /// Codebase is ready (with metric count).
    Ready { metric_count: usize },
}

/// A telescope/fzf-style finder for codebase search.
pub struct CodebaseFinder {
    /// Whether the finder is open.
    is_open: bool,
    /// Current search query.
    query: String,
    /// Current filter mode.
    filter: SearchFilter,
    /// Search results.
    results: Vec<SearchResult>,
    /// Selected result index.
    selected_index: usize,
    /// Current theme.
    theme: AppTheme,
    /// Whether to request focus on next frame.
    request_focus: bool,
    /// Codebase status for display.
    status: CodebaseFinderStatus,
}

impl Default for CodebaseFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl CodebaseFinder {
    /// Creates a new codebase finder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            filter: SearchFilter::All,
            results: Vec::new(),
            selected_index: 0,
            theme: AppTheme::Dark,
            request_focus: false,
            status: CodebaseFinderStatus::default(),
        }
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

    /// Opens the finder.
    pub fn open(&mut self) {
        self.is_open = true;
        self.query.clear();
        self.results.clear();
        self.selected_index = 0;
        self.request_focus = true;
    }

    /// Closes the finder.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Gets the current query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Gets the current filter.
    #[must_use]
    pub fn filter(&self) -> SearchFilter {
        self.filter
    }

    /// Updates search results from the codebase manager.
    pub fn set_results(&mut self, results: Vec<SearchResult>) {
        self.results = results;
        // Keep selection in bounds
        if self.selected_index >= self.results.len() && !self.results.is_empty() {
            self.selected_index = self.results.len() - 1;
        }
    }

    /// Sets the codebase status for display.
    pub fn set_status(&mut self, status: CodebaseFinderStatus) {
        self.status = status;
    }

    /// Cycles to the next filter mode.
    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            SearchFilter::All => SearchFilter::Metrics,
            SearchFilter::Metrics => SearchFilter::Alerts,
            SearchFilter::Alerts => SearchFilter::Commits,
            SearchFilter::Commits => SearchFilter::All,
        };
    }

    /// Shows the finder and returns the selected result if any.
    #[must_use]
    pub fn show(&mut self, ctx: &egui::Context) -> Option<CodebaseFinderResult> {
        if !self.is_open {
            return None;
        }

        let mut result = None;
        let mut should_close = false;

        // Handle keyboard input - use consume_key to prevent multiple processing
        ctx.input_mut(|input| {
            // Escape to close
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_close = true;
            }

            // Tab to cycle filter
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                self.cycle_filter();
            }

            // Arrow keys for navigation
            if (input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::K))
                && self.selected_index > 0
            {
                self.selected_index -= 1;
            }
            if (input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::J))
                && self.selected_index + 1 < self.results.len()
            {
                self.selected_index += 1;
            }

            // Enter to select
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                && !self.results.is_empty()
            {
                if let Some(selected) = self.results.get(self.selected_index) {
                    result = Some(CodebaseFinderResult {
                        result: selected.clone(),
                    });
                    should_close = true;
                }
            }
        });

        if should_close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
            return result;
        }

        let colors = OverlayColors::new(self.theme);
        let bg_color = palette::bg_surface(self.theme);

        // Modal overlay
        egui::Area::new(egui::Id::new("codebase_finder"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -50.0))
            .constrain_to(ctx.available_rect())
            .show(ctx, |ui| {
                // Background dimming
                #[allow(deprecated)]
                let content_rect = ui.ctx().input(|i| i.screen_rect());
                ui.painter()
                    .rect_filled(content_rect, 0.0, Color32::from_black_alpha(180));

                // Main container
                let width = 700.0_f32.min(content_rect.width() - 40.0);
                let height = 500.0_f32.min(content_rect.height() - 100.0);

                egui::Frame::new()
                    .fill(bg_color)
                    .corner_radius(8.0)
                    .stroke(egui::Stroke::new(1.0, colors.separator))
                    .shadow(egui::epaint::Shadow::NONE)
                    .inner_margin(egui::Margin::same(0))
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(width, height));
                        ui.set_max_size(egui::vec2(width, height));

                        ui.vertical(|ui| {
                            // Header with search input and filter
                            self.render_header(ui, &colors);

                            ui.add_space(4.0);

                            // Separator
                            ui.painter().hline(
                                ui.available_rect_before_wrap().x_range(),
                                ui.cursor().top(),
                                egui::Stroke::new(1.0, colors.separator),
                            );

                            // Results list and preview
                            self.render_content(ui, &colors);
                        });
                    });
            });

        result
    }

    /// Renders the header with search input and filter buttons.
    fn render_header(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);

            // Search icon
            ui.label(
                RichText::new(egui_nerdfonts::regular::MAGNIFY)
                    .color(colors.accent)
                    .size(16.0),
            );

            ui.add_space(8.0);

            // Search input
            let response = ui.add_sized(
                egui::vec2(ui.available_width() - 180.0, 24.0),
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text(
                        RichText::new("Search codebase...")
                            .color(colors.faint_text)
                            .size(typography::MD),
                    )
                    .frame(false)
                    .font(typography::proportional(typography::MD)),
            );

            if self.request_focus {
                response.request_focus();
                self.request_focus = false;
            }

            ui.add_space(8.0);

            // Filter buttons
            self.render_filter_buttons(ui, colors);

            ui.add_space(12.0);
        });
        ui.add_space(8.0);
    }

    /// Renders the filter toggle buttons.
    fn render_filter_buttons(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        let filters = [
            (SearchFilter::All, "All"),
            (SearchFilter::Metrics, "Metrics"),
            (SearchFilter::Alerts, "Alerts"),
            (SearchFilter::Commits, "Commits"),
        ];

        for (filter, label) in filters {
            let is_active = self.filter == filter;
            let (bg, text_color) = if is_active {
                (colors.accent, Color32::WHITE)
            } else {
                (colors.elevated_bg, colors.muted_text)
            };

            let response = egui::Frame::new()
                .fill(bg)
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(6, 2))
                .show(ui, |ui| {
                    ui.label(RichText::new(label).color(text_color).size(typography::XS));
                })
                .response;

            if response.clicked() {
                self.filter = filter;
            }
        }
    }

    /// Renders the main content area with results and preview.
    fn render_content(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        ui.add_space(4.0);

        // Check codebase status first
        match &self.status {
            CodebaseFinderStatus::NoCodebase => {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.label(
                        RichText::new(egui_nerdfonts::regular::COG)
                            .color(colors.faint_text)
                            .size(32.0),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("No codebase configured")
                            .color(colors.faint_text)
                            .size(typography::MD),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Create a workspace with a Git repository to enable code search",
                        )
                        .color(colors.faint_text)
                        .size(typography::SM),
                    );
                });
                return;
            }
            CodebaseFinderStatus::Indexing => {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.label(
                        RichText::new(egui_nerdfonts::regular::LOADING)
                            .color(colors.accent)
                            .size(32.0),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Indexing codebase...")
                            .color(colors.faint_text)
                            .size(typography::MD),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Search will be available once indexing completes")
                            .color(colors.faint_text)
                            .size(typography::SM),
                    );
                });
                return;
            }
            CodebaseFinderStatus::Ready { .. } => {
                // Continue to show search UI
            }
        }

        if self.query.is_empty() {
            // Empty state - show helpful message
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(
                    RichText::new("Type to search metrics, alerts, and commits")
                        .color(colors.faint_text)
                        .size(typography::MD),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Tab to cycle filters | j/k to navigate | Enter to select")
                        .color(colors.faint_text)
                        .size(typography::SM),
                );
            });
            return;
        }

        if self.results.is_empty() {
            // No results
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(
                    RichText::new("No results found")
                        .color(colors.faint_text)
                        .size(typography::MD),
                );
            });
            return;
        }

        // Two-column layout: results list and preview
        // Capture available height before horizontal layout (which doesn't propagate height well)
        let content_height = ui.available_height();

        ui.horizontal(|ui| {
            // Results list (left side)
            ui.allocate_ui_with_layout(
                egui::vec2(350.0, content_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.render_results_list(ui, colors, content_height);
                },
            );

            // Separator
            ui.painter().vline(
                ui.cursor().left(),
                ui.available_rect_before_wrap().y_range(),
                egui::Stroke::new(1.0, colors.separator),
            );

            // Preview (right side)
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), content_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.render_preview(ui, colors);
                },
            );
        });
    }

    /// Renders the results list.
    fn render_results_list(&mut self, ui: &mut egui::Ui, colors: &OverlayColors, max_height: f32) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(max_height)
            .show(ui, |ui| {
                for (i, result) in self.results.iter().enumerate() {
                    let is_selected = i == self.selected_index;

                    let row_bg_color = if is_selected {
                        colors.elevated_bg
                    } else {
                        Color32::TRANSPARENT
                    };

                    let response = egui::Frame::new()
                        .fill(row_bg_color)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    // Type icon
                                    let (icon, icon_color) =
                                        Self::result_icon(result, colors, self.theme);
                                    ui.label(RichText::new(icon).color(icon_color).size(12.0));
                                    ui.add_space(6.0);

                                    // Name (truncated)
                                    let name = if result.name.len() > 40 {
                                        format!("{}...", &result.name[..37])
                                    } else {
                                        result.name.clone()
                                    };
                                    ui.label(
                                        RichText::new(name).color(colors.text).size(typography::SM),
                                    );

                                    // Score badge
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(format!("{:.1}", result.score))
                                                    .color(colors.faint_text)
                                                    .size(typography::XS),
                                            );
                                        },
                                    );
                                });

                                // Show file location if available (to distinguish duplicates)
                                if !result.file.as_os_str().is_empty() {
                                    ui.horizontal(|ui| {
                                        ui.add_space(18.0); // Align with name (after icon)
                                        // Show just the filename and line number for compact display
                                        let file_display = result
                                            .file
                                            .file_name()
                                            .map(|f| f.to_string_lossy().into_owned())
                                            .unwrap_or_default();
                                        ui.label(
                                            RichText::new(format!(
                                                "{}:{}",
                                                file_display, result.line
                                            ))
                                            .color(colors.faint_text)
                                            .size(typography::XS),
                                        );
                                    });
                                }
                            });
                        })
                        .response;

                    if response.clicked() {
                        self.selected_index = i;
                    }

                    if response.hovered() && !is_selected {
                        self.selected_index = i;
                    }
                }
            });
    }

    /// Renders the preview pane for the selected result.
    fn render_preview(&self, ui: &mut egui::Ui, colors: &OverlayColors) {
        let Some(result) = self.results.get(self.selected_index) else {
            return;
        };

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.vertical(|ui| {
                self.render_preview_content(ui, result, colors);
            });
        });
    }

    /// Renders the preview content for a search result.
    fn render_preview_content(
        &self,
        ui: &mut egui::Ui,
        result: &SearchResult,
        colors: &OverlayColors,
    ) {
        // Type and name header
        let (icon, icon_color) = Self::result_icon(result, colors, self.theme);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon).color(icon_color).size(16.0));
            ui.add_space(6.0);
            ui.label(
                RichText::new(&result.name)
                    .color(colors.text)
                    .size(typography::LG)
                    .strong(),
            );
        });

        ui.add_space(8.0);

        // Type-specific details
        match &result.kind {
            SearchResultKind::Metric(kind) => {
                ui.label(
                    RichText::new(format!("Type: {kind:?}"))
                        .color(colors.muted_text)
                        .size(typography::SM),
                );
            }
            SearchResultKind::Alert { severity } => {
                if let Some(sev) = severity {
                    let sev_color = match sev.as_str() {
                        "critical" => palette::semantic::ERROR,
                        "warning" => palette::semantic::WARNING,
                        _ => colors.muted_text,
                    };
                    ui.label(
                        RichText::new(format!("Severity: {sev}"))
                            .color(sev_color)
                            .size(typography::SM),
                    );
                }
            }
            SearchResultKind::Commit {
                hash, timestamp, ..
            } => {
                ui.label(
                    RichText::new(format!("Commit: {}", &hash[..7.min(hash.len())]))
                        .color(colors.muted_text)
                        .size(typography::SM),
                );
                let datetime = format_unix_timestamp(*timestamp);
                ui.label(
                    RichText::new(datetime)
                        .color(colors.faint_text)
                        .size(typography::XS),
                );
            }
        }

        ui.add_space(8.0);

        // File path and line
        if !result.file.as_os_str().is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(egui_nerdfonts::regular::FILE)
                        .color(colors.muted_text)
                        .size(12.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{}:{}", result.file.display(), result.line))
                        .color(colors.muted_text)
                        .size(typography::SM),
                );
            });
        }

        ui.add_space(8.0);

        // Code snippet (for non-commits show code preview; for commits show hint)
        if matches!(result.kind, SearchResultKind::Commit { .. }) {
            ui.label(
                RichText::new("Press Enter to view diff with file navigation")
                    .color(colors.faint_text)
                    .size(typography::SM)
                    .italics(),
            );
        } else if let Some(snippet) = &result.snippet {
            ui.add_space(4.0);
            egui::Frame::new()
                .fill(colors.elevated_bg)
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(snippet)
                            .color(colors.text)
                            .size(typography::SM)
                            .monospace(),
                    );
                });
        }

        ui.add_space(12.0);

        // Score
        ui.label(
            RichText::new(format!("Relevance score: {:.2}", result.score))
                .color(colors.faint_text)
                .size(typography::XS),
        );
    }

    /// Returns the icon and color for a search result.
    fn result_icon(
        result: &SearchResult,
        colors: &OverlayColors,
        theme: AppTheme,
    ) -> (&'static str, Color32) {
        use egui_nerdfonts::regular;
        match &result.kind {
            SearchResultKind::Metric(_) => (regular::CHART_LINE, colors.accent),
            SearchResultKind::Alert { .. } => (regular::BELL_ALERT, palette::semantic::WARNING),
            SearchResultKind::Commit { .. } => (regular::GIT_COMMIT, theme.chart_commit_marker()),
        }
    }
}

/// Format a Unix timestamp as YYYY-MM-DD HH:MM (UTC).
fn format_unix_timestamp(timestamp: i64) -> String {
    if timestamp < 0 {
        return "Unknown".to_string();
    }

    const SECS_PER_MIN: i64 = 60;
    const SECS_PER_HOUR: i64 = 3600;
    const SECS_PER_DAY: i64 = 86400;

    let days_since_epoch = timestamp / SECS_PER_DAY;
    let time_of_day = timestamp % SECS_PER_DAY;
    let hours = (time_of_day / SECS_PER_HOUR) % 24;
    let minutes = (time_of_day % SECS_PER_HOUR) / SECS_PER_MIN;

    // Calculate year and day of year
    let mut days = days_since_epoch;
    let mut year = 1970i64;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_months: [i64; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for (i, &dim) in days_in_months.iter().enumerate() {
        if days < dim {
            month = (i + 1) as u32;
            break;
        }
        days -= dim;
    }
    let day = (days + 1) as u32;

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}
