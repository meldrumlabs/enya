use egui::{Color32, FontId, Key, RichText, Stroke, TextFormat, text::LayoutJob};
use egui_plot::{Line, Plot, PlotPoints};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

use super::time_series_chart::DataPoint;

/// An item that can be searched in the fuzzy finder
#[derive(Debug, Clone)]
pub enum FuzzyItem {
    /// A metric from the metrics tree
    Metric {
        name: String,
        category: String,
        description: Option<String>,
    },
    /// A custom query
    CustomQuery {
        id: u64,
        name: String,
        query: String,
    },
}

impl FuzzyItem {
    /// Get the primary searchable text for this item
    pub fn search_text(&self) -> &str {
        match self {
            Self::Metric { name, .. } => name,
            Self::CustomQuery { name, .. } => name,
        }
    }

    /// Get a secondary label for display
    pub fn category_label(&self) -> &str {
        match self {
            Self::Metric { category, .. } => category,
            Self::CustomQuery { .. } => "Query",
        }
    }

    /// Get the icon for this item type
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Metric { .. } => egui_phosphor::regular::CHART_LINE,
            Self::CustomQuery { .. } => egui_phosphor::regular::CODE,
        }
    }
}

/// A fuzzy match result with score and match positions
#[derive(Debug, Clone)]
pub struct FuzzyResult {
    /// The matched item
    pub item: FuzzyItem,
    /// Match score (higher is better)
    pub score: i64,
    /// Character positions that matched
    pub match_positions: Vec<usize>,
}

/// A telescope/fzf-style fuzzy finder modal with live preview
pub struct FuzzyFinder {
    /// Current search query
    query: String,
    /// All searchable items
    items: Vec<FuzzyItem>,
    /// Filtered and scored results
    results: Vec<FuzzyResult>,
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
    /// Cache of last selected item name for preview generation
    last_preview_item: Option<String>,
    /// Cached preview data points
    preview_data: Vec<DataPoint>,
}

impl Default for FuzzyFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyFinder {
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
            last_preview_item: None,
            preview_data: Vec::new(),
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

    /// Open the fuzzy finder
    pub fn open(&mut self) {
        self.is_open = true;
        self.query.clear();
        self.selected_index = 0;
        self.needs_refresh = true;
    }

    /// Close the fuzzy finder
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.selected_index = 0;
        self.last_preview_item = None;
        self.preview_data.clear();
    }

    /// Set the searchable items
    pub fn set_items(&mut self, items: Vec<FuzzyItem>) {
        self.items = items;
        self.needs_refresh = true;
    }

    /// Refresh the filtered results based on the current query
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.query.is_empty() {
            // Show all items when query is empty, sorted by name
            for item in &self.items {
                self.results.push(FuzzyResult {
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
                    self.results.push(FuzzyResult {
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

    /// Show the fuzzy finder modal. Returns the selected item if one was chosen.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<FuzzyItem> {
        if !self.is_open {
            return None;
        }

        // Refresh results if needed
        if self.needs_refresh {
            self.refresh_results();
        }

        let mut selected_item: Option<FuzzyItem> = None;
        let mut should_close = false;
        let mut toggle_preview = false;

        // Handle keyboard input first (before rendering)
        let (navigate_up, navigate_down, confirm, escape, ctrl_p) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowUp) || (i.key_pressed(Key::K) && i.modifiers.ctrl),
                i.key_pressed(Key::ArrowDown)
                    || (i.key_pressed(Key::J) && i.modifiers.ctrl)
                    || (i.key_pressed(Key::N) && i.modifiers.ctrl),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::P) && i.modifiers.ctrl,
            )
        });

        if escape {
            should_close = true;
        }

        if ctrl_p {
            toggle_preview = true;
        }

        if navigate_up && self.selected_index > 0 {
            self.selected_index -= 1;
        }

        if navigate_down && self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }

        if confirm && !self.results.is_empty() {
            selected_item = Some(self.results[self.selected_index].item.clone());
            should_close = true;
        }

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let list_width = (screen_rect.width() * 0.35).clamp(350.0, 500.0);
        let preview_width = if self.show_preview {
            (screen_rect.width() * 0.35).clamp(300.0, 450.0)
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

        egui::Area::new(egui::Id::new("fuzzy_finder_popup"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let bg_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(250, 250, 250),
                    AppTheme::Dark => Color32::from_rgb(30, 30, 35),
                };
                let border_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(200, 200, 200),
                    AppTheme::Dark => Color32::from_rgb(60, 60, 70),
                };
                let separator_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(220, 220, 220),
                    AppTheme::Dark => Color32::from_rgb(50, 50, 55),
                };

                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
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
                                    RichText::new("Search metrics and queries...")
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
                            egui::Stroke::new(1.0, separator_color),
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
                                                let clicked =
                                                    self.render_result_row(ui, result, is_selected);
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
                                    egui::Stroke::new(1.0, separator_color),
                                );

                                ui.vertical(|ui| {
                                    ui.set_width(preview_width);
                                    ui.set_height(content_height);
                                    self.render_preview_pane(
                                        ui,
                                        selected_result_for_preview.as_ref(),
                                    );
                                });
                            }
                        });

                        ui.add_space(4.0);

                        // Footer with keyboard hints
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            let hint_color = text_color(self.theme).gamma_multiply(0.4);
                            ui.label(RichText::new("↑↓").color(hint_color).size(11.0));
                            ui.label(RichText::new("navigate").color(hint_color).size(11.0));
                            ui.add_space(12.0);
                            ui.label(RichText::new("↵").color(hint_color).size(11.0));
                            ui.label(RichText::new("select").color(hint_color).size(11.0));
                            ui.add_space(12.0);
                            ui.label(RichText::new("ctrl+p").color(hint_color).size(11.0));
                            ui.label(RichText::new("preview").color(hint_color).size(11.0));
                            ui.add_space(12.0);
                            ui.label(RichText::new("esc").color(hint_color).size(11.0));
                            ui.label(RichText::new("close").color(hint_color).size(11.0));
                        });
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
        result: &FuzzyResult,
        is_selected: bool,
    ) -> bool {
        let text_col = text_color(self.theme);
        let highlight_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(200, 150, 0),
            AppTheme::Dark => Color32::from_rgb(255, 200, 50),
        };
        let selected_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(230, 240, 255),
            AppTheme::Dark => Color32::from_rgb(45, 50, 70),
        };
        let hover_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(240, 245, 250),
            AppTheme::Dark => Color32::from_rgb(40, 42, 50),
        };
        let category_color = text_col.gamma_multiply(0.5);

        let row_height = 36.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        // Background
        let bg_color = if is_selected {
            selected_bg
        } else if response.hovered() {
            hover_bg
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
                .rect_filled(indicator_rect, 0.0, highlight_color);
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
        let text_galley = self.create_highlighted_text(
            ui,
            search_text,
            &result.match_positions,
            text_col,
            highlight_color,
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

    /// Create a text galley with highlighted match positions
    fn create_highlighted_text(
        &self,
        ui: &egui::Ui,
        text: &str,
        positions: &[usize],
        normal_color: Color32,
        highlight_color: Color32,
    ) -> std::sync::Arc<egui::Galley> {
        let mut job = LayoutJob::default();
        let font_id = FontId::proportional(14.0);

        for (i, ch) in text.chars().enumerate() {
            let color = if positions.contains(&i) {
                highlight_color
            } else {
                normal_color
            };

            let format = TextFormat {
                font_id: font_id.clone(),
                color,
                ..Default::default()
            };

            job.append(&ch.to_string(), 0.0, format);
        }

        ui.fonts_mut(|f| f.layout_job(job))
    }

    /// Generate demo preview data for a given item name
    fn generate_preview_data(&mut self, item_name: &str) {
        // Only regenerate if the item changed
        if self.last_preview_item.as_deref() == Some(item_name) {
            return;
        }

        self.last_preview_item = Some(item_name.to_string());
        self.preview_data.clear();

        // Generate deterministic demo data based on the item name
        let seed: u64 = item_name.bytes().map(|b| b as u64).sum();
        let now = 1_700_000_000.0;
        let duration = 3600.0; // 1 hour
        let num_points = 60;

        for i in 0..num_points {
            let t = now + (i as f64 / num_points as f64) * duration;
            // Create a unique but deterministic wave pattern based on seed
            let phase = (seed % 10) as f64 * 0.3;
            let amplitude = 20.0 + (seed % 30) as f64;
            let base = 50.0 + amplitude * ((t / 300.0) + phase).sin();
            let noise = ((t * (17.0 + (seed % 7) as f64)).sin()) * 5.0;
            self.preview_data.push(DataPoint {
                timestamp: t,
                value: base + noise,
            });
        }
    }

    /// Render the preview chart pane
    fn render_preview_pane(&mut self, ui: &mut egui::Ui, selected_item: Option<&FuzzyResult>) {
        let text_col = text_color(self.theme);
        let bg_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(245, 247, 250),
            AppTheme::Dark => Color32::from_rgb(25, 27, 32),
        };

        egui::Frame::new()
            .fill(bg_color)
            .inner_margin(12.0)
            .show(ui, |ui| {
                if let Some(result) = selected_item {
                    // Generate preview data if needed
                    self.generate_preview_data(result.item.search_text());

                    // Header with item info
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

                    // Description or query info
                    match &result.item {
                        FuzzyItem::Metric {
                            description,
                            category,
                            ..
                        } => {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("Category: {category}"))
                                    .color(text_col.gamma_multiply(0.6))
                                    .size(11.0),
                            );
                            if let Some(desc) = description {
                                ui.label(
                                    RichText::new(desc)
                                        .color(text_col.gamma_multiply(0.5))
                                        .size(11.0)
                                        .italics(),
                                );
                            }
                        }
                        FuzzyItem::CustomQuery { query, .. } => {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("Query: {query}"))
                                    .color(text_col.gamma_multiply(0.6))
                                    .size(11.0)
                                    .monospace(),
                            );
                        }
                    }

                    ui.add_space(8.0);

                    // Separator line
                    let separator_color = match self.theme {
                        AppTheme::Light => Color32::from_rgb(220, 220, 220),
                        AppTheme::Dark => Color32::from_rgb(50, 50, 55),
                    };
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(8.0);

                    // Preview chart with proper background
                    if !self.preview_data.is_empty() {
                        let chart_color = match self.theme {
                            AppTheme::Light => Color32::from_rgb(59, 130, 246),
                            AppTheme::Dark => Color32::from_rgb(97, 175, 239),
                        };

                        // Chart background and grid colors
                        let plot_bg = match self.theme {
                            AppTheme::Light => Color32::from_rgb(252, 252, 254),
                            AppTheme::Dark => Color32::from_rgb(18, 20, 24),
                        };

                        let points: PlotPoints<'_> = self
                            .preview_data
                            .iter()
                            .map(|p| [p.timestamp, p.value])
                            .collect();

                        // Wrap plot in a frame with background
                        egui::Frame::new()
                            .fill(plot_bg)
                            .corner_radius(4.0)
                            .inner_margin(4.0)
                            .show(ui, |ui| {
                                let plot = Plot::new("fuzzy_preview_plot")
                                    .show_axes(true)
                                    .show_grid(true)
                                    .allow_zoom(false)
                                    .allow_drag(false)
                                    .allow_scroll(false)
                                    .allow_boxed_zoom(false)
                                    .allow_double_click_reset(false)
                                    .show_x(false)
                                    .show_y(false)
                                    .auto_bounds(egui::Vec2b::new(true, true))
                                    .height(ui.available_height() - 8.0);

                                plot.show(ui, |plot_ui| {
                                    let line = Line::new("preview", points)
                                        .color(chart_color)
                                        .stroke(Stroke::new(2.0, chart_color));
                                    plot_ui.line(line);
                                });
                            });

                        // Preview label
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Preview (demo data)")
                                    .color(text_col.gamma_multiply(0.4))
                                    .size(10.0)
                                    .italics(),
                            );
                        });
                    }
                } else {
                    // No selection - show placeholder
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("Select an item to preview")
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
