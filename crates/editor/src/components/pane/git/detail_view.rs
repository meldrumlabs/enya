//! PR detail view — shows file list, conversation, and checks tabs.

use egui::RichText;
use rustc_hash::FxHashSet;

use crate::git::api::{DraftComment, PrComment, PrFile, ReviewEvent, relative_time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::{DetailTab, PrReviewPane, PrReviewView};

/// A row in the flattened file tree.
enum FileTreeRow {
    Directory {
        path: String,
        name: String,
        depth: usize,
        collapsed: bool,
        file_count: usize,
    },
    File {
        file_index: usize,
        name: String,
        depth: usize,
        comment_count: usize,
        unseen_count: usize,
        reviewed: bool,
    },
}

/// Build a flattened list of tree rows from PR files, respecting collapsed directories.
fn build_file_tree_rows(
    pr_files: &[PrFile],
    collapsed_dirs: &FxHashSet<String>,
    review_comments: &[PrComment],
    draft_comments: &[DraftComment],
    seen_comment_ids: &rustc_hash::FxHashSet<u64>,
    reviewed_files: &rustc_hash::FxHashSet<String>,
) -> Vec<FileTreeRow> {
    // Collect unique directory prefixes and count files per directory
    let mut dir_files: Vec<(Vec<&str>, usize)> = Vec::new();
    for (i, file) in pr_files.iter().enumerate() {
        let parts: Vec<&str> = file.filename.split('/').collect();
        dir_files.push((parts, i));
    }

    // Sort by path for grouping
    dir_files.sort_by(|a, b| a.0.cmp(&b.0));

    // Track which directory prefixes we've emitted
    let mut emitted_dirs: FxHashSet<String> = FxHashSet::default();
    let mut rows = Vec::new();

    for (parts, file_index) in &dir_files {
        let file_name = parts.last().copied().unwrap_or("");
        let dir_parts = &parts[..parts.len() - 1];

        // Emit directory rows for each prefix we haven't seen yet
        let mut skip_file = false;
        for depth in 0..dir_parts.len() {
            let dir_path = dir_parts[..=depth].join("/");
            if emitted_dirs.contains(&dir_path) {
                // Check if this dir is collapsed — if so, skip children
                if collapsed_dirs.contains(&dir_path) {
                    skip_file = true;
                    break;
                }
                continue;
            }
            emitted_dirs.insert(dir_path.clone());

            let is_collapsed = collapsed_dirs.contains(&dir_path);

            // Count files under this directory prefix
            let prefix_with_slash = format!("{dir_path}/");
            let file_count = pr_files
                .iter()
                .filter(|f| f.filename.starts_with(&prefix_with_slash))
                .count();

            rows.push(FileTreeRow::Directory {
                path: dir_path,
                name: dir_parts[depth].to_string(),
                depth,
                collapsed: is_collapsed,
                file_count,
            });

            if is_collapsed {
                skip_file = true;
                break;
            }
        }

        if skip_file {
            continue;
        }

        // Count comments for this file
        let filename = &pr_files[*file_index].filename;
        let review_count = review_comments
            .iter()
            .filter(|c| c.path.as_deref() == Some(filename.as_str()))
            .count();
        let draft_count = draft_comments
            .iter()
            .filter(|c| c.path == *filename)
            .count();
        let unseen_count = review_comments
            .iter()
            .filter(|c| {
                c.path.as_deref() == Some(filename.as_str()) && !seen_comment_ids.contains(&c.id)
            })
            .count();

        rows.push(FileTreeRow::File {
            file_index: *file_index,
            name: file_name.to_string(),
            depth: dir_parts.len(),
            comment_count: review_count + draft_count,
            unseen_count,
            reviewed: reviewed_files.contains(filename),
        });
    }

    rows
}

impl PrReviewPane {
    /// Render the PR detail view.
    pub(super) fn show_detail_view(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;

        // Tab bar: back + #number + tabs + review actions
        ui.add_space(4.0);
        let mut clicked_event: Option<ReviewEvent> = None;
        let mut go_back = false;
        let mut approve_btn_anchor = egui::Rect::NOTHING;
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Back chevron
            let back_btn = ui.add(
                egui::Button::new(
                    RichText::new(egui_nerdfonts::regular::CHEVRON_LEFT)
                        .size(typography::SM)
                        .color(theme.text_secondary()),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            );
            if back_btn.clicked() {
                go_back = true;
            }

            // PR number + open in GitHub button
            if let Some(pr) = &self.current_pr {
                ui.label(
                    RichText::new(format!("#{}", pr.number))
                        .color(theme.accent_primary())
                        .font(typography::monospace(typography::SM)),
                );

                let open_btn = ui.add(
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::EXTERNAL_LINK)
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
                );
                if open_btn.clicked() {
                    let url = format!(
                        "https://github.com/{}/{}/pull/{}",
                        self.owner, self.repo, pr.number
                    );
                    ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                }
                open_btn.on_hover_text("Open in GitHub");
            }

            ui.add_space(8.0);

            // Tabs
            render_tab(ui, theme, "Files", DetailTab::Files, &mut self.active_tab);
            ui.add_space(8.0);
            render_tab(
                ui,
                theme,
                "Conversation",
                DetailTab::Conversation,
                &mut self.active_tab,
            );
            ui.add_space(8.0);
            render_tab(ui, theme, "Checks", DetailTab::Checks, &mut self.active_tab);

            // Review actions on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);

                let has_content = !self.draft_comments.is_empty() || !self.draft_body.is_empty();
                let can_submit = self.token.is_some() && !self.submitting_review;

                // Approve (toggles popup for optional message)
                let approve_btn = ui.add_enabled(
                    can_submit,
                    egui::Button::new(
                        RichText::new(format!("Approve {}", egui_nerdfonts::regular::CHEVRON_DOWN))
                            .size(typography::XS)
                            .color(if can_submit {
                                theme.diff_added_text()
                            } else {
                                theme.text_secondary().gamma_multiply(0.5)
                            }),
                    )
                    .fill(if can_submit {
                        if self.approve_popup_open {
                            theme.diff_added_bg().gamma_multiply(1.3)
                        } else {
                            theme.diff_added_bg()
                        }
                    } else {
                        theme.bg_elevated()
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if can_submit {
                            theme.diff_added_gutter().gamma_multiply(0.3)
                        } else {
                            theme.border_subtle()
                        },
                    ))
                    .corner_radius(4.0),
                );
                if approve_btn.clicked() {
                    self.approve_popup_open = !self.approve_popup_open;
                }
                approve_btn_anchor = approve_btn.rect;

                ui.add_space(4.0);

                // Request Changes (needs content)
                let rc_enabled = has_content && can_submit;
                let rc_btn = ui.add_enabled(
                    rc_enabled,
                    egui::Button::new(RichText::new("Request Changes").size(typography::XS).color(
                        if rc_enabled {
                            theme.diff_removed_text()
                        } else {
                            theme.text_secondary().gamma_multiply(0.5)
                        },
                    ))
                    .fill(if rc_enabled {
                        theme.diff_removed_bg()
                    } else {
                        theme.bg_elevated()
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if rc_enabled {
                            theme.diff_removed_gutter().gamma_multiply(0.3)
                        } else {
                            theme.border_subtle()
                        },
                    ))
                    .corner_radius(4.0),
                );
                if rc_btn.clicked() {
                    clicked_event = Some(ReviewEvent::RequestChanges);
                }

                ui.add_space(4.0);

                // Comment (needs content)
                let comment_enabled = has_content && can_submit;
                let comment_btn = ui.add_enabled(
                    comment_enabled,
                    egui::Button::new(RichText::new("Comment").size(typography::XS).color(
                        if comment_enabled {
                            theme.text_primary()
                        } else {
                            theme.text_secondary().gamma_multiply(0.5)
                        },
                    ))
                    .fill(theme.bg_elevated())
                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                    .corner_radius(4.0),
                );
                if comment_btn.clicked() {
                    clicked_event = Some(ReviewEvent::Comment);
                }

                // Submitting indicator
                if self.submitting_review {
                    ui.add_space(4.0);
                    ui.spinner();
                }

                // Reviewed files progress
                let reviewed_count = self.reviewed_files.len();
                let total_files = self.pr_files.len();
                if reviewed_count > 0 {
                    ui.add_space(8.0);
                    let progress_color = if reviewed_count == total_files {
                        theme.diff_added_gutter()
                    } else {
                        theme.text_secondary()
                    };
                    ui.label(
                        RichText::new(format!(
                            "{} {reviewed_count}/{total_files}",
                            egui_nerdfonts::regular::CHECK,
                        ))
                        .color(progress_color)
                        .font(typography::proportional(typography::XS)),
                    );
                }

                // Draft count badge
                let draft_count = self.draft_comments.len();
                if draft_count > 0 {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            egui_nerdfonts::regular::COMMENT,
                            draft_count
                        ))
                        .color(theme.accent_primary())
                        .font(typography::proportional(typography::XS)),
                    );
                }

                // Success/error messages
                if let Some(ref msg) = self.submit_success {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("{} {msg}", egui_nerdfonts::regular::CHECK))
                            .color(theme.diff_added_text())
                            .font(typography::proportional(typography::XS)),
                    );
                }
                if let Some(ref msg) = self.submit_error {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("{} {msg}", egui_nerdfonts::regular::X))
                            .color(theme.diff_removed_text())
                            .font(typography::proportional(typography::XS)),
                    );
                }
            });
        });

        // Approve popup (floating below the Approve button)
        if self.approve_popup_open {
            let popup_id = ui.id().with("approve_popup");
            let popup_pos = egui::pos2(
                approve_btn_anchor.right() - 280.0,
                approve_btn_anchor.bottom() + 4.0,
            );
            let area_resp = egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(popup_pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(theme.bg_elevated())
                        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(260.0);
                            ui.label(
                                RichText::new("Approve with message")
                                    .color(theme.text_primary())
                                    .font(typography::proportional(typography::SM))
                                    .strong(),
                            );
                            ui.add_space(6.0);
                            ui.add(
                                egui::TextEdit::multiline(&mut self.draft_body)
                                    .hint_text("Leave a comment (optional)")
                                    .desired_rows(3)
                                    .desired_width(260.0)
                                    .font(typography::proportional(typography::SM)),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let submit_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new("Submit Approval")
                                            .size(typography::XS)
                                            .color(theme.diff_added_text()),
                                    )
                                    .fill(theme.diff_added_bg())
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        theme.diff_added_gutter().gamma_multiply(0.3),
                                    ))
                                    .corner_radius(4.0),
                                );
                                if submit_btn.clicked() {
                                    clicked_event = Some(ReviewEvent::Approve);
                                    self.approve_popup_open = false;
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
                                    self.approve_popup_open = false;
                                }
                            });
                        });
                });

            // Close popup when clicking outside
            let popup_rect = area_resp.response.rect;
            if ui.input(|i| i.pointer.any_click())
                && !popup_rect.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
                && !approve_btn_anchor
                    .contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
            {
                self.approve_popup_open = false;
            }
        }

        // Handle deferred actions outside closures
        if go_back {
            self.view = PrReviewView::List;
            self.current_pr = None;
            self.pr_files.clear();
            self.file_diffs.clear();
            self.review_comments.clear();
            self.cached_threads.clear();
            self.issue_comments.clear();
            self.check_runs.clear();
            self.clear_review_state();
            return;
        }
        if let Some(event) = clicked_event {
            self.submit_review(event);
        }
        ui.add_space(4.0);

        // Separator below tabs
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        // ── PR description banner (collapsible) ──
        self.show_description_banner(ui, theme);

        // Loading state
        if self.detail_loading {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Loading PR details...")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::SM)),
                );
            });
            return;
        }

        // Error state
        if let Some(ref error) = self.detail_error {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(egui_nerdfonts::regular::WARNING)
                        .color(theme.diff_removed_text())
                        .size(24.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(error)
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::SM)),
                );
            });
            return;
        }

        // Tab content
        match self.active_tab {
            DetailTab::Files => self.show_files_tab(ui),
            DetailTab::Conversation => self.show_conversation_tab(ui, theme),
            DetailTab::Checks => self.show_checks_tab(ui, theme),
        }

        // Keybinding hints footer
        self.render_keybinding_footer(ui, theme);
    }

    /// Render keybinding hints at the bottom of the detail view.
    fn render_keybinding_footer(&self, ui: &mut egui::Ui, theme: AppTheme) {
        let muted = theme.text_secondary();
        let separator_color = theme.border_subtle();

        // Separator above footer
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Current file path
            if let Some(file_diff) = self.file_diffs.get(self.selected_file_index) {
                ui.label(
                    RichText::new(&file_diff.path)
                        .color(muted)
                        .font(typography::monospace(typography::SM)),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                let view_mode = if self.diff_renderer.split_view() { "split" } else { "stacked" };
                let hint = if self.file_diffs.len() > 1 {
                    format!(
                        "v viewed \u{2022} n/p files \u{2022} j/k scroll \u{2022} gg/G top/bottom \u{2022} s {view_mode} \u{2022} Esc back"
                    )
                } else {
                    format!(
                        "v viewed \u{2022} j/k scroll \u{2022} gg/G top/bottom \u{2022} s {view_mode} \u{2022} Esc back"
                    )
                };
                ui.label(
                    RichText::new(hint)
                        .color(muted.gamma_multiply(0.7))
                        .font(typography::proportional(typography::XS)),
                );
            });
        });
        ui.add_space(8.0);
    }

    /// Render the Files tab — file list + diff view.
    fn show_files_tab(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let available_height = (ui.available_height() - 50.0).max(100.0);
        let total_width = ui.available_width();

        if self.file_panel_collapsed {
            // Collapsed: show a thin expand button + full-width diff
            let toggle_width = 24.0;
            let diff_width = (total_width - toggle_width - 4.0).max(200.0);

            ui.horizontal(|ui| {
                // Expand toggle strip
                ui.allocate_ui_with_layout(
                    egui::vec2(toggle_width, available_height),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        ui.add_space(8.0);
                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(egui_nerdfonts::regular::CHEVRON_RIGHT)
                                    .size(typography::SM)
                                    .color(theme.text_secondary()),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        );
                        if btn.clicked() {
                            self.file_panel_collapsed = false;
                        }
                        btn.on_hover_text("Show file tree");
                    },
                );

                // Vertical separator
                let sep_rect = ui.available_rect_before_wrap();
                ui.painter().vline(
                    sep_rect.left(),
                    sep_rect.y_range(),
                    egui::Stroke::new(1.0, theme.border_subtle()),
                );

                // Diff content area (full width)
                ui.allocate_ui_with_layout(
                    egui::vec2(diff_width, available_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.show_diff_view(ui);
                    },
                );
            });
        } else {
            let file_panel_width = (total_width * 0.28).clamp(180.0, 320.0);
            let diff_width = (total_width - file_panel_width - 12.0).max(200.0);

            ui.horizontal(|ui| {
                // File panel (left)
                ui.allocate_ui_with_layout(
                    egui::vec2(file_panel_width, available_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.show_file_panel(ui, theme);
                    },
                );

                // Vertical separator
                let sep_rect = ui.available_rect_before_wrap();
                ui.painter().vline(
                    sep_rect.left(),
                    sep_rect.y_range(),
                    egui::Stroke::new(1.0, theme.border_subtle()),
                );

                // Diff content area (right)
                ui.allocate_ui_with_layout(
                    egui::vec2(diff_width, available_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.show_diff_view(ui);
                    },
                );
            });
        }
    }

    /// Render the file panel (right side in files tab).
    fn show_file_panel(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Changed Files")
                    .color(theme.text_primary().gamma_multiply(0.9))
                    .font(typography::proportional(typography::SM))
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("{}", self.pr_files.len()))
                    .color(theme.text_secondary())
                    .font(typography::proportional(typography::XS)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                let collapse_btn = ui.add(
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::CHEVRON_LEFT)
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
                );
                if collapse_btn.clicked() {
                    self.file_panel_collapsed = true;
                }
                collapse_btn.on_hover_text("Hide file tree");
            });
        });
        ui.add_space(6.0);

        // Build flattened tree rows from file paths
        let tree_rows = build_file_tree_rows(
            &self.pr_files,
            &self.collapsed_dirs,
            &self.review_comments,
            &self.draft_comments,
            &self.seen_comment_ids,
            &self.reviewed_files,
        );

        let mut toggle_dir: Option<String> = None;
        let mut clicked_file: Option<usize> = None;

        egui::ScrollArea::vertical()
            .id_salt("pr_file_panel")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in &tree_rows {
                    let row_height = 24.0;
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    let is_hovered = response.hovered();

                    match row {
                        FileTreeRow::Directory {
                            path,
                            name,
                            depth,
                            collapsed,
                            file_count,
                        } => {
                            if is_hovered {
                                ui.painter().rect_filled(
                                    rect,
                                    3.0,
                                    theme.text_primary().gamma_multiply(0.04),
                                );
                            }

                            let indent = 8.0 + *depth as f32 * 12.0;
                            let mut cx = rect.left() + indent;

                            // Chevron
                            let chevron = if *collapsed {
                                egui_nerdfonts::regular::CHEVRON_RIGHT
                            } else {
                                egui_nerdfonts::regular::CHEVRON_DOWN
                            };
                            let chev_galley = ui.painter().layout_no_wrap(
                                chevron.to_string(),
                                typography::proportional(typography::XS),
                                theme.text_secondary(),
                            );
                            ui.painter().galley(
                                egui::pos2(cx, rect.center().y - chev_galley.size().y / 2.0),
                                chev_galley.clone(),
                                theme.text_secondary(),
                            );
                            cx += chev_galley.size().x + 2.0;

                            // Folder icon
                            let folder_icon = if *collapsed {
                                egui_nerdfonts::regular::FOLDER
                            } else {
                                egui_nerdfonts::regular::FOLDER_OPEN
                            };
                            let folder_galley = ui.painter().layout_no_wrap(
                                folder_icon.to_string(),
                                typography::proportional(typography::XS),
                                theme.text_secondary().gamma_multiply(0.8),
                            );
                            ui.painter().galley(
                                egui::pos2(cx, rect.center().y - folder_galley.size().y / 2.0),
                                folder_galley.clone(),
                                theme.text_secondary().gamma_multiply(0.8),
                            );
                            cx += folder_galley.size().x + 4.0;

                            // Directory name
                            let dir_galley = ui.painter().layout_no_wrap(
                                name.clone(),
                                typography::monospace(typography::XS),
                                theme.text_primary().gamma_multiply(0.8),
                            );
                            ui.painter().galley(
                                egui::pos2(cx, rect.center().y - dir_galley.size().y / 2.0),
                                dir_galley,
                                theme.text_primary().gamma_multiply(0.8),
                            );

                            // File count on right
                            let count_text = format!("{file_count}");
                            let count_galley = ui.painter().layout_no_wrap(
                                count_text,
                                typography::proportional(typography::XS),
                                theme.text_secondary().gamma_multiply(0.5),
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    rect.right() - 12.0 - count_galley.size().x,
                                    rect.center().y - count_galley.size().y / 2.0,
                                ),
                                count_galley,
                                theme.text_secondary().gamma_multiply(0.5),
                            );

                            if response.clicked() {
                                toggle_dir = Some(path.clone());
                            }
                            if is_hovered {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        }
                        FileTreeRow::File {
                            file_index,
                            name,
                            depth,
                            comment_count,
                            unseen_count,
                            reviewed,
                        } => {
                            let file = &self.pr_files[*file_index];
                            let is_selected = self
                                .file_diffs
                                .get(self.selected_file_index)
                                .is_some_and(|d| d.path == file.filename);

                            // Background
                            if is_selected {
                                ui.painter().rect_filled(
                                    rect,
                                    3.0,
                                    theme.accent_primary().gamma_multiply(0.12),
                                );
                                let bar_rect = egui::Rect::from_min_size(
                                    rect.min,
                                    egui::vec2(3.0, row_height),
                                );
                                ui.painter()
                                    .rect_filled(bar_rect, 2.0, theme.accent_primary());
                            } else if is_hovered {
                                ui.painter().rect_filled(
                                    rect,
                                    3.0,
                                    theme.text_primary().gamma_multiply(0.04),
                                );
                            }

                            let indent = 8.0 + *depth as f32 * 12.0;
                            let mut cx = rect.left() + indent;

                            // File status icon
                            let icon = if file.status == "removed" {
                                egui_nerdfonts::regular::FILE_MINUS
                            } else if file.status == "added" {
                                egui_nerdfonts::regular::FILE_PLUS
                            } else {
                                egui_nerdfonts::regular::FILE_EDIT
                            };
                            let icon_color = if is_selected {
                                theme.accent_primary()
                            } else {
                                theme.text_secondary()
                            };
                            let icon_galley = ui.painter().layout_no_wrap(
                                icon.to_string(),
                                typography::proportional(typography::XS),
                                icon_color,
                            );
                            ui.painter().galley(
                                egui::pos2(cx, rect.center().y - icon_galley.size().y / 2.0),
                                icon_galley.clone(),
                                icon_color,
                            );
                            cx += icon_galley.size().x + 4.0;

                            // Pre-compute right-side stats to know their width
                            let del_galley = (file.deletions > 0).then(|| {
                                ui.painter().layout_no_wrap(
                                    format!("-{}", file.deletions),
                                    typography::monospace(typography::XS),
                                    theme.diff_removed_gutter(),
                                )
                            });
                            let add_galley = (file.additions > 0).then(|| {
                                ui.painter().layout_no_wrap(
                                    format!("+{}", file.additions),
                                    typography::monospace(typography::XS),
                                    theme.diff_added_gutter(),
                                )
                            });
                            let comment_galley = (*comment_count > 0).then(|| {
                                ui.painter().layout_no_wrap(
                                    format!(
                                        "{} {comment_count}",
                                        egui_nerdfonts::regular::COMMENT_TEXT
                                    ),
                                    typography::proportional(typography::XS),
                                    theme.accent_primary(),
                                )
                            });

                            let mut stats_width = 0.0;
                            if let Some(ref g) = del_galley {
                                stats_width += g.size().x + 3.0;
                            }
                            if let Some(ref g) = add_galley {
                                stats_width += g.size().x;
                            }
                            if let Some(ref g) = comment_galley {
                                stats_width += g.size().x + 6.0;
                            }

                            // Filename (just the name, no path)
                            let name_color = if is_selected {
                                theme.text_primary()
                            } else {
                                theme.text_primary().gamma_multiply(0.85)
                            };
                            let max_name_width =
                                (rect.right() - 12.0 - cx - stats_width - 6.0).max(20.0);
                            let name_galley = ui.painter().layout(
                                name.clone(),
                                typography::monospace(typography::XS),
                                name_color,
                                max_name_width,
                            );
                            ui.painter().galley(
                                egui::pos2(cx, rect.center().y - name_galley.size().y / 2.0),
                                name_galley,
                                name_color,
                            );

                            // Paint stats on right
                            let mut right_x = rect.right() - 12.0;

                            if let Some(del_galley) = del_galley {
                                right_x -= del_galley.size().x;
                                ui.painter().galley(
                                    egui::pos2(
                                        right_x,
                                        rect.center().y - del_galley.size().y / 2.0,
                                    ),
                                    del_galley,
                                    theme.diff_removed_gutter(),
                                );
                                right_x -= 3.0;
                            }

                            if let Some(add_galley) = add_galley {
                                right_x -= add_galley.size().x;
                                ui.painter().galley(
                                    egui::pos2(
                                        right_x,
                                        rect.center().y - add_galley.size().y / 2.0,
                                    ),
                                    add_galley,
                                    theme.diff_added_gutter(),
                                );
                            }

                            if let Some(comment_galley) = comment_galley {
                                right_x -= 6.0;
                                right_x -= comment_galley.size().x;
                                ui.painter().galley(
                                    egui::pos2(
                                        right_x,
                                        rect.center().y - comment_galley.size().y / 2.0,
                                    ),
                                    comment_galley,
                                    theme.accent_primary(),
                                );
                            }

                            // Unseen comment dot indicator
                            if *unseen_count > 0 {
                                right_x -= 8.0;
                                let dot_center = egui::pos2(right_x - 3.0, rect.center().y);
                                ui.painter()
                                    .circle_filled(dot_center, 3.0, theme.accent_primary());
                            }

                            // Reviewed checkmark
                            if *reviewed {
                                right_x -= 14.0;
                                let check_galley = ui.painter().layout_no_wrap(
                                    egui_nerdfonts::regular::CHECK.to_string(),
                                    typography::proportional(typography::XS),
                                    theme.diff_added_gutter(),
                                );
                                ui.painter().galley(
                                    egui::pos2(
                                        right_x,
                                        rect.center().y - check_galley.size().y / 2.0,
                                    ),
                                    check_galley,
                                    theme.diff_added_gutter(),
                                );
                            }
                            let _ = right_x;

                            // Auto-scroll to keep selected row visible on n/p navigation
                            if is_selected && self.file_tree_scroll_to_selected {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }

                            if response.clicked() {
                                clicked_file = Some(*file_index);
                            }

                            response.on_hover_text(&file.filename);
                        }
                    }
                }
            });

        // Clear scroll flag after rendering
        self.file_tree_scroll_to_selected = false;

        // Process deferred actions outside borrow
        if let Some(dir_path) = toggle_dir {
            if !self.collapsed_dirs.remove(&dir_path) {
                self.collapsed_dirs.insert(dir_path);
            }
        }
        if let Some(pr_idx) = clicked_file {
            // Resolve pr_files index to file_diffs index by matching path
            let filename = &self.pr_files[pr_idx].filename;
            if let Some(diff_idx) = self.file_diffs.iter().position(|d| d.path == *filename) {
                self.selected_file_index = diff_idx;
                self.mark_current_file_comments_seen();
            }
        }
    }

    /// Render the collapsible PR description banner between the tab bar and tab content.
    ///
    /// IMPORTANT: This method must NOT create any focusable widgets (Button, ScrollArea,
    /// TextEdit, or ui.interact with Sense::click). The pane's `handle_keyboard()` guard
    /// (`ctx.memory(|m| m.focused().is_some())`) bails out when *any* widget has focus,
    /// which would permanently break hjkl navigation until focus is cleared.
    /// All interactivity here uses raw pointer checks instead.
    fn show_description_banner(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        // Only show if the PR has a non-empty body
        let body = match self.current_pr.as_ref().and_then(|pr| pr.body.as_deref()) {
            Some(b) if !b.is_empty() => b.to_string(),
            _ => return,
        };
        let pr_author = self
            .current_pr
            .as_ref()
            .map(|pr| pr.user.login.clone())
            .unwrap_or_default();
        let pr_created = self
            .current_pr
            .as_ref()
            .map(|pr| pr.created_at.clone())
            .unwrap_or_default();

        // Frosted-glass banner — semi-transparent surface with a subtle accent
        // tint, a thin border, and a top-edge highlight for depth.
        let overlay_bg = theme.overlay_bg();
        let accent = theme.accent_primary();
        // Blend: 96% overlay_bg + 4% accent for a very subtle tint
        let banner_fill = egui::Color32::from_rgba_unmultiplied(
            ((overlay_bg.r() as u16 * 24 + accent.r() as u16) / 25) as u8,
            ((overlay_bg.g() as u16 * 24 + accent.g() as u16) / 25) as u8,
            ((overlay_bg.b() as u16 * 24 + accent.b() as u16) / 25) as u8,
            overlay_bg.a().min(240), // keep slightly translucent
        );
        let border_color = theme.overlay_border().gamma_multiply(0.5);
        let highlight_color = theme.overlay_highlight();

        // Height limits for the markdown body (not including header).
        let collapsed_max_h = 80.0;
        let expanded_max_h = (ui.available_height() * 0.4).max(120.0);

        let frame_resp = egui::Frame::new()
            .fill(banner_fill)
            .stroke(egui::Stroke::new(0.5, border_color))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 8,
                bottom: 4,
            })
            .outer_margin(egui::Margin {
                left: 4,
                right: 4,
                top: 2,
                bottom: 2,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // ── Header row: chevron + icon + "Description" + author + timestamp ──
                let header_resp = ui.horizontal(|ui| {
                    let chevron = if self.description_collapsed {
                        egui_nerdfonts::regular::CHEVRON_RIGHT
                    } else {
                        egui_nerdfonts::regular::CHEVRON_DOWN
                    };
                    ui.label(
                        RichText::new(chevron)
                            .size(typography::XS)
                            .color(theme.text_secondary()),
                    );
                    ui.label(
                        RichText::new(egui_nerdfonts::regular::TEXT_BOX)
                            .color(theme.accent_primary().gamma_multiply(0.7))
                            .size(typography::SM),
                    );
                    ui.label(
                        RichText::new("Description")
                            .color(theme.text_primary())
                            .font(typography::proportional(typography::XS))
                            .strong(),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&pr_author)
                            .color(theme.text_secondary())
                            .font(typography::proportional(typography::XS)),
                    );
                    ui.label(
                        RichText::new(relative_time(&pr_created))
                            .color(theme.text_secondary().gamma_multiply(0.6))
                            .font(typography::proportional(typography::XS)),
                    );
                });

                // ── Body (unless collapsed) ──
                if !self.description_collapsed {
                    ui.add_space(4.0);

                    let max_h = if self.description_expanded {
                        expanded_max_h
                    } else {
                        collapsed_max_h
                    };

                    let available_w = ui.available_width();

                    if self.description_expanded {
                        // Expanded: use ScrollArea for vertical scrolling.
                        // Hide scroll bars so no focusable widgets are created —
                        // mouse-wheel / trackpad scrolling still works.
                        egui::ScrollArea::vertical()
                            .id_salt("pr_desc_scroll")
                            .max_height(max_h)
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(ui, |ui| {
                                ui.set_width(available_w);
                                ui.disable();
                                crate::components::overlay::markdown_renderer::render_markdown(
                                    ui, &body, theme,
                                );
                            });
                    } else {
                        // Collapsed preview: clipped child UI, no scroll.
                        let child_rect = egui::Rect::from_min_size(
                            ui.cursor().left_top(),
                            egui::vec2(available_w, max_h),
                        );
                        let mut child_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(child_rect)
                                .id_salt("pr_desc_banner"),
                        );
                        child_ui.set_clip_rect(child_rect);
                        child_ui.disable();
                        crate::components::overlay::markdown_renderer::render_markdown(
                            &mut child_ui,
                            &body,
                            theme,
                        );
                        let content_h = child_ui.min_rect().height();
                        let used_h = content_h.min(max_h);
                        ui.allocate_space(egui::vec2(available_w, used_h));

                        // Fade-out gradient when content is truncated
                        if content_h > collapsed_max_h {
                            let fade_h = 28.0;
                            let fade_rect = egui::Rect::from_min_max(
                                egui::pos2(child_rect.left(), child_rect.min.y + used_h - fade_h),
                                egui::pos2(child_rect.right(), child_rect.min.y + used_h),
                            );
                            let fade_top = egui::Color32::from_rgba_unmultiplied(
                                banner_fill.r(),
                                banner_fill.g(),
                                banner_fill.b(),
                                0,
                            );
                            let mesh = {
                                let mut mesh = egui::Mesh::default();
                                mesh.colored_vertex(fade_rect.left_top(), fade_top);
                                mesh.colored_vertex(fade_rect.right_top(), fade_top);
                                mesh.colored_vertex(fade_rect.right_bottom(), banner_fill);
                                mesh.colored_vertex(fade_rect.left_bottom(), banner_fill);
                                mesh.add_triangle(0, 1, 2);
                                mesh.add_triangle(0, 2, 3);
                                mesh
                            };
                            ui.painter().add(egui::Shape::mesh(mesh));
                        }
                    }

                    // Show "more/less" toggle when body is long enough to overflow the
                    // collapsed height, or when already expanded (so user can collapse back).
                    let is_clipped = self.description_expanded || body.len() > 200;

                    // "more / less" toggle — rendered as a label, toggled via pointer
                    if is_clipped {
                        ui.add_space(2.0);
                        let toggle_resp = ui.horizontal(|ui| {
                            ui.add_space(ui.available_width() - 80.0);
                            let label_text = if self.description_expanded {
                                format!("less {}", egui_nerdfonts::regular::CHEVRON_UP)
                            } else {
                                format!("more {}", egui_nerdfonts::regular::CHEVRON_DOWN)
                            };
                            ui.label(
                                RichText::new(label_text)
                                    .size(typography::XS)
                                    .color(theme.accent_primary()),
                            );
                        });
                        let toggle_rect = toggle_resp.response.rect;
                        if ui.input(|i| i.pointer.any_pressed())
                            && toggle_rect.contains(
                                ui.input(|i| i.pointer.interact_pos().unwrap_or_default()),
                            )
                        {
                            self.description_expanded = !self.description_expanded;
                        }
                        if toggle_rect
                            .contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
                        {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                }

                // Header click-to-toggle (check after body so body clicks don't toggle)
                let header_rect = header_resp.response.rect;
                if ui.input(|i| i.pointer.any_pressed())
                    && header_rect
                        .contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
                {
                    self.description_collapsed = !self.description_collapsed;
                    if self.description_collapsed {
                        self.description_expanded = false;
                    }
                }
                if header_rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });

        // Top-edge highlight glow (frosted glass effect)
        let frame_rect = frame_resp.response.rect;
        ui.painter().hline(
            frame_rect.x_range(),
            frame_rect.top() + 0.5,
            egui::Stroke::new(1.0, highlight_color),
        );
    }

    /// Render the Conversation tab — PR body + issue-level discussion only.
    /// Review comments are shown inline in the Files tab.
    fn show_conversation_tab(&self, ui: &mut egui::Ui, theme: AppTheme) {
        let has_pr_body = self
            .current_pr
            .as_ref()
            .and_then(|pr| pr.body.as_deref())
            .is_some_and(|b| !b.is_empty());

        if !has_pr_body && self.issue_comments.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No discussion yet")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::MD)),
                );
                if !self.review_comments.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{} review comments are shown inline in the Files tab",
                            self.review_comments.len()
                        ))
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::XS)),
                    );
                }
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("pr_conversation")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // PR description — distinct from regular comments
                if let Some(pr) = &self.current_pr {
                    if let Some(body) = &pr.body {
                        if !body.is_empty() {
                            ui.add_space(8.0);
                            egui::Frame::new()
                                .fill(theme.bg_elevated())
                                .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                                .corner_radius(6.0)
                                .inner_margin(egui::Margin::same(12))
                                .outer_margin(egui::Margin::symmetric(12, 0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(egui_nerdfonts::regular::TEXT_BOX)
                                                .color(theme.accent_primary().gamma_multiply(0.6))
                                                .size(typography::SM),
                                        );
                                        ui.label(
                                            RichText::new("Description")
                                                .color(theme.text_secondary())
                                                .font(typography::proportional(typography::XS))
                                                .strong(),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(&pr.user.login)
                                                .color(theme.text_primary())
                                                .font(typography::proportional(typography::SM))
                                                .strong(),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(relative_time(&pr.created_at))
                                                .color(theme.text_secondary().gamma_multiply(0.7))
                                                .font(typography::proportional(typography::XS)),
                                        );
                                    });
                                    ui.add_space(6.0);
                                    crate::components::overlay::markdown_renderer::render_markdown(
                                        ui, body, theme,
                                    );
                                });
                        }
                    }
                }

                // Issue comments (PR-level discussion)
                for comment in &self.issue_comments {
                    render_comment(
                        ui,
                        theme,
                        &comment.user.login,
                        &comment.created_at,
                        &comment.body,
                    );
                }

                // Hint about inline comments
                if !self.review_comments.is_empty() {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(format!(
                                "{} {} review comments shown inline in Files tab",
                                egui_nerdfonts::regular::COMMENT_TEXT,
                                self.review_comments.len()
                            ))
                            .color(theme.text_secondary())
                            .font(typography::proportional(typography::XS)),
                        );
                    });
                }
            });
    }

    /// Render the Checks tab.
    fn show_checks_tab(&self, ui: &mut egui::Ui, theme: AppTheme) {
        if self.check_runs.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No checks")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::MD)),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("pr_checks")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                for check in &self.check_runs {
                    let row_height = 36.0;
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::hover(),
                    );

                    let content_rect = rect.shrink2(egui::vec2(16.0, 0.0));

                    // Status icon
                    let (icon, icon_color) = match check.conclusion.as_deref() {
                        Some("success") => (
                            egui_nerdfonts::regular::CHECK_CIRCLE,
                            theme.diff_added_text(),
                        ),
                        Some("failure") | Some("cancelled") => {
                            (egui_nerdfonts::regular::X_CIRCLE, theme.diff_removed_text())
                        }
                        Some("skipped") => (
                            egui_nerdfonts::regular::SKIP_FORWARD,
                            theme.text_secondary(),
                        ),
                        _ => (egui_nerdfonts::regular::CLOCK, theme.diff_hunk_text()),
                    };

                    let icon_galley = ui.painter().layout_no_wrap(
                        icon.to_string(),
                        typography::proportional(typography::MD),
                        icon_color,
                    );
                    ui.painter().galley(
                        egui::pos2(
                            content_rect.left(),
                            content_rect.center().y - icon_galley.size().y / 2.0,
                        ),
                        icon_galley.clone(),
                        icon_color,
                    );

                    // Check name
                    let name_x = content_rect.left() + icon_galley.size().x + 8.0;
                    let name_galley = ui.painter().layout_no_wrap(
                        check.name.clone(),
                        typography::proportional(typography::SM),
                        theme.text_primary(),
                    );
                    ui.painter().galley(
                        egui::pos2(name_x, content_rect.center().y - name_galley.size().y / 2.0),
                        name_galley,
                        theme.text_primary(),
                    );

                    // Status/conclusion on right
                    let status_text = check.conclusion.as_deref().unwrap_or(&check.status);
                    let status_galley = ui.painter().layout_no_wrap(
                        status_text.to_string(),
                        typography::proportional(typography::XS),
                        theme.text_secondary(),
                    );
                    ui.painter().galley(
                        egui::pos2(
                            content_rect.right() - status_galley.size().x,
                            content_rect.center().y - status_galley.size().y / 2.0,
                        ),
                        status_galley,
                        theme.text_secondary(),
                    );

                    // Bottom border
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom(),
                        egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.5)),
                    );
                }
            });
    }
}

/// Render a tab button.
fn render_tab(
    ui: &mut egui::Ui,
    theme: AppTheme,
    label: &str,
    tab: DetailTab,
    active_tab: &mut DetailTab,
) {
    let is_active = *active_tab == tab;
    let text_color = if is_active {
        theme.accent_primary()
    } else {
        theme.text_secondary()
    };

    let btn = ui.add(
        egui::Button::new(
            RichText::new(label)
                .size(typography::SM)
                .color(text_color)
                .strong(),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE),
    );

    // Active tab underline
    if is_active {
        let rect = btn.rect;
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() + 3.0,
            egui::Stroke::new(2.0, theme.accent_primary()),
        );
    }

    if btn.clicked() {
        *active_tab = tab;
    }
}

/// Render a single comment.
fn render_comment(ui: &mut egui::Ui, theme: AppTheme, author: &str, timestamp: &str, body: &str) {
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(theme.bg_elevated())
        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(12))
        .outer_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            // Author + timestamp
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(author)
                        .color(theme.text_primary())
                        .font(typography::proportional(typography::SM))
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(relative_time(timestamp))
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::XS)),
                );
            });
            ui.add_space(6.0);

            // Body (rendered as markdown)
            crate::components::overlay::markdown_renderer::render_markdown(ui, body, theme);
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::api::PrFile;

    fn make_pr_file(filename: &str) -> PrFile {
        PrFile {
            filename: filename.to_string(),
            status: "modified".to_string(),
            additions: 1,
            deletions: 1,
            changes: 2,
        }
    }

    fn collect_file_names(rows: &[FileTreeRow]) -> Vec<String> {
        rows.iter()
            .filter_map(|r| match r {
                FileTreeRow::File { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    fn collect_file_indices(rows: &[FileTreeRow]) -> Vec<usize> {
        rows.iter()
            .filter_map(|r| match r {
                FileTreeRow::File { file_index, .. } => Some(*file_index),
                _ => None,
            })
            .collect()
    }

    fn collect_dir_names(rows: &[FileTreeRow]) -> Vec<String> {
        rows.iter()
            .filter_map(|r| match r {
                FileTreeRow::Directory { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn flat_files_no_directories() {
        let files = vec![make_pr_file("README.md"), make_pr_file("Cargo.toml")];
        let rows = build_file_tree_rows(
            &files,
            &FxHashSet::default(),
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        let names = collect_file_names(&rows);
        assert_eq!(names, vec!["Cargo.toml", "README.md"]); // sorted alphabetically
        assert!(collect_dir_names(&rows).is_empty());
    }

    #[test]
    fn files_in_directories() {
        let files = vec![
            make_pr_file("src/main.rs"),
            make_pr_file("src/lib.rs"),
            make_pr_file("tests/test.rs"),
        ];
        let rows = build_file_tree_rows(
            &files,
            &FxHashSet::default(),
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        let dirs = collect_dir_names(&rows);
        assert!(dirs.contains(&"src".to_string()));
        assert!(dirs.contains(&"tests".to_string()));

        let file_names = collect_file_names(&rows);
        assert!(file_names.contains(&"lib.rs".to_string()));
        assert!(file_names.contains(&"main.rs".to_string()));
        assert!(file_names.contains(&"test.rs".to_string()));
    }

    #[test]
    fn collapsed_directory_hides_children() {
        let files = vec![
            make_pr_file("src/main.rs"),
            make_pr_file("src/lib.rs"),
            make_pr_file("tests/test.rs"),
        ];
        let mut collapsed = FxHashSet::default();
        collapsed.insert("src".to_string());

        let rows = build_file_tree_rows(
            &files,
            &collapsed,
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        // src directory should be present but collapsed
        let src_dir = rows
            .iter()
            .find(|r| matches!(r, FileTreeRow::Directory { name, .. } if name == "src"));
        assert!(src_dir.is_some());

        // src files should NOT be in the rows
        let file_names = collect_file_names(&rows);
        assert!(!file_names.contains(&"main.rs".to_string()));
        assert!(!file_names.contains(&"lib.rs".to_string()));
        // tests files should still be visible
        assert!(file_names.contains(&"test.rs".to_string()));
    }

    #[test]
    fn nested_directories() {
        let files = vec![
            make_pr_file("a/b/c.rs"),
            make_pr_file("a/b/d.rs"),
            make_pr_file("a/e.rs"),
        ];
        let rows = build_file_tree_rows(
            &files,
            &FxHashSet::default(),
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        let dirs = collect_dir_names(&rows);
        assert!(dirs.contains(&"a".to_string()));
        assert!(dirs.contains(&"b".to_string()));
    }

    #[test]
    fn collapsing_parent_hides_nested_files() {
        let files = vec![make_pr_file("a/b/c.rs"), make_pr_file("a/d.rs")];
        let mut collapsed = FxHashSet::default();
        collapsed.insert("a".to_string());

        let rows = build_file_tree_rows(
            &files,
            &collapsed,
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        // No files should be visible
        let file_names = collect_file_names(&rows);
        assert!(file_names.is_empty());
    }

    #[test]
    fn file_indices_refer_to_original_pr_files() {
        let files = vec![
            make_pr_file("z_file.rs"),
            make_pr_file("a_file.rs"),
            make_pr_file("m_file.rs"),
        ];
        let rows = build_file_tree_rows(
            &files,
            &FxHashSet::default(),
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        // Tree sorts alphabetically, but file_index should refer back to the
        // original position in pr_files
        let indices = collect_file_indices(&rows);
        let names: Vec<&str> = indices
            .iter()
            .map(|&i| files[i].filename.as_str())
            .collect();
        // Sorted order: a_file, m_file, z_file
        assert_eq!(names, vec!["a_file.rs", "m_file.rs", "z_file.rs"]);
    }

    #[test]
    fn directory_file_count() {
        let files = vec![
            make_pr_file("src/a.rs"),
            make_pr_file("src/b.rs"),
            make_pr_file("src/sub/c.rs"),
        ];
        let rows = build_file_tree_rows(
            &files,
            &FxHashSet::default(),
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        let src_dir = rows.iter().find_map(|r| match r {
            FileTreeRow::Directory {
                name, file_count, ..
            } if name == "src" => Some(*file_count),
            _ => None,
        });
        // All 3 files are under src/
        assert_eq!(src_dir, Some(3));
    }

    #[test]
    fn comment_counts_on_files() {
        let files = vec![make_pr_file("src/main.rs")];
        let review_comments = vec![PrComment {
            id: 1,
            body: "fix this".to_string(),
            user: crate::git::api::PrUser {
                login: "alice".to_string(),
                avatar_url: String::new(),
            },
            created_at: "2024-01-01T00:00:00Z".to_string(),
            path: Some("src/main.rs".to_string()),
            line: Some(10),
            in_reply_to_id: None,
        }];
        let draft_comments = vec![DraftComment {
            path: "src/main.rs".to_string(),
            line: 20,
            side: "RIGHT".to_string(),
            body: "draft note".to_string(),
        }];

        let rows = build_file_tree_rows(
            &files,
            &FxHashSet::default(),
            &review_comments,
            &draft_comments,
            &FxHashSet::default(),
            &FxHashSet::default(),
        );

        let comment_count = rows.iter().find_map(|r| match r {
            FileTreeRow::File { comment_count, .. } => Some(*comment_count),
            _ => None,
        });
        assert_eq!(comment_count, Some(2)); // 1 review + 1 draft
    }

    #[test]
    fn cross_directory_navigation_uses_file_diffs() {
        // This is the core regression test for the bug:
        // n/p should navigate through file_diffs, not pr_files,
        // and clicking a file in the tree should resolve to the correct diff index.
        use crate::git::diff::parse_diff_into_files;

        let diff = concat!(
            "diff --git a/crates/editor/src/foo.rs b/crates/editor/src/foo.rs\n",
            "--- a/crates/editor/src/foo.rs\n",
            "+++ b/crates/editor/src/foo.rs\n",
            "@@ -1,2 +1,2 @@\n",
            "-old\n",
            "+new\n",
            " ctx\n",
            "diff --git a/crates/client/src/bar.rs b/crates/client/src/bar.rs\n",
            "--- a/crates/client/src/bar.rs\n",
            "+++ b/crates/client/src/bar.rs\n",
            "@@ -1,2 +1,2 @@\n",
            "-old2\n",
            "+new2\n",
            " ctx2\n",
        );

        let file_diffs = parse_diff_into_files(diff);
        assert_eq!(file_diffs.len(), 2);

        // Diff order: foo first, bar second
        assert_eq!(file_diffs[0].path, "crates/editor/src/foo.rs");
        assert_eq!(file_diffs[1].path, "crates/client/src/bar.rs");

        // pr_files from API might be in different order (alphabetical)
        let pr_files = vec![
            make_pr_file("crates/client/src/bar.rs"),
            make_pr_file("crates/editor/src/foo.rs"),
        ];

        // Build tree from pr_files — clicking bar.rs gives pr_files index 0
        let rows = build_file_tree_rows(
            &pr_files,
            &FxHashSet::default(),
            &[],
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
        );
        let bar_pr_idx = rows
            .iter()
            .find_map(|r| match r {
                FileTreeRow::File {
                    file_index, name, ..
                } if name == "bar.rs" => Some(*file_index),
                _ => None,
            })
            .unwrap();

        // Resolve to file_diffs index
        let bar_filename = &pr_files[bar_pr_idx].filename;
        let bar_diff_idx = file_diffs
            .iter()
            .position(|d| d.path == *bar_filename)
            .unwrap();
        assert_eq!(bar_diff_idx, 1); // bar is at index 1 in file_diffs

        // n/p navigation bounded by file_diffs.len()
        let max = file_diffs.len().saturating_sub(1);
        let mut idx = 0;
        idx = (idx + 1).min(max);
        assert_eq!(idx, 1);
        assert_eq!(file_diffs[idx].path, "crates/client/src/bar.rs");

        idx = idx.saturating_sub(1);
        assert_eq!(idx, 0);
        assert_eq!(file_diffs[idx].path, "crates/editor/src/foo.rs");
    }
}
