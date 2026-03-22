//! Diff view for the PR review pane — renders per-file diffs with inline commenting.

use egui::RichText;

use crate::git::diff::DiffLine;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::icons::APP_GHOSTTY;
use crate::ui::theme::AppTheme;
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
                let split_label = if self.diff_renderer.split_view() {
                    "Stacked"
                } else {
                    "Split"
                };
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
                    self.diff_renderer.toggle_split_view();
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

        // Search bar (when active)
        if self.diff_renderer.search_active() {
            if let Some(new_file) = self.diff_renderer.render_search_bar(
                ui,
                theme,
                &self.file_diffs,
                self.selected_file_index,
            ) {
                self.selected_file_index = new_file;
            }
        }

        // Render diff content via shared DiffRenderer with inline comment callback
        let file_diff = self.file_diffs[self.selected_file_index].clone();
        let file_idx = self.selected_file_index;

        // Extract fields needed by the inline comment callback to avoid borrowing all of self
        let review_comments = &self.review_comments;
        let draft_comments = &self.draft_comments;
        let commenting_line = self.commenting_line;
        let comment_input = &mut self.comment_input;
        let mut pending_add_comment: Option<(String, usize, String)> = None;
        let mut clear_commenting = false;

        self.diff_renderer.render_diff(
            ui,
            &file_diff,
            file_idx,
            theme,
            Some(&mut |ui, line_idx, line: &DiffLine| {
                if let Some(new_line) = line.new_line_num {
                    render_inline_comments(
                        ui,
                        &file_diff.path,
                        new_line,
                        line_idx,
                        file_idx,
                        theme,
                        review_comments,
                        draft_comments,
                        commenting_line,
                        comment_input,
                        &mut pending_add_comment,
                        &mut clear_commenting,
                    );
                }
            }),
        );

        // Process deferred comment actions
        if let Some((path, line, body)) = pending_add_comment {
            self.add_draft_comment(path, line, body);
            self.comment_input.clear();
            self.commenting_line = None;
        }
        if clear_commenting {
            self.comment_input.clear();
            self.commenting_line = None;
        }

        // Process hunk expansion
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(hunk_idx) = self.diff_renderer.take_pending_expand() {
            if let Some(file_diff) = self.file_diffs.get_mut(self.selected_file_index) {
                self.diff_renderer.expand_context(file_diff, hunk_idx);
            }
        }
    }
}

/// Render inline comments for a specific line (standalone function for borrow splitting).
#[allow(clippy::too_many_arguments)]
fn render_inline_comments(
    ui: &mut egui::Ui,
    file_path: &str,
    line_num: usize,
    line_idx: usize,
    file_idx: usize,
    theme: AppTheme,
    review_comments: &[crate::git::api::PrComment],
    draft_comments: &[crate::git::api::DraftComment],
    commenting_line: Option<(usize, usize)>,
    comment_input: &mut String,
    pending_add_comment: &mut Option<(String, usize, String)>,
    clear_commenting: &mut bool,
) {
    // Show existing review comments for this line
    let comments_for_line: Vec<_> = review_comments
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
                        RichText::new(crate::git::api::relative_time(&comment.created_at))
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
    let draft_indices: Vec<usize> = draft_comments
        .iter()
        .enumerate()
        .filter(|(_, c)| c.path == file_path && c.line == line_num)
        .map(|(i, _)| i)
        .collect();

    for &idx in &draft_indices {
        let draft = &draft_comments[idx];
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
    if commenting_line == Some((file_idx, line_idx)) {
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
                    egui::TextEdit::multiline(comment_input)
                        .hint_text("Add a comment...")
                        .desired_rows(2)
                        .desired_width(ui.available_width())
                        .font(typography::proportional(typography::SM)),
                );

                // Focus the text input
                if response.gained_focus() || comment_input.is_empty() {
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
                    if submit_btn.clicked() && !comment_input.is_empty() {
                        *pending_add_comment =
                            Some((file_path.to_string(), line_num, comment_input.clone()));
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
                        *clear_commenting = true;
                    }
                });
            });
        ui.add_space(2.0);
    }
}
