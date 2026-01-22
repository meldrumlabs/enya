use egui::{Color32, FontId, Key, RichText};

use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop, render_key_badge};

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
    #[profiling::function]
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
        draw_backdrop(ctx, self.theme, "multi_edit");

        // Main modal panel - wider to accommodate query excerpts
        #[allow(deprecated)]
        let screen_rect = ctx.screen_rect();
        let popup_width = (screen_rect.width() * 0.8).clamp(600.0, 1100.0);
        let max_excerpts_height = (screen_rect.height() * 0.5).min(500.0);

        egui::Area::new(egui::Id::new("multi_edit_panel"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                // Muted accent for badges/buttons
                let accent_color = self.theme.accent_primary();

                let frame_response = overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header with icon and pane count (premium styling)
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        // Multi-edit icon with accent tint
                        ui.label(
                            RichText::new(semantic_icons::mode::REPLACE)
                                .color(accent_color)
                                .size(typography::LG),
                        );

                        ui.add_space(10.0);

                        // Title with pane count
                        ui.label(
                            RichText::new(format!("Edit {} Panes", self.excerpts.len()))
                                .color(text_color(self.theme))
                                .size(typography::XL),
                        );

                        // Modified indicator
                        let modified = self.modified_count();
                        if modified > 0 {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!("[+{modified}]"))
                                    .color(self.theme.semantic_warning())
                                    .size(typography::MD),
                            );
                        }

                        // Spacer and close hint
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("Esc to cancel")
                                    .color(text_color(self.theme).gamma_multiply(0.4))
                                    .size(typography::SM),
                            );
                        });
                    });

                    ui.add_space(12.0);

                    // Separator
                    let separator_color = self.theme.border_subtle();
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );

                    ui.add_space(8.0);

                    // Find/Replace section label
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new(semantic_icons::action::SEARCH)
                                .color(text_color(self.theme).gamma_multiply(0.6))
                                .size(typography::XL),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Find & Replace")
                                .color(text_color(self.theme).gamma_multiply(0.7))
                                .size(typography::MD),
                        );

                        // Match count indicator
                        if !self.find_pattern.is_empty() {
                            let match_count = self.count_matches();
                            ui.add_space(8.0);
                            let (icon, color) = if match_count == 0 {
                                (
                                    semantic_icons::status::WARNING,
                                    self.theme.semantic_warning(),
                                )
                            } else {
                                (
                                    semantic_icons::status::SUCCESS,
                                    self.theme.semantic_success(),
                                )
                            };
                            ui.label(
                                RichText::new(format!("{icon} {match_count} matches"))
                                    .color(color)
                                    .size(typography::SM),
                            );
                        }
                    });

                    ui.add_space(8.0);

                    // Find/Replace inputs with premium styled background
                    let editor_bg = self.theme.bg_inset();
                    let editor_border = self.theme.border_subtle();

                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        // Find field with premium frame
                        let find_frame_response = egui::Frame::new()
                            .fill(editor_bg)
                            .corner_radius(6.0)
                            .inner_margin(egui::vec2(10.0, 8.0))
                            .stroke(egui::Stroke::new(1.0, editor_border))
                            .show(ui, |ui| {
                                let find_id = egui::Id::new("multi_edit_find");
                                let find_response = ui.add(
                                    egui::TextEdit::singleline(&mut self.find_pattern)
                                        .id(find_id)
                                        .desired_width(200.0)
                                        .font(typography::code_lg())
                                        .frame(false)
                                        .hint_text(
                                            RichText::new("Find pattern...")
                                                .font(typography::code_lg())
                                                .color(text_color(self.theme).gamma_multiply(0.35)),
                                        ),
                                );

                                // Auto-focus the find field when opening
                                if self.needs_focus && self.focused_excerpt == -1 {
                                    find_response.request_focus();
                                    self.needs_focus = false;
                                }
                            });

                        // Draw inner shadow for depth
                        let find_rect = find_frame_response.response.rect;
                        let inset = egui::Rect::from_min_size(
                            find_rect.left_top() + egui::vec2(1.0, 1.0),
                            egui::vec2(find_rect.width() - 2.0, 2.0),
                        );
                        ui.painter().rect_filled(
                            inset,
                            4.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, 10),
                        );

                        ui.add_space(12.0);

                        // Arrow indicator with accent color
                        ui.label(
                            RichText::new("→")
                                .color(accent_color.gamma_multiply(0.6))
                                .size(typography::LG),
                        );

                        ui.add_space(12.0);

                        // Replace field with premium frame
                        let replace_frame_response = egui::Frame::new()
                            .fill(editor_bg)
                            .corner_radius(6.0)
                            .inner_margin(egui::vec2(10.0, 8.0))
                            .stroke(egui::Stroke::new(1.0, editor_border))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.replace_with)
                                        .desired_width(200.0)
                                        .font(typography::code_lg())
                                        .frame(false)
                                        .hint_text(
                                            RichText::new("Replace with...")
                                                .font(typography::code_lg())
                                                .color(text_color(self.theme).gamma_multiply(0.35)),
                                        ),
                                );
                            });

                        // Draw inner shadow for depth
                        let replace_rect = replace_frame_response.response.rect;
                        let inset2 = egui::Rect::from_min_size(
                            replace_rect.left_top() + egui::vec2(1.0, 1.0),
                            egui::vec2(replace_rect.width() - 2.0, 2.0),
                        );
                        ui.painter().rect_filled(
                            inset2,
                            4.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, 10),
                        );

                        ui.add_space(16.0);

                        // Replace All button with premium styling
                        let match_count = self.count_matches();
                        let button_enabled = !self.find_pattern.is_empty() && match_count > 0;

                        let replace_btn = egui::Button::new(
                            RichText::new(format!("{} Replace All", semantic_icons::mode::REPLACE))
                                .size(typography::MD)
                                .color(if button_enabled {
                                    Color32::WHITE
                                } else {
                                    text_color(self.theme).gamma_multiply(0.5)
                                })
                                .strong(),
                        )
                        .fill(if button_enabled {
                            accent_color
                        } else {
                            editor_bg
                        })
                        .corner_radius(6.0)
                        .min_size(egui::vec2(0.0, 32.0));

                        let replace_response = ui.add_enabled(button_enabled, replace_btn);

                        // Draw glow on hover when enabled
                        if button_enabled && replace_response.hovered() {
                            let glow_rect = replace_response.rect.expand(3.0);
                            ui.painter().rect_filled(
                                glow_rect,
                                8.0,
                                accent_color.gamma_multiply(0.25),
                            );
                            ui.painter().rect_filled(
                                replace_response.rect,
                                6.0,
                                self.theme.accent_hover(),
                            );
                        }

                        if replace_response.clicked() {
                            self.apply_replace_all();
                        }

                        ui.add_space(20.0);
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

                    // Footer with premium keyboard hints and buttons
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        let hint_color = text_color(self.theme).gamma_multiply(0.35);

                        // Premium keyboard hints with key badges
                        let key_bg = self.theme.bg_elevated();
                        render_key_badge(ui, "⌘↵", key_bg, hint_color);
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("apply")
                                .color(hint_color)
                                .size(typography::SM),
                        );
                        ui.add_space(16.0);
                        render_key_badge(ui, "⌘⇧R", key_bg, hint_color);
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("replace all")
                                .color(hint_color)
                                .size(typography::SM),
                        );
                        ui.add_space(16.0);
                        render_key_badge(ui, "Tab", key_bg, hint_color);
                        ui.add_space(4.0);
                        ui.label(RichText::new("next").color(hint_color).size(typography::SM));

                        // Right side - Apply and Cancel buttons with premium styling
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(20.0);

                            // Premium Apply button with hover glow
                            let apply_btn = egui::Button::new(
                                RichText::new(format!("{} Apply", semantic_icons::action::SAVE))
                                    .size(typography::MD)
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(accent_color)
                            .corner_radius(6.0)
                            .min_size(egui::vec2(80.0, 32.0));

                            let apply_response = ui.add(apply_btn);

                            // Draw glow behind apply button on hover
                            if apply_response.hovered() {
                                let glow_rect = apply_response.rect.expand(3.0);
                                ui.painter().rect_filled(
                                    glow_rect,
                                    8.0,
                                    accent_color.gamma_multiply(0.25),
                                );
                                ui.painter().rect_filled(
                                    apply_response.rect,
                                    6.0,
                                    self.theme.accent_hover(),
                                );
                            }

                            if apply_response.clicked() {
                                should_apply = true;
                            }

                            ui.add_space(10.0);

                            // Cancel button with refined ghost styling
                            let cancel_bg = self.theme.bg_elevated();
                            let cancel_border = self.theme.border_subtle();
                            let cancel_btn = egui::Button::new(
                                RichText::new("Cancel")
                                    .size(typography::MD)
                                    .color(text_color(self.theme).gamma_multiply(0.7)),
                            )
                            .fill(cancel_bg)
                            .stroke(egui::Stroke::new(1.0, cancel_border))
                            .corner_radius(6.0)
                            .min_size(egui::vec2(72.0, 32.0));

                            if ui.add(cancel_btn).clicked() {
                                should_close = true;
                            }
                        });
                    });

                    ui.add_space(14.0);
                });

                // Draw inner highlight on the frame for glass effect
                overlay_style.draw_inner_highlight(ui, frame_response.response.rect);
            });

        // Handle close/apply actions
        if should_close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
            result = MultiEditResult::Cancelled;
        } else if should_apply {
            let changes = self.collect_changes();
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
            result = MultiEditResult::Applied(changes);
        }

        result
    }

    /// Render the excerpts section
    fn show_excerpts(&mut self, ui: &mut egui::Ui) {
        // Premium excerpt card styling
        let excerpt_bg = self.theme.bg_card();
        let excerpt_border = self.theme.border_subtle();
        let label_color = text_color(self.theme).gamma_multiply(0.7);
        let highlight_bg = self.theme.highlight_match();
        let text_col = text_color(self.theme);
        let accent_color = self.theme.accent_hover();
        let warning_color = self.theme.semantic_warning();

        // Clone find_pattern to avoid borrow issues
        let find_pattern = self.find_pattern.clone();

        for (idx, excerpt) in self.excerpts.iter_mut().enumerate() {
            let is_focused = self.focused_excerpt == idx as i32;
            let is_modified = excerpt.is_modified();

            // Excerpt container with focus ring and glow
            let frame_stroke = if is_focused {
                egui::Stroke::new(2.0, accent_color)
            } else {
                egui::Stroke::new(1.0, excerpt_border)
            };

            // Draw glow behind focused card
            if is_focused {
                let glow_rect = ui.available_rect_before_wrap();
                let glow_rect = egui::Rect::from_min_size(
                    glow_rect.min,
                    egui::vec2(glow_rect.width(), 80.0), // Approximate card height
                )
                .expand(2.0);
                ui.painter()
                    .rect_filled(glow_rect, 10.0, accent_color.gamma_multiply(0.1));
            }

            let frame_response = egui::Frame::new()
                .fill(excerpt_bg)
                .stroke(frame_stroke)
                .corner_radius(8.0) // More rounded
                .inner_margin(12.0) // More padding
                .shadow(egui::epaint::Shadow {
                    offset: [0, 2],
                    blur: 8,
                    spread: 0,
                    color: Color32::from_black_alpha(30),
                })
                .show(ui, |ui| {
                    // Label header with match count
                    ui.horizontal(|ui| {
                        // Modified indicator with glow
                        if is_modified {
                            ui.label(RichText::new("●").color(warning_color).size(typography::SM));
                            ui.add_space(4.0);
                        }

                        ui.label(
                            RichText::new(&excerpt.label)
                                .color(label_color)
                                .size(typography::MD)
                                .strong(),
                        );

                        // Show match count for this excerpt
                        if !find_pattern.is_empty() {
                            let match_count = excerpt.content.matches(&find_pattern).count();
                            if match_count > 0 {
                                ui.add_space(8.0);
                                // Match count badge
                                egui::Frame::new()
                                    .fill(accent_color.gamma_multiply(0.15))
                                    .corner_radius(4.0)
                                    .inner_margin(egui::vec2(6.0, 2.0))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!("{match_count} matches"))
                                                .color(accent_color)
                                                .size(typography::XS),
                                        );
                                    });
                            }
                        }
                    });

                    ui.add_space(6.0);

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

                        // Draw background for the text area with premium styling
                        let text_bg = self.theme.bg_inset();
                        painter.rect_filled(response.rect, 6.0, text_bg);

                        // Draw the highlighted text
                        painter.galley(response.rect.min + egui::vec2(8.0, 6.0), galley, text_col);

                        // If clicked, switch to edit mode (clear find pattern)
                        if response.clicked() {
                            // User can clear find to edit directly
                        }
                    } else {
                        // No matches or no search - show editable TextEdit with premium styling
                        let text_edit_id = egui::Id::new(format!("excerpt_{}", excerpt.source_id));

                        // Premium editor background
                        let editor_bg = self.theme.bg_inset();

                        egui::Frame::new()
                            .fill(editor_bg)
                            .corner_radius(6.0)
                            .inner_margin(egui::vec2(8.0, 6.0))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut excerpt.content)
                                        .id(text_edit_id)
                                        .font(typography::code_lg())
                                        .desired_width(ui.available_width())
                                        .desired_rows(2)
                                        .frame(false),
                                );
                            });
                    }
                });

            // Draw top highlight on card for glass effect
            let card_rect = frame_response.response.rect;
            let highlight_rect = egui::Rect::from_min_size(
                card_rect.left_top() + egui::vec2(1.0, 1.0),
                egui::vec2(card_rect.width() - 2.0, 1.0),
            );
            ui.painter().rect_filled(
                highlight_rect,
                6.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 8),
            );

            ui.add_space(10.0);
        }
    }
}
