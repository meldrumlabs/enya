//! ViewportFilter component - A vim-style `/` search filter for the dashboard viewport.
//!
//! This component provides a ripgrep-style filter that shows only panes whose queries
//! contain the search term. Similar to vim's `/` search but for filtering visible panes.

use egui::{FontId, Key, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

use super::finder_utils::OverlayStyle;

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

    /// Check if a query matches the current filter (live filtering while typing)
    pub fn matches(&self, query: &str) -> bool {
        // Use the live pattern if filter is open, otherwise use applied pattern
        let pattern = if self.is_open {
            &self.pattern
        } else {
            &self.applied_pattern
        };

        if pattern.is_empty() {
            return true;
        }
        // Case-insensitive search
        query.to_lowercase().contains(&pattern.to_lowercase())
    }

    /// Show the filter input overlay
    pub fn show(&mut self, ctx: &egui::Context) -> ViewportFilterResult {
        if !self.is_open {
            return ViewportFilterResult::None;
        }

        let mut result = ViewportFilterResult::None;
        let mut should_close = false;
        let mut should_apply = false;
        let mut should_clear = false;

        // Handle keyboard shortcuts
        ctx.input(|input| {
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

        // Get available rect for sizing
        let available_rect = ctx.available_rect();

        // Position at center of screen
        let popup_width = (available_rect.width() * 0.5).clamp(300.0, 600.0);

        egui::Area::new(egui::Id::new("viewport_filter_area"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);

                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.add_space(12.0);

                        // Search icon and slash indicator
                        ui.label(
                            RichText::new(format!("{} /", semantic_icons::action::SEARCH))
                                .color(text_color(self.theme).gamma_multiply(0.7))
                                .size(14.0)
                                .family(egui::FontFamily::Monospace),
                        );

                        ui.add_space(4.0);

                        // Search input
                        let input_id = egui::Id::new("viewport_filter_input");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.pattern)
                                .id(input_id)
                                .desired_width(popup_width - 180.0)
                                .font(FontId::monospace(14.0))
                                .frame(false)
                                .text_color(text_color(self.theme))
                                .hint_text(
                                    RichText::new("filter panes...")
                                        .color(text_color(self.theme).gamma_multiply(0.4)),
                                ),
                        );

                        // Auto-focus
                        if self.needs_focus {
                            response.request_focus();
                            self.needs_focus = false;
                        }

                        ui.add_space(8.0);

                        // Match count indicator
                        let count_text =
                            if self.pattern.is_empty() && self.applied_pattern.is_empty() {
                                format!("{} panes", self.total_count)
                            } else {
                                format!("{}/{}", self.match_count, self.total_count)
                            };

                        let count_color = if self.match_count == 0 && !self.pattern.is_empty() {
                            palette::semantic::WARNING
                        } else {
                            text_color(self.theme).gamma_multiply(0.5)
                        };

                        ui.label(
                            RichText::new(count_text)
                                .color(count_color)
                                .size(12.0)
                                .family(egui::FontFamily::Monospace),
                        );

                        ui.add_space(12.0);
                    });

                    ui.add_space(4.0);

                    // Hint text
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let hint_color = text_color(self.theme).gamma_multiply(0.4);
                        ui.label(RichText::new("Enter").color(hint_color).size(10.0));
                        ui.label(RichText::new("apply").color(hint_color).size(10.0));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Esc").color(hint_color).size(10.0));
                        ui.label(RichText::new("close").color(hint_color).size(10.0));

                        if self.is_active() {
                            ui.add_space(8.0);
                            ui.label(RichText::new("Esc×2").color(hint_color).size(10.0));
                            ui.label(RichText::new("clear").color(hint_color).size(10.0));
                        }
                    });

                    ui.add_space(8.0);
                });
            });

        // Handle actions - surrender focus when closing
        let input_id = egui::Id::new("viewport_filter_input");
        if should_clear {
            ctx.memory_mut(|mem| mem.surrender_focus(input_id));
            self.clear();
            result = ViewportFilterResult::Cleared;
        } else if should_apply {
            ctx.memory_mut(|mem| mem.surrender_focus(input_id));
            self.applied_pattern = self.pattern.clone();
            self.is_open = false;
            self.pattern.clear();
            if self.applied_pattern.is_empty() {
                result = ViewportFilterResult::Cleared;
            } else {
                result = ViewportFilterResult::Applied(self.applied_pattern.clone());
            }
        } else if should_close {
            ctx.memory_mut(|mem| mem.surrender_focus(input_id));
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
