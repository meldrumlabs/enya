use egui::{Color32, FontId, Key, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

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

        // Semi-transparent backdrop
        #[allow(deprecated)]
        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new("multi_edit_backdrop"))
            .fixed_pos(screen_rect.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let backdrop_color = match self.theme {
                    AppTheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 180),
                    AppTheme::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, 200),
                };
                ui.painter().rect_filled(screen_rect, 0.0, backdrop_color);
            });

        // Main modal panel - dynamic height based on content
        let panel_width = (screen_rect.width() * 0.7).clamp(500.0, 900.0);
        // Calculate max height for excerpts scroll area
        let max_excerpts_height = (screen_rect.height() * 0.5).min(400.0);

        egui::Area::new(egui::Id::new("multi_edit_panel"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
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

                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .corner_radius(8.0)
                    .inner_margin(16.0)
                    .show(ui, |ui| {
                        ui.set_width(panel_width - 32.0);

                        // Header
                        self.show_header(ui);

                        ui.add_space(12.0);

                        // Excerpts area (scrollable, with max height)
                        egui::ScrollArea::vertical()
                            .max_height(max_excerpts_height)
                            .show(ui, |ui| {
                                self.show_excerpts(ui);
                            });

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Find/Replace bar
                        self.show_find_replace_bar(ui);

                        ui.add_space(12.0);

                        // Footer with shortcuts
                        self.show_footer(ui);
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

    /// Render the header section
    fn show_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Title with purple accent to match V-MULTI mode
            let title_color = match self.theme {
                AppTheme::Light => Color32::from_rgb(180, 100, 180),
                AppTheme::Dark => Color32::from_rgb(220, 140, 220),
            };
            ui.label(
                RichText::new("MULTI-EDIT")
                    .color(title_color)
                    .size(16.0)
                    .strong(),
            );

            ui.add_space(8.0);

            // Excerpt count
            let count_text = format!("{} panes", self.excerpts.len());
            ui.label(
                RichText::new(count_text)
                    .color(text_color(self.theme))
                    .size(14.0),
            );

            // Modified indicator
            let modified = self.modified_count();
            if modified > 0 {
                ui.add_space(8.0);
                let mod_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(200, 120, 50),
                    AppTheme::Dark => Color32::from_rgb(255, 180, 100),
                };
                ui.label(
                    RichText::new(format!("({modified} modified)"))
                        .color(mod_color)
                        .size(12.0),
                );
            }
        });
    }

    /// Render the excerpts section
    fn show_excerpts(&mut self, ui: &mut egui::Ui) {
        let excerpt_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(245, 245, 248),
            AppTheme::Dark => Color32::from_rgb(40, 40, 48),
        };
        let excerpt_border = match self.theme {
            AppTheme::Light => Color32::from_rgb(220, 220, 225),
            AppTheme::Dark => Color32::from_rgb(55, 55, 65),
        };
        let label_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(100, 100, 110),
            AppTheme::Dark => Color32::from_rgb(160, 160, 170),
        };
        let highlight_color = match self.theme {
            AppTheme::Light => Color32::from_rgba_unmultiplied(180, 100, 180, 30),
            AppTheme::Dark => Color32::from_rgba_unmultiplied(220, 140, 220, 20),
        };

        for (idx, excerpt) in self.excerpts.iter_mut().enumerate() {
            let is_focused = self.focused_excerpt == idx as i32;
            let is_modified = excerpt.is_modified();

            // Excerpt container
            let frame_stroke = if is_focused {
                egui::Stroke::new(
                    2.0,
                    match self.theme {
                        AppTheme::Light => Color32::from_rgb(180, 100, 180),
                        AppTheme::Dark => Color32::from_rgb(220, 140, 220),
                    },
                )
            } else {
                egui::Stroke::new(1.0, excerpt_border)
            };

            egui::Frame::new()
                .fill(excerpt_bg)
                .stroke(frame_stroke)
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    // Label header
                    ui.horizontal(|ui| {
                        // Modified indicator
                        if is_modified {
                            let mod_color = match self.theme {
                                AppTheme::Light => Color32::from_rgb(200, 120, 50),
                                AppTheme::Dark => Color32::from_rgb(255, 180, 100),
                            };
                            ui.label(RichText::new("●").color(mod_color).size(10.0));
                        }

                        ui.label(
                            RichText::new(&excerpt.label)
                                .color(label_color)
                                .size(12.0)
                                .strong(),
                        );
                    });

                    ui.add_space(4.0);

                    // Query text edit
                    let text_edit_id = egui::Id::new(format!("excerpt_{}", excerpt.source_id));
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut excerpt.content)
                            .id(text_edit_id)
                            .font(FontId::monospace(13.0))
                            .desired_width(ui.available_width())
                            .desired_rows(2),
                    );

                    // Highlight matches in the text
                    if !self.find_pattern.is_empty() && excerpt.content.contains(&self.find_pattern)
                    {
                        // Draw highlight overlay on the response rect
                        let painter = ui.painter();
                        // Simple highlight: tint the whole area if it contains a match
                        // (A more sophisticated version would highlight just the matching text)
                        painter.rect_filled(response.rect, 2.0, highlight_color);
                    }
                });

            ui.add_space(8.0);
        }
    }

    /// Render the find/replace bar
    fn show_find_replace_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Find field
            ui.label(RichText::new("Find:").color(text_color(self.theme)));
            let find_id = egui::Id::new("multi_edit_find");
            let find_response = ui.add(
                egui::TextEdit::singleline(&mut self.find_pattern)
                    .id(find_id)
                    .desired_width(200.0)
                    .font(FontId::monospace(13.0)),
            );

            // Auto-focus the find field when opening
            if self.needs_focus && self.focused_excerpt == -1 {
                find_response.request_focus();
                self.needs_focus = false;
            }

            ui.add_space(16.0);

            // Replace field
            ui.label(RichText::new("Replace:").color(text_color(self.theme)));
            ui.add(
                egui::TextEdit::singleline(&mut self.replace_with)
                    .desired_width(200.0)
                    .font(FontId::monospace(13.0)),
            );

            ui.add_space(16.0);

            // Replace All button
            let match_count = self.count_matches();
            let button_text = if match_count > 0 {
                format!("Replace All ({match_count})")
            } else {
                "Replace All".to_string()
            };

            let button_enabled = !self.find_pattern.is_empty() && match_count > 0;
            if ui
                .add_enabled(button_enabled, egui::Button::new(button_text))
                .clicked()
            {
                self.apply_replace_all();
            }
        });

        // Match count indicator
        if !self.find_pattern.is_empty() {
            let match_count = self.count_matches();
            let match_text = if match_count == 0 {
                "No matches".to_string()
            } else if match_count == 1 {
                "1 match".to_string()
            } else {
                format!("{match_count} matches")
            };

            let match_color = if match_count == 0 {
                match self.theme {
                    AppTheme::Light => Color32::from_rgb(180, 80, 80),
                    AppTheme::Dark => Color32::from_rgb(220, 120, 120),
                }
            } else {
                match self.theme {
                    AppTheme::Light => Color32::from_rgb(80, 150, 80),
                    AppTheme::Dark => Color32::from_rgb(120, 200, 120),
                }
            };

            ui.add_space(4.0);
            ui.label(RichText::new(match_text).color(match_color).size(12.0));
        }
    }

    /// Render the footer with keyboard shortcuts
    fn show_footer(&self, ui: &mut egui::Ui) {
        let hint_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(130, 130, 140),
            AppTheme::Dark => Color32::from_rgb(120, 120, 130),
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new("Esc").color(hint_color).size(11.0).strong());
            ui.label(RichText::new("Close").color(hint_color).size(11.0));

            ui.add_space(16.0);

            ui.label(RichText::new("Tab").color(hint_color).size(11.0).strong());
            ui.label(RichText::new("Next excerpt").color(hint_color).size(11.0));

            ui.add_space(16.0);

            ui.label(RichText::new("⌘⇧R").color(hint_color).size(11.0).strong());
            ui.label(RichText::new("Replace all").color(hint_color).size(11.0));

            ui.add_space(16.0);

            ui.label(RichText::new("⌘↵").color(hint_color).size(11.0).strong());
            ui.label(RichText::new("Apply & close").color(hint_color).size(11.0));
        });
    }
}
