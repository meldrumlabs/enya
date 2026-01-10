//! ViewportFilter component - A vim-style `/` search filter for the dashboard viewport.
//!
//! This component provides a ripgrep-style filter that shows only panes whose queries
//! contain the search term. Similar to vim's `/` search but for filtering visible panes.
//! Renders as a bottom bar above the status line (like vim's command line).

use egui::{FontId, Key, RichText};

use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

use crate::components::util::finder_utils::{OverlayStyle, render_key_badge};

/// Result of viewport filter interaction
#[derive(Debug, Clone, PartialEq)]
pub enum ViewportFilterResult {
    /// No action (filter still active or inactive)
    None,
    /// Filter was applied/updated - contains the search pattern
    Applied(String),
    /// Filter was cleared
    Cleared,
}

/// State for the viewport filter
#[derive(Debug, Clone, Default)]
pub struct ViewportFilter {
    /// Whether the filter input is currently open
    is_open: bool,
    /// The current search pattern
    pattern: String,
    /// The applied pattern (committed with Enter)
    applied_pattern: String,
    /// Current theme
    theme: AppTheme,
    /// Whether the input needs focus
    needs_focus: bool,
    /// Number of matching panes (for display)
    match_count: usize,
    /// Total number of panes
    total_count: usize,
}

impl ViewportFilter {
    /// Create a new viewport filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the filter input is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Check if there's an active filter (either applied or currently typing)
    pub fn is_active(&self) -> bool {
        // Active if we have an applied pattern OR if we're typing a live pattern
        !self.applied_pattern.is_empty() || (self.is_open && !self.pattern.is_empty())
    }

    /// Get the applied pattern
    pub fn applied_pattern(&self) -> &str {
        &self.applied_pattern
    }

    /// Open the filter input
    pub fn open(&mut self) {
        self.is_open = true;
        self.pattern = self.applied_pattern.clone();
        self.needs_focus = true;
    }

    /// Close the filter input without applying
    pub fn close(&mut self) {
        self.is_open = false;
        self.pattern.clear();
        self.needs_focus = false;
    }

    /// Clear the filter entirely
    pub fn clear(&mut self) {
        self.is_open = false;
        self.pattern.clear();
        self.applied_pattern.clear();
        self.needs_focus = false;
        self.match_count = 0;
    }

    /// Update the match counts for display
    pub fn update_counts(&mut self, match_count: usize, total_count: usize) {
        self.match_count = match_count;
        self.total_count = total_count;
    }

    /// Get the current active pattern (live if open, otherwise applied)
    pub fn current_pattern(&self) -> &str {
        if self.is_open {
            &self.pattern
        } else {
            &self.applied_pattern
        }
    }

    /// Check if a query matches the current filter (live filtering while typing)
    pub fn matches(&self, query: &str) -> bool {
        let pattern = self.current_pattern();
        if pattern.is_empty() {
            return true;
        }
        // Case-insensitive search
        query.to_lowercase().contains(&pattern.to_lowercase())
    }

    /// Find the match range in text (case-insensitive)
    /// Returns (start, end) byte indices if found
    pub fn find_match_range(&self, text: &str) -> Option<(usize, usize)> {
        let pattern = self.current_pattern();
        if pattern.is_empty() {
            return None;
        }
        let text_lower = text.to_lowercase();
        let pattern_lower = pattern.to_lowercase();
        text_lower.find(&pattern_lower).map(|start| {
            // Find the actual byte position in original text
            (start, start + pattern.len())
        })
    }

    /// Show the filter input bar (renders above status line in bottom panel)
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) -> ViewportFilterResult {
        if !self.is_open {
            return ViewportFilterResult::None;
        }

        let mut result = ViewportFilterResult::None;
        let mut should_close = false;
        let mut should_apply = false;
        let mut should_clear = false;

        // Handle keyboard shortcuts
        ui.ctx().input(|input| {
            // Escape - close without applying (if pattern is empty, also clear filter)
            if input.key_pressed(Key::Escape) {
                if self.pattern.is_empty() && !self.applied_pattern.is_empty() {
                    should_clear = true;
                } else {
                    should_close = true;
                }
            }
            // Enter - apply filter and close
            if input.key_pressed(Key::Enter) {
                should_apply = true;
            }
        });

        // Get theme-aware colors
        let text_primary = palette::text_primary(self.theme);
        let text_secondary = palette::text_secondary(self.theme);
        let text_tertiary = palette::text_tertiary(self.theme);
        let badge_bg = palette::bg_elevated(self.theme);

        let overlay_style = OverlayStyle::frosted_glass(self.theme);

        // Render as a horizontal bar with glass styling
        let frame = overlay_style
            .frame()
            .inner_margin(egui::Margin::symmetric(16, 8))
            .corner_radius(0.0); // No rounded corners for bottom bar

        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());

            ui.horizontal(|ui| {
                // Slash indicator (vim-style)
                ui.label(
                    RichText::new("/")
                        .color(text_secondary)
                        .size(14.0)
                        .family(egui::FontFamily::Monospace),
                );

                ui.add_space(8.0);

                // Search input - takes most of the width
                let input_id = egui::Id::new("viewport_filter_input");
                let available_width = ui.available_width() - 200.0; // Reserve space for count and hints
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.pattern)
                        .id(input_id)
                        .desired_width(available_width.max(100.0))
                        .font(FontId::monospace(14.0))
                        .frame(false)
                        .text_color(text_primary)
                        .hint_text(RichText::new("filter panes...").color(text_tertiary)),
                );

                // Auto-focus
                if self.needs_focus {
                    response.request_focus();
                    self.needs_focus = false;
                }

                // Right side: count and hints
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Keyboard hints
                    render_key_badge(ui, "Esc", badge_bg, text_tertiary);

                    ui.add_space(8.0);

                    render_key_badge(ui, "Enter", badge_bg, text_tertiary);

                    ui.add_space(16.0);

                    // Match count indicator
                    let count_text = if self.pattern.is_empty() && self.applied_pattern.is_empty() {
                        format!("{} panes", self.total_count)
                    } else {
                        format!("{}/{}", self.match_count, self.total_count)
                    };

                    let count_color = if self.match_count == 0 && !self.pattern.is_empty() {
                        palette::semantic::WARNING
                    } else {
                        text_secondary
                    };

                    ui.label(
                        RichText::new(count_text)
                            .color(count_color)
                            .size(12.0)
                            .family(egui::FontFamily::Monospace),
                    );
                });
            });
        });

        // Handle actions - surrender focus when closing
        let input_id = egui::Id::new("viewport_filter_input");
        if should_clear {
            ui.ctx().memory_mut(|mem| mem.surrender_focus(input_id));
            self.clear();
            result = ViewportFilterResult::Cleared;
        } else if should_apply {
            ui.ctx().memory_mut(|mem| mem.surrender_focus(input_id));
            self.applied_pattern = self.pattern.clone();
            self.is_open = false;
            self.pattern.clear();
            if self.applied_pattern.is_empty() {
                result = ViewportFilterResult::Cleared;
            } else {
                result = ViewportFilterResult::Applied(self.applied_pattern.clone());
            }
        } else if should_close {
            ui.ctx().memory_mut(|mem| mem.surrender_focus(input_id));
            self.close();
        }

        result
    }

    /// Render a status line indicator for the active filter
    pub fn status_indicator(&self, _theme: AppTheme) -> Option<String> {
        if self.is_active() {
            Some(format!(
                "{} /{} ({}/{})",
                semantic_icons::action::FILTER,
                self.applied_pattern,
                self.match_count,
                self.total_count
            ))
        } else {
            None
        }
    }
}
