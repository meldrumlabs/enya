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

                // Copy file contents
                let copy_btn = ui.add(
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::COPY)
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(theme.bg_elevated())
                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                    .corner_radius(4.0),
                );
                if copy_btn.clicked() {
                    let contents: String = file_diff
                        .lines
                        .iter()
                        .filter(|l| {
                            matches!(
                                l.kind,
                                crate::git::diff::DiffLineKind::Context
                                    | crate::git::diff::DiffLineKind::Addition
                            )
                        })
                        .map(|l| l.content.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(contents);
                }
                copy_btn.on_hover_text("Copy file contents");

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

        // Filter cached threads for this file
        let file_threads: Vec<_> = self
            .cached_threads
            .iter()
            .filter(|t| t.path == file_diff.path)
            .collect();

        // Extract fields needed by the inline comment callback to avoid borrowing all of self
        let draft_comments = &self.draft_comments;
        let commenting_line = self.commenting_line;
        let comment_input = &mut self.comment_input;
        let collapsed_threads = &mut self.collapsed_threads;
        let mut pending_add_comment: Option<(String, usize, String)> = None;
        let mut clear_commenting = false;
        let mut pending_start_reply: Option<(usize, usize)> = None;

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
                        &file_threads,
                        draft_comments,
                        commenting_line,
                        comment_input,
                        collapsed_threads,
                        &mut pending_add_comment,
                        &mut clear_commenting,
                        &mut pending_start_reply,
                    );
                }
            }),
        );

        // Process deferred comment actions — post directly to GitHub API
        if let Some((path, line, body)) = pending_add_comment {
            self.post_single_comment(path, line, body);
            self.comment_input.clear();
            self.commenting_line = None;
        }
        if clear_commenting {
            self.comment_input.clear();
            self.commenting_line = None;
        }

        // Process "+" comment button clicks
        if let Some((_file_idx, line_idx)) = self.diff_renderer.take_pending_comment() {
            self.commenting_line = Some((file_idx, line_idx));
        }

        // Process reply button clicks
        if let Some((fi, li)) = pending_start_reply {
            self.commenting_line = Some((fi, li));
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

/// Render threaded inline comments for a specific line (standalone function for borrow splitting).
#[allow(clippy::too_many_arguments)]
fn render_inline_comments(
    ui: &mut egui::Ui,
    file_path: &str,
    line_num: usize,
    line_idx: usize,
    file_idx: usize,
    theme: AppTheme,
    file_threads: &[&crate::git::api::CommentThread],
    draft_comments: &[crate::git::api::DraftComment],
    commenting_line: Option<(usize, usize)>,
    comment_input: &mut String,
    collapsed_threads: &mut rustc_hash::FxHashSet<(String, usize)>,
    pending_add_comment: &mut Option<(String, usize, String)>,
    clear_commenting: &mut bool,
    pending_start_reply: &mut Option<(usize, usize)>,
) {
    // Find thread for this line
    let thread = file_threads.iter().find(|t| t.line == line_num);

    // Find draft comments for this line
    let drafts: Vec<_> = draft_comments
        .iter()
        .filter(|c| c.path == file_path && c.line == line_num)
        .collect();

    let is_commenting = commenting_line == Some((file_idx, line_idx));
    let has_content = thread.is_some() || !drafts.is_empty() || is_commenting;

    if !has_content {
        return;
    }

    let thread_key = (file_path.to_string(), line_num);
    let accent = theme.accent_primary();

    ui.add_space(2.0);

    // Single thread card with left accent border
    egui::Frame::new()
        .fill(theme.bg_elevated())
        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(0))
        .outer_margin(egui::Margin {
            left: 40,
            right: 8,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            // Left accent border via painter
            let card_rect = ui.max_rect();
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    card_rect.left_top(),
                    egui::vec2(3.0, card_rect.height().max(1.0)),
                ),
                egui::CornerRadius {
                    nw: 4,
                    sw: 4,
                    ..Default::default()
                },
                accent.gamma_multiply(0.5),
            );

            ui.add_space(4.0);

            // Render review comments in thread
            if let Some(thread) = thread {
                let comments = &thread.comments;
                let is_collapsed = collapsed_threads.contains(&thread_key);
                let collapse_threshold = 3;
                let should_collapse = comments.len() > collapse_threshold;

                let visible_comments = if should_collapse && is_collapsed {
                    &comments[..1]
                } else {
                    comments
                };

                for (i, comment) in visible_comments.iter().enumerate() {
                    if i > 0 {
                        // Subtle divider between comments
                        ui.add_space(2.0);
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            (rect.left() + 12.0)..=(rect.right() - 8.0),
                            rect.top(),
                            egui::Stroke::new(0.5, theme.border_subtle()),
                        );
                        ui.add_space(2.0);
                    }

                    render_comment_in_thread(ui, theme, comment);
                }

                // "Show N more replies" / "Collapse" toggle
                if should_collapse {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let hidden = comments.len() - 1;
                        let label = if is_collapsed {
                            format!("Show {hidden} more replies")
                        } else {
                            "Collapse".to_string()
                        };
                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(label).size(typography::XS).color(accent),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        );
                        if btn.clicked() {
                            if is_collapsed {
                                collapsed_threads.remove(&thread_key);
                            } else {
                                collapsed_threads.insert(thread_key.clone());
                            }
                        }
                    });
                }
            }

            // Draft comments appended at bottom of thread with accent tint
            for draft in &drafts {
                if thread.is_some() {
                    // Divider before draft
                    ui.add_space(2.0);
                    let rect = ui.available_rect_before_wrap();
                    ui.painter().hline(
                        (rect.left() + 12.0)..=(rect.right() - 8.0),
                        rect.top(),
                        egui::Stroke::new(0.5, accent.gamma_multiply(0.3)),
                    );
                    ui.add_space(2.0);
                }

                // Draft comment with accent tint background
                egui::Frame::new()
                    .fill(accent.gamma_multiply(0.05))
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 8,
                        top: 6,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Draft")
                                    .color(accent)
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
            }

            // Reply / New comment input
            if is_commenting {
                if thread.is_some() || !drafts.is_empty() {
                    ui.add_space(2.0);
                    let rect = ui.available_rect_before_wrap();
                    ui.painter().hline(
                        (rect.left() + 12.0)..=(rect.right() - 8.0),
                        rect.top(),
                        egui::Stroke::new(0.5, accent.gamma_multiply(0.3)),
                    );
                    ui.add_space(2.0);
                }

                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 8,
                        top: 6,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        let response = ui.add(
                            egui::TextEdit::multiline(comment_input)
                                .hint_text(if thread.is_some() {
                                    "Reply..."
                                } else {
                                    "Add a comment..."
                                })
                                .desired_rows(2)
                                .desired_width(ui.available_width())
                                .font(typography::proportional(typography::SM)),
                        );

                        if response.gained_focus() || comment_input.is_empty() {
                            response.request_focus();
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let submit_label = if thread.is_some() {
                                "Reply"
                            } else {
                                "Add comment"
                            };
                            let submit_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(submit_label)
                                        .size(typography::XS)
                                        .color(theme.text_primary()),
                                )
                                .fill(accent.gamma_multiply(0.2))
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

                            // Keyboard shortcut hint
                            ui.label(
                                RichText::new("\u{2318}\u{23CE} submit \u{2022} Esc cancel")
                                    .color(theme.text_secondary().gamma_multiply(0.5))
                                    .font(typography::proportional(typography::XS)),
                            );
                        });
                    });
            } else if thread.is_some() {
                // Reply button at bottom of thread
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let reply_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Reply").size(typography::XS).color(accent),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    );
                    if reply_btn.clicked() {
                        *pending_start_reply = Some((file_idx, line_idx));
                    }
                });
            }

            ui.add_space(4.0);
        });

    ui.add_space(2.0);
}

/// Render a single comment within a thread card.
fn render_comment_in_thread(
    ui: &mut egui::Ui,
    theme: AppTheme,
    comment: &crate::git::api::PrComment,
) {
    use crate::git::api::relative_time;

    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 12,
            right: 8,
            top: 6,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Avatar placeholder — first letter in a circle
                let letter = comment
                    .user
                    .login
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let (avatar_rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().circle_filled(
                    avatar_rect.center(),
                    8.0,
                    theme.accent_primary().gamma_multiply(0.2),
                );
                ui.painter().text(
                    avatar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &letter,
                    typography::proportional(8.0),
                    theme.accent_primary(),
                );

                ui.label(
                    RichText::new(&comment.user.login)
                        .color(theme.text_primary())
                        .font(typography::proportional(typography::XS))
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(relative_time(&comment.created_at))
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::XS)),
                );
            });
            ui.add_space(2.0);
            crate::components::overlay::markdown_renderer::render_markdown(
                ui,
                &comment.body,
                theme,
            );
        });
}
