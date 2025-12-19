//! WorkspaceFinder - A telescope/fzf-style finder for saved workspaces

use egui::{Color32, RichText};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::typography;

use crate::components::util::finder_utils::{
    FinderColors, FinderKeyboardInput, OverlayStyle, create_highlighted_text,
};

/// A workspace item for the workspace finder
#[derive(Debug, Clone)]
pub struct WorkspaceItem {
    /// Workspace name (filename without extension)
    pub name: String,
    /// Optional description
    pub description: Option<String>,
}

/// A fuzzy match result with score and match positions
#[derive(Debug, Clone)]
pub struct WorkspaceResult {
    /// The matched workspace item
    pub item: WorkspaceItem,
    /// Match score (higher is better)
    pub score: i64,
    /// Character positions that matched
    pub match_positions: Vec<usize>,
}

/// A telescope/fzf-style finder for saved workspaces
pub struct WorkspaceFinder {
    /// Current search query
    search_query: String,
    /// All saved workspaces
    items: Vec<WorkspaceItem>,
    /// Filtered and scored results
    results: Vec<WorkspaceResult>,
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
}

impl Default for WorkspaceFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceFinder {
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

    /// Open the workspace finder
    pub fn open(&mut self) {
        self.is_open = true;
        self.search_query.clear();
        self.selected_index = 0;
        self.needs_refresh = true;
    }

    /// Close the workspace finder
    pub fn close(&mut self) {
        self.is_open = false;
        self.search_query.clear();
        self.selected_index = 0;
    }

    /// Set the workspaces to search
    pub fn set_workspaces(&mut self, workspaces: Vec<WorkspaceItem>) {
        self.items = workspaces;
        self.needs_refresh = true;
    }

    /// Refresh the filtered results based on the current search query
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.search_query.is_empty() {
            // Show all items when query is empty, sorted by name
            for item in &self.items {
                self.results.push(WorkspaceResult {
                    item: item.clone(),
                    score: 0,
                    match_positions: Vec::new(),
                });
            }
            // Sort alphabetically by name
            self.results.sort_by(|a, b| a.item.name.cmp(&b.item.name));
        } else {
            // Parse the query into a pattern for fuzzy matching
            let pattern = Pattern::new(
                &self.search_query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );

            // Fuzzy match and score items
            let mut indices: Vec<u32> = Vec::new();
            let mut buf = Vec::new();
            for item in &self.items {
                indices.clear();
                let haystack = Utf32Str::new(&item.name, &mut buf);

                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    self.results.push(WorkspaceResult {
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

    /// Show the workspace finder modal. Returns the selected workspace name if one was chosen.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<String> {
        if !self.is_open {
            return None;
        }

        // Refresh results if needed
        if self.needs_refresh {
            self.refresh_results();
        }

        let mut selected_name: Option<String> = None;
        let mut should_close = false;

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
            selected_name = Some(self.results[self.selected_index].item.name.clone());
            should_close = true;
        }

        // Calculate popup dimensions (match metrics/query finder widths)
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.70).clamp(600.0, 850.0);
        let popup_max_height = (screen_rect.height() * 0.65).min(550.0);

        let colors = FinderColors::new(self.theme);
        let overlay_style = OverlayStyle::frosted_glass(self.theme);

        egui::Area::new(egui::Id::new("workspace_finder_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);
                    ui.set_max_height(popup_max_height);

                    // Search input section
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(semantic_icons::file::FOLDER_OPEN)
                                .color(text_color(self.theme).gamma_multiply(0.6))
                                .size(typography::HEADING),
                        );
                        ui.add_space(8.0);

                        let text_edit = egui::TextEdit::singleline(&mut self.search_query)
                            .font(typography::heading())
                            .hint_text(
                                RichText::new("Search workspaces...")
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

                    // Separator below search
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );
                    ui.add_space(4.0);

                    // Results list
                    let content_height = popup_max_height - 90.0;
                    egui::ScrollArea::vertical()
                        .max_height(content_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(popup_width - 8.0);
                            if self.results.is_empty() {
                                ui.add_space(20.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new(if self.items.is_empty() {
                                            "No saved workspaces"
                                        } else {
                                            "No results found"
                                        })
                                        .color(text_color(self.theme).gamma_multiply(0.5))
                                        .size(typography::XL),
                                    );
                                });
                                ui.add_space(20.0);
                            } else {
                                for (i, result) in self.results.iter().enumerate() {
                                    let is_selected = i == self.selected_index;
                                    let clicked =
                                        self.render_result_row(ui, result, is_selected, &colors);
                                    if clicked {
                                        selected_name = Some(result.item.name.clone());
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
                        egui::Stroke::new(1.0, colors.separator),
                    );
                    ui.add_space(6.0);
                    self.render_keyboard_hints(ui, text_color(self.theme).gamma_multiply(0.4));
                    ui.add_space(8.0);
                });
            });

        if should_close {
            self.close();
        }

        selected_name
    }

    /// Render a single result row
    fn render_result_row(
        &self,
        ui: &mut egui::Ui,
        result: &WorkspaceResult,
        is_selected: bool,
        colors: &FinderColors,
    ) -> bool {
        let text_col = text_color(self.theme);

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
            semantic_icons::file::FOLDER.to_string(),
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

        // Description (if any)
        if let Some(desc) = &result.item.description {
            let desc_galley = ui.painter().layout_no_wrap(
                desc.clone(),
                typography::proportional(typography::SM),
                text_col.gamma_multiply(0.5),
            );
            ui.painter().galley(
                egui::pos2(
                    cursor_x,
                    content_rect.center().y - desc_galley.size().y / 2.0,
                ),
                desc_galley,
                text_col.gamma_multiply(0.5),
            );
        }

        // Scroll selected item into view
        if is_selected {
            response.scroll_to_me(Some(egui::Align::Center));
        }

        response.clicked()
    }

    /// Render keyboard hints in footer
    fn render_keyboard_hints(&self, ui: &mut egui::Ui, color: Color32) {
        ui.horizontal(|ui| {
            ui.add_space(12.0);

            let hint_style = |text: &str| RichText::new(text).color(color).size(typography::SM);
            let key_style = |text: &str| {
                RichText::new(text)
                    .color(color.gamma_multiply(1.2))
                    .size(typography::SM)
                    .strong()
            };

            ui.label(key_style("↑↓"));
            ui.label(hint_style("navigate"));
            ui.add_space(12.0);

            ui.label(key_style("Enter"));
            ui.label(hint_style("select"));
            ui.add_space(12.0);

            ui.label(key_style("Esc"));
            ui.label(hint_style("close"));
        });
    }
}
