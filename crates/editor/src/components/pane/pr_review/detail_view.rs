//! PR detail view — shows file list, conversation, and checks tabs.

use egui::RichText;

use crate::components::util::diff_widget;
use crate::github_api::{ReviewEvent, relative_time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::{DetailTab, PrReviewPane, PrReviewView};

impl PrReviewPane {
    /// Render the PR detail view.
    pub(super) fn show_detail_view(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;

        // Header: back button + PR info
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Back button
            let back_btn = ui.add(
                egui::Button::new(
                    RichText::new(format!("{} Back", egui_nerdfonts::regular::ARROW_LEFT))
                        .size(typography::SM)
                        .color(theme.text_secondary()),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            );
            if back_btn.clicked() {
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
            }

            ui.add_space(4.0);

            if let Some(pr) = &self.current_pr {
                // PR number
                ui.label(
                    RichText::new(format!("#{}", pr.number))
                        .color(theme.accent_primary())
                        .font(typography::monospace(typography::MD))
                        .strong(),
                );
                ui.add_space(8.0);

                // Title
                ui.label(
                    RichText::new(&pr.title)
                        .color(theme.text_primary())
                        .font(typography::proportional(typography::MD)),
                );

                // Stats on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);

                    // Check status indicator
                    let check_icon = if self.check_runs.is_empty() {
                        egui_nerdfonts::regular::CIRCLE_SMALL
                    } else if self
                        .check_runs
                        .iter()
                        .all(|c| c.conclusion.as_deref() == Some("success"))
                    {
                        egui_nerdfonts::regular::CHECK
                    } else if self
                        .check_runs
                        .iter()
                        .any(|c| c.conclusion.as_deref() == Some("failure"))
                    {
                        egui_nerdfonts::regular::X
                    } else {
                        egui_nerdfonts::regular::CLOCK
                    };

                    let check_color = if self.check_runs.is_empty() {
                        theme.text_secondary()
                    } else if self
                        .check_runs
                        .iter()
                        .all(|c| c.conclusion.as_deref() == Some("success"))
                    {
                        theme.diff_added_text()
                    } else if self
                        .check_runs
                        .iter()
                        .any(|c| c.conclusion.as_deref() == Some("failure"))
                    {
                        theme.diff_removed_text()
                    } else {
                        theme.diff_hunk_text()
                    };

                    ui.label(
                        RichText::new(format!("{check_icon} {} checks", self.check_runs.len()))
                            .color(check_color)
                            .font(typography::proportional(typography::SM)),
                    );

                    ui.add_space(12.0);

                    // File count and stats
                    if pr.deletions > 0 {
                        diff_widget::render_stat_badge(ui, pr.deletions as usize, false, theme);
                        ui.add_space(4.0);
                    }
                    if pr.additions > 0 {
                        diff_widget::render_stat_badge(ui, pr.additions as usize, true, theme);
                        ui.add_space(8.0);
                    }
                    ui.label(
                        RichText::new(format!("{} files", pr.changed_files))
                            .color(theme.text_secondary())
                            .font(typography::proportional(typography::SM)),
                    );

                    ui.add_space(12.0);

                    // Branch info
                    ui.label(
                        RichText::new(format!(
                            "{} {} {}",
                            pr.head.ref_name,
                            egui_nerdfonts::regular::ARROW_RIGHT,
                            pr.base.ref_name
                        ))
                        .color(theme.text_secondary())
                        .font(typography::monospace(typography::XS)),
                    );
                });
            }
        });
        ui.add_space(6.0);

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        // Tab bar with review actions on the right
        ui.add_space(4.0);
        let mut clicked_event: Option<ReviewEvent> = None;
        ui.horizontal(|ui| {
            ui.add_space(12.0);
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

        // Handle review submission outside the layout closure
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

                let view_mode = if self.split_view { "split" } else { "unified" };
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
            // Diff content area
            ui.allocate_ui_with_layout(
                egui::vec2(diff_width, available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.show_diff_view(ui);
                },
            );

            // Vertical separator
            let sep_rect = ui.available_rect_before_wrap();
            ui.painter().vline(
                sep_rect.left(),
                sep_rect.y_range(),
                egui::Stroke::new(1.0, theme.border_subtle()),
            );

            // File panel
            ui.allocate_ui_with_layout(
                egui::vec2(file_panel_width, available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.show_file_panel(ui, theme);
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
        });
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .id_salt("pr_file_panel")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, file) in self.pr_files.iter().enumerate() {
                    let is_selected = i == self.selected_file_index;
                    let row_height = 28.0;

                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    let is_hovered = response.hovered();

                    // Background
                    if is_selected {
                        ui.painter().rect_filled(
                            rect,
                            3.0,
                            theme.accent_primary().gamma_multiply(0.12),
                        );
                        // Left accent bar
                        let bar_rect =
                            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
                        ui.painter()
                            .rect_filled(bar_rect, 2.0, theme.accent_primary());
                    } else if is_hovered {
                        ui.painter().rect_filled(
                            rect,
                            3.0,
                            theme.text_primary().gamma_multiply(0.04),
                        );
                    }

                    let content_rect = rect.shrink2(egui::vec2(8.0, 0.0));

                    // File icon
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

                    // Extract just the filename
                    let filename = std::path::Path::new(&file.filename)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&file.filename);

                    let mut cursor_x = content_rect.left() + 4.0;

                    // Icon
                    let icon_galley = ui.painter().layout_no_wrap(
                        icon.to_string(),
                        typography::proportional(typography::XS),
                        icon_color,
                    );
                    ui.painter().galley(
                        egui::pos2(
                            cursor_x,
                            content_rect.center().y - icon_galley.size().y / 2.0,
                        ),
                        icon_galley.clone(),
                        icon_color,
                    );
                    cursor_x += icon_galley.size().x + 4.0;

                    // Filename
                    let name_color = if is_selected {
                        theme.text_primary()
                    } else {
                        theme.text_primary().gamma_multiply(0.85)
                    };
                    let max_name_width =
                        (content_rect.width() - (cursor_x - content_rect.left()) - 40.0).max(20.0);
                    let name_galley = ui.painter().layout(
                        filename.to_string(),
                        typography::monospace(typography::XS),
                        name_color,
                        max_name_width,
                    );
                    ui.painter().galley(
                        egui::pos2(
                            cursor_x,
                            content_rect.center().y - name_galley.size().y / 2.0,
                        ),
                        name_galley,
                        name_color,
                    );

                    // Stats on right
                    let stats_x = content_rect.right() - 4.0;
                    let mut right_x = stats_x;

                    if file.deletions > 0 {
                        let del_text = format!("-{}", file.deletions);
                        let del_galley = ui.painter().layout_no_wrap(
                            del_text,
                            typography::monospace(typography::XS),
                            theme.diff_removed_gutter(),
                        );
                        right_x -= del_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(
                                right_x,
                                content_rect.center().y - del_galley.size().y / 2.0,
                            ),
                            del_galley,
                            theme.diff_removed_gutter(),
                        );
                        right_x -= 3.0;
                    }

                    if file.additions > 0 {
                        let add_text = format!("+{}", file.additions);
                        let add_galley = ui.painter().layout_no_wrap(
                            add_text,
                            typography::monospace(typography::XS),
                            theme.diff_added_gutter(),
                        );
                        right_x -= add_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(
                                right_x,
                                content_rect.center().y - add_galley.size().y / 2.0,
                            ),
                            add_galley,
                            theme.diff_added_gutter(),
                        );
                    }

                    if response.clicked() {
                        self.selected_file_index = i;
                    }

                    // Tooltip with full path
                    response.on_hover_text(&file.filename);
                }
            });
    }

    /// Render the Conversation tab.
    fn show_conversation_tab(&self, ui: &mut egui::Ui, theme: AppTheme) {
        if self.issue_comments.is_empty() && self.review_comments.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No comments yet")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::MD)),
                );
            });
            return;
        }

        // Show PR description first
        if let Some(pr) = &self.current_pr {
            if let Some(body) = &pr.body {
                if !body.is_empty() {
                    egui::ScrollArea::vertical()
                        .id_salt("pr_conversation")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // PR description
                            render_comment(ui, theme, &pr.user.login, &pr.created_at, body);

                            // Issue comments
                            for comment in &self.issue_comments {
                                render_comment(
                                    ui,
                                    theme,
                                    &comment.user.login,
                                    &comment.created_at,
                                    &comment.body,
                                );
                            }

                            // Review comments (inline)
                            for comment in &self.review_comments {
                                if let Some(path) = &comment.path {
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(16.0);
                                        ui.label(
                                            RichText::new(format!(
                                                "{} {}:{}",
                                                egui_nerdfonts::regular::FILE_CODE,
                                                path,
                                                comment.line.unwrap_or(0)
                                            ))
                                            .color(theme.text_secondary())
                                            .font(typography::monospace(typography::XS)),
                                        );
                                    });
                                }
                                render_comment(
                                    ui,
                                    theme,
                                    &comment.user.login,
                                    &comment.created_at,
                                    &comment.body,
                                );
                            }
                        });
                    return;
                }
            }
        }

        // If no PR body, just show comments
        egui::ScrollArea::vertical()
            .id_salt("pr_conversation")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for comment in &self.issue_comments {
                    render_comment(
                        ui,
                        theme,
                        &comment.user.login,
                        &comment.created_at,
                        &comment.body,
                    );
                }
                for comment in &self.review_comments {
                    render_comment(
                        ui,
                        theme,
                        &comment.user.login,
                        &comment.created_at,
                        &comment.body,
                    );
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

            // Body
            ui.label(
                RichText::new(body)
                    .color(theme.text_primary().gamma_multiply(0.9))
                    .font(typography::proportional(typography::SM)),
            );
        });
}
