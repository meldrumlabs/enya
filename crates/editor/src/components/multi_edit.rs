use egui::{Color32, FontId, Key, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

use super::finder_utils::OverlayStyle;

/// An excerpt from a query pane shown in the multi-edit overlay
#[derive(Debug, Clone)]
pub struct EditExcerpt {
    /// Unique identifier for the source pane
    pub source_id: usize,
    /// Display label for this excerpt (e.g., pane name)
    pub label: String,
    /// The query content being edited
    pub content: String,
    /// Original content (for detecting changes)
    pub original_content: String,
}

impl EditExcerpt {
    pub fn new(source_id: usize, label: String, content: String) -> Self {
        Self {
            source_id,
            label,
            original_content: content.clone(),
            content,
        }
    }

    /// Check if this excerpt has been modified
    pub fn is_modified(&self) -> bool {
        self.content != self.original_content
    }
}

/// Result of the multi-edit overlay interaction
#[derive(Debug, Clone)]
pub enum MultiEditResult {
    /// No action (modal still open)
    None,
    /// Changes were applied - contains list of (source_id, new_content) pairs
    Applied(Vec<(usize, String)>),
    /// Editor was cancelled
    Cancelled,
}

/// A modal overlay for editing multiple query excerpts simultaneously.
/// Supports find/replace across all excerpts and direct editing of individual queries.
pub struct MultiEditOverlay {
    /// Whether the overlay is currently open
    is_open: bool,
    /// The excerpts being edited
    excerpts: Vec<EditExcerpt>,
    /// Find pattern for search/replace
    find_pattern: String,
    /// Replacement text
    replace_with: String,
    /// Current theme
    theme: AppTheme,
    /// Whether the find field should request focus
    needs_focus: bool,
    /// Index of the currently focused excerpt (-1 for find field)
    focused_excerpt: i32,
}

impl Default for MultiEditOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiEditOverlay {
    pub fn new() -> Self {
        Self {
            is_open: false,
            excerpts: Vec::new(),
            find_pattern: String::new(),
            replace_with: String::new(),
            theme: AppTheme::default(),
            needs_focus: false,
            focused_excerpt: -1, // Start with find field focused
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the overlay is currently open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Open the overlay with the given excerpts
    pub fn open(&mut self, excerpts: Vec<EditExcerpt>) {
        self.is_open = true;
        self.excerpts = excerpts;
        self.find_pattern.clear();
        self.replace_with.clear();
        self.needs_focus = true;
        self.focused_excerpt = -1;
    }

    /// Close the overlay without applying changes
    pub fn close(&mut self) {
        self.is_open = false;
        self.excerpts.clear();
        self.find_pattern.clear();
        self.replace_with.clear();
        self.focused_excerpt = -1;
    }

    /// Get the number of excerpts
    pub fn excerpt_count(&self) -> usize {
        self.excerpts.len()
    }

    /// Get the number of modified excerpts
    pub fn modified_count(&self) -> usize {
        self.excerpts.iter().filter(|e| e.is_modified()).count()
    }

    /// Count matches of the find pattern across all excerpts
    fn count_matches(&self) -> usize {
        if self.find_pattern.is_empty() {
            return 0;
        }
        self.excerpts
            .iter()
            .map(|e| e.content.matches(&self.find_pattern).count())
            .sum()
    }

    /// Apply find/replace to all excerpts
    fn apply_replace_all(&mut self) {
        if self.find_pattern.is_empty() {
            return;
        }
        for excerpt in &mut self.excerpts {
            excerpt.content = excerpt
                .content
                .replace(&self.find_pattern, &self.replace_with);
        }
    }

    /// Collect all changes for applying back to source panes
    fn collect_changes(&self) -> Vec<(usize, String)> {
        self.excerpts
            .iter()
            .filter(|e| e.is_modified())
            .map(|e| (e.source_id, e.content.clone()))
            .collect()
    }

    /// Show the multi-edit overlay. Returns the result of the interaction.
    pub fn show(&mut self, ctx: &egui::Context) -> MultiEditResult {
        if !self.is_open {
            return MultiEditResult::None;
        }

        let mut result = MultiEditResult::None;
        let mut should_close = false;
        let mut should_apply = false;
        let mut should_replace_all = false;

        // Handle keyboard shortcuts
        ctx.input(|input| {
            // Escape - cancel and close
            if input.key_pressed(Key::Escape) {
                should_close = true;
            }
            // Ctrl+Enter or Cmd+Enter - apply and close
            if input.key_pressed(Key::Enter) && input.modifiers.command {
                should_apply = true;
            }
            // Ctrl+Shift+R - replace all
            if input.key_pressed(Key::R) && input.modifiers.command && input.modifiers.shift {
                should_replace_all = true;
            }
            // Tab - cycle through excerpts
            if input.key_pressed(Key::Tab) && !input.modifiers.shift {
                self.focused_excerpt =
                    (self.focused_excerpt + 1) % (self.excerpts.len() as i32 + 1) - 1;
            }
            // Shift+Tab - cycle backwards
            if input.key_pressed(Key::Tab) && input.modifiers.shift {
                let total = self.excerpts.len() as i32 + 1;
                self.focused_excerpt = (self.focused_excerpt - 1 + total) % total - 1;
            }
        });

        if should_replace_all {
            self.apply_replace_all();
        }

        // Semi-transparent backdrop (matching BufferEditor)
        #[allow(deprecated)]
        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new("multi_edit_backdrop"))
            .fixed_pos(screen_rect.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let backdrop_color = match self.theme {
                    AppTheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 120),
                    AppTheme::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                };
                ui.painter().rect_filled(screen_rect, 0.0, backdrop_color);
            });

        // Main modal panel - wider to accommodate query excerpts
        let popup_width = (screen_rect.width() * 0.8).clamp(600.0, 1100.0);
        let max_excerpts_height = (screen_rect.height() * 0.5).min(500.0);

        egui::Area::new(egui::Id::new("multi_edit_panel"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                // Muted accent for badges/buttons
                let accent_color = match self.theme {
                    AppTheme::Light => palette::accent::LIGHT,
                    AppTheme::Dark => Color32::from_rgb(13, 148, 103),
                };

                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header with mode indicator
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);

                        // MULTI-EDIT mode badge
                        egui::Frame::new()
                            .fill(accent_color)
                            .corner_radius(3.0)
                            .inner_margin(egui::vec2(8.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("MULTI-EDIT")
                                        .color(Color32::WHITE)
                                        .size(11.0)
                                        .strong(),
                                );
                            });

                        ui.add_space(12.0);

                        // Pane count
                        ui.label(
                            RichText::new(format!("{} panes", self.excerpts.len()))
                                .color(text_color(self.theme))
                                .size(14.0),
                        );

                        // Modified indicator
                        let modified = self.modified_count();
                        if modified > 0 {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!("[+{modified}]"))
                                    .color(palette::semantic::WARNING)
                                    .size(12.0),
                            );
                        }

                        // Spacer and close hint
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("Esc to cancel")
                                    .color(text_color(self.theme).gamma_multiply(0.4))
                                    .size(11.0),
                            );
                        });
                    });

                    ui.add_space(12.0);

                    // Separator
                    let separator_color = match self.theme {
                        AppTheme::Light => palette::light_border::SUBTLE,
                        AppTheme::Dark => palette::border::SUBTLE,
                    };
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );

                    ui.add_space(8.0);

                    // Find/Replace section label
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::action::SEARCH)
                                .color(text_color(self.theme).gamma_multiply(0.6))
                                .size(14.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Find & Replace")
                                .color(text_color(self.theme).gamma_multiply(0.7))
                                .size(12.0),
                        );

                        // Match count indicator
                        if !self.find_pattern.is_empty() {
                            let match_count = self.count_matches();
                            ui.add_space(8.0);
                            let (icon, color) = if match_count == 0 {
                                (semantic_icons::status::WARNING, palette::semantic::WARNING)
                            } else {
                                (semantic_icons::status::SUCCESS, palette::semantic::SUCCESS)
                            };
                            ui.label(
                                RichText::new(format!("{icon} {match_count} matches"))
                                    .color(color)
                                    .size(11.0),
                            );
                        }
                    });

                    ui.add_space(8.0);

                    // Find/Replace inputs with styled background
                    let editor_bg = match self.theme {
                        AppTheme::Light => palette::light_bg::ELEVATED,
                        AppTheme::Dark => palette::bg::ELEVATED,
                    };

                    ui.horizontal(|ui| {
                        ui.add_space(16.0);

                        // Find field with background frame
                        egui::Frame::new()
                            .fill(editor_bg)
                            .corner_radius(4.0)
                            .inner_margin(egui::vec2(8.0, 6.0))
                            .show(ui, |ui| {
                                let find_id = egui::Id::new("multi_edit_find");
                                let find_response = ui.add(
                                    egui::TextEdit::singleline(&mut self.find_pattern)
                                        .id(find_id)
                                        .desired_width(180.0)
                                        .font(FontId::monospace(13.0))
                                        .frame(false)
                                        .hint_text(
                                            RichText::new("Find pattern...")
                                                .color(text_color(self.theme).gamma_multiply(0.4)),
                                        ),
                                );

                                // Auto-focus the find field when opening
                                if self.needs_focus && self.focused_excerpt == -1 {
                                    find_response.request_focus();
                                    self.needs_focus = false;
                                }
                            });

                        ui.add_space(8.0);

                        // Arrow indicator
                        ui.label(
                            RichText::new("→")
                                .color(text_color(self.theme).gamma_multiply(0.4))
                                .size(14.0),
                        );

                        ui.add_space(8.0);

                        // Replace field with background frame
                        egui::Frame::new()
                            .fill(editor_bg)
                            .corner_radius(4.0)
                            .inner_margin(egui::vec2(8.0, 6.0))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.replace_with)
                                        .desired_width(180.0)
                                        .font(FontId::monospace(13.0))
                                        .frame(false)
                                        .hint_text(
                                            RichText::new("Replace with...")
                                                .color(text_color(self.theme).gamma_multiply(0.4)),
                                        ),
                                );
                            });

                        ui.add_space(12.0);

                        // Replace All button
                        let match_count = self.count_matches();
                        let button_enabled = !self.find_pattern.is_empty() && match_count > 0;

                        let replace_btn = egui::Button::new(
                            RichText::new(format!("{} Replace All", semantic_icons::mode::REPLACE))
                                .size(12.0),
                        )
                        .fill(if button_enabled {
                            accent_color
                        } else {
                            editor_bg
                        });

                        if ui.add_enabled(button_enabled, replace_btn).clicked() {
                            self.apply_replace_all();
                        }

                        ui.add_space(16.0);
                    });

                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );

                    ui.add_space(8.0);

                    // Excerpts section label
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::file::CODE)
                                .color(text_color(self.theme).gamma_multiply(0.6))
                                .size(14.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Queries")
                                .color(text_color(self.theme).gamma_multiply(0.7))
                                .size(12.0),
                        );
                    });

                    ui.add_space(8.0);

                    // Excerpts area (scrollable) - vertical layout with padding
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(16, 0))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(max_excerpts_height)
                                .show(ui, |ui| {
                                    ui.set_width(popup_width - 32.0);
                                    self.show_excerpts(ui);
                                });
                        });

                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );

                    ui.add_space(8.0);

                    // Footer with keyboard hints and buttons
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);

                        let hint_color = text_color(self.theme).gamma_multiply(0.4);

                        // Keyboard hints
                        ui.label(RichText::new("⌘↵").color(hint_color).size(11.0));
                        ui.label(RichText::new("apply").color(hint_color).size(11.0));
                        ui.add_space(16.0);
                        ui.label(RichText::new("⌘⇧R").color(hint_color).size(11.0));
                        ui.label(RichText::new("replace all").color(hint_color).size(11.0));
                        ui.add_space(16.0);
                        ui.label(RichText::new("Tab").color(hint_color).size(11.0));
                        ui.label(RichText::new("next").color(hint_color).size(11.0));

                        // Right side - Apply and Cancel buttons
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);

                            let apply_btn = egui::Button::new(
                                RichText::new(format!("{} Apply", semantic_icons::action::SAVE))
                                    .size(12.0),
                            )
                            .fill(accent_color);

                            if ui.add(apply_btn).clicked() {
                                should_apply = true;
                            }

                            // Cancel button
                            let cancel_btn = egui::Button::new(
                                RichText::new("Cancel")
                                    .size(12.0)
                                    .color(text_color(self.theme)),
                            );

                            if ui.add(cancel_btn).clicked() {
                                should_close = true;
                            }
                        });
                    });

                    ui.add_space(12.0);
                });
            });

        // Handle close/apply actions
        if should_close {
            self.close();
            result = MultiEditResult::Cancelled;
        } else if should_apply {
            let changes = self.collect_changes();
            self.close();
            result = MultiEditResult::Applied(changes);
        }

        result
    }

    /// Render the excerpts section
    fn show_excerpts(&mut self, ui: &mut egui::Ui) {
        let excerpt_bg = match self.theme {
            AppTheme::Light => palette::light_bg::ELEVATED,
            AppTheme::Dark => palette::bg::ELEVATED,
        };
        let excerpt_border = match self.theme {
            AppTheme::Light => palette::light_border::SUBTLE,
            AppTheme::Dark => palette::border::SUBTLE,
        };
        let label_color = text_color(self.theme).gamma_multiply(0.6);
        let highlight_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(187, 247, 208), // Light green
            AppTheme::Dark => Color32::from_rgb(6, 78, 59),      // Dark green
        };
        let text_col = text_color(self.theme);
        let accent_color = match self.theme {
            AppTheme::Light => palette::accent::LIGHT,
            AppTheme::Dark => palette::accent::HOVER,
        };

        // Clone find_pattern to avoid borrow issues
        let find_pattern = self.find_pattern.clone();

        for (idx, excerpt) in self.excerpts.iter_mut().enumerate() {
            let is_focused = self.focused_excerpt == idx as i32;
            let is_modified = excerpt.is_modified();

            // Excerpt container with focus ring
            let frame_stroke = if is_focused {
                egui::Stroke::new(2.0, accent_color)
            } else {
                egui::Stroke::new(1.0, excerpt_border)
            };

            egui::Frame::new()
                .fill(excerpt_bg)
                .stroke(frame_stroke)
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    // Label header with match count
                    ui.horizontal(|ui| {
                        // Modified indicator
                        if is_modified {
                            ui.label(
                                RichText::new("●")
                                    .color(palette::semantic::WARNING)
                                    .size(10.0),
                            );
                        }

                        ui.label(
                            RichText::new(&excerpt.label)
                                .color(label_color)
                                .size(12.0)
                                .strong(),
                        );

                        // Show match count for this excerpt
                        if !find_pattern.is_empty() {
                            let match_count = excerpt.content.matches(&find_pattern).count();
                            if match_count > 0 {
                                ui.label(
                                    RichText::new(format!("({match_count} matches)"))
                                        .color(accent_color)
                                        .size(11.0),
                                );
                            }
                        }
                    });

                    ui.add_space(4.0);

                    // Render content with highlighted matches
                    if !find_pattern.is_empty() && excerpt.content.contains(&find_pattern) {
                        // Use LayoutJob to highlight matches
                        let font_id = FontId::monospace(13.0);
                        let mut job = egui::text::LayoutJob::default();

                        let content = &excerpt.content;
                        let mut last_end = 0;

                        // Find all matches and build highlighted text
                        for (match_start, _) in content.match_indices(&find_pattern) {
                            // Add text before match
                            if match_start > last_end {
                                job.append(
                                    &content[last_end..match_start],
                                    0.0,
                                    egui::TextFormat {
                                        font_id: font_id.clone(),
                                        color: text_col,
                                        ..Default::default()
                                    },
                                );
                            }

                            // Add highlighted match
                            job.append(
                                &find_pattern,
                                0.0,
                                egui::TextFormat {
                                    font_id: font_id.clone(),
                                    color: text_col,
                                    background: highlight_bg,
                                    ..Default::default()
                                },
                            );

                            last_end = match_start + find_pattern.len();
                        }

                        // Add remaining text after last match
                        if last_end < content.len() {
                            job.append(
                                &content[last_end..],
                                0.0,
                                egui::TextFormat {
                                    font_id: font_id.clone(),
                                    color: text_col,
                                    ..Default::default()
                                },
                            );
                        }

                        // Display highlighted text (read-only when highlighting)
                        let galley = ui.fonts_mut(|f| f.layout_job(job));
                        let (response, painter) = ui.allocate_painter(
                            egui::vec2(ui.available_width(), galley.rect.height().max(40.0)),
                            egui::Sense::click(),
                        );

                        // Draw background for the text area
                        let text_bg = match self.theme {
                            AppTheme::Light => palette::light_bg::SURFACE,
                            AppTheme::Dark => palette::bg::SURFACE,
                        };
                        painter.rect_filled(response.rect, 2.0, text_bg);

                        // Draw the highlighted text
                        painter.galley(response.rect.min + egui::vec2(4.0, 4.0), galley, text_col);

                        // If clicked, switch to edit mode (clear find pattern)
                        if response.clicked() {
                            // User can clear find to edit directly
                        }
                    } else {
                        // No matches or no search - show editable TextEdit
                        let text_edit_id = egui::Id::new(format!("excerpt_{}", excerpt.source_id));
                        ui.add(
                            egui::TextEdit::multiline(&mut excerpt.content)
                                .id(text_edit_id)
                                .font(FontId::monospace(13.0))
                                .desired_width(ui.available_width())
                                .desired_rows(2),
                        );
                    }
                });

            ui.add_space(8.0);
        }
    }
}
