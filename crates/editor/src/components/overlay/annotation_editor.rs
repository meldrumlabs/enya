//! Annotation editor overlay for creating and editing chart annotations.
//!
//! A sleek modal dialog for entering annotation details:
//! - Message text
//! - Priority level (Normal/Important/Critical)
//! - Target type (Point/Range)

use egui::{Color32, Id, Key, RichText};

use crate::components::pane::annotation::{
    Annotation, AnnotationAuthor, AnnotationId, AnnotationPriority, AnnotationTarget,
};
use crate::components::util::finder_utils::OverlayStyle;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Result from the annotation editor.
#[derive(Debug, Clone)]
pub enum AnnotationEditorResult {
    /// No action (still editing or closed without saving).
    None,
    /// User submitted a new annotation.
    Created(Annotation),
    /// User updated an existing annotation.
    Updated(Annotation),
    /// User deleted the annotation.
    Deleted(AnnotationId),
    /// User canceled the editor.
    Cancelled,
}

/// Annotation editor overlay state and rendering.
pub struct AnnotationEditor {
    /// Whether the editor is open.
    is_open: bool,
    /// Current theme.
    theme: AppTheme,
    /// The annotation being edited (None = creating new).
    editing: Option<Annotation>,
    /// Message text buffer.
    message: String,
    /// Selected priority.
    priority: AnnotationPriority,
    /// Target timestamp (for point annotations).
    target_timestamp: f64,
    /// End timestamp (for range annotations).
    target_end: Option<f64>,
    /// Author name.
    author_name: String,
    /// Whether the message input has focus.
    focus_message: bool,
}

impl Default for AnnotationEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl AnnotationEditor {
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
            editing: None,
            message: String::new(),
            priority: AnnotationPriority::Normal,
            target_timestamp: 0.0,
            target_end: None,
            author_name: "You".to_string(),
            focus_message: false,
        }
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the editor for a new annotation at the given timestamp.
    pub fn open_new(&mut self, timestamp: f64, author_name: Option<&str>) {
        self.is_open = true;
        self.editing = None;
        self.message.clear();
        self.priority = AnnotationPriority::Normal;
        self.target_timestamp = timestamp;
        self.target_end = None;
        if let Some(name) = author_name {
            self.author_name = name.to_string();
        }
        self.focus_message = true;
    }

    /// Open the editor for a new range annotation.
    pub fn open_new_range(&mut self, start: f64, end: f64, author_name: Option<&str>) {
        self.is_open = true;
        self.editing = None;
        self.message.clear();
        self.priority = AnnotationPriority::Normal;
        self.target_timestamp = start;
        self.target_end = Some(end);
        if let Some(name) = author_name {
            self.author_name = name.to_string();
        }
        self.focus_message = true;
    }

    /// Open the editor to edit an existing annotation.
    pub fn open_edit(&mut self, annotation: Annotation) {
        self.is_open = true;
        self.message = annotation.message.clone();
        self.priority = annotation.priority;
        self.target_timestamp = annotation.timestamp();
        self.target_end = match &annotation.target {
            AnnotationTarget::Range { end, .. } => Some(*end),
            _ => None,
        };
        self.author_name = annotation.author.display_name.clone();
        self.editing = Some(annotation);
        self.focus_message = true;
    }

    /// Check if the editor is open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Close the editor without saving.
    pub fn close(&mut self) {
        self.is_open = false;
        self.editing = None;
        self.message.clear();
    }

    /// Show the annotation editor overlay.
    pub fn show(&mut self, ctx: &egui::Context) -> AnnotationEditorResult {
        if !self.is_open {
            return AnnotationEditorResult::None;
        }

        let mut result = AnnotationEditorResult::None;
        // Extract colors from theme (Custom variant handles plugin colors internally)
        let overlay_style = OverlayStyle::frosted_glass(self.theme);

        // Handle keyboard shortcuts - use consume_key to prevent multiple processing
        let mut escape_pressed = false;
        let mut enter_pressed = false;
        ctx.input_mut(|i| {
            escape_pressed = i.consume_key(egui::Modifiers::NONE, Key::Escape);
            enter_pressed = i.consume_key(egui::Modifiers::COMMAND, Key::Enter);
        });

        if escape_pressed {
            self.close();
            return AnnotationEditorResult::Cancelled;
        }

        // Modal area
        let area = egui::Area::new(Id::new("annotation_editor_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -50.0])
            .order(egui::Order::Foreground);

        area.show(ctx, |ui| {
            // Backdrop
            let screen_rect =
                ctx.input(|i| i.viewport().inner_rect.unwrap_or(egui::Rect::EVERYTHING));
            let backdrop_layer = egui::LayerId::new(egui::Order::Background, Id::new("ann_ed_bg"));
            ctx.layer_painter(backdrop_layer).rect_filled(
                screen_rect,
                0.0,
                Color32::from_black_alpha(180),
            );

            // Main frame
            let frame = egui::Frame::new()
                .fill(overlay_style.bg)
                .corner_radius(overlay_style.corner_radius)
                .shadow(overlay_style.shadow)
                .stroke(egui::Stroke::new(1.0, overlay_style.border));

            frame.show(ui, |ui| {
                ui.set_min_width(400.0);
                ui.set_max_width(500.0);

                // Padding
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        // Header
                        let title = if self.editing.is_some() {
                            "Edit Annotation"
                        } else {
                            "New Annotation"
                        };
                        ui.label(
                            RichText::new(title)
                                .size(typography::LG)
                                .color(self.theme.text_primary())
                                .strong(),
                        );

                        ui.add_space(16.0);

                        // Priority selector
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Priority:")
                                    .size(typography::SM)
                                    .color(self.theme.text_secondary()),
                            );
                            ui.add_space(8.0);

                            for priority in [
                                AnnotationPriority::Normal,
                                AnnotationPriority::Important,
                                AnnotationPriority::Critical,
                            ] {
                                let is_selected = self.priority == priority;
                                let priority_color = priority.color_for_theme(self.theme);
                                let color = if is_selected {
                                    priority_color
                                } else {
                                    self.theme.text_tertiary()
                                };

                                let btn = egui::Button::new(
                                    RichText::new(format!(
                                        "{} {}",
                                        priority.icon(),
                                        priority.label()
                                    ))
                                    .size(typography::SM)
                                    .color(color),
                                )
                                .fill(if is_selected {
                                    priority_color.gamma_multiply(0.15)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(6.0);

                                if ui.add(btn).clicked() {
                                    self.priority = priority;
                                }
                            }
                        });

                        ui.add_space(12.0);

                        // Message input
                        ui.label(
                            RichText::new("Message:")
                                .size(typography::SM)
                                .color(self.theme.text_secondary()),
                        );
                        ui.add_space(4.0);

                        let text_edit = egui::TextEdit::multiline(&mut self.message)
                            .desired_width(f32::INFINITY)
                            .desired_rows(3)
                            .hint_text("Describe the annotation...")
                            .font(egui::FontId::proportional(typography::MD));

                        let response = ui.add(text_edit);

                        // Focus on first frame
                        if self.focus_message {
                            response.request_focus();
                            self.focus_message = false;
                        }

                        ui.add_space(16.0);

                        // Buttons
                        ui.horizontal(|ui| {
                            // Delete button (only for editing)
                            if let Some(ref ann) = self.editing {
                                let delete_btn = egui::Button::new(
                                    RichText::new(format!(
                                        "{} Delete",
                                        semantic_icons::action::DELETE
                                    ))
                                    .size(typography::SM)
                                    .color(Color32::from_rgb(220, 53, 69)),
                                )
                                .fill(Color32::TRANSPARENT);

                                if ui.add(delete_btn).clicked() {
                                    result = AnnotationEditorResult::Deleted(ann.id);
                                    self.close();
                                }

                                let spacer = (ui.available_width() - 160.0).max(0.0);
                                ui.add_space(spacer);
                            } else {
                                let spacer = (ui.available_width() - 160.0).max(0.0);
                                ui.add_space(spacer);
                            }

                            // Cancel button
                            let cancel_btn = egui::Button::new(
                                RichText::new("Cancel")
                                    .size(typography::SM)
                                    .color(self.theme.text_secondary()),
                            )
                            .fill(Color32::TRANSPARENT);

                            if ui.add(cancel_btn).clicked() {
                                result = AnnotationEditorResult::Cancelled;
                                self.close();
                            }

                            ui.add_space(8.0);

                            // Save button
                            let can_save = !self.message.trim().is_empty();
                            let save_color = if can_save {
                                self.theme.accent_primary()
                            } else {
                                self.theme.text_tertiary()
                            };

                            let save_btn = egui::Button::new(
                                RichText::new(format!("{} Save", semantic_icons::action::SAVE))
                                    .size(typography::SM)
                                    .color(save_color),
                            )
                            .fill(if can_save {
                                self.theme.accent_primary().gamma_multiply(0.15)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(6.0);

                            if ui.add_enabled(can_save, save_btn).clicked()
                                || (enter_pressed && can_save)
                            {
                                result = self.build_result();
                                self.close();
                            }
                        });

                        ui.add_space(8.0);

                        // Hint text
                        ui.label(
                            RichText::new("Cmd+Enter to save, Esc to cancel")
                                .size(typography::XS)
                                .color(self.theme.text_tertiary()),
                        );
                    });
                    ui.add_space(20.0);
                });
                ui.add_space(16.0);
            });
        });

        result
    }

    /// Build the annotation result from current state.
    fn build_result(&self) -> AnnotationEditorResult {
        let target = if let Some(end) = self.target_end {
            AnnotationTarget::Range {
                start: self.target_timestamp,
                end,
            }
        } else {
            AnnotationTarget::Point {
                timestamp: self.target_timestamp,
            }
        };

        if let Some(ref existing) = self.editing {
            // Update existing
            let mut updated = existing.clone();
            updated.message = self.message.trim().to_string();
            updated.priority = self.priority;
            updated.target = target;
            AnnotationEditorResult::Updated(updated)
        } else {
            // Create new
            let annotation = match target {
                AnnotationTarget::Point { timestamp } => {
                    Annotation::at_point(timestamp, self.message.trim())
                }
                AnnotationTarget::Range { start, end } => {
                    Annotation::at_range(start, end, self.message.trim())
                }
                AnnotationTarget::DataPoint { timestamp, value } => {
                    Annotation::at_data_point(timestamp, value, self.message.trim())
                }
            }
            .with_author(AnnotationAuthor::local(&self.author_name))
            .with_priority(self.priority);

            AnnotationEditorResult::Created(annotation)
        }
    }
}
