//! Diff view for the PR review pane — renders per-file diffs with inline commenting.

use egui::RichText;

use crate::components::util::diff_rendering::{DiffLineKind, build_split_view_lines};
use crate::components::util::diff_widget;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::icons::APP_GHOSTTY;
use crate::ui::typography;

use super::PrReviewPane;

impl PrReviewPane {
    /// Render the diff view for the currently selected file.
    pub(super) fn show_diff_view(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;

        if self.file_diffs.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No file selected")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::MD)),
                );
            });
            return;
        }

        let Some(file_diff) = self.file_diffs.get(self.selected_file_index) else {
            return;
        };

        // File path header
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(&file_diff.path)
                    .color(theme.text_primary())
                    .font(typography::monospace(typography::SM)),
            );

            ui.add_space(8.0);

            // Navigation: file N of M
            let file_count = self.file_diffs.len();
            let file_index = self.selected_file_index + 1;
            ui.label(
                RichText::new(format!("{file_index}/{file_count}"))
                    .color(theme.text_secondary())
                    .font(typography::proportional(typography::XS)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Open button with Ghostty icon preview (native only)
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let open_btn = ui.add(
                        egui::Button::image_and_text(
                            egui::Image::new(APP_GHOSTTY.as_image_source())
                                .fit_to_exact_size(egui::vec2(14.0, 14.0)),
                            RichText::new(format!(
                                "Open {}",
                                egui_nerdfonts::regular::CHEVRON_DOWN
                            ))
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                        )
                        .fill(theme.bg_elevated())
                        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                        .corner_radius(4.0),
                    );

                    if open_btn.clicked() || self.pending_open_file_opener {
                        self.pending_open_file_opener = false;
                        let popup_pos = open_btn.rect.left_bottom();
                        let file_path = if let Some(root) = &self.repo_root {
                            root.join(&file_diff.path)
                        } else {
                            std::path::PathBuf::from(&file_diff.path)
                        };
                        self.file_opener.open_with_base(
                            popup_pos,
                            file_path,
                            self.repo_root.clone(),
                        );
                    }
                }

                ui.add_space(4.0);

                // Split view toggle
                let split_label = if self.split_view { "Unified" } else { "Split" };
                let split_btn = ui.add(
                    egui::Button::new(
                        RichText::new(split_label)
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(theme.bg_elevated())
                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                    .corner_radius(4.0),
                );
                if split_btn.clicked() {
                    self.split_view = !self.split_view;
                }
            });
        });
        ui.add_space(4.0);

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        // Render diff content
        let file_diff = self.file_diffs[self.selected_file_index].clone();
        let line_num_width = diff_widget::max_line_num_width(&file_diff);

        if self.split_view {
            self.render_split_diff(ui, &file_diff, line_num_width);
        } else {
            self.render_unified_diff(ui, &file_diff, line_num_width);
        }
    }

    /// Render unified diff view.
    fn render_unified_diff(
        &mut self,
        ui: &mut egui::Ui,
        file_diff: &crate::components::util::diff_rendering::FileDiff,
        line_num_width: usize,
    ) {
        let theme = self.theme;

        egui::ScrollArea::both()
            .id_salt("pr_diff_unified")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.style_mut().spacing.item_spacing.y = 0.0;

                for (line_idx, line) in file_diff.lines.iter().enumerate() {
                    diff_widget::render_diff_line(ui, line, line_num_width, theme);

                    // Show inline comments for this line
                    if let Some(new_line) = line.new_line_num {
                        self.render_inline_comments(ui, &file_diff.path, new_line, line_idx);
                    }
                }

                ui.add_space(8.0);
            });
    }

    /// Render split (side-by-side) diff view.
    fn render_split_diff(
        &self,
        ui: &mut egui::Ui,
        file_diff: &crate::components::util::diff_rendering::FileDiff,
        line_num_width: usize,
    ) {
        let theme = self.theme;
        let available_width = ui.available_width();
        let side_width = ((available_width - 8.0) / 2.0).max(1.0);

        let paired_lines = build_split_view_lines(&file_diff.lines);

        // Column headers
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, typography::SM + 4.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Old")
                            .color(theme.diff_removed_text().gamma_multiply(0.7))
                            .font(typography::proportional(typography::XS))
                            .strong(),
                    );
                },
            );
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, typography::SM + 4.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("New")
                            .color(theme.diff_added_text().gamma_multiply(0.7))
                            .font(typography::proportional(typography::XS))
                            .strong(),
                    );
                },
            );
        });

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        egui::ScrollArea::vertical()
            .id_salt("pr_diff_split")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(available_width);
                ui.add_space(4.0);
                ui.style_mut().spacing.item_spacing.y = 0.0;

                for (left, right) in &paired_lines {
                    let is_header = left.as_ref().is_some_and(|l| {
                        matches!(l.kind, DiffLineKind::HunkHeader | DiffLineKind::FileHeader)
                    });

                    if is_header {
                        if let Some(line) = left.as_ref() {
                            diff_widget::render_split_header_line(ui, line, available_width, theme);
                        }
                    } else {
                        ui.horizontal(|ui| {
                            ui.set_max_width(available_width);

                            ui.allocate_ui_with_layout(
                                egui::vec2(side_width, typography::MD + 4.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(side_width);
                                    diff_widget::render_split_line(
                                        ui,
                                        left.as_ref(),
                                        line_num_width,
                                        true,
                                        side_width,
                                        theme,
                                    );
                                },
                            );

                            // Separator
                            let sep = ui.available_rect_before_wrap();
                            ui.painter().vline(
                                sep.left(),
                                sep.y_range(),
                                egui::Stroke::new(1.0, theme.border_subtle()),
                            );
                            ui.add_space(4.0);

                            ui.allocate_ui_with_layout(
                                egui::vec2(side_width, typography::MD + 4.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(side_width);
                                    diff_widget::render_split_line(
                                        ui,
                                        right.as_ref(),
                                        line_num_width,
                                        false,
                                        side_width,
                                        theme,
                                    );
                                },
                            );
                        });
                    }
                }

                ui.add_space(8.0);
            });
    }

    /// Render inline comments and the comment input for a specific line.
    fn render_inline_comments(
        &mut self,
        ui: &mut egui::Ui,
        file_path: &str,
        line_num: usize,
        line_idx: usize,
    ) {
        let theme = self.theme;

        // Show existing review comments for this line
        let comments_for_line: Vec<_> = self
            .review_comments
            .iter()
            .filter(|c| c.path.as_deref() == Some(file_path) && c.line == Some(line_num))
            .collect();

        for comment in &comments_for_line {
            ui.add_space(2.0);
            egui::Frame::new()
                .fill(theme.bg_elevated())
                .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(8))
                .outer_margin(egui::Margin {
                    left: 40,
                    right: 8,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&comment.user.login)
                                .color(theme.text_primary())
                                .font(typography::proportional(typography::XS))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(crate::github_api::relative_time(&comment.created_at))
                                .color(theme.text_secondary())
                                .font(typography::proportional(typography::XS)),
                        );
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(&comment.body)
                            .color(theme.text_primary().gamma_multiply(0.9))
                            .font(typography::proportional(typography::XS)),
                    );
                });
            ui.add_space(2.0);
        }

        // Show draft comments for this line
        let draft_indices: Vec<usize> = self
            .draft_comments
            .iter()
            .enumerate()
            .filter(|(_, c)| c.path == file_path && c.line == line_num)
            .map(|(i, _)| i)
            .collect();

        for &idx in &draft_indices {
            let draft = &self.draft_comments[idx];
            ui.add_space(2.0);
            egui::Frame::new()
                .fill(theme.accent_primary().gamma_multiply(0.08))
                .stroke(egui::Stroke::new(
                    1.0,
                    theme.accent_primary().gamma_multiply(0.3),
                ))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(8))
                .outer_margin(egui::Margin {
                    left: 40,
                    right: 8,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Draft")
                                .color(theme.accent_primary())
                                .font(typography::proportional(typography::XS))
                                .strong(),
                        );
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(&draft.body)
                            .color(theme.text_primary().gamma_multiply(0.9))
                            .font(typography::proportional(typography::XS)),
                    );
                });
            ui.add_space(2.0);
        }

        // Comment input (if this line is being commented on)
        if self.commenting_line == Some((self.selected_file_index, line_idx)) {
            ui.add_space(2.0);
            egui::Frame::new()
                .fill(theme.bg_elevated())
                .stroke(egui::Stroke::new(
                    1.0,
                    theme.accent_primary().gamma_multiply(0.4),
                ))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(8))
                .outer_margin(egui::Margin {
                    left: 40,
                    right: 8,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut self.comment_input)
                            .hint_text("Add a comment...")
                            .desired_rows(2)
                            .desired_width(ui.available_width())
                            .font(typography::proportional(typography::SM)),
                    );

                    // Focus the text input
                    if response.gained_focus() || self.comment_input.is_empty() {
                        response.request_focus();
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let submit_btn = ui.add(
                            egui::Button::new(
                                RichText::new("Add comment")
                                    .size(typography::XS)
                                    .color(theme.text_primary()),
                            )
                            .fill(theme.accent_primary().gamma_multiply(0.2))
                            .corner_radius(3.0),
                        );
                        if submit_btn.clicked() && !self.comment_input.is_empty() {
                            self.add_draft_comment(
                                file_path.to_string(),
                                line_num,
                                self.comment_input.clone(),
                            );
                            self.comment_input.clear();
                            self.commenting_line = None;
                        }

                        let cancel_btn = ui.add(
                            egui::Button::new(
                                RichText::new("Cancel")
                                    .size(typography::XS)
                                    .color(theme.text_secondary()),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        );
                        if cancel_btn.clicked() {
                            self.comment_input.clear();
                            self.commenting_line = None;
                        }
                    });
                });
            ui.add_space(2.0);
        }
    }
}
