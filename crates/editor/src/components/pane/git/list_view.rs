//! PR list view — shows open pull requests for the configured repository.

use egui::RichText;

use crate::git::api::relative_time;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::{PrReviewPane, ReviewState};

impl PrReviewPane {
    /// Get the indices of PRs matching the current filter query.
    pub(super) fn filtered_pr_indices(&self) -> Vec<usize> {
        if self.filter_query.is_empty() {
            return (0..self.pull_requests.len()).collect();
        }

        let query = self.filter_query.to_lowercase();
        self.pull_requests
            .iter()
            .enumerate()
            .filter(|(_, pr)| {
                pr.title.to_lowercase().contains(&query)
                    || pr.user.login.to_lowercase().contains(&query)
                    || format!("#{}", pr.number).contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Render the PR list view.
    pub(super) fn show_list_view(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;

        // No token — prompt to sign in
        if self.token.is_none() {
            render_empty_state(
                ui,
                theme,
                egui_nerdfonts::regular::LOCK,
                "Sign in to GitHub",
                "Go to Settings to connect your GitHub account.",
            );
            return;
        }

        ui.add_space(2.0);

        // Loading state
        if self.list_loading {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Loading pull requests...")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::SM)),
                );
            });
            return;
        }

        // Error state
        if let Some(ref error) = self.list_error {
            render_empty_state(
                ui,
                theme,
                egui_nerdfonts::regular::WARNING,
                "Failed to load PRs",
                error,
            );
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                let retry = ui.add(
                    egui::Button::new(
                        RichText::new(format!("{} Retry", egui_nerdfonts::regular::REFRESH))
                            .color(theme.accent_primary())
                            .font(typography::proportional(typography::SM)),
                    )
                    .fill(theme.accent_primary().gamma_multiply(0.1))
                    .stroke(egui::Stroke::new(
                        1.0,
                        theme.accent_primary().gamma_multiply(0.3),
                    ))
                    .corner_radius(4.0),
                );
                if retry.clicked() {
                    self.list_error = None;
                    self.fetch_pr_list();
                }
            });
            return;
        }

        // Empty state
        if self.pull_requests.is_empty() {
            render_empty_state(
                ui,
                theme,
                egui_nerdfonts::regular::GIT_PULL_REQUEST,
                "No open pull requests",
                "There are no open pull requests for this repository.",
            );
            return;
        }

        // ── Filter bar ──
        let filter_id = ui.id().with("pr_filter_input");
        if self.filter_active {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("/")
                        .color(theme.accent_primary())
                        .font(typography::monospace(typography::SM))
                        .strong(),
                );
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.filter_query)
                        .hint_text("Filter by title, author, or #number...")
                        .desired_width(ui.available_width() - 24.0)
                        .font(typography::proportional(typography::SM))
                        .text_color(theme.text_primary()),
                );
                // Auto-focus on first frame
                if !ui.ctx().memory(|m| m.focused() == Some(filter_id)) {
                    response.request_focus();
                }
                // Close filter on Escape (TextEdit may unfocus itself but
                // we also need to deactivate the filter bar)
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.filter_active = false;
                    self.filter_query.clear();
                    self.selected_pr_index = 0;
                }
            });
            ui.add_space(4.0);
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, theme.accent_primary().gamma_multiply(0.3)),
            );
        } else if !self.filter_query.is_empty() {
            // Show active filter badge (query persists after Enter)
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!(
                        "{} \"{}\"",
                        egui_nerdfonts::regular::FILTER_1,
                        self.filter_query
                    ))
                    .color(theme.accent_primary())
                    .font(typography::proportional(typography::SM)),
                );
                ui.add_space(4.0);
                let clear_btn = ui.add(
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::X)
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
                );
                if clear_btn.clicked() {
                    self.filter_query.clear();
                    self.selected_pr_index = 0;
                }
            });
            ui.add_space(4.0);
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, theme.border_subtle()),
            );
        }

        // Compute filtered indices
        let filtered_indices = self.filtered_pr_indices();

        // Clamp selected index to filtered range
        if self.selected_pr_index >= filtered_indices.len() {
            self.selected_pr_index = filtered_indices.len().saturating_sub(1);
        }

        if filtered_indices.is_empty() && !self.filter_query.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No matching pull requests")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::MD)),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Try a different search query")
                        .color(theme.text_secondary().gamma_multiply(0.7))
                        .font(typography::proportional(typography::SM)),
                );
            });
        }

        // ── Header bar: icon + count pill + refresh ──
        let muted = theme.text_secondary();
        let header_rect = egui::Rect::from_min_size(
            ui.cursor().left_top(),
            egui::vec2(ui.available_width(), 30.0),
        );
        ui.painter()
            .rect_filled(header_rect, 0.0, theme.bg_elevated().gamma_multiply(0.3));
        ui.painter().hline(
            header_rect.x_range(),
            header_rect.bottom(),
            egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.5)),
        );

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 30.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(12.0);

                // PR icon
                ui.label(
                    RichText::new(egui_nerdfonts::regular::GIT_PULL_REQUEST)
                        .color(theme.diff_added_text().gamma_multiply(0.7))
                        .size(typography::SM),
                );
                ui.add_space(2.0);

                // Count in a pill badge
                let count_text = if self.filter_query.is_empty() {
                    format!("{} open", self.pull_requests.len())
                } else {
                    format!(
                        "{}/{} matched",
                        filtered_indices.len(),
                        self.pull_requests.len()
                    )
                };
                let count_galley = ui.painter().layout_no_wrap(
                    count_text,
                    typography::proportional(typography::XS),
                    muted,
                );
                let pill_w = count_galley.size().x + 10.0;
                let pill_h = count_galley.size().y + 2.0;
                let (pill_rect, _) =
                    ui.allocate_exact_size(egui::vec2(pill_w, pill_h), egui::Sense::hover());
                ui.painter().rect_filled(
                    pill_rect,
                    pill_h / 2.0,
                    theme.border_subtle().gamma_multiply(0.5),
                );
                ui.painter().galley(
                    egui::pos2(
                        pill_rect.center().x - count_galley.size().x / 2.0,
                        pill_rect.center().y - count_galley.size().y / 2.0,
                    ),
                    count_galley,
                    muted,
                );

                // Refresh button (right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if self.list_loading {
                        ui.spinner();
                    } else {
                        let icon_color = if ui.rect_contains_pointer(ui.cursor()) {
                            theme.accent_primary()
                        } else {
                            muted.gamma_multiply(0.7)
                        };
                        let refresh_btn = ui.add(
                            egui::Button::new(
                                RichText::new(egui_nerdfonts::regular::REFRESH)
                                    .size(typography::SM)
                                    .color(icon_color),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        );
                        if refresh_btn.clicked() {
                            self.fetch_pr_list();
                        }
                        refresh_btn.on_hover_text("Refresh pull requests (r)");
                    }
                });
            },
        );

        // PR list — reserve space for the footer keybinding hints
        let footer_height = 32.0;
        let scroll_max = (ui.available_height() - footer_height).max(40.0);
        let mut clicked_pr_number: Option<u32> = None;
        egui::ScrollArea::vertical()
            .id_salt("pr_list_scroll")
            .max_height(scroll_max)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (display_idx, &pr_idx) in filtered_indices.iter().enumerate() {
                    let pr = &self.pull_requests[pr_idx];
                    let is_selected = display_idx == self.selected_pr_index;
                    let row_height = 56.0;

                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    let is_hovered = response.hovered();

                    // Row background
                    if is_selected {
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            theme.accent_primary().gamma_multiply(0.08),
                        );
                        // Left accent bar
                        let bar_rect =
                            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
                        ui.painter()
                            .rect_filled(bar_rect, 0.0, theme.accent_primary());
                    } else if is_hovered {
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            theme.text_primary().gamma_multiply(0.03),
                        );
                    }

                    // Bottom border
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom(),
                        egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.4)),
                    );

                    let left_pad = 16.0;
                    let right_pad = 12.0;
                    let top_line_y = rect.top() + 10.0;
                    let bottom_line_y = rect.bottom() - 16.0;

                    // ── Top line: state icon + number + title ──

                    let mut cx = rect.left() + left_pad;

                    // PR state icon
                    let (state_icon, state_color) = if pr.draft {
                        (
                            egui_nerdfonts::regular::GIT_PULL_REQUEST_DRAFT,
                            theme.text_secondary().gamma_multiply(0.6),
                        )
                    } else {
                        (
                            egui_nerdfonts::regular::GIT_PULL_REQUEST,
                            theme.diff_added_text(),
                        )
                    };
                    let icon_galley = ui.painter().layout_no_wrap(
                        state_icon.to_string(),
                        typography::proportional(typography::MD),
                        state_color,
                    );
                    ui.painter().galley(
                        egui::pos2(cx, top_line_y),
                        icon_galley.clone(),
                        state_color,
                    );
                    cx += icon_galley.size().x + 6.0;

                    // PR number
                    let number_text = format!("#{}", pr.number);
                    let number_color = if is_selected {
                        theme.accent_primary()
                    } else {
                        theme.text_secondary()
                    };
                    let number_galley = ui.painter().layout_no_wrap(
                        number_text,
                        typography::monospace(typography::SM),
                        number_color,
                    );
                    ui.painter().galley(
                        egui::pos2(cx, top_line_y + 1.0),
                        number_galley.clone(),
                        number_color,
                    );
                    cx += number_galley.size().x + 8.0;

                    // Determine review state and merge-readiness for badge
                    let review_state = self.review_state_for_pr(pr.number);
                    let is_merge_ready = self
                        .preloaded_merge_ready
                        .get(&pr.number)
                        .copied()
                        .unwrap_or(false);

                    // Title — fill remaining width (leave space for right-side badges)
                    let badge_reserve = if is_merge_ready {
                        130.0
                    } else {
                        match review_state {
                            Some(ReviewState::Approved) => 90.0,
                            Some(ReviewState::ChangesRequested) => 130.0,
                            None if pr.draft => 60.0,
                            None => 0.0,
                        }
                    };
                    let title_max = (rect.right() - right_pad - cx - badge_reserve).max(40.0);
                    let title_color = if is_selected {
                        theme.text_primary()
                    } else {
                        theme.text_primary().gamma_multiply(0.9)
                    };
                    let title_galley = ui.painter().layout(
                        pr.title.clone(),
                        typography::proportional(typography::SM),
                        title_color,
                        title_max,
                    );
                    ui.painter().galley(
                        egui::pos2(cx, top_line_y + 1.0),
                        title_galley,
                        title_color,
                    );

                    // Status badge (top-right) — Draft, Merge Ready, Approved, or Changes Requested
                    let badge_info: Option<(String, egui::Color32, egui::Color32)> = if pr.draft {
                        Some((
                            "Draft".to_string(),
                            theme.text_secondary().gamma_multiply(0.8),
                            theme.border_subtle().gamma_multiply(0.8),
                        ))
                    } else if is_merge_ready {
                        Some((
                            format!("{} Ready to merge", egui_nerdfonts::regular::CHECK_CIRCLE),
                            theme.diff_added_text(),
                            theme.diff_added_bg(),
                        ))
                    } else {
                        match review_state {
                            Some(ReviewState::Approved) => Some((
                                format!("{} Approved", egui_nerdfonts::regular::CHECK),
                                theme.diff_added_text(),
                                theme.diff_added_bg(),
                            )),
                            Some(ReviewState::ChangesRequested) => Some((
                                format!("{} Changes requested", egui_nerdfonts::regular::X_CIRCLE),
                                theme.diff_removed_gutter(),
                                theme.diff_removed_bg(),
                            )),
                            None => None,
                        }
                    };

                    if let Some((text, fg, bg)) = badge_info {
                        let badge_galley = ui.painter().layout_no_wrap(
                            text,
                            typography::proportional(typography::XS),
                            fg,
                        );
                        let badge_w = badge_galley.size().x + 10.0;
                        let badge_h = badge_galley.size().y + 4.0;
                        let badge_x = rect.right() - right_pad - badge_w;
                        let badge_y = top_line_y + 1.0;
                        let badge_rect = egui::Rect::from_min_size(
                            egui::pos2(badge_x, badge_y),
                            egui::vec2(badge_w, badge_h),
                        );
                        ui.painter().rect_filled(badge_rect, 3.0, bg);
                        ui.painter().galley(
                            egui::pos2(badge_x + 5.0, badge_y + 2.0),
                            badge_galley,
                            fg,
                        );
                    }

                    // ── Bottom line: avatar + author + time + comments + stats ──

                    let mut bx = rect.left() + left_pad + icon_galley.size().x + 6.0;

                    // Author avatar (circular)
                    let avatar_size = 14.0;
                    let avatar_radius = avatar_size / 2.0;
                    let avatar_center =
                        egui::pos2(bx + avatar_radius, bottom_line_y + avatar_radius - 1.0);
                    if let Some(texture) = self.avatar_textures.get(&pr.user.login) {
                        let mut mesh = egui::Mesh::with_texture(texture.id());
                        let segments: u32 = 20;
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: avatar_center,
                            uv: egui::pos2(0.5, 0.5),
                            color: egui::Color32::WHITE,
                        });
                        for i in 0..=segments {
                            let angle = std::f32::consts::TAU * i as f32 / segments as f32;
                            let (sin, cos) = angle.sin_cos();
                            mesh.vertices.push(egui::epaint::Vertex {
                                pos: avatar_center
                                    + egui::vec2(cos * avatar_radius, sin * avatar_radius),
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
                        // Fallback: circle with first initial
                        let letter = pr
                            .user
                            .login
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string();
                        ui.painter().circle_filled(
                            avatar_center,
                            avatar_radius,
                            theme.accent_primary().gamma_multiply(0.2),
                        );
                        ui.painter().text(
                            avatar_center,
                            egui::Align2::CENTER_CENTER,
                            &letter,
                            typography::proportional(7.0),
                            theme.accent_primary(),
                        );
                    }
                    bx += avatar_size + 4.0;

                    // Author name
                    let author_galley = ui.painter().layout_no_wrap(
                        pr.user.login.clone(),
                        typography::proportional(typography::XS),
                        theme.text_secondary(),
                    );
                    ui.painter().galley(
                        egui::pos2(bx, bottom_line_y),
                        author_galley.clone(),
                        theme.text_secondary(),
                    );
                    bx += author_galley.size().x;

                    // Separator dot
                    let sep_galley = ui.painter().layout_no_wrap(
                        format!(" {} ", egui_nerdfonts::regular::CIRCLE_SMALL),
                        typography::proportional(typography::XS),
                        theme.text_secondary().gamma_multiply(0.5),
                    );
                    ui.painter().galley(
                        egui::pos2(bx, bottom_line_y),
                        sep_galley.clone(),
                        theme.text_secondary().gamma_multiply(0.5),
                    );
                    bx += sep_galley.size().x;

                    // Timestamp
                    let time_galley = ui.painter().layout_no_wrap(
                        relative_time(&pr.updated_at),
                        typography::proportional(typography::XS),
                        theme.text_secondary().gamma_multiply(0.7),
                    );
                    ui.painter().galley(
                        egui::pos2(bx, bottom_line_y),
                        time_galley.clone(),
                        theme.text_secondary().gamma_multiply(0.7),
                    );
                    bx += time_galley.size().x;

                    // Comment count (if any)
                    if let Some(&count) = self.preloaded_comment_counts.get(&pr.number) {
                        if count > 0 {
                            let sep2 = ui.painter().layout_no_wrap(
                                format!(" {} ", egui_nerdfonts::regular::CIRCLE_SMALL),
                                typography::proportional(typography::XS),
                                theme.text_secondary().gamma_multiply(0.5),
                            );
                            ui.painter().galley(
                                egui::pos2(bx, bottom_line_y),
                                sep2.clone(),
                                theme.text_secondary().gamma_multiply(0.5),
                            );
                            bx += sep2.size().x;

                            let comment_galley = ui.painter().layout_no_wrap(
                                format!("{} {}", egui_nerdfonts::regular::COMMENT, count),
                                typography::proportional(typography::XS),
                                theme.text_secondary().gamma_multiply(0.7),
                            );
                            ui.painter().galley(
                                egui::pos2(bx, bottom_line_y),
                                comment_galley,
                                theme.text_secondary().gamma_multiply(0.7),
                            );
                        }
                    }

                    // Stats on the right of the bottom line
                    let mut rx = rect.right() - right_pad;

                    if pr.deletions > 0 {
                        let del_text = format!("-{}", pr.deletions);
                        let del_galley = ui.painter().layout_no_wrap(
                            del_text,
                            typography::monospace(typography::XS),
                            theme.diff_removed_gutter(),
                        );
                        rx -= del_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(rx, bottom_line_y),
                            del_galley,
                            theme.diff_removed_gutter(),
                        );
                        rx -= 6.0;
                    }

                    if pr.additions > 0 {
                        let add_text = format!("+{}", pr.additions);
                        let add_galley = ui.painter().layout_no_wrap(
                            add_text,
                            typography::monospace(typography::XS),
                            theme.diff_added_gutter(),
                        );
                        rx -= add_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(rx, bottom_line_y),
                            add_galley,
                            theme.diff_added_gutter(),
                        );
                        rx -= 6.0;
                    }

                    // Mini proportional add/del bar
                    let total_changes = pr.additions + pr.deletions;
                    if total_changes > 0 {
                        let bar_width = 24.0;
                        let bar_height = 3.0;
                        let bar_y = bottom_line_y + 5.0;
                        rx -= bar_width;
                        let add_frac = pr.additions as f32 / total_changes as f32;
                        let add_w = (bar_width * add_frac).max(0.0);
                        // Green portion
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(rx, bar_y),
                                egui::vec2(add_w, bar_height),
                            ),
                            1.0,
                            theme.diff_added_gutter(),
                        );
                        // Red portion
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(rx + add_w, bar_y),
                                egui::vec2((bar_width - add_w).max(0.0), bar_height),
                            ),
                            1.0,
                            theme.diff_removed_gutter(),
                        );
                        rx -= 6.0;
                    }

                    if pr.changed_files > 0 {
                        let files_text = format!(
                            "{} {}",
                            pr.changed_files,
                            if pr.changed_files == 1 {
                                "file"
                            } else {
                                "files"
                            }
                        );
                        let files_galley = ui.painter().layout_no_wrap(
                            files_text,
                            typography::proportional(typography::XS),
                            theme.text_secondary().gamma_multiply(0.6),
                        );
                        rx -= files_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(rx, bottom_line_y),
                            files_galley,
                            theme.text_secondary().gamma_multiply(0.6),
                        );
                    }

                    // CI status dot (between stats and file count)
                    if let Some(checks) = self.preloaded_merge_ready.get(&pr.number) {
                        let dot_color = if *checks {
                            theme.diff_added_gutter()
                        } else {
                            theme.diff_removed_gutter()
                        };
                        rx -= 8.0;
                        ui.painter().circle_filled(
                            egui::pos2(rx, bottom_line_y + 5.0),
                            3.0,
                            dot_color,
                        );
                    }

                    // Auto-scroll to keep selected row visible when navigating via keyboard
                    if is_selected && self.list_scroll_to_selected {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }

                    // Pointer cursor on hover
                    if is_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Handle click — open PR detail
                    if response.clicked() {
                        self.selected_pr_index = display_idx;
                        clicked_pr_number = Some(pr.number);
                    }
                }
            });

        // Clear scroll flag after rendering
        self.list_scroll_to_selected = false;

        // Open PR outside the iterator borrow
        if let Some(number) = clicked_pr_number {
            self.open_pr(number);
        }

        // ── Footer: keybinding hints (elevated) ──
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

                let key_color = theme.text_secondary().gamma_multiply(0.5);
                let sep_color = theme.text_secondary().gamma_multiply(0.25);
                let accent_key = theme.accent_primary().gamma_multiply(0.6);
                let key_font = typography::monospace(typography::XS);
                let label_font = typography::proportional(typography::XS);

                ui.label(RichText::new("/").color(accent_key).font(key_font.clone()));
                ui.label(
                    RichText::new("filter")
                        .color(key_color)
                        .font(label_font.clone()),
                );
                ui.label(
                    RichText::new("\u{2022}")
                        .color(sep_color)
                        .font(label_font.clone()),
                );
                ui.label(RichText::new("j/k").color(key_color).font(key_font.clone()));
                ui.label(
                    RichText::new("navigate")
                        .color(key_color)
                        .font(label_font.clone()),
                );
                ui.label(
                    RichText::new("\u{2022}")
                        .color(sep_color)
                        .font(label_font.clone()),
                );
                ui.label(
                    RichText::new("Enter")
                        .color(key_color)
                        .font(key_font.clone()),
                );
                ui.label(
                    RichText::new("open")
                        .color(key_color)
                        .font(label_font.clone()),
                );
                ui.label(
                    RichText::new("\u{2022}")
                        .color(sep_color)
                        .font(label_font.clone()),
                );
                ui.label(RichText::new("g/G").color(key_color).font(key_font.clone()));
                ui.label(
                    RichText::new("top/bottom")
                        .color(key_color)
                        .font(label_font.clone()),
                );
                ui.label(
                    RichText::new("\u{2022}")
                        .color(sep_color)
                        .font(label_font.clone()),
                );
                ui.label(RichText::new("r").color(key_color).font(key_font));
                ui.label(RichText::new("refresh").color(key_color).font(label_font));
            },
        );
    }
}

/// Render a centered empty state with icon, title, subtitle, and optional hint.
fn render_empty_state(ui: &mut egui::Ui, theme: AppTheme, icon: &str, title: &str, subtitle: &str) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        // Icon with subtle background circle
        let icon_size = 40.0;
        let (icon_rect, _) =
            ui.allocate_exact_size(egui::vec2(icon_size, icon_size), egui::Sense::hover());
        ui.painter().circle_filled(
            icon_rect.center(),
            icon_size / 2.0,
            theme.accent_primary().gamma_multiply(0.06),
        );
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            typography::proportional(24.0),
            theme.text_secondary().gamma_multiply(0.6),
        );

        ui.add_space(12.0);
        ui.label(
            RichText::new(title)
                .color(theme.text_primary())
                .font(typography::proportional(typography::LG)),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(subtitle)
                .color(theme.text_secondary())
                .font(typography::proportional(typography::SM)),
        );
    });
}
