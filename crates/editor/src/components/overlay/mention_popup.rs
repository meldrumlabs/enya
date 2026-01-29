//! Mention popup for @metric selection.
//!
//! This module provides a popup for selecting metrics by typing `@` followed by
//! a search query. It uses fuzzy matching and displays results in a premium
//! Obsidian Glass styled popup.

use egui::{Color32, RichText, ScrollArea};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;

/// State for the @mention popup
#[derive(Default)]
pub struct MentionPopup {
    /// Whether the popup is visible
    pub active: bool,
    /// The search query (text after @)
    query: String,
    /// Position in input where @ was typed
    at_position: usize,
    /// Available metrics to search
    metrics: Vec<String>,
    /// Filtered results with scores: (metric_name, score, match_positions)
    results: Vec<(String, i64, Vec<usize>)>,
    /// Selected index in results
    selected_index: usize,
    /// Fuzzy matcher
    matcher: Matcher,
    /// Current theme
    theme: AppTheme,
}

impl MentionPopup {
    /// Create a new mention popup.
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            ..Default::default()
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the popup is open.
    pub fn is_open(&self) -> bool {
        self.active
    }

    /// Get the position where @ was typed.
    pub fn get_at_position(&self) -> usize {
        self.at_position
    }

    /// Start the mention popup at the given cursor position.
    pub fn start(&mut self, at_position: usize) {
        self.active = true;
        self.at_position = at_position;
        self.query.clear();
        self.selected_index = 0;
        self.refresh_results();
    }

    /// Close the popup.
    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.selected_index = 0;
        self.results.clear();
    }

    /// Update the search query.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.refresh_results();
    }

    /// Set available metrics.
    pub fn set_metrics(&mut self, metrics: Vec<String>) {
        self.metrics = metrics;
        if self.active {
            self.refresh_results();
        }
    }

    /// Refresh filtered results based on query.
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.query.is_empty() {
            // Show all metrics when query is empty, sorted alphabetically
            let mut sorted = self.metrics.to_vec();
            sorted.sort();
            for metric in sorted.into_iter().take(10) {
                self.results.push((metric, 0, Vec::new()));
            }
        } else {
            // Fuzzy match
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );

            let mut indices: Vec<u32> = Vec::new();
            let mut buf = Vec::new();
            for metric in &self.metrics {
                indices.clear();
                let haystack = Utf32Str::new(metric, &mut buf);

                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    self.results.push((
                        metric.clone(),
                        i64::from(score),
                        indices.iter().map(|&i| i as usize).collect(),
                    ));
                }
            }
            // Sort by score descending
            self.results.sort_by(|a, b| b.1.cmp(&a.1));
            self.results.truncate(10);
        }

        // Reset selection if out of bounds
        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }
    }

    /// Get the currently selected metric.
    pub fn selected(&self) -> Option<&str> {
        self.results
            .get(self.selected_index)
            .map(|(s, _, _)| s.as_str())
    }

    /// Show the mention popup.
    ///
    /// # Arguments
    /// * `ui` - The egui UI context
    /// * `input_rect` - The rectangle of the input field (for positioning)
    /// * `cursor_x` - Optional cursor X position for precise alignment
    pub fn show(&self, ui: &mut egui::Ui, input_rect: egui::Rect, cursor_x: Option<f32>) {
        if self.results.is_empty() {
            return;
        }

        let text_col = self.theme.text_primary();
        let popup_width = 480.0;
        let row_height = 32.0;
        let header_height = 32.0;
        let footer_height = 28.0;
        let results_height = self.results.len() as f32 * row_height;
        let popup_height = header_height + results_height.min(320.0) + footer_height;

        // Position popup above cursor if available, otherwise center above input
        let popup_x = if let Some(cx) = cursor_x {
            cx.max(8.0).min(input_rect.right() - popup_width)
        } else {
            (input_rect.center().x - popup_width / 2.0).max(8.0)
        };

        // Position popup well above the input bar (24px gap)
        let ideal_y = input_rect.top() - popup_height - 24.0;
        let popup_y = ideal_y.max(8.0);

        let popup_pos = egui::pos2(popup_x, popup_y);

        // Premium Obsidian Glass styling
        let style = OverlayStyle::frosted_glass(self.theme);

        // Accent colors
        let emerald_accent = self.theme.accent_hover();
        let emerald_primary = self.theme.accent_primary();
        let accent_col = self.theme.accent_primary();
        let separator_color = self.theme.border_subtle();
        let muted_text = text_col.gamma_multiply(0.6);
        let faint_text = text_col.gamma_multiply(0.4);

        egui::Area::new(egui::Id::new("mention_popup"))
            .fixed_pos(popup_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                let frame_response = style
                    .frame()
                    .inner_margin(egui::Margin::symmetric(0, 6))
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header with emerald accent
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("@")
                                    .size(typography::MD)
                                    .color(emerald_primary)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Select metric")
                                    .size(typography::SM)
                                    .color(muted_text),
                            );
                        });

                        ui.add_space(6.0);

                        // Premium separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(2.0);

                        // Results list
                        let max_results_height = results_height.min(320.0);
                        ScrollArea::vertical()
                            .max_height(max_results_height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for (i, (metric, _score, match_positions)) in
                                    self.results.iter().enumerate()
                                {
                                    let is_selected = i == self.selected_index;

                                    let (row_rect, response) = ui.allocate_exact_size(
                                        egui::vec2(popup_width, row_height),
                                        egui::Sense::hover(),
                                    );
                                    let is_hovered = response.hovered();

                                    // Background
                                    let bg_color = if is_selected {
                                        accent_col.gamma_multiply(0.12)
                                    } else if is_hovered {
                                        text_col.gamma_multiply(0.05)
                                    } else {
                                        Color32::TRANSPARENT
                                    };

                                    if bg_color != Color32::TRANSPARENT {
                                        ui.painter().rect_filled(row_rect, 6.0, bg_color);
                                    }

                                    // Selection indicator bar
                                    if is_selected {
                                        let indicator_rect = egui::Rect::from_min_size(
                                            row_rect.left_top(),
                                            egui::vec2(3.0, row_height),
                                        );
                                        ui.painter().rect_filled(indicator_rect, 2.0, accent_col);
                                    }

                                    // Metric icon
                                    let icon_pos = row_rect.left_center() + egui::vec2(18.0, 0.0);
                                    let icon_color = if is_selected || is_hovered {
                                        accent_col
                                    } else {
                                        text_col.gamma_multiply(0.6)
                                    };
                                    ui.painter().text(
                                        icon_pos,
                                        egui::Align2::LEFT_CENTER,
                                        semantic_icons::metric_type_icon(metric),
                                        typography::proportional(typography::MD),
                                        icon_color,
                                    );

                                    // Metric name with fuzzy match highlighting
                                    let text_pos = row_rect.left_center() + egui::vec2(44.0, 0.0);
                                    if match_positions.is_empty() {
                                        let text_color = if is_selected {
                                            text_col
                                        } else {
                                            text_col.gamma_multiply(0.9)
                                        };
                                        ui.painter().text(
                                            text_pos,
                                            egui::Align2::LEFT_CENTER,
                                            metric,
                                            typography::proportional(typography::MD),
                                            text_color,
                                        );
                                    } else {
                                        let mut job = egui::text::LayoutJob::default();
                                        for (idx, c) in metric.chars().enumerate() {
                                            let is_match = match_positions.contains(&idx);
                                            let color = if is_match {
                                                emerald_accent
                                            } else if is_selected {
                                                text_col
                                            } else {
                                                text_col.gamma_multiply(0.9)
                                            };
                                            job.append(
                                                &c.to_string(),
                                                0.0,
                                                egui::TextFormat {
                                                    font_id: typography::proportional(
                                                        typography::MD,
                                                    ),
                                                    color,
                                                    ..Default::default()
                                                },
                                            );
                                        }
                                        let galley = ui.fonts_mut(|f| f.layout_job(job));
                                        ui.painter().galley(
                                            egui::pos2(
                                                text_pos.x,
                                                text_pos.y - galley.size().y / 2.0,
                                            ),
                                            galley,
                                            text_col,
                                        );
                                    }

                                    // Scroll selected into view
                                    if is_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                }
                            });

                        ui.add_space(2.0);

                        // Footer separator
                        ui.add_space(6.0);
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(6.0);

                        // Footer with keyboard hints
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("↑↓")
                                    .size(typography::XS)
                                    .color(emerald_accent),
                            );
                            ui.label(
                                RichText::new("navigate")
                                    .size(typography::XS)
                                    .color(faint_text),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("⏎")
                                    .size(typography::XS)
                                    .color(emerald_accent),
                            );
                            ui.label(
                                RichText::new("select")
                                    .size(typography::XS)
                                    .color(faint_text),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("esc")
                                    .size(typography::XS)
                                    .color(emerald_accent),
                            );
                            ui.label(
                                RichText::new("cancel")
                                    .size(typography::XS)
                                    .color(faint_text),
                            );
                        });
                    });

                // Draw inner highlight for premium glass effect
                let rect = frame_response.response.rect;
                if let Some(highlight_color) = style.inner_highlight() {
                    let highlight_rect = egui::Rect::from_min_size(
                        rect.left_top() + egui::vec2(1.0, 1.0),
                        egui::vec2(rect.width() - 2.0, 1.5),
                    );
                    ui.painter().rect_filled(
                        highlight_rect,
                        style.corner_radius - 1.0,
                        highlight_color,
                    );
                }
            });
    }
}
