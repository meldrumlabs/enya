//! QueryFinder - A telescope/fzf-style finder for saved queries with vertical preview

use egui::{Color32, RichText, Stroke, TextFormat, text::LayoutJob};
use egui_plot::{Line, Plot, PlotPoints};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::typography;

use super::finder_utils::{
    FinderColors, FinderKeyboardInput, chart_color, create_highlighted_text,
    generate_demo_preview_data, render_keyboard_hints,
};
use super::time_series_chart::DataPoint;

/// A saved query item for the query finder
#[derive(Debug, Clone)]
pub struct QueryItem {
    /// Query ID
    pub id: u64,
    /// Display name
    pub name: String,
    /// The query string
    pub query: String,
    /// Tags associated with this query
    pub tags: Vec<String>,
}

/// A fuzzy match result with score and match positions
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// The matched query item
    pub item: QueryItem,
    /// Match score (higher is better)
    pub score: i64,
    /// Character positions that matched
    pub match_positions: Vec<usize>,
}

/// A telescope/fzf-style finder for saved queries with vertical preview
/// Shows query code on top and chart preview below
pub struct QueryFinder {
    /// Current search query
    search_query: String,
    /// All saved queries
    items: Vec<QueryItem>,
    /// Filtered and scored results
    results: Vec<QueryResult>,
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

impl Default for QueryFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryFinder {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
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

    /// Open the query finder
    pub fn open(&mut self) {
        self.is_open = true;
        self.search_query.clear();
        self.selected_index = 0;
        self.needs_refresh = true;
    }

    /// Close the query finder
    pub fn close(&mut self) {
        self.is_open = false;
        self.search_query.clear();
        self.selected_index = 0;
        self.last_preview_item = None;
        self.preview_data.clear();
    }

    /// Set the saved queries to search
    pub fn set_queries(&mut self, queries: Vec<QueryItem>) {
        self.items = queries;
        self.needs_refresh = true;
    }

    /// Refresh the filtered results based on the current search query
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.search_query.is_empty() {
            // Show all items when query is empty, sorted by name
            for item in &self.items {
                self.results.push(QueryResult {
                    item: item.clone(),
                    score: 0,
                    match_positions: Vec::new(),
                });
            }
            // Sort alphabetically by name
            self.results.sort_by(|a, b| a.item.name.cmp(&b.item.name));
        } else {
            // Parse search query to extract tag filters (words starting with #)
            let (tag_filters, name_query) = self.parse_search_query(&self.search_query.clone());

            // First filter by tags if any tag filters are present
            let tag_filtered_items: Vec<&QueryItem> = if tag_filters.is_empty() {
                self.items.iter().collect()
            } else {
                self.items
                    .iter()
                    .filter(|item| {
                        // Item must have ALL specified tags (case-insensitive)
                        tag_filters.iter().all(|filter| {
                            item.tags
                                .iter()
                                .any(|tag| tag.to_lowercase().contains(&filter.to_lowercase()))
                        })
                    })
                    .collect()
            };

            if name_query.is_empty() {
                // Only tag filtering, no name search
                for item in tag_filtered_items {
                    self.results.push(QueryResult {
                        item: item.clone(),
                        score: 0,
                        match_positions: Vec::new(),
                    });
                }
                // Sort alphabetically by name
                self.results.sort_by(|a, b| a.item.name.cmp(&b.item.name));
            } else {
                // Parse the name query into a pattern for fuzzy matching
                let pattern = Pattern::new(
                    &name_query,
                    CaseMatching::Ignore,
                    Normalization::Smart,
                    AtomKind::Fuzzy,
                );

                // Fuzzy match and score items
                let mut indices: Vec<u32> = Vec::new();
                let mut buf = Vec::new();
                for item in tag_filtered_items {
                    indices.clear();
                    let haystack = Utf32Str::new(&item.name, &mut buf);

                    if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices)
                    {
                        self.results.push(QueryResult {
                            item: item.clone(),
                            score: i64::from(score),
                            match_positions: indices.iter().map(|&i| i as usize).collect(),
                        });
                    }
                }
                // Sort by score descending (best matches first)
                self.results.sort_by(|a, b| b.score.cmp(&a.score));
            }
        }

        // Reset selection if it's out of bounds
        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }

        self.needs_refresh = false;
    }

    /// Parse search query to extract tag filters (words starting with #) and the remaining name query
    fn parse_search_query(&self, query: &str) -> (Vec<String>, String) {
        let mut tag_filters = Vec::new();
        let mut name_parts = Vec::new();

        for word in query.split_whitespace() {
            if let Some(tag) = word.strip_prefix('#') {
                if !tag.is_empty() {
                    tag_filters.push(tag.to_string());
                }
            } else {
                name_parts.push(word);
            }
        }

        (tag_filters, name_parts.join(" "))
    }

    /// Show the query finder modal. Returns the selected query if one was chosen.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<QueryItem> {
        if !self.is_open {
            return None;
        }

        // Refresh results if needed
        if self.needs_refresh {
            self.refresh_results();
        }

        let mut selected_item: Option<QueryItem> = None;
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
        let list_width = (screen_rect.width() * 0.30).clamp(300.0, 450.0);
        let preview_width = if self.show_preview {
            // Preview width for vertical query + chart layout
            (screen_rect.width() * 0.35).clamp(350.0, 500.0)
        } else {
            0.0
        };
        let total_width = list_width + preview_width;
        let popup_max_height = (screen_rect.height() * 0.70).min(600.0);

        // Get the currently selected result for preview
        let selected_result_for_preview = if !self.results.is_empty() {
            Some(self.results[self.selected_index].clone())
        } else {
            None
        };

        let colors = FinderColors::new(self.theme);

        egui::Area::new(egui::Id::new("query_finder_popup"))
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
                                RichText::new(semantic_icons::file::CODE)
                                    .color(text_color(self.theme).gamma_multiply(0.6))
                                    .size(typography::HEADING),
                            );
                            ui.add_space(8.0);

                            let text_edit = egui::TextEdit::singleline(&mut self.search_query)
                                .font(typography::heading())
                                .hint_text(
                                    RichText::new("Search queries... (use #tag to filter)")
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
                                                    RichText::new(if self.items.is_empty() {
                                                        "No saved queries"
                                                    } else {
                                                        "No results found"
                                                    })
                                                    .color(
                                                        text_color(self.theme).gamma_multiply(0.5),
                                                    )
                                                    .size(typography::XL),
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
        result: &QueryResult,
        is_selected: bool,
        colors: &FinderColors,
    ) -> bool {
        let text_col = text_color(self.theme);
        let tag_color = text_col.gamma_multiply(0.5);

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
            semantic_icons::file::CODE.to_string(),
            typography::proportional(typography::XL),
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
        let text_galley = create_highlighted_text(
            ui,
            &result.item.name,
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

        // Tags (if any)
        if !result.item.tags.is_empty() {
            let tags_text = result
                .item
                .tags
                .iter()
                .take(2)
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" ");
            let tags_galley = ui.painter().layout_no_wrap(
                tags_text,
                typography::proportional(typography::XS),
                tag_color,
            );
            ui.painter().galley(
                egui::pos2(
                    cursor_x,
                    content_rect.center().y - tags_galley.size().y / 2.0,
                ),
                tags_galley,
                tag_color,
            );
        }

        // Scroll selected item into view
        if is_selected {
            response.scroll_to_me(Some(egui::Align::Center));
        }

        response.clicked()
    }

    /// Create a syntax-highlighted text layout for query strings
    fn create_query_syntax_highlight(
        &self,
        ui: &egui::Ui,
        query: &str,
    ) -> std::sync::Arc<egui::Galley> {
        let mut job = LayoutJob::default();
        let font_id = typography::monospace(typography::MD);

        let text_col = text_color(self.theme);
        let keyword_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(175, 80, 175), // purple
            AppTheme::Dark => Color32::from_rgb(198, 120, 221), // light purple
        };
        let operator_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(200, 100, 50), // orange
            AppTheme::Dark => Color32::from_rgb(230, 150, 100), // light orange
        };
        let key_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(50, 120, 180), // blue
            AppTheme::Dark => Color32::from_rgb(97, 175, 239),  // light blue
        };
        let value_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(80, 140, 80), // green
            AppTheme::Dark => Color32::from_rgb(152, 195, 121), // light green
        };
        let number_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(180, 100, 50), // brown/orange
            AppTheme::Dark => Color32::from_rgb(209, 154, 102), // light orange
        };

        // Simple tokenizer for query syntax
        let keywords = ["AND", "OR", "NOT"];
        let chars: Vec<char> = query.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let remaining: String = chars[i..].iter().collect();

            // Check for keywords
            let mut matched_keyword = false;
            for keyword in &keywords {
                if remaining.to_uppercase().starts_with(keyword) {
                    // Check if it's a whole word (followed by space or end)
                    let after_idx = i + keyword.len();
                    let is_whole_word =
                        after_idx >= chars.len() || !chars[after_idx].is_alphanumeric();

                    if is_whole_word {
                        let actual_text: String = chars[i..after_idx].iter().collect();
                        job.append(
                            &actual_text,
                            0.0,
                            TextFormat {
                                font_id: font_id.clone(),
                                color: keyword_color,
                                ..Default::default()
                            },
                        );
                        i = after_idx;
                        matched_keyword = true;
                        break;
                    }
                }
            }
            if matched_keyword {
                continue;
            }

            let ch = chars[i];

            // Operators: :, =, >, <, >=, <=
            if matches!(ch, ':' | '=' | '>' | '<') {
                let mut op = ch.to_string();
                // Check for compound operators like >= or <=
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    op.push('=');
                    i += 1;
                }
                job.append(
                    &op,
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: operator_color,
                        ..Default::default()
                    },
                );
                i += 1;
                continue;
            }

            // Numbers (including percentages like 80%)
            if ch.is_ascii_digit() {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '%')
                {
                    i += 1;
                }
                let num: String = chars[start..i].iter().collect();
                job.append(
                    &num,
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: number_color,
                        ..Default::default()
                    },
                );
                continue;
            }

            // Identifiers (keys before : or values after :)
            if ch.is_alphanumeric() || ch == '_' {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-')
                {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();

                // Check if followed by : (it's a key) or preceded by : (it's a value)
                let is_key = i < chars.len() && chars[i] == ':';
                let color = if is_key { key_color } else { value_color };

                job.append(
                    &word,
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color,
                        ..Default::default()
                    },
                );
                continue;
            }

            // Whitespace and other characters
            job.append(
                &ch.to_string(),
                0.0,
                TextFormat {
                    font_id: font_id.clone(),
                    color: text_col.gamma_multiply(0.7),
                    ..Default::default()
                },
            );
            i += 1;
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
        self.preview_data = generate_demo_preview_data(item_name);
    }

    /// Render the preview pane with vertical query and chart layout
    fn render_preview_pane(
        &mut self,
        ui: &mut egui::Ui,
        selected_item: Option<&QueryResult>,
        colors: &FinderColors,
    ) {
        let text_col = text_color(self.theme);

        egui::Frame::new()
            .fill(colors.preview_bg)
            .inner_margin(12.0)
            .show(ui, |ui| {
                if let Some(result) = selected_item {
                    // Generate preview data if needed
                    self.generate_preview_data(&result.item.name);

                    // Header with item info
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(semantic_icons::file::CODE)
                                .color(text_col)
                                .size(typography::HEADING),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(&result.item.name)
                                .color(text_col)
                                .strong()
                                .size(typography::XL),
                        );
                    });

                    // Tags line
                    if !result.item.tags.is_empty() {
                        ui.add_space(2.0);
                        let tags_text = result
                            .item
                            .tags
                            .iter()
                            .map(|t| format!("#{t}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        ui.label(
                            RichText::new(tags_text)
                                .color(text_col.gamma_multiply(0.5))
                                .size(typography::SM),
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

                    // Vertical layout: Query preview (top) | Chart preview (bottom)
                    let available_width = ui.available_width();
                    let available_height = ui.available_height() - 20.0; // Leave room for footer label
                    let query_height = (available_height * 0.45).min(180.0); // Query panel takes ~45%
                    let chart_height = available_height - query_height - 12.0; // Rest for chart, minus gap

                    // Top panel: Query preview with syntax highlighting
                    egui::Frame::new()
                        .fill(colors.panel_bg)
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .stroke(Stroke::new(1.0, colors.separator))
                        .show(ui, |ui| {
                            ui.set_width(available_width - 16.0);
                            ui.set_height(query_height);

                            // Panel header
                            ui.label(
                                RichText::new("Query")
                                    .color(text_col.gamma_multiply(0.6))
                                    .size(typography::XS),
                            );
                            ui.add_space(4.0);

                            // Syntax-highlighted query display
                            egui::ScrollArea::vertical()
                                .max_height(query_height - 30.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let galley =
                                        self.create_query_syntax_highlight(ui, &result.item.query);
                                    ui.painter()
                                        .galley(ui.cursor().min, galley.clone(), text_col);
                                    ui.allocate_space(galley.size());
                                });
                        });

                    ui.add_space(8.0);

                    // Horizontal separator between panels
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );
                    ui.add_space(4.0);

                    // Bottom panel: Chart preview
                    egui::Frame::new()
                        .fill(colors.panel_bg)
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .stroke(Stroke::new(1.0, colors.separator))
                        .show(ui, |ui| {
                            ui.set_width(available_width - 16.0);
                            ui.set_height(chart_height);

                            // Panel header
                            ui.label(
                                RichText::new("Preview")
                                    .color(text_col.gamma_multiply(0.6))
                                    .size(typography::XS),
                            );
                            ui.add_space(4.0);

                            if !self.preview_data.is_empty() {
                                let line_color = chart_color(self.theme);

                                let points: PlotPoints<'_> = self
                                    .preview_data
                                    .iter()
                                    .map(|p| [p.timestamp, p.value])
                                    .collect();

                                let plot = Plot::new("query_finder_preview_plot")
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
                                    .height(chart_height - 30.0);

                                plot.show(ui, |plot_ui| {
                                    // PlanetScale-style: thin line with soft gradient fill
                                    let line = Line::new("preview", points)
                                        .color(line_color)
                                        .stroke(Stroke::new(1.5, line_color))
                                        .fill(0.0)
                                        .fill_alpha(0.15);
                                    plot_ui.line(line);
                                });
                            }
                        });

                    // Footer label
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Preview (demo data)")
                                .color(text_col.gamma_multiply(0.4))
                                .size(typography::XS)
                                .italics(),
                        );
                    });
                } else {
                    // No selection - show placeholder
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("Select a query to preview")
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
