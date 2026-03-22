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
    },
}

/// Build a flattened list of tree rows from PR files, respecting collapsed directories.
fn build_file_tree_rows(
    pr_files: &[PrFile],
    collapsed_dirs: &FxHashSet<String>,
    review_comments: &[PrComment],
    draft_comments: &[DraftComment],
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

        rows.push(FileTreeRow::File {
            file_index: *file_index,
            name: file_name.to_string(),
            depth: dir_parts.len(),
            comment_count: review_count + draft_count,
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

            // PR number
            if let Some(pr) = &self.current_pr {
                ui.label(
                    RichText::new(format!("#{}", pr.number))
                        .color(theme.accent_primary())
                        .font(typography::monospace(typography::SM)),
                );
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

                // Approve (always enabled if signed in)
                let approve_btn = ui.add_enabled(
                    can_submit,
                    egui::Button::new(RichText::new("Approve").size(typography::XS).color(
                        if can_submit {
                            theme.diff_added_text()
                        } else {
                            theme.text_secondary().gamma_multiply(0.5)
                        },
                    ))
                    .fill(if can_submit {
                        theme.diff_added_bg()
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
                    clicked_event = Some(ReviewEvent::Approve);
                }

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

        // Handle deferred actions outside closures
        if go_back {
            self.view = PrReviewView::List;
            self.current_pr = None;
            self.pr_files.clear();
            self.file_diffs.clear();
            self.review_comments.clear();
            self.issue_comments.clear();
            self.check_runs.clear();
            self.draft_comments.clear();
            self.draft_body.clear();
            self.commenting_line = None;
            self.comment_input.clear();
            self.submit_error = None;
            self.submit_success = None;
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
                        "o open \u{2022} s {view_mode} \u{2022} n/p files \u{2022} j/k scroll \u{2022} 1/2/3 tabs \u{2022} Esc back"
                    )
                } else {
                    format!(
                        "o open \u{2022} s {view_mode} \u{2022} j/k scroll \u{2022} 1/2/3 tabs \u{2022} Esc back"
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
        let file_panel_width = 220.0;
        let diff_width = (ui.available_width() - file_panel_width - 12.0).max(200.0);

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
        });
        ui.add_space(6.0);

        // Build flattened tree rows from file paths
        let tree_rows = build_file_tree_rows(
            &self.pr_files,
            &self.collapsed_dirs,
            &self.review_comments,
            &self.draft_comments,
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
                        } => {
                            let is_selected = *file_index == self.selected_file_index;
                            let file = &self.pr_files[*file_index];

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

                            // Filename (just the name, no path)
                            let name_color = if is_selected {
                                theme.text_primary()
                            } else {
                                theme.text_primary().gamma_multiply(0.85)
                            };
                            let max_name_width = (rect.right() - 12.0 - cx - 40.0).max(20.0);
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

                            // Stats on right
                            let mut right_x = rect.right() - 12.0;

                            if file.deletions > 0 {
                                let del_galley = ui.painter().layout_no_wrap(
                                    format!("-{}", file.deletions),
                                    typography::monospace(typography::XS),
                                    theme.diff_removed_gutter(),
                                );
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

                            if file.additions > 0 {
                                let add_galley = ui.painter().layout_no_wrap(
                                    format!("+{}", file.additions),
                                    typography::monospace(typography::XS),
                                    theme.diff_added_gutter(),
                                );
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

                            if *comment_count > 0 {
                                right_x -= 6.0;
                                let badge_galley = ui.painter().layout_no_wrap(
                                    format!(
                                        "{} {comment_count}",
                                        egui_nerdfonts::regular::COMMENT_TEXT
                                    ),
                                    typography::proportional(typography::XS),
                                    theme.accent_primary(),
                                );
                                right_x -= badge_galley.size().x;
                                ui.painter().galley(
                                    egui::pos2(
                                        right_x,
                                        rect.center().y - badge_galley.size().y / 2.0,
                                    ),
                                    badge_galley,
                                    theme.accent_primary(),
                                );
                            }
                            let _ = right_x;

                            if response.clicked() {
                                clicked_file = Some(*file_index);
                            }

                            response.on_hover_text(&file.filename);
                        }
                    }
                }
            });

        // Process deferred actions outside borrow
        if let Some(dir_path) = toggle_dir {
            if !self.collapsed_dirs.remove(&dir_path) {
                self.collapsed_dirs.insert(dir_path);
            }
        }
        if let Some(idx) = clicked_file {
            self.selected_file_index = idx;
        }
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
                // PR description
                if let Some(pr) = &self.current_pr {
                    if let Some(body) = &pr.body {
                        if !body.is_empty() {
                            render_comment(ui, theme, &pr.user.login, &pr.created_at, body);
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
