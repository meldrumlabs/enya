//! PR list view — shows open pull requests for the configured repository.

use egui::RichText;

use crate::github_api::relative_time;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::PrReviewPane;

impl PrReviewPane {
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

        // Header
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!(
                    "{} Pull Requests",
                    egui_nerdfonts::regular::GIT_PULL_REQUEST
                ))
                .color(theme.text_primary())
                .font(typography::proportional(typography::LG))
                .strong(),
            );

            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{}/{}", self.owner, self.repo))
                    .color(theme.text_secondary())
                    .font(typography::proportional(typography::SM)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                let refresh_btn = ui.add(
                    egui::Button::new(
                        RichText::new(format!("{} Refresh", egui_nerdfonts::regular::REFRESH))
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(theme.bg_elevated())
                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                    .corner_radius(4.0),
                );
                if refresh_btn.clicked() {
                    self.fetch_pr_list();
                }
            });
        });
        ui.add_space(8.0);

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

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

        // PR list
        let mut clicked_pr_number: Option<u32> = None;
        egui::ScrollArea::vertical()
            .id_salt("pr_list_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, pr) in self.pull_requests.iter().enumerate() {
                    let is_selected = i == self.selected_pr_index;
                    let row_height = 52.0;

                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    let is_hovered = response.hovered();

                    // Background
                    if is_selected {
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            theme.accent_primary().gamma_multiply(0.08),
                        );
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
                        egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.5)),
                    );

                    let content_rect = rect.shrink2(egui::vec2(12.0, 4.0));

                    // Status dot
                    let dot_color = if pr.draft {
                        theme.text_secondary().gamma_multiply(0.5)
                    } else {
                        theme.accent_primary()
                    };
                    let dot_center = egui::pos2(content_rect.left() + 6.0, content_rect.center().y);
                    ui.painter().circle_filled(dot_center, 4.0, dot_color);

                    // PR number
                    let number_text = format!("#{}", pr.number);
                    let number_galley = ui.painter().layout_no_wrap(
                        number_text,
                        typography::monospace(typography::SM),
                        theme.text_secondary(),
                    );
                    let number_x = content_rect.left() + 18.0;
                    ui.painter().galley(
                        egui::pos2(number_x, content_rect.top() + 6.0),
                        number_galley.clone(),
                        theme.text_secondary(),
                    );

                    // Title
                    let title_x = number_x + number_galley.size().x + 8.0;
                    let title_max_width = (content_rect.right() - title_x - 100.0).max(50.0);
                    let title_galley = ui.painter().layout(
                        pr.title.clone(),
                        typography::proportional(typography::MD),
                        theme.text_primary(),
                        title_max_width,
                    );
                    ui.painter().galley(
                        egui::pos2(title_x, content_rect.top() + 4.0),
                        title_galley,
                        theme.text_primary(),
                    );

                    // Author and timestamp
                    let subtitle = format!(
                        "{} {} {}",
                        pr.user.login,
                        egui_nerdfonts::regular::CIRCLE_SMALL,
                        relative_time(&pr.updated_at)
                    );
                    let subtitle_galley = ui.painter().layout_no_wrap(
                        subtitle,
                        typography::proportional(typography::XS),
                        theme.text_secondary(),
                    );
                    ui.painter().galley(
                        egui::pos2(
                            title_x,
                            content_rect.bottom() - subtitle_galley.size().y - 4.0,
                        ),
                        subtitle_galley,
                        theme.text_secondary(),
                    );

                    // Draft badge
                    if pr.draft {
                        let badge_text = "Draft";
                        let badge_galley = ui.painter().layout_no_wrap(
                            badge_text.to_string(),
                            typography::proportional(typography::XS),
                            theme.text_secondary(),
                        );
                        let badge_x = content_rect.right() - badge_galley.size().x - 4.0;
                        let badge_rect = egui::Rect::from_min_size(
                            egui::pos2(badge_x - 4.0, content_rect.center().y - 9.0),
                            egui::vec2(badge_galley.size().x + 8.0, 18.0),
                        );
                        ui.painter()
                            .rect_filled(badge_rect, 4.0, theme.border_subtle());
                        ui.painter().galley(
                            egui::pos2(
                                badge_x,
                                content_rect.center().y - badge_galley.size().y / 2.0,
                            ),
                            badge_galley,
                            theme.text_secondary(),
                        );
                    }

                    // Handle click — open PR detail
                    if response.clicked() {
                        self.selected_pr_index = i;
                        clicked_pr_number = Some(pr.number);
                    }
                }
            });

        // Open PR outside the iterator borrow
        if let Some(number) = clicked_pr_number {
            self.open_pr(number);
        }

        // Footer hints
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{} open", self.pull_requests.len()))
                    .color(theme.text_secondary().gamma_multiply(0.7))
                    .font(typography::proportional(typography::XS)),
            );
        });
        ui.add_space(4.0);
    }
}

/// Render a centered empty state with icon, title, and subtitle.
fn render_empty_state(ui: &mut egui::Ui, theme: AppTheme, icon: &str, title: &str, subtitle: &str) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(icon)
                .color(theme.text_secondary().gamma_multiply(0.5))
                .size(32.0),
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
