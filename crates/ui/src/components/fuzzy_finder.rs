use egui::{Color32, FontId, Key, RichText, TextFormat, text::LayoutJob};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

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

/// A telescope/fzf-style fuzzy finder modal
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
    matcher: SkimMatcherV2,
    /// Whether query changed and results need refresh
    needs_refresh: bool,
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
            matcher: SkimMatcherV2::default(),
            needs_refresh: true,
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
            // Fuzzy match and score items
            for item in &self.items {
                if let Some((score, indices)) =
                    self.matcher.fuzzy_indices(item.search_text(), &self.query)
                {
                    self.results.push(FuzzyResult {
                        item: item.clone(),
                        score,
                        match_positions: indices,
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

        // Handle keyboard input first (before rendering)
        let (navigate_up, navigate_down, confirm, escape) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowUp)
                    || (i.key_pressed(Key::K) && i.modifiers.ctrl)
                    || (i.key_pressed(Key::P) && i.modifiers.ctrl),
                i.key_pressed(Key::ArrowDown)
                    || (i.key_pressed(Key::J) && i.modifiers.ctrl)
                    || (i.key_pressed(Key::N) && i.modifiers.ctrl),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
            )
        });

        if escape {
            should_close = true;
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

        // Main finder popup (no backdrop dimming for terminal-like experience)
        let screen_rect = ctx.screen_rect();
        let popup_width = (screen_rect.width() * 0.6).clamp(400.0, 700.0);
        let popup_max_height = (screen_rect.height() * 0.6).min(500.0);

        egui::Area::new(egui::Id::new("fuzzy_finder_popup"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
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

                egui::Frame::none()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .rounding(8.0)
                    .inner_margin(0.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0.0, 4.0].into(),
                        blur: 16.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Search input section
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
                                .desired_width(popup_width - 60.0);

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

                        // Separator
                        let separator_color = match self.theme {
                            AppTheme::Light => Color32::from_rgb(220, 220, 220),
                            AppTheme::Dark => Color32::from_rgb(50, 50, 55),
                        };
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(4.0);

                        // Results section
                        let results_height = popup_max_height - 60.0;
                        egui::ScrollArea::vertical()
                            .max_height(results_height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if self.results.is_empty() {
                                    ui.add_space(20.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            RichText::new("No results found")
                                                .color(text_color(self.theme).gamma_multiply(0.5))
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
                            ui.label(RichText::new("esc").color(hint_color).size(11.0));
                            ui.label(RichText::new("close").color(hint_color).size(11.0));
                        });
                        ui.add_space(8.0);
                    });
            });

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

        ui.fonts(|f| f.layout_job(job))
    }
}
