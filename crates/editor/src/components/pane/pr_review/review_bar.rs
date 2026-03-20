//! Review bar — shows draft comment count and submit actions.

use egui::RichText;

use crate::github_api::ReviewEvent;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::PrReviewPane;

impl PrReviewPane {
    /// Render the review bar at the bottom of the detail view.
    pub(super) fn show_review_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;

        // Only show if we have a PR open
        if self.current_pr.is_none() {
            return;
        }

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);

            // Draft comment count
            let draft_count = self.draft_comments.len();
            if draft_count > 0 {
                ui.label(
                    RichText::new(format!(
                        "{} {} draft comment{}",
                        egui_nerdfonts::regular::COMMENT,
                        draft_count,
                        if draft_count == 1 { "" } else { "s" }
                    ))
                    .color(theme.accent_primary())
                    .font(typography::proportional(typography::SM)),
                );
                ui.add_space(12.0);
            }

            // Success/error messages
            if let Some(ref msg) = self.submit_success {
                ui.label(
                    RichText::new(format!("{} {msg}", egui_nerdfonts::regular::CHECK))
                        .color(theme.diff_added_text())
                        .font(typography::proportional(typography::SM)),
                );
            }
            if let Some(ref msg) = self.submit_error {
                ui.label(
                    RichText::new(format!("{} {msg}", egui_nerdfonts::regular::X))
                        .color(theme.diff_removed_text())
                        .font(typography::proportional(typography::SM)),
                );
            }

            // Submit buttons on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);

                let has_content = !self.draft_comments.is_empty() || !self.draft_body.is_empty();
                let enabled = has_content && !self.submitting_review && self.token.is_some();

                // Approve button
                render_review_button(
                    ui,
                    theme,
                    "Approve",
                    theme.diff_added_text(),
                    theme.diff_added_bg(),
                    enabled || self.token.is_some(), // Can approve without comments
                    self.submitting_review,
                    ReviewEvent::Approve,
                    &mut None, // We'll handle the click below
                );
                let _approve_clicked = ui.ctx().input(|i| i.pointer.primary_clicked())
                    && ui.rect_contains_pointer(ui.min_rect());

                ui.add_space(4.0);

                // Request Changes button
                let mut clicked_event: Option<ReviewEvent> = None;

                let rc_btn = ui.add_enabled(
                    enabled,
                    egui::Button::new(RichText::new("Request Changes").size(typography::XS).color(
                        if enabled {
                            theme.diff_removed_text()
                        } else {
                            theme.text_secondary().gamma_multiply(0.5)
                        },
                    ))
                    .fill(if enabled {
                        theme.diff_removed_bg()
                    } else {
                        theme.bg_elevated()
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if enabled {
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

                // Comment button
                let comment_btn = ui.add_enabled(
                    enabled,
                    egui::Button::new(RichText::new("Comment").size(typography::XS).color(
                        if enabled {
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

                // Handle click
                if let Some(event) = clicked_event {
                    self.submit_review(event);
                }

                // Submitting indicator
                if self.submitting_review {
                    ui.add_space(8.0);
                    ui.spinner();
                }
            });
        });
        ui.add_space(6.0);
    }
}

/// Render a styled review button.
#[allow(clippy::too_many_arguments)]
fn render_review_button(
    ui: &mut egui::Ui,
    theme: AppTheme,
    label: &str,
    text_color: egui::Color32,
    bg_color: egui::Color32,
    enabled: bool,
    _submitting: bool,
    event: ReviewEvent,
    clicked: &mut Option<ReviewEvent>,
) {
    let btn = ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(typography::XS).color(if enabled {
            text_color
        } else {
            theme.text_secondary().gamma_multiply(0.5)
        }))
        .fill(if enabled {
            bg_color
        } else {
            theme.bg_elevated()
        })
        .stroke(egui::Stroke::new(
            1.0,
            if enabled {
                text_color.gamma_multiply(0.3)
            } else {
                theme.border_subtle()
            },
        ))
        .corner_radius(4.0),
    );
    if btn.clicked() {
        *clicked = Some(event);
    }
}
