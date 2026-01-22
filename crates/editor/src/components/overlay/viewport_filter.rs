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

        // Handle keyboard shortcuts - use consume_key to prevent multiple processing
        ui.ctx().input_mut(|input| {
            // Escape - close without applying (if pattern is empty, also clear filter)
            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                if self.pattern.is_empty() && !self.applied_pattern.is_empty() {
                    should_clear = true;
                } else {
                    should_close = true;
                }
            }
            // Enter - apply filter and close
            if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
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

    /// Show an inline compact filter input for the top toolbar.
    /// Premium vim-inspired design with polished UX.
    #[profiling::function]
    pub fn show_inline(&mut self, ui: &mut egui::Ui) -> ViewportFilterResult {
        let mut result = ViewportFilterResult::None;

        let text_primary = palette::text_primary(self.theme);
        let text_secondary = palette::text_secondary(self.theme);
        let text_tertiary = palette::text_tertiary(self.theme);
        let accent = self.theme.accent_primary();
        let bg_subtle = accent.gamma_multiply(0.08);
        let bg_active = accent.gamma_multiply(0.15);

        let is_active_or_open = self.is_open || self.is_active();
        let input_id = egui::Id::new("viewport_filter_inline");

        // Calculate container width
        let container_width = if self.is_open { 220.0 } else { 160.0 };

        // Allocate rect for the filter container
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(container_width, 22.0), egui::Sense::click());

        // Click anywhere in container to focus
        if response.clicked() && !self.is_open {
            self.open();
        }

        if ui.is_rect_visible(rect) {
            // Premium container background with subtle rounded corners
            let bg_color = if self.is_open {
                bg_active
            } else if self.is_active() {
                bg_subtle
            } else {
                egui::Color32::TRANSPARENT
            };

            if bg_color != egui::Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, 4.0, bg_color);
            }

            // Render content inside the container
            let inner_rect = rect.shrink2(egui::vec2(8.0, 2.0));
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );

            child_ui.spacing_mut().item_spacing.x = 4.0;

            // Slash prefix - accent when active
            let slash_color = if is_active_or_open {
                accent
            } else {
                text_tertiary
            };
            child_ui.label(
                RichText::new("/")
                    .color(slash_color)
                    .size(13.0)
                    .family(egui::FontFamily::Monospace),
            );

            // Input field
            let input_width = container_width - 50.0;

            let hint = if self.is_active() && !self.is_open {
                self.applied_pattern.clone()
            } else {
                "filter panes".to_string()
            };

            let text_color = if self.is_open {
                text_primary
            } else if self.is_active() {
                accent
            } else {
                text_tertiary
            };

            let text_response = child_ui.add(
                egui::TextEdit::singleline(&mut self.pattern)
                    .id(input_id)
                    .desired_width(input_width)
                    .font(egui::FontId::proportional(13.0))
                    .frame(false)
                    .text_color(text_color)
                    .hint_text(RichText::new(hint).color(text_tertiary)),
            );

            // Auto-focus when opened
            if self.needs_focus {
                ui.ctx()
                    .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
                text_response.request_focus();
                self.needs_focus = false;
            }

            // Track focus changes
            if text_response.gained_focus() && !self.is_open {
                self.is_open = true;
                self.pattern = self.applied_pattern.clone();
            }

            // Handle keyboard - use consume_key to prevent multiple processing
            if text_response.has_focus() {
                ui.ctx().input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                        self.applied_pattern = self.pattern.clone();
                        self.is_open = false;
                        result = if self.applied_pattern.is_empty() {
                            ViewportFilterResult::Cleared
                        } else {
                            ViewportFilterResult::Applied(self.applied_pattern.clone())
                        };
                    }
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                        if self.pattern.is_empty() && !self.applied_pattern.is_empty() {
                            self.clear();
                            result = ViewportFilterResult::Cleared;
                        } else {
                            self.close();
                        }
                    }
                });

                if !self.is_open {
                    ui.ctx().memory_mut(|mem| mem.surrender_focus(input_id));
                    self.pattern.clear();
                }
            } else if self.is_open && !text_response.has_focus() {
                self.is_open = false;
                self.pattern.clear();
            }
        }

        // Match count badge (outside container, to the right)
        if self.is_active() || (self.is_open && !self.pattern.is_empty()) {
            ui.add_space(8.0);

            let (count_color, count_bg) =
                if self.match_count == 0 && !self.current_pattern().is_empty() {
                    (
                        palette::semantic::WARNING,
                        palette::semantic::WARNING.gamma_multiply(0.15),
                    )
                } else {
                    (text_secondary, bg_subtle)
                };

            let count_text = format!("{}/{}", self.match_count, self.total_count);
            let text_size = 11.0;

            // Badge background
            let badge_width = 36.0;
            let (badge_rect, _) =
                ui.allocate_exact_size(egui::vec2(badge_width, 18.0), egui::Sense::hover());

            if ui.is_rect_visible(badge_rect) {
                ui.painter().rect_filled(badge_rect, 3.0, count_bg);
                ui.painter().text(
                    badge_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    count_text,
                    egui::FontId::proportional(text_size),
                    count_color,
                );
            }
        }

        result
    }
}
