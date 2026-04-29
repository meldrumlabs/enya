//! PR detail view — shows file list, conversation, and checks tabs.

use egui::RichText;
use rustc_hash::FxHashSet;

use crate::git::api::{
    CommentThread, DraftComment, MergeMethod, PrComment, PrFile, ReviewEvent, relative_time,
};
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
        /// Path used as key for expansion state.
        path: String,
        /// Whether this file's comment list is expanded below.
        threads_expanded: bool,
        /// Whether any threads exist to expand.
        has_threads: bool,
    },
    /// A comment thread appearing under an expanded File row.
    Thread {
        path: String,
        line: usize,
        depth: usize,
        author: String,
        snippet: String,
        count: usize,
        unseen: bool,
        resolved: bool,
    },
}

/// Build a flattened list of tree rows from PR files, respecting collapsed directories.
#[allow(clippy::too_many_arguments)]
fn build_file_tree_rows(
    pr_files: &[PrFile],
    collapsed_dirs: &FxHashSet<String>,
    review_comments: &[PrComment],
    draft_comments: &[DraftComment],
    seen_comment_ids: &rustc_hash::FxHashSet<u64>,
    reviewed_files: &rustc_hash::FxHashSet<String>,
    threads: &[CommentThread],
    expanded_comment_files: &FxHashSet<String>,
    resolved_thread_lines: &FxHashSet<(String, usize)>,
    show_only_unresolved: bool,
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

        let has_threads = threads.iter().any(|t| t.path == *filename);
        let threads_expanded = expanded_comment_files.contains(filename);

        rows.push(FileTreeRow::File {
            file_index: *file_index,
            name: file_name.to_string(),
            depth: dir_parts.len(),
            comment_count: review_count + draft_count,
            unseen_count,
            reviewed: reviewed_files.contains(filename),
            path: filename.clone(),
            threads_expanded,
            has_threads,
        });

        if has_threads && threads_expanded {
            let thread_depth = dir_parts.len() + 1;
            let mut file_threads: Vec<&CommentThread> =
                threads.iter().filter(|t| t.path == *filename).collect();
            file_threads.sort_by_key(|t| t.line);
            for thread in file_threads {
                let resolved = resolved_thread_lines.contains(&(thread.path.clone(), thread.line));
                if show_only_unresolved && resolved {
                    continue;
                }
                let first = thread.comments.first();
                let author = first.map(|c| c.user.login.clone()).unwrap_or_default();
                let snippet = first.map(|c| snippet_of(&c.body)).unwrap_or_default();
                let unseen = thread
                    .comments
                    .iter()
                    .any(|c| !seen_comment_ids.contains(&c.id));
                rows.push(FileTreeRow::Thread {
                    path: thread.path.clone(),
                    line: thread.line,
                    depth: thread_depth,
                    author,
                    snippet,
                    count: thread.comments.len(),
                    unseen,
                    resolved,
                });
            }
        }
    }

    rows
}

/// Produce a one-line snippet from a comment body for display in the tree.
fn snippet_of(body: &str) -> String {
    let trimmed = body.trim();
    let first_line = trimmed.lines().next().unwrap_or("");
    let cleaned = first_line.trim_start_matches(['>', '#', '-', '*']).trim();
    const MAX: usize = 80;
    if cleaned.chars().count() <= MAX {
        cleaned.to_string()
    } else {
        let truncated: String = cleaned.chars().take(MAX).collect();
        format!("{truncated}…")
    }
}

impl PrReviewPane {
    /// Aggregate CI check status for the header strip.
    fn aggregate_ci_status(&self) -> (&'static str, egui::Color32) {
        let theme = self.theme;
        if self.check_runs.is_empty() {
            return ("\u{2014}", theme.text_secondary()); // em dash
        }
        let all_success = self
            .check_runs
            .iter()
            .all(|c| c.conclusion.as_deref() == Some("success"));
        let any_failure = self
            .check_runs
            .iter()
            .any(|c| matches!(c.conclusion.as_deref(), Some("failure") | Some("cancelled")));
        if all_success {
            ("\u{2713}", theme.diff_added_text()) // checkmark
        } else if any_failure {
            ("\u{2717}", theme.diff_removed_text()) // X
        } else {
            ("\u{25CB}", theme.diff_hunk_text()) // circle (pending)
        }
    }

    /// Compute aggregate review state from the current PR's reviews.
    fn compute_review_state(&self) -> Option<super::ReviewState> {
        if self.reviews.is_empty() {
            return None;
        }
        let mut per_user: rustc_hash::FxHashMap<&str, &str> = rustc_hash::FxHashMap::default();
        for r in &self.reviews {
            match r.state.as_str() {
                "APPROVED" | "CHANGES_REQUESTED" => {
                    per_user.insert(&r.user.login, &r.state);
                }
                _ => {}
            }
        }
        if per_user.is_empty() {
            return None;
        }
        if per_user.values().any(|s| *s == "CHANGES_REQUESTED") {
            Some(super::ReviewState::ChangesRequested)
        } else {
            Some(super::ReviewState::Approved)
        }
    }

    /// Render the PR detail view.
    pub(super) fn show_detail_view(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let total_width = ui.available_width();
        let narrow = total_width < 600.0;

        // ── Row 1: PR identity + context ────────────────────────────────
        ui.add_space(4.0);
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

            if let Some(pr) = &self.current_pr {
                // Right-side metadata (title is already in the pane tab label)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);

                    // Open in GitHub
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

                    ui.add_space(4.0);

                    // Review state
                    // Review state (only show when there's an actual review)
                    let review_state = self.compute_review_state();
                    if let Some(state) = review_state {
                        let (review_label, review_color) = match state {
                            super::ReviewState::Approved => ("Approved", theme.diff_added_text()),
                            super::ReviewState::ChangesRequested => {
                                ("Changes", theme.diff_removed_text())
                            }
                        };
                        ui.label(
                            RichText::new(review_label)
                                .color(review_color)
                                .font(typography::proportional(typography::XS)),
                        );
                        ui.label(
                            RichText::new("\u{b7}")
                                .color(theme.text_secondary().gamma_multiply(0.5))
                                .font(typography::proportional(typography::XS)),
                        );
                    }

                    // CI status
                    let (ci_icon, ci_color) = self.aggregate_ci_status();
                    ui.label(
                        RichText::new(format!("{ci_icon} CI"))
                            .color(ci_color)
                            .font(typography::proportional(typography::XS)),
                    );

                    ui.add_space(8.0);

                    // Branch info
                    ui.label(
                        RichText::new(format!(
                            "{} \u{2192} {}",
                            pr.head.ref_name, pr.base.ref_name
                        ))
                        .color(theme.text_secondary())
                        .font(typography::monospace(typography::XS)),
                    );

                    ui.add_space(4.0);

                    // Author
                    ui.label(
                        RichText::new(&pr.user.login)
                            .color(theme.text_secondary())
                            .font(typography::proportional(typography::XS)),
                    );
                });
            }
        });
        ui.add_space(2.0);

        // ── Row 2: Tabs + progress + actions ────────────────────────────
        let mut clicked_event: Option<ReviewEvent> = None;
        let mut submit_btn_anchor = egui::Rect::NOTHING;
        let mut merge_btn_anchor = egui::Rect::NOTHING;
        let mut do_merge = false;

        // Compute tab badges
        let files_badge = {
            let count = self.review_comments.len() + self.draft_comments.len();
            if count > 0 {
                Some(format!("{count}"))
            } else {
                None
            }
        };
        let conv_badge = if !self.issue_comments.is_empty() {
            Some(format!("{}", self.issue_comments.len()))
        } else {
            None
        };
        let checks_badge = if !self.check_runs.is_empty() {
            let (icon, _) = self.aggregate_ci_status();
            Some(icon.to_string())
        } else {
            None
        };

        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Tabs with badges
            render_tab_with_badge(
                ui,
                theme,
                "Files",
                files_badge.as_deref(),
                DetailTab::Files,
                &mut self.active_tab,
            );
            ui.add_space(8.0);
            render_tab_with_badge(
                ui,
                theme,
                "Conversation",
                conv_badge.as_deref(),
                DetailTab::Conversation,
                &mut self.active_tab,
            );
            ui.add_space(8.0);
            render_tab_with_badge(
                ui,
                theme,
                "Checks",
                checks_badge.as_deref(),
                DetailTab::Checks,
                &mut self.active_tab,
            );

            // Right side: progress bar + action buttons
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);

                let can_submit = self.token.is_some() && !self.submitting_review;

                // ── Merge button ──
                let is_open = self
                    .current_pr
                    .as_ref()
                    .is_some_and(|pr| pr.state == "open");
                let can_merge = can_submit
                    && is_open
                    && !self.merging
                    && self
                        .current_pr
                        .as_ref()
                        .and_then(|pr| pr.mergeable)
                        .unwrap_or(false);
                let merge_btn = ui.add_enabled(
                    can_merge,
                    egui::Button::new(
                        RichText::new(format!("Merge {}", egui_nerdfonts::regular::CHEVRON_DOWN))
                            .size(typography::XS)
                            .color(if can_merge {
                                theme.accent_primary()
                            } else {
                                theme.text_secondary().gamma_multiply(0.5)
                            }),
                    )
                    .fill(if can_merge {
                        if self.merge_popup_open {
                            theme.accent_primary().gamma_multiply(0.2)
                        } else {
                            theme.accent_primary().gamma_multiply(0.12)
                        }
                    } else {
                        theme.bg_elevated()
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if can_merge {
                            theme.accent_primary().gamma_multiply(0.3)
                        } else {
                            theme.border_subtle()
                        },
                    ))
                    .corner_radius(4.0),
                );
                if merge_btn.clicked() {
                    self.merge_popup_open = !self.merge_popup_open;
                }
                merge_btn_anchor = merge_btn.rect;

                ui.add_space(4.0);

                // ── Submit Review button (consolidated) ──
                let draft_count = self.draft_comments.len();
                let submit_label = if narrow {
                    if draft_count > 0 {
                        format!(
                            "Review ({draft_count}) {}",
                            egui_nerdfonts::regular::CHEVRON_DOWN
                        )
                    } else {
                        format!("Review {}", egui_nerdfonts::regular::CHEVRON_DOWN)
                    }
                } else if draft_count > 0 {
                    format!(
                        "Submit Review ({draft_count}) {}",
                        egui_nerdfonts::regular::CHEVRON_DOWN
                    )
                } else {
                    format!("Submit Review {}", egui_nerdfonts::regular::CHEVRON_DOWN)
                };

                // Flash animation on submit button
                let flash_alpha = self
                    .flash_start
                    .map(|start| {
                        let elapsed = crate::util::Instant::now()
                            .duration_since(start)
                            .as_secs_f32();
                        if elapsed > 1.5 {
                            0.0
                        } else {
                            (1.0 - elapsed / 1.5).powi(2)
                        }
                    })
                    .unwrap_or(0.0);

                if flash_alpha > 0.0 {
                    ui.ctx().request_repaint();
                } else if self.flash_start.is_some() {
                    self.flash_start = None;
                }

                let submit_fill = if flash_alpha > 0.0 {
                    if self.flash_is_success {
                        theme.diff_added_bg().gamma_multiply(flash_alpha)
                    } else {
                        theme.diff_removed_bg().gamma_multiply(flash_alpha)
                    }
                } else if self.submit_panel_open {
                    theme.accent_primary().gamma_multiply(0.15)
                } else {
                    theme.bg_elevated()
                };

                let submit_btn = ui.add_enabled(
                    can_submit,
                    egui::Button::new(RichText::new(submit_label).size(typography::XS).color(
                        if can_submit {
                            theme.text_primary()
                        } else {
                            theme.text_secondary().gamma_multiply(0.5)
                        },
                    ))
                    .fill(submit_fill)
                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                    .corner_radius(4.0),
                );
                if submit_btn.clicked() {
                    self.submit_panel_open = !self.submit_panel_open;
                }
                submit_btn_anchor = submit_btn.rect;

                ui.add_space(4.0);

                // ── Organize button ──
                if !narrow {
                    const BRAILLE_FRAMES: [char; 10] =
                        ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

                    let is_loading = matches!(
                        self.walkthrough_state,
                        super::walkthrough::WalkthroughState::Loading
                    );
                    let is_ready = matches!(
                        self.walkthrough_state,
                        super::walkthrough::WalkthroughState::Ready(_)
                    );

                    let label = if is_loading {
                        let time = ui.ctx().input(|i| i.time);
                        let frame = ((time * 10.0) as usize) % BRAILLE_FRAMES.len();
                        ui.ctx().request_repaint();
                        format!("{} Organizing...", BRAILLE_FRAMES[frame])
                    } else {
                        "Organize".to_string()
                    };

                    let organize_btn = ui.add_enabled(
                        !is_loading && !self.file_diffs.is_empty(),
                        egui::Button::new(RichText::new(label).size(typography::XS).color(
                            if is_loading || is_ready {
                                theme.accent_primary()
                            } else {
                                theme.text_primary()
                            },
                        ))
                        .fill(if is_ready {
                            theme.accent_primary().gamma_multiply(0.12)
                        } else {
                            theme.bg_elevated()
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if is_ready {
                                theme.accent_primary().gamma_multiply(0.3)
                            } else {
                                theme.border_subtle()
                            },
                        ))
                        .corner_radius(4.0),
                    );
                    if organize_btn.clicked() {
                        if is_ready {
                            self.walkthrough_state = super::walkthrough::WalkthroughState::Idle;
                        } else {
                            self.request_walkthrough();
                        }
                    }
                }

                // Submitting / merging indicator
                if self.submitting_review || self.merging {
                    ui.add_space(4.0);
                    ui.spinner();
                }

                // ── Review progress bar ──
                let reviewed_count = self.reviewed_files.len();
                let total_files = self.pr_files.len();
                if total_files > 0 {
                    ui.add_space(8.0);
                    let fraction = reviewed_count as f32 / total_files as f32;
                    let bar_width = if narrow { 40.0 } else { 60.0 };
                    let bar_height = 4.0;
                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::vec2(bar_width, bar_height),
                        egui::Sense::hover(),
                    );
                    // Track
                    ui.painter()
                        .rect_filled(bar_rect, 2.0, theme.border_subtle());
                    // Fill
                    let fill_width = (bar_width * fraction).max(0.0);
                    let fill_rect =
                        egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_width, bar_height));
                    let fill_color = if reviewed_count == total_files {
                        theme.diff_added_gutter()
                    } else {
                        theme.accent_primary()
                    };
                    ui.painter().rect_filled(fill_rect, 2.0, fill_color);

                    if !narrow {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("{reviewed_count}/{total_files}"))
                                .color(if reviewed_count == total_files {
                                    theme.diff_added_gutter()
                                } else {
                                    theme.text_secondary()
                                })
                                .font(typography::proportional(typography::XS)),
                        );
                    }
                }
            });
        });

        // ── Submit Review panel (floating popup) ──
        if self.submit_panel_open {
            let popup_id = ui.id().with("submit_review_panel");
            let popup_pos = egui::pos2(
                submit_btn_anchor.right() - 320.0,
                submit_btn_anchor.bottom() + 4.0,
            );
            let area_resp = egui::Area::new(popup_id)
                .order(egui::Order::Tooltip)
                .fixed_pos(popup_pos)
                .show(ui.ctx(), |ui| {
                    crate::components::util::OverlayStyle::elevated_card(theme)
                        .frame()
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(300.0);

                            ui.label(
                                RichText::new("Finish your review")
                                    .color(theme.text_primary())
                                    .font(typography::proportional(typography::SM))
                                    .strong(),
                            );

                            let draft_count = self.draft_comments.len();
                            if draft_count > 0 {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{draft_count} pending comment{}",
                                        if draft_count == 1 { "" } else { "s" }
                                    ))
                                    .color(theme.text_secondary())
                                    .font(typography::proportional(typography::XS)),
                                );
                            }

                            ui.add_space(8.0);
                            ui.add(
                                egui::TextEdit::multiline(&mut self.draft_body)
                                    .hint_text("Leave a comment")
                                    .desired_rows(4)
                                    .desired_width(296.0)
                                    .font(typography::proportional(typography::SM)),
                            );

                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                // Approve (green)
                                let approve_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new("Approve")
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
                                if approve_btn.clicked() {
                                    clicked_event = Some(ReviewEvent::Approve);
                                    self.submit_panel_open = false;
                                }

                                // Comment (neutral)
                                let has_content =
                                    !self.draft_comments.is_empty() || !self.draft_body.is_empty();
                                let comment_btn = ui.add_enabled(
                                    has_content,
                                    egui::Button::new(
                                        RichText::new("Comment").size(typography::XS).color(
                                            if has_content {
                                                theme.text_primary()
                                            } else {
                                                theme.text_secondary().gamma_multiply(0.5)
                                            },
                                        ),
                                    )
                                    .fill(theme.bg_elevated())
                                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                                    .corner_radius(4.0),
                                );
                                if comment_btn.clicked() {
                                    clicked_event = Some(ReviewEvent::Comment);
                                    self.submit_panel_open = false;
                                }

                                // Request Changes (red)
                                let rc_btn = ui.add_enabled(
                                    has_content,
                                    egui::Button::new(
                                        RichText::new("Request Changes")
                                            .size(typography::XS)
                                            .color(if has_content {
                                                theme.diff_removed_text()
                                            } else {
                                                theme.text_secondary().gamma_multiply(0.5)
                                            }),
                                    )
                                    .fill(if has_content {
                                        theme.diff_removed_bg()
                                    } else {
                                        theme.bg_elevated()
                                    })
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        if has_content {
                                            theme.diff_removed_gutter().gamma_multiply(0.3)
                                        } else {
                                            theme.border_subtle()
                                        },
                                    ))
                                    .corner_radius(4.0),
                                );
                                if rc_btn.clicked() {
                                    clicked_event = Some(ReviewEvent::RequestChanges);
                                    self.submit_panel_open = false;
                                }
                            });
                        });
                });

            // Close on outside click
            let popup_rect = area_resp.response.rect;
            if ui.input(|i| i.pointer.any_click())
                && !popup_rect.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
                && !submit_btn_anchor
                    .contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
            {
                self.submit_panel_open = false;
            }
        }

        // Merge popup (floating below the Merge button)
        if self.merge_popup_open {
            let popup_id = ui.id().with("merge_popup");
            let popup_pos = egui::pos2(
                merge_btn_anchor.right() - 260.0,
                merge_btn_anchor.bottom() + 4.0,
            );
            let area_resp = egui::Area::new(popup_id)
                .order(egui::Order::Tooltip)
                .fixed_pos(popup_pos)
                .show(ui.ctx(), |ui| {
                    crate::components::util::OverlayStyle::elevated_card(theme)
                        .frame()
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(240.0);
                            ui.label(
                                RichText::new("Merge pull request")
                                    .color(theme.text_primary())
                                    .font(typography::proportional(typography::SM))
                                    .strong(),
                            );
                            ui.add_space(8.0);

                            // Strategy radio buttons
                            for method in
                                [MergeMethod::Squash, MergeMethod::Merge, MergeMethod::Rebase]
                            {
                                let selected = self.merge_method == method;
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 28.0),
                                    egui::Sense::click(),
                                );

                                if response.hovered() || selected {
                                    ui.painter().rect_filled(
                                        rect,
                                        4.0,
                                        if selected {
                                            theme.accent_primary().gamma_multiply(0.12)
                                        } else {
                                            theme.text_primary().gamma_multiply(0.04)
                                        },
                                    );
                                }

                                // Radio circle
                                let circle_center = egui::pos2(rect.left() + 12.0, rect.center().y);
                                ui.painter().circle_stroke(
                                    circle_center,
                                    5.0,
                                    egui::Stroke::new(
                                        1.0,
                                        if selected {
                                            theme.accent_primary()
                                        } else {
                                            theme.text_secondary()
                                        },
                                    ),
                                );
                                if selected {
                                    ui.painter().circle_filled(
                                        circle_center,
                                        3.0,
                                        theme.accent_primary(),
                                    );
                                }

                                // Label
                                let label_galley = ui.painter().layout_no_wrap(
                                    method.label().to_string(),
                                    typography::proportional(typography::XS),
                                    if selected {
                                        theme.text_primary()
                                    } else {
                                        theme.text_secondary()
                                    },
                                );
                                ui.painter().galley(
                                    egui::pos2(
                                        rect.left() + 24.0,
                                        rect.center().y - label_galley.size().y / 2.0,
                                    ),
                                    label_galley,
                                    theme.text_primary(),
                                );

                                if response.clicked() {
                                    self.merge_method = method;
                                }
                            }

                            ui.add_space(8.0);

                            // Confirm merge button
                            ui.horizontal(|ui| {
                                let confirm_label = match self.merge_method {
                                    MergeMethod::Squash => "Squash and merge",
                                    MergeMethod::Merge => "Confirm merge",
                                    MergeMethod::Rebase => "Rebase and merge",
                                };
                                let confirm_btn = ui.add(
                                    egui::Button::new(
                                        RichText::new(confirm_label)
                                            .size(typography::XS)
                                            .color(theme.accent_primary()),
                                    )
                                    .fill(theme.accent_primary().gamma_multiply(0.15))
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        theme.accent_primary().gamma_multiply(0.3),
                                    ))
                                    .corner_radius(4.0),
                                );
                                if confirm_btn.clicked() {
                                    do_merge = true;
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
                                    self.merge_popup_open = false;
                                }
                            });

                            // Merging spinner
                            if self.merging {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("Merging...")
                                            .color(theme.text_secondary())
                                            .font(typography::proportional(typography::XS)),
                                    );
                                });
                            }
                        });
                });

            // Close popup when clicking outside
            let popup_rect = area_resp.response.rect;
            if ui.input(|i| i.pointer.any_click())
                && !popup_rect.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
                && !merge_btn_anchor
                    .contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
            {
                self.merge_popup_open = false;
            }
        }

        // Handle deferred actions outside closures
        if do_merge {
            self.merge_pull();
        }
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

        // ── Collapsible PR description card ──
        if let Some(pr) = &self.current_pr {
            if let Some(body) = &pr.body {
                if !body.is_empty() {
                    ui.add_space(2.0);
                    egui::Frame::new()
                        .fill(theme.bg_elevated().gamma_multiply(0.4))
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(12, 6))
                        .outer_margin(egui::Margin::symmetric(4, 0))
                        .show(ui, |ui| {
                            // Header row: allocate clickable rect, then paint contents
                            let row_height = 18.0;
                            let (header_rect, header_response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), row_height),
                                egui::Sense::click(),
                            );

                            // Paint header contents manually on the allocated rect
                            let mut hx = header_rect.left();
                            let cy = header_rect.center().y;

                            let chevron = if self.description_collapsed {
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
                                egui::pos2(hx, cy - chev_galley.size().y / 2.0),
                                chev_galley.clone(),
                                theme.text_secondary(),
                            );
                            hx += chev_galley.size().x + 4.0;

                            let desc_galley = ui.painter().layout_no_wrap(
                                "Description".to_string(),
                                typography::proportional(typography::XS),
                                theme.text_secondary(),
                            );
                            ui.painter().galley(
                                egui::pos2(hx, cy - desc_galley.size().y / 2.0),
                                desc_galley.clone(),
                                theme.text_secondary(),
                            );
                            hx += desc_galley.size().x + 6.0;

                            let author_galley = ui.painter().layout_no_wrap(
                                pr.user.login.clone(),
                                typography::proportional(typography::XS),
                                theme.text_secondary().gamma_multiply(0.7),
                            );
                            ui.painter().galley(
                                egui::pos2(hx, cy - author_galley.size().y / 2.0),
                                author_galley.clone(),
                                theme.text_secondary().gamma_multiply(0.7),
                            );
                            hx += author_galley.size().x + 4.0;

                            let time_galley = ui.painter().layout_no_wrap(
                                format!("\u{b7} {}", relative_time(&pr.created_at)),
                                typography::proportional(typography::XS),
                                theme.text_secondary().gamma_multiply(0.5),
                            );
                            ui.painter().galley(
                                egui::pos2(hx, cy - time_galley.size().y / 2.0),
                                time_galley,
                                theme.text_secondary().gamma_multiply(0.5),
                            );

                            if header_response.clicked() {
                                self.description_collapsed = !self.description_collapsed;
                            }
                            if header_response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            // Body (only if expanded)
                            if !self.description_collapsed {
                                ui.add_space(4.0);
                                let max_h = (ui.available_height() * 0.3).clamp(60.0, 200.0);
                                egui::ScrollArea::vertical()
                                    .id_salt("pr_desc_main")
                                    .max_height(max_h)
                                    .scroll_bar_visibility(
                                        egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                    )
                                    .show(ui, |ui| {
                                        crate::components::overlay::markdown_renderer::render_markdown(
                                            ui, body, theme,
                                        );
                                    });
                            }
                        });
                    ui.add_space(2.0);
                }
            }
        }

        // ── AI walkthrough summary banner ──
        self.show_walkthrough_banner(ui, theme);

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

        // ── Auto-surface review submission when all files reviewed ──
        let all_reviewed = !self.pr_files.is_empty()
            && self.reviewed_files.len() == self.pr_files.len()
            && !self.auto_surface_dismissed
            && !self.submit_panel_open;
        if all_reviewed {
            ui.add_space(4.0);
            egui::Frame::new()
                .fill(theme.diff_added_bg().gamma_multiply(0.5))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(12, 6))
                .outer_margin(egui::Margin::symmetric(8, 0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} All files reviewed",
                                egui_nerdfonts::regular::CHECK_CIRCLE,
                            ))
                            .color(theme.diff_added_text())
                            .font(typography::proportional(typography::SM)),
                        );
                        ui.add_space(8.0);
                        let submit_btn = ui.add(
                            egui::Button::new(
                                RichText::new("Submit Review")
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
                            self.submit_panel_open = true;
                        }
                        let dismiss_btn = ui.add(
                            egui::Button::new(
                                RichText::new("Dismiss")
                                    .size(typography::XS)
                                    .color(theme.text_secondary()),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        );
                        if dismiss_btn.clicked() {
                            self.auto_surface_dismissed = true;
                        }
                    });
                });
        }

        // Keybinding hints footer
        self.render_keybinding_footer(ui, theme);
    }

    /// Render keybinding hints at the bottom of the detail view.
    fn render_keybinding_footer(&self, ui: &mut egui::Ui, theme: AppTheme) {
        // Elevated footer bar
        let footer_rect = egui::Rect::from_min_size(
            ui.cursor().left_top(),
            egui::vec2(ui.available_width(), 30.0),
        );
        ui.painter()
            .rect_filled(footer_rect, 0.0, theme.bg_elevated().gamma_multiply(0.5));
        ui.painter().hline(
            footer_rect.x_range(),
            footer_rect.top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 30.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(16.0);

                // Current file name only — the full path is already shown in the
                // diff header toolbar, so repeating it here risks overlapping the
                // keyboard hints on narrow panes.
                if let Some(file_diff) = self.file_diffs.get(self.selected_file_index) {
                    let name = file_diff
                        .path
                        .rfind('/')
                        .map_or(file_diff.path.as_str(), |pos| &file_diff.path[pos + 1..]);
                    ui.label(
                        RichText::new(name)
                            .color(theme.text_secondary())
                            .font(typography::monospace(typography::XS)),
                    );
                }

                // Right-side keybinding hints with grouped styling
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(16.0);

                    let key_color = theme.text_secondary().gamma_multiply(0.5);
                    let sep_color = theme.text_secondary().gamma_multiply(0.25);
                    let accent_key = theme.accent_primary().gamma_multiply(0.6);
                    let key_font = typography::monospace(typography::XS);
                    let label_font = typography::proportional(typography::XS);

                    // Esc back (accent — primary escape action)
                    ui.label(
                        RichText::new("back")
                            .color(key_color)
                            .font(label_font.clone()),
                    );
                    ui.label(
                        RichText::new("Esc")
                            .color(accent_key)
                            .font(key_font.clone()),
                    );

                    ui.label(
                        RichText::new("\u{2022}")
                            .color(sep_color)
                            .font(label_font.clone()),
                    );

                    // View mode
                    let view_mode = if self.diff_renderer.split_view() {
                        "split"
                    } else {
                        "stacked"
                    };
                    ui.label(
                        RichText::new(view_mode)
                            .color(key_color)
                            .font(label_font.clone()),
                    );
                    ui.label(RichText::new("s").color(key_color).font(key_font.clone()));

                    ui.label(
                        RichText::new("\u{2022}")
                            .color(sep_color)
                            .font(label_font.clone()),
                    );

                    // Scroll
                    ui.label(
                        RichText::new("top/bottom")
                            .color(key_color)
                            .font(label_font.clone()),
                    );
                    ui.label(
                        RichText::new("gg/G")
                            .color(key_color)
                            .font(key_font.clone()),
                    );

                    ui.label(
                        RichText::new("\u{2022}")
                            .color(sep_color)
                            .font(label_font.clone()),
                    );

                    ui.label(
                        RichText::new("scroll")
                            .color(key_color)
                            .font(label_font.clone()),
                    );
                    ui.label(RichText::new("j/k").color(key_color).font(key_font.clone()));

                    // File navigation (only if multiple files)
                    if self.file_diffs.len() > 1 {
                        ui.label(
                            RichText::new("\u{2022}")
                                .color(sep_color)
                                .font(label_font.clone()),
                        );
                        ui.label(
                            RichText::new("files")
                                .color(key_color)
                                .font(label_font.clone()),
                        );
                        ui.label(RichText::new("n/p").color(key_color).font(key_font.clone()));
                    }

                    ui.label(
                        RichText::new("\u{2022}")
                            .color(sep_color)
                            .font(label_font.clone()),
                    );

                    ui.label(
                        RichText::new("viewed")
                            .color(key_color)
                            .font(label_font.clone()),
                    );
                    ui.label(RichText::new("v").color(key_color).font(key_font.clone()));

                    ui.label(
                        RichText::new("\u{2022}")
                            .color(sep_color)
                            .font(label_font.clone()),
                    );

                    ui.label(
                        RichText::new("resolve")
                            .color(key_color)
                            .font(label_font.clone()),
                    );
                    ui.label(RichText::new("R").color(key_color).font(key_font));
                });
            },
        );
    }

    /// Render the Files tab — file list + diff view.
    fn show_files_tab(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let available_height = (ui.available_height() - 50.0).max(100.0);
        let total_width = ui.available_width();

        // Auto-collapse/expand file panel based on pane width.
        if total_width < 700.0 && !self.file_panel_collapsed && !self.file_panel_auto_collapsed {
            self.file_panel_collapsed = true;
            self.file_panel_auto_collapsed = true;
        } else if total_width >= 750.0 && self.file_panel_auto_collapsed {
            self.file_panel_collapsed = false;
            self.file_panel_auto_collapsed = false;
        }

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
                            self.file_panel_auto_collapsed = false;
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
            let min_panel = if total_width < 900.0 { 140.0 } else { 180.0 };
            let file_panel_width = (total_width * 0.28).clamp(min_panel, 320.0);
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
        // ── Elevated file panel header ──
        let header_rect = egui::Rect::from_min_size(
            ui.cursor().left_top(),
            egui::vec2(ui.available_width(), 32.0),
        );
        ui.painter()
            .rect_filled(header_rect, 0.0, theme.bg_elevated().gamma_multiply(0.4));
        ui.painter().hline(
            header_rect.x_range(),
            header_rect.bottom(),
            egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.5)),
        );

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);

                // Folder icon
                ui.label(
                    RichText::new(egui_nerdfonts::regular::FOLDER_OPEN)
                        .color(theme.text_secondary().gamma_multiply(0.7))
                        .size(typography::SM),
                );
                ui.add_space(2.0);

                ui.label(
                    RichText::new("Files")
                        .color(theme.text_primary().gamma_multiply(0.9))
                        .font(typography::proportional(typography::SM))
                        .strong(),
                );
                ui.add_space(4.0);

                // Count in a pill badge
                let count_text = format!("{}", self.pr_files.len());
                let count_galley = ui.painter().layout_no_wrap(
                    count_text.clone(),
                    typography::proportional(typography::XS),
                    theme.text_secondary(),
                );
                let pill_width = count_galley.size().x + 8.0;
                let pill_height = count_galley.size().y + 2.0;
                let (pill_rect, _) = ui
                    .allocate_exact_size(egui::vec2(pill_width, pill_height), egui::Sense::hover());
                ui.painter().rect_filled(
                    pill_rect,
                    pill_height / 2.0,
                    theme.border_subtle().gamma_multiply(0.5),
                );
                ui.painter().galley(
                    egui::pos2(
                        pill_rect.center().x - count_galley.size().x / 2.0,
                        pill_rect.center().y - count_galley.size().y / 2.0,
                    ),
                    count_galley,
                    theme.text_secondary(),
                );

                // Right-aligned controls: Unresolved-only filter + collapse
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
                        self.file_panel_auto_collapsed = false;
                    }
                    collapse_btn.on_hover_text("Hide file tree");

                    // Unresolved-only filter — only show when resolution state exists
                    let has_any_resolved = !self.resolved_thread_lines.is_empty();
                    if has_any_resolved || self.show_only_unresolved {
                        ui.add_space(2.0);
                        let (filter_color, filter_bg) = if self.show_only_unresolved {
                            (
                                theme.accent_primary(),
                                theme.accent_primary().gamma_multiply(0.12),
                            )
                        } else {
                            (theme.text_secondary(), egui::Color32::TRANSPARENT)
                        };
                        let filter_btn = ui.add(
                            egui::Button::new(
                                RichText::new(format!(
                                    "{} Unresolved",
                                    egui_nerdfonts::regular::FILTER_1
                                ))
                                .size(typography::XS)
                                .color(filter_color),
                            )
                            .fill(filter_bg)
                            .corner_radius(3.0)
                            .stroke(egui::Stroke::NONE),
                        );
                        if filter_btn.clicked() {
                            self.show_only_unresolved = !self.show_only_unresolved;
                        }
                        filter_btn.on_hover_text("Show only unresolved comment threads");
                    }
                });
            },
        );

        // ── Additions/deletions summary bar ──
        let total_add: u32 = self.pr_files.iter().map(|f| f.additions).sum();
        let total_del: u32 = self.pr_files.iter().map(|f| f.deletions).sum();
        let total_changes = total_add + total_del;
        if total_changes > 0 {
            ui.add_space(4.0);
            let bar_width = (ui.available_width() - 16.0).max(20.0);
            let bar_height = 3.0;
            let (bar_rect, _) =
                ui.allocate_exact_size(egui::vec2(bar_width, bar_height), egui::Sense::hover());
            let bar_rect =
                egui::Rect::from_min_size(bar_rect.min + egui::vec2(8.0, 0.0), bar_rect.size());

            let add_frac = total_add as f32 / total_changes as f32;
            let add_width = (bar_width * add_frac).max(0.0);

            // Green portion (additions)
            let add_rect =
                egui::Rect::from_min_size(bar_rect.min, egui::vec2(add_width, bar_height));
            ui.painter()
                .rect_filled(add_rect, 1.5, theme.diff_added_gutter());

            // Red portion (deletions)
            let del_rect = egui::Rect::from_min_size(
                egui::pos2(bar_rect.min.x + add_width, bar_rect.min.y),
                egui::vec2((bar_width - add_width).max(0.0), bar_height),
            );
            ui.painter()
                .rect_filled(del_rect, 1.5, theme.diff_removed_gutter());

            ui.add_space(2.0);
        } else {
            ui.add_space(6.0);
        }

        // Use walkthrough grouped view if active, otherwise normal tree
        let walkthrough_groups = self.walkthrough_file_order();
        if let Some(ref groups) = walkthrough_groups {
            let mut clicked_file: Option<usize> = None;
            self.show_walkthrough_file_panel(ui, theme, groups, &mut clicked_file);

            self.file_tree_scroll_to_selected = false;

            if let Some(diff_idx) = clicked_file {
                self.selected_file_index = diff_idx;
                self.mark_current_file_comments_seen();
                self.markdown_preview = false;
                self.markdown_scroll_y = 0.0;
                self.markdown_content_cache = None;
            }
            return;
        }

        // Build flattened tree rows from file paths
        let tree_rows = build_file_tree_rows(
            &self.pr_files,
            &self.collapsed_dirs,
            &self.review_comments,
            &self.draft_comments,
            &self.seen_comment_ids,
            &self.reviewed_files,
            &self.cached_threads,
            &self.expanded_comment_files,
            &self.resolved_thread_lines,
            self.show_only_unresolved,
        );

        let mut toggle_dir: Option<String> = None;
        let mut clicked_file: Option<usize> = None;
        let mut toggle_threads_for: Option<String> = None;
        let mut clicked_thread: Option<(String, usize)> = None;

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
                            path: file_path,
                            threads_expanded,
                            has_threads,
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

                            // File status icon + color coding by change type
                            let (icon, status_color) = match file.status.as_str() {
                                "removed" => (
                                    egui_nerdfonts::regular::FILE_MINUS,
                                    theme.diff_removed_gutter(),
                                ),
                                "added" => (
                                    egui_nerdfonts::regular::FILE_PLUS,
                                    theme.diff_added_gutter(),
                                ),
                                "renamed" => (
                                    egui_nerdfonts::regular::FILE_SYMLINK_FILE,
                                    theme.accent_primary(),
                                ),
                                _ => (egui_nerdfonts::regular::FILE_EDIT, theme.text_secondary()),
                            };
                            let icon_color = if is_selected {
                                theme.accent_primary()
                            } else if *reviewed {
                                status_color.gamma_multiply(0.4)
                            } else {
                                status_color
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

                            // Filename — color-coded by change type, dimmed when reviewed
                            let name_color = if is_selected {
                                theme.text_primary()
                            } else if *reviewed {
                                theme.text_secondary().gamma_multiply(0.45)
                            } else {
                                match file.status.as_str() {
                                    "added" => theme.diff_added_text(),
                                    "removed" => theme.diff_removed_text(),
                                    "renamed" => theme.accent_primary().gamma_multiply(0.85),
                                    _ => theme.text_primary().gamma_multiply(0.85),
                                }
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

                            let mut comment_chip_rect: Option<egui::Rect> = None;
                            if let Some(comment_galley) = comment_galley {
                                let chip_text_width = comment_galley.size().x;
                                let chip_text_height = comment_galley.size().y;
                                right_x -= 6.0;
                                right_x -= chip_text_width;
                                let chip_text_left = right_x;
                                ui.painter().galley(
                                    egui::pos2(
                                        chip_text_left,
                                        rect.center().y - chip_text_height / 2.0,
                                    ),
                                    comment_galley,
                                    theme.accent_primary(),
                                );
                                // Leading chevron (expansion indicator) — always visible when there are threads
                                if *has_threads {
                                    let chev = if *threads_expanded {
                                        egui_nerdfonts::regular::CHEVRON_DOWN
                                    } else {
                                        egui_nerdfonts::regular::CHEVRON_RIGHT
                                    };
                                    let chev_galley = ui.painter().layout_no_wrap(
                                        chev.to_string(),
                                        typography::proportional(typography::XS),
                                        theme.accent_primary().gamma_multiply(0.8),
                                    );
                                    right_x -= chev_galley.size().x + 2.0;
                                    ui.painter().galley(
                                        egui::pos2(
                                            right_x,
                                            rect.center().y - chev_galley.size().y / 2.0,
                                        ),
                                        chev_galley,
                                        theme.accent_primary().gamma_multiply(0.8),
                                    );
                                }
                                let chip_left = right_x - 4.0;
                                let chip_right = chip_text_left + chip_text_width + 4.0;
                                comment_chip_rect = Some(egui::Rect::from_min_max(
                                    egui::pos2(chip_left, rect.top()),
                                    egui::pos2(chip_right.min(rect.right()), rect.bottom()),
                                ));
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
                                let click_pos = response
                                    .interact_pointer_pos()
                                    .or_else(|| ui.ctx().pointer_interact_pos());
                                let clicked_chip = match (comment_chip_rect, click_pos) {
                                    (Some(chip), Some(pos)) if *has_threads => chip.contains(pos),
                                    _ => false,
                                };
                                if clicked_chip {
                                    toggle_threads_for = Some(file_path.clone());
                                } else {
                                    clicked_file = Some(*file_index);
                                }
                            }

                            response.on_hover_text(&file.filename);
                        }
                        FileTreeRow::Thread {
                            path,
                            line,
                            depth,
                            author,
                            snippet,
                            count,
                            unseen,
                            resolved,
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
                            let name_y = rect.center().y;

                            // Comment icon (dimmed when resolved)
                            let icon_color = if *resolved {
                                theme.text_secondary().gamma_multiply(0.45)
                            } else {
                                theme.accent_primary().gamma_multiply(0.85)
                            };
                            let icon_galley = ui.painter().layout_no_wrap(
                                egui_nerdfonts::regular::COMMENT_TEXT.to_string(),
                                typography::proportional(typography::XS),
                                icon_color,
                            );
                            ui.painter().galley(
                                egui::pos2(cx, name_y - icon_galley.size().y / 2.0),
                                icon_galley.clone(),
                                icon_color,
                            );
                            cx += icon_galley.size().x + 4.0;

                            // Line number badge "L42"
                            let line_text = format!("L{line}");
                            let line_color = theme.text_secondary().gamma_multiply(0.7);
                            let line_galley = ui.painter().layout_no_wrap(
                                line_text,
                                typography::monospace(typography::XS),
                                line_color,
                            );
                            ui.painter().galley(
                                egui::pos2(cx, name_y - line_galley.size().y / 2.0),
                                line_galley.clone(),
                                line_color,
                            );
                            cx += line_galley.size().x + 6.0;

                            // Reserve right-side space for count badge & markers
                            let mut right_x = rect.right() - 10.0;
                            if *count > 1 {
                                let count_text = format!("{count}");
                                let count_galley = ui.painter().layout_no_wrap(
                                    count_text,
                                    typography::proportional(typography::XS),
                                    theme.text_secondary().gamma_multiply(0.7),
                                );
                                right_x -= count_galley.size().x;
                                ui.painter().galley(
                                    egui::pos2(right_x, name_y - count_galley.size().y / 2.0),
                                    count_galley,
                                    theme.text_secondary().gamma_multiply(0.7),
                                );
                                right_x -= 6.0;
                            }
                            if *resolved {
                                let check_galley = ui.painter().layout_no_wrap(
                                    egui_nerdfonts::regular::CHECK_CIRCLE.to_string(),
                                    typography::proportional(typography::XS),
                                    theme.diff_added_gutter().gamma_multiply(0.75),
                                );
                                right_x -= check_galley.size().x;
                                ui.painter().galley(
                                    egui::pos2(right_x, name_y - check_galley.size().y / 2.0),
                                    check_galley,
                                    theme.diff_added_gutter().gamma_multiply(0.75),
                                );
                                right_x -= 4.0;
                            } else if *unseen {
                                let dot_center = egui::pos2(right_x - 3.0, name_y);
                                ui.painter()
                                    .circle_filled(dot_center, 3.0, theme.accent_primary());
                                right_x -= 10.0;
                            }

                            // Author + snippet fill remaining width
                            let body_text = if author.is_empty() {
                                snippet.clone()
                            } else {
                                format!("{author} · {snippet}")
                            };
                            let body_color = if *resolved {
                                theme.text_secondary().gamma_multiply(0.55)
                            } else {
                                theme.text_primary().gamma_multiply(0.85)
                            };
                            let body_max = (right_x - cx - 4.0).max(20.0);
                            let body_galley = ui.painter().layout(
                                body_text,
                                typography::proportional(typography::XS),
                                body_color,
                                body_max,
                            );
                            ui.painter().galley(
                                egui::pos2(cx, name_y - body_galley.size().y / 2.0),
                                body_galley,
                                body_color,
                            );

                            if response.clicked() {
                                clicked_thread = Some((path.clone(), *line));
                            }
                            if is_hovered {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
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
        if let Some(path) = toggle_threads_for {
            if !self.expanded_comment_files.remove(&path) {
                self.expanded_comment_files.insert(path);
            }
        }
        if let Some(pr_idx) = clicked_file {
            // Resolve pr_files index to file_diffs index by matching path
            let filename = &self.pr_files[pr_idx].filename;
            if let Some(diff_idx) = self.file_diffs.iter().position(|d| d.path == *filename) {
                self.selected_file_index = diff_idx;
                self.mark_current_file_comments_seen();
                self.markdown_preview = false;
                self.markdown_scroll_y = 0.0;
                self.markdown_content_cache = None;
            }
        }
        if let Some((path, line)) = clicked_thread {
            self.navigate_to_thread(&path, line);
        }
    }

    /// Render the walkthrough-grouped file panel (replaces the normal tree when active).
    fn show_walkthrough_file_panel(
        &self,
        ui: &mut egui::Ui,
        theme: AppTheme,
        groups: &[(&str, Vec<usize>)],
        clicked_file: &mut Option<usize>,
    ) {
        let accent = theme.accent_primary();

        egui::ScrollArea::vertical()
            .id_salt("pr_walkthrough_panel")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (group_label, file_indices) in groups {
                    // ── Group header ──
                    let (header_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 22.0),
                        egui::Sense::hover(),
                    );

                    // Subtle accent bar on the left
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(header_rect.min, egui::vec2(2.0, 22.0)),
                        1.0,
                        accent.gamma_multiply(0.4),
                    );

                    let label_max_width = (header_rect.width() - 12.0).max(20.0);
                    let label_galley = ui.painter().layout(
                        (*group_label).to_string(),
                        typography::proportional(typography::XS),
                        accent.gamma_multiply(0.9),
                        label_max_width,
                    );
                    ui.painter().galley(
                        egui::pos2(
                            header_rect.left() + 8.0,
                            header_rect.center().y - label_galley.size().y / 2.0,
                        ),
                        label_galley,
                        accent.gamma_multiply(0.9),
                    );

                    // ── Files in this group ──
                    for &diff_idx in file_indices {
                        let Some(file_diff) = self.file_diffs.get(diff_idx) else {
                            continue;
                        };
                        let file_path = &file_diff.path;

                        // File row — compact, no annotations (insights are inline in gutter)
                        let row_height = 24.0;
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), row_height),
                            egui::Sense::click(),
                        );

                        let is_selected = self.selected_file_index == diff_idx;
                        let is_hovered = response.hovered();
                        let is_reviewed = self.reviewed_files.contains(file_path);

                        // Background
                        if is_selected {
                            ui.painter()
                                .rect_filled(rect, 3.0, accent.gamma_multiply(0.12));
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height)),
                                2.0,
                                accent,
                            );
                        } else if is_hovered {
                            ui.painter().rect_filled(
                                rect,
                                3.0,
                                theme.text_primary().gamma_multiply(0.04),
                            );
                        }

                        let indent = 10.0;
                        let mut cx = rect.left() + indent;
                        let name_y = rect.center().y - 6.0;

                        // File icon
                        let pr_file = self.pr_files.iter().find(|f| f.filename == *file_path);
                        let icon = match pr_file.map(|f| f.status.as_str()) {
                            Some("removed") => egui_nerdfonts::regular::FILE_MINUS,
                            Some("added") => egui_nerdfonts::regular::FILE_PLUS,
                            _ => egui_nerdfonts::regular::FILE_EDIT,
                        };
                        let icon_color = if is_selected {
                            accent
                        } else if is_reviewed {
                            theme.text_secondary().gamma_multiply(0.4)
                        } else {
                            theme.text_secondary()
                        };
                        let icon_galley = ui.painter().layout_no_wrap(
                            icon.to_string(),
                            typography::proportional(typography::XS),
                            icon_color,
                        );
                        ui.painter().galley(
                            egui::pos2(cx, name_y),
                            icon_galley.clone(),
                            icon_color,
                        );
                        cx += icon_galley.size().x + 4.0;

                        // Filename (just the name, not full path)
                        let display_name = file_path.rsplit('/').next().unwrap_or(file_path);
                        let name_color = if is_selected {
                            theme.text_primary()
                        } else if is_reviewed {
                            theme.text_secondary().gamma_multiply(0.45)
                        } else {
                            theme.text_primary().gamma_multiply(0.85)
                        };

                        // Reviewed checkmark on the right
                        let mut right_x = rect.right() - 8.0;
                        if is_reviewed {
                            let check_galley = ui.painter().layout_no_wrap(
                                egui_nerdfonts::regular::CHECK.to_string(),
                                typography::proportional(typography::XS),
                                theme.diff_added_gutter(),
                            );
                            right_x -= check_galley.size().x;
                            ui.painter().galley(
                                egui::pos2(right_x, name_y),
                                check_galley,
                                theme.diff_added_gutter(),
                            );
                            right_x -= 4.0;
                        }

                        let max_name_width = (right_x - cx - 4.0).max(20.0);
                        let name_galley = ui.painter().layout(
                            display_name.to_string(),
                            typography::monospace(typography::XS),
                            name_color,
                            max_name_width,
                        );
                        ui.painter()
                            .galley(egui::pos2(cx, name_y), name_galley, name_color);

                        // Auto-scroll on keyboard nav
                        if is_selected && self.file_tree_scroll_to_selected {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }

                        if response.clicked() {
                            *clicked_file = Some(diff_idx);
                        }

                        response.on_hover_text(file_path);
                    }

                    ui.add_space(4.0);
                }
            });
    }

    /// Render the AI walkthrough summary banner when a walkthrough is ready.
    ///
    /// Uses the same raw-pointer interaction approach as show_description_banner
    /// to avoid stealing keyboard focus.
    /// Show walkthrough error state (if any). The summary banner is intentionally
    /// omitted — the walkthrough value lives in the inline gutter insights.
    fn show_walkthrough_banner(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        if let super::walkthrough::WalkthroughState::Error(ref err) = self.walkthrough_state {
            ui.add_space(4.0);
            egui::Frame::new()
                .fill(theme.diff_removed_bg())
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} Organize failed: {err}",
                            egui_nerdfonts::regular::WARNING
                        ))
                        .color(theme.diff_removed_text())
                        .font(typography::proportional(typography::XS)),
                    );
                });
            ui.add_space(4.0);
        }
    }

    /// Get the walkthrough-ordered file indices when a walkthrough is active.
    /// Returns (group_label, Vec<file_diff_index>) pairs, or None if no walkthrough.
    fn walkthrough_file_order(&self) -> Option<Vec<(&str, Vec<usize>)>> {
        let wt = match &self.walkthrough_state {
            super::walkthrough::WalkthroughState::Ready(wt) => wt,
            _ => return None,
        };

        let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
        let mut seen_indices: rustc_hash::FxHashSet<usize> = rustc_hash::FxHashSet::default();

        for group in &wt.groups {
            let mut file_indices = Vec::new();
            for path in &group.files {
                if let Some(idx) = self.file_diffs.iter().position(|d| d.path == *path) {
                    if seen_indices.insert(idx) {
                        file_indices.push(idx);
                    }
                }
            }
            if !file_indices.is_empty() {
                groups.push((&group.label, file_indices));
            }
        }

        // Append any files not mentioned by the AI into an "Other" group
        let mut other = Vec::new();
        for (i, _) in self.file_diffs.iter().enumerate() {
            if !seen_indices.contains(&i) {
                other.push(i);
            }
        }
        if !other.is_empty() {
            groups.push(("Other changes", other));
        }

        if groups.is_empty() {
            None
        } else {
            Some(groups)
        }
    }

    /// Render the Conversation tab — PR body + issue-level discussion only.
    /// Review comments are shown inline in the Files tab.
    fn show_conversation_tab(&mut self, ui: &mut egui::Ui, theme: AppTheme) {
        let has_pr_body = self
            .current_pr
            .as_ref()
            .and_then(|pr| pr.body.as_deref())
            .is_some_and(|b| !b.is_empty());

        if !has_pr_body && self.issue_comments.is_empty() && self.review_comments.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No discussion yet")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::MD)),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("pr_conversation")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // ── PR description (collapsible) ──
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
                                    // Header row with collapse toggle
                                    let header_resp = ui.horizontal(|ui| {
                                        let chevron = if self.conv_description_collapsed {
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
                                                .color(
                                                    theme.accent_primary().gamma_multiply(0.6),
                                                )
                                                .size(typography::SM),
                                        );
                                        ui.label(
                                            RichText::new("Description")
                                                .color(theme.text_secondary())
                                                .font(typography::proportional(typography::XS))
                                                .strong(),
                                        );
                                        ui.add_space(8.0);
                                        render_comment_avatar(
                                            ui,
                                            theme,
                                            &pr.user.login,
                                            &self.avatar_textures,
                                        );
                                        ui.label(
                                            RichText::new(&pr.user.login)
                                                .color(theme.text_primary())
                                                .font(typography::proportional(typography::SM))
                                                .strong(),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(relative_time(&pr.created_at))
                                                .color(
                                                    theme.text_secondary().gamma_multiply(0.7),
                                                )
                                                .font(typography::proportional(typography::XS)),
                                        );
                                    });
                                    // Toggle on click (use raw pointer — ui.horizontal
                                    // doesn't allocate with click sense)
                                    let header_rect = header_resp.response.rect;
                                    if ui.input(|i| i.pointer.any_pressed())
                                        && header_rect.contains(
                                            ui.input(|i| {
                                                i.pointer.interact_pos().unwrap_or_default()
                                            }),
                                        )
                                    {
                                        self.conv_description_collapsed =
                                            !self.conv_description_collapsed;
                                    }
                                    if header_rect.contains(
                                        ui.input(|i| i.pointer.hover_pos().unwrap_or_default()),
                                    ) {
                                        ui.ctx()
                                            .set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    // Body (only if not collapsed)
                                    if !self.conv_description_collapsed {
                                        ui.add_space(6.0);
                                        crate::components::overlay::markdown_renderer::render_markdown(
                                            ui, body, theme,
                                        );
                                    }
                                });
                        }
                    }
                }

                // ── Issue comments (PR-level discussion) ──
                for comment in &self.issue_comments {
                    render_comment(
                        ui,
                        theme,
                        &comment.user.login,
                        &comment.created_at,
                        &comment.body,
                        &self.avatar_textures,
                    );
                }

                // ── Review comments grouped by file ──
                if !self.cached_threads.is_empty() {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(format!(
                                "{} Review Comments",
                                egui_nerdfonts::regular::COMMENT_TEXT,
                            ))
                            .color(theme.text_primary().gamma_multiply(0.9))
                            .font(typography::proportional(typography::SM))
                            .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("{}", self.review_comments.len()))
                                .color(theme.text_secondary())
                                .font(typography::proportional(typography::XS)),
                        );
                    });

                    for thread in &self.cached_threads {
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(theme.bg_elevated())
                            .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::same(12))
                            .outer_margin(egui::Margin::symmetric(12, 0))
                            .show(ui, |ui| {
                                // File path + line number header
                                let file_name = thread
                                    .path
                                    .rsplit('/')
                                    .next()
                                    .unwrap_or(&thread.path);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(
                                            egui_nerdfonts::regular::FILE_EDIT,
                                        )
                                        .color(theme.text_secondary())
                                        .size(typography::XS),
                                    );
                                    ui.label(
                                        RichText::new(file_name)
                                            .color(theme.text_primary())
                                            .font(typography::monospace(typography::XS))
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(format!("L{}", thread.line))
                                            .color(theme.text_secondary().gamma_multiply(0.6))
                                            .font(typography::monospace(typography::XS)),
                                    );
                                });

                                // Quoted diff context (if we have the file diff)
                                if let Some(file_diff) =
                                    self.file_diffs.iter().find(|d| d.path == thread.path)
                                {
                                    // Find the line in the diff that matches thread.line
                                    let context_line = file_diff.lines.iter().find(|l| {
                                        l.new_line_num == Some(thread.line)
                                            || l.old_line_num == Some(thread.line)
                                    });
                                    if let Some(line) = context_line {
                                        ui.add_space(4.0);
                                        egui::Frame::new()
                                            .fill(theme.diff_line_number_bg())
                                            .corner_radius(3.0)
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .show(ui, |ui| {
                                                let prefix = match line.kind {
                                                    crate::git::diff::DiffLineKind::Addition => {
                                                        "+"
                                                    }
                                                    crate::git::diff::DiffLineKind::Deletion => {
                                                        "-"
                                                    }
                                                    _ => " ",
                                                };
                                                let line_color = match line.kind {
                                                    crate::git::diff::DiffLineKind::Addition => {
                                                        theme.diff_added_text()
                                                    }
                                                    crate::git::diff::DiffLineKind::Deletion => {
                                                        theme.diff_removed_text()
                                                    }
                                                    _ => theme.text_secondary(),
                                                };
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{prefix} {}",
                                                        line.content.trim_end()
                                                    ))
                                                    .color(line_color)
                                                    .font(typography::monospace(typography::XS)),
                                                );
                                            });
                                    }
                                }

                                // Thread comments
                                for (i, comment) in thread.comments.iter().enumerate() {
                                    if i > 0 {
                                        ui.add_space(4.0);
                                        ui.painter().hline(
                                            ui.available_rect_before_wrap().x_range(),
                                            ui.cursor().top(),
                                            egui::Stroke::new(
                                                0.5,
                                                theme.border_subtle().gamma_multiply(0.5),
                                            ),
                                        );
                                    }
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        render_comment_avatar(
                                            ui,
                                            theme,
                                            &comment.user.login,
                                            &self.avatar_textures,
                                        );
                                        ui.label(
                                            RichText::new(&comment.user.login)
                                                .color(theme.text_primary())
                                                .font(typography::proportional(typography::SM))
                                                .strong(),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(relative_time(&comment.created_at))
                                                .color(
                                                    theme.text_secondary().gamma_multiply(0.7),
                                                )
                                                .font(typography::proportional(typography::XS)),
                                        );
                                    });
                                    ui.add_space(2.0);
                                    crate::components::overlay::markdown_renderer::render_markdown(
                                        ui,
                                        &comment.body,
                                        theme,
                                    );
                                }
                            });
                    }
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

/// Render a tab button with an optional badge.
fn render_tab_with_badge(
    ui: &mut egui::Ui,
    theme: AppTheme,
    label: &str,
    badge: Option<&str>,
    tab: DetailTab,
    active_tab: &mut DetailTab,
) {
    let is_active = *active_tab == tab;
    let text_color = if is_active {
        theme.accent_primary()
    } else {
        theme.text_secondary()
    };

    let display_label = if let Some(badge_text) = badge {
        format!("{label} {badge_text}")
    } else {
        label.to_string()
    };

    let btn = ui.add(
        egui::Button::new(
            RichText::new(display_label)
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
/// Render a circular avatar (texture or fallback initial) for a comment author.
fn render_comment_avatar(
    ui: &mut egui::Ui,
    theme: AppTheme,
    login: &str,
    avatar_textures: &rustc_hash::FxHashMap<String, egui::TextureHandle>,
) {
    let avatar_size = 16.0;
    let (avatar_rect, _) =
        ui.allocate_exact_size(egui::vec2(avatar_size, avatar_size), egui::Sense::hover());
    let center = avatar_rect.center();
    let radius = avatar_size / 2.0;

    if let Some(texture) = avatar_textures.get(login) {
        let mut mesh = egui::Mesh::with_texture(texture.id());
        let segments: u32 = 20;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center,
            uv: egui::pos2(0.5, 0.5),
            color: egui::Color32::WHITE,
        });
        for i in 0..=segments {
            let angle = std::f32::consts::TAU * i as f32 / segments as f32;
            let (sin, cos) = angle.sin_cos();
            mesh.vertices.push(egui::epaint::Vertex {
                pos: center + egui::vec2(cos * radius, sin * radius),
                uv: egui::pos2(0.5 + cos * 0.5, 0.5 + sin * 0.5),
                color: egui::Color32::WHITE,
            });
            if i > 0 {
                mesh.indices.push(0);
                mesh.indices.push(i);
                mesh.indices.push(i + 1);
            }
        }
        ui.painter().add(egui::Shape::mesh(mesh));
    } else {
        let letter = login
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        ui.painter()
            .circle_filled(center, radius, theme.accent_primary().gamma_multiply(0.2));
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            &letter,
            typography::proportional(8.0),
            theme.accent_primary(),
        );
    }
}

/// Render a single comment with avatar.
fn render_comment(
    ui: &mut egui::Ui,
    theme: AppTheme,
    author: &str,
    timestamp: &str,
    body: &str,
    avatar_textures: &rustc_hash::FxHashMap<String, egui::TextureHandle>,
) {
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(theme.bg_elevated())
        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(12))
        .outer_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            // Avatar + Author + timestamp
            ui.horizontal(|ui| {
                render_comment_avatar(ui, theme, author, avatar_textures);
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
            patch: None,
            previous_filename: None,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
            &[],
            &FxHashSet::default(),
            &FxHashSet::default(),
            false,
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
