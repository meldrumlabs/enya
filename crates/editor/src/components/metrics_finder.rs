//! MetricsFinder - A telescope/fzf-style finder modal for metrics with tag preview

use std::collections::{HashMap, HashSet};

use egui::{Color32, FontId, RichText};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

use super::finder_utils::{
    FinderColors, FinderKeyboardInput, create_highlighted_text, render_keyboard_hints,
};

/// A metric item that can be searched in the metrics finder
#[derive(Debug, Clone)]
pub struct MetricItem {
    /// Metric name
    pub name: String,
    /// Metric category
    pub category: String,
    /// Optional description
    pub description: Option<String>,
    /// Optional unit of measurement
    pub unit: Option<String>,
    /// Tags associated with this metric (key -> set of values)
    pub tags: HashMap<String, HashSet<String>>,
    /// Number of active series for this metric
    pub series_count: usize,
}

impl MetricItem {
    /// Get the primary searchable text for this item
    pub fn search_text(&self) -> &str {
        &self.name
    }

    /// Get the category label for display
    pub fn category_label(&self) -> &str {
        &self.category
    }

    /// Get the icon for metrics
    pub fn icon(&self) -> &'static str {
        egui_phosphor::regular::CHART_LINE
    }
}

/// A fuzzy match result with score and match positions
#[derive(Debug, Clone)]
pub struct MetricResult {
    /// The matched item
    pub item: MetricItem,
    /// Match score (higher is better)
    pub score: i64,
    /// Character positions that matched
    pub match_positions: Vec<usize>,
}

/// A telescope/fzf-style finder modal for metrics with tag preview
pub struct MetricsFinder {
    /// Current search query
    query: String,
    /// All searchable metrics
    items: Vec<MetricItem>,
    /// Filtered and scored results
    results: Vec<MetricResult>,
    /// Currently selected index in results
    selected_index: usize,
    /// Whether the modal is open
    is_open: bool,
    /// Current theme
    theme: AppTheme,
    /// The fuzzy matcher
    matcher: Matcher,
    /// Whether query changed and results need refresh
    needs_refresh: bool,
    /// Whether to show the preview pane
    show_preview: bool,
}

impl Default for MetricsFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsFinder {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            items: Vec::new(),
            results: Vec::new(),
            selected_index: 0,
            is_open: false,
            theme: AppTheme::default(),
            matcher: Matcher::new(Config::DEFAULT),
            needs_refresh: true,
            show_preview: true,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the finder is currently open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Open the metrics finder
    pub fn open(&mut self) {
        self.is_open = true;
        self.query.clear();
        self.selected_index = 0;
        self.needs_refresh = true;
    }

    /// Close the metrics finder
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    /// Set the searchable metrics
    pub fn set_items(&mut self, items: Vec<MetricItem>) {
        self.items = items;
        self.needs_refresh = true;
    }

    /// Refresh the filtered results based on the current query
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.query.is_empty() {
            // Show all items when query is empty, sorted by name
            for item in &self.items {
                self.results.push(MetricResult {
                    item: item.clone(),
                    score: 0,
                    match_positions: Vec::new(),
                });
            }
            // Sort alphabetically by search text
            self.results
                .sort_by(|a, b| a.item.search_text().cmp(b.item.search_text()));
        } else {
            // Parse the query into a pattern for fuzzy matching
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );

            // Fuzzy match and score items
            let mut indices: Vec<u32> = Vec::new();
            let mut buf = Vec::new();
            for item in &self.items {
                indices.clear();
                let haystack = Utf32Str::new(item.search_text(), &mut buf);

                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    self.results.push(MetricResult {
                        item: item.clone(),
                        score: i64::from(score),
                        match_positions: indices.iter().map(|&i| i as usize).collect(),
                    });
                }
            }
            // Sort by score descending (best matches first)
            self.results.sort_by(|a, b| b.score.cmp(&a.score));
        }

        // Reset selection if it's out of bounds
        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }

        self.needs_refresh = false;
    }

    /// Show the metrics finder modal. Returns the selected item if one was chosen.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<MetricItem> {
        if !self.is_open {
            return None;
        }

        // Refresh results if needed
        if self.needs_refresh {
            self.refresh_results();
        }

        let mut selected_item: Option<MetricItem> = None;
        let mut should_close = false;
        let mut toggle_preview = false;

        // Handle keyboard input
        let input = FinderKeyboardInput::read(ctx);

        if input.escape {
            should_close = true;
        }

        if input.toggle_preview {
            toggle_preview = true;
        }

        if input.navigate_up && self.selected_index > 0 {
            self.selected_index -= 1;
        }

        if input.navigate_down && self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }

        if input.confirm && !self.results.is_empty() {
            selected_item = Some(self.results[self.selected_index].item.clone());
            should_close = true;
        }

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let list_width = (screen_rect.width() * 0.35).clamp(300.0, 450.0);
        let preview_width = if self.show_preview {
            (screen_rect.width() * 0.35).clamp(300.0, 400.0)
        } else {
            0.0
        };
        let total_width = list_width + preview_width;
        let popup_max_height = (screen_rect.height() * 0.65).min(550.0);

        // Get the currently selected result for preview
        let selected_result_for_preview = if !self.results.is_empty() {
            Some(self.results[self.selected_index].clone())
        } else {
            None
        };

        let colors = FinderColors::new(self.theme);

        egui::Area::new(egui::Id::new("metrics_finder_popup"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(colors.bg)
                    .stroke(egui::Stroke::new(1.0, colors.border))
                    .corner_radius(8.0)
                    .inner_margin(0.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(total_width);
                        ui.set_max_height(popup_max_height);

                        // Search input section (spans full width)
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                                    .color(text_color(self.theme).gamma_multiply(0.6))
                                    .size(18.0),
                            );
                            ui.add_space(8.0);

                            let text_edit = egui::TextEdit::singleline(&mut self.query)
                                .font(FontId::proportional(16.0))
                                .hint_text(
                                    RichText::new("Search metrics...")
                                        .color(text_color(self.theme).gamma_multiply(0.4)),
                                )
                                .frame(false)
                                .desired_width(total_width - 60.0);

                            let response = ui.add(text_edit);

                            // Request focus on the text input
                            response.request_focus();

                            // Check if query changed
                            if response.changed() {
                                self.needs_refresh = true;
                                self.selected_index = 0;
                            }
                        });

                        ui.add_space(8.0);

                        // Separator below search
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, colors.separator),
                        );
                        ui.add_space(4.0);

                        // Main content area: results list + preview pane
                        let content_height = popup_max_height - 90.0;
                        ui.horizontal(|ui| {
                            // Results list (left side)
                            ui.vertical(|ui| {
                                ui.set_width(list_width);
                                ui.set_height(content_height);

                                egui::ScrollArea::vertical()
                                    .max_height(content_height)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.set_width(list_width - 8.0);
                                        if self.results.is_empty() {
                                            ui.add_space(20.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    RichText::new("No results found")
                                                        .color(
                                                            text_color(self.theme)
                                                                .gamma_multiply(0.5),
                                                        )
                                                        .size(14.0),
                                                );
                                            });
                                            ui.add_space(20.0);
                                        } else {
                                            for (i, result) in self.results.iter().enumerate() {
                                                let is_selected = i == self.selected_index;
                                                let clicked = self.render_result_row(
                                                    ui,
                                                    result,
                                                    is_selected,
                                                    &colors,
                                                );
                                                if clicked {
                                                    selected_item = Some(result.item.clone());
                                                    should_close = true;
                                                }
                                            }
                                        }
                                    });
                            });

                            // Preview pane (right side)
                            if self.show_preview {
                                // Vertical separator between list and preview
                                let line_rect = ui.available_rect_before_wrap();
                                ui.painter().vline(
                                    line_rect.left(),
                                    line_rect.y_range(),
                                    egui::Stroke::new(1.0, colors.separator),
                                );

                                ui.vertical(|ui| {
                                    ui.set_width(preview_width);
                                    ui.set_height(content_height);
                                    self.render_preview_pane(
                                        ui,
                                        selected_result_for_preview.as_ref(),
                                        &colors,
                                    );
                                });
                            }
                        });

                        ui.add_space(4.0);

                        // Footer with keyboard hints
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, colors.separator),
                        );
                        ui.add_space(6.0);
                        render_keyboard_hints(ui, text_color(self.theme).gamma_multiply(0.4));
                        ui.add_space(8.0);
                    });
            });

        if toggle_preview {
            self.toggle_preview();
        }

        if should_close {
            self.close();
        }

        selected_item
    }

    /// Render a single result row
    fn render_result_row(
        &self,
        ui: &mut egui::Ui,
        result: &MetricResult,
        is_selected: bool,
        colors: &FinderColors,
    ) -> bool {
        let text_col = text_color(self.theme);
        let category_color = text_col.gamma_multiply(0.5);

        let row_height = 36.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        // Background
        let bg_color = if is_selected {
            colors.selected_bg
        } else if response.hovered() {
            colors.hover_bg
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 0.0, bg_color);
        }

        // Selection indicator bar
        if is_selected {
            let indicator_rect = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
            ui.painter()
                .rect_filled(indicator_rect, 0.0, colors.highlight);
        }

        // Content layout
        let content_rect = rect.shrink2(egui::vec2(16.0, 0.0));
        let mut cursor_x = content_rect.left();

        // Icon
        let icon_galley = ui.painter().layout_no_wrap(
            result.item.icon().to_string(),
            FontId::proportional(14.0),
            text_col.gamma_multiply(0.6),
        );
        ui.painter().galley(
            egui::pos2(
                cursor_x,
                content_rect.center().y - icon_galley.size().y / 2.0,
            ),
            icon_galley.clone(),
            text_col,
        );
        cursor_x += icon_galley.size().x + 10.0;

        // Main text with highlighted matches
        let search_text = result.item.search_text();
        let text_galley = create_highlighted_text(
            ui,
            search_text,
            &result.match_positions,
            text_col,
            colors.highlight,
        );
        ui.painter().galley(
            egui::pos2(
                cursor_x,
                content_rect.center().y - text_galley.size().y / 2.0,
            ),
            text_galley.clone(),
            text_col,
        );
        cursor_x += text_galley.size().x + 12.0;

        // Category label
        let category_text = format!("[{}]", result.item.category_label());
        let category_galley =
            ui.painter()
                .layout_no_wrap(category_text, FontId::proportional(11.0), category_color);
        ui.painter().galley(
            egui::pos2(
                cursor_x,
                content_rect.center().y - category_galley.size().y / 2.0,
            ),
            category_galley,
            category_color,
        );

        // Scroll selected item into view
        if is_selected {
            response.scroll_to_me(Some(egui::Align::Center));
        }

        response.clicked()
    }

    /// Render the preview pane with metric info and tags
    fn render_preview_pane(
        &self,
        ui: &mut egui::Ui,
        selected_item: Option<&MetricResult>,
        colors: &FinderColors,
    ) {
        let text_col = text_color(self.theme);
        let tag_key_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(50, 120, 180), // blue
            AppTheme::Dark => Color32::from_rgb(97, 175, 239),  // light blue
        };
        let tag_value_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(80, 140, 80), // green
            AppTheme::Dark => Color32::from_rgb(152, 195, 121), // light green
        };

        // Fill the entire preview area
        let available_height = ui.available_height();

        egui::Frame::new()
            .fill(colors.preview_bg)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_min_height(available_height - 24.0); // Account for margins

                if let Some(result) = selected_item {
                    // Header with metric info
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(result.item.icon()).color(text_col).size(16.0));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(result.item.search_text())
                                .color(text_col)
                                .strong()
                                .size(14.0),
                        );
                    });

                    // Category and unit line
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Category: {}", result.item.category_label()))
                                .color(text_col.gamma_multiply(0.5))
                                .size(11.0),
                        );
                        if let Some(unit) = &result.item.unit {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(format!("Unit: {unit}"))
                                    .color(text_col.gamma_multiply(0.5))
                                    .size(11.0),
                            );
                        }
                    });

                    // Series count
                    if result.item.series_count > 0 {
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!("{} active series", result.item.series_count))
                                .color(text_col.gamma_multiply(0.5))
                                .size(11.0),
                        );
                    }

                    ui.add_space(8.0);

                    // Separator line
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );
                    ui.add_space(8.0);

                    // Description section (if available)
                    if let Some(desc) = &result.item.description {
                        ui.label(
                            RichText::new("Description")
                                .color(text_col.gamma_multiply(0.6))
                                .size(10.0),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(desc)
                                .color(text_col.gamma_multiply(0.8))
                                .size(12.0),
                        );
                        ui.add_space(12.0);
                    }

                    // Tags section - takes remaining space
                    ui.label(
                        RichText::new("Available Tags")
                            .color(text_col.gamma_multiply(0.6))
                            .size(10.0),
                    );
                    ui.add_space(6.0);

                    if result.item.tags.is_empty() {
                        // Show placeholder in remaining space
                        let remaining = ui.available_height();
                        ui.allocate_space(egui::vec2(0.0, remaining / 3.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("No tags available")
                                    .color(text_col.gamma_multiply(0.4))
                                    .italics()
                                    .size(12.0),
                            );
                        });
                    } else {
                        // Show tags in a scrollable area that fills remaining space
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                // Sort tag keys for consistent display
                                let mut tag_keys: Vec<_> = result.item.tags.keys().collect();
                                tag_keys.sort();

                                for (idx, key) in tag_keys.iter().enumerate() {
                                    if let Some(values) = result.item.tags.get(*key) {
                                        // Tag key
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("{key}:"))
                                                    .color(tag_key_color)
                                                    .size(12.0)
                                                    .strong(),
                                            );
                                        });

                                        // Tag values (show up to 5, with ellipsis if more)
                                        let mut sorted_values: Vec<_> = values.iter().collect();
                                        sorted_values.sort();
                                        let display_count = sorted_values.len().min(5);
                                        let has_more = sorted_values.len() > 5;

                                        // Use unique ID for each tag's indent
                                        ui.indent(egui::Id::new(("tag_values", idx)), |ui| {
                                            for value in sorted_values.iter().take(display_count) {
                                                ui.label(
                                                    RichText::new(format!("• {value}"))
                                                        .color(tag_value_color)
                                                        .size(11.0),
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
                                                    .size(10.0),
                                                );
                                            }
                                        });

                                        ui.add_space(6.0);
                                    }
                                }
                            });
                    }
                } else {
                    // No selection - show placeholder centered in the preview area
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("Select a metric to preview")
                                .color(text_col.gamma_multiply(0.4))
                                .italics(),
                        );
                    });
                }
            });
    }

    /// Toggle preview pane visibility
    pub fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
    }
}
