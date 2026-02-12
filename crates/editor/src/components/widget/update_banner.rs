//! Update notification banner - displays when a new version is available.
//!
//! Non-intrusive frosted glass banner in the bottom-right corner with
//! "See changes" and "Restart" buttons, similar to Conductor.build's update popup.

use egui::{Color32, RichText};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Actions returned by the update banner.
#[derive(Debug, Clone)]
pub enum UpdateBannerAction {
    /// No action taken.
    None,
    /// User clicked "See changes" - open this URL.
    SeeChanges(String),
    /// User clicked "Restart" to apply the update.
    Restart,
    /// User dismissed the banner for this version.
    Dismissed(String),
}

/// A non-intrusive update notification banner.
pub struct UpdateBanner {
    theme: AppTheme,
    version: String,
    release_url: String,
    has_download: bool,
    is_downloading: bool,
}

impl UpdateBanner {
    /// Create a new update banner.
    pub fn new(
        version: String,
        release_url: String,
        _release_notes: String,
        has_download: bool,
    ) -> Self {
        Self {
            theme: AppTheme::default(),
            version,
            release_url,
            has_download,
            is_downloading: false,
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set whether a download is currently in progress.
    pub fn set_downloading(&mut self, downloading: bool) {
        self.is_downloading = downloading;
    }

    /// Show the update banner. Returns the user's action.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> UpdateBannerAction {
        let mut action = UpdateBannerAction::None;

        let theme = self.theme;
        let accent_color = theme.accent_primary();
        let bg_color = theme.bg_surface().gamma_multiply(0.95);
        let border_color = theme.border_subtle();
        let text_color = theme.text_primary();
        let muted_color = theme.text_secondary();

        let banner_width = 340.0;

        egui::Area::new(egui::Id::new("update_banner"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        ui.set_width(banner_width);

                        // Outer horizontal: accent bar on left, content column on right
                        ui.horizontal(|ui| {
                            // Accent bar spanning full height (painted after layout)
                            let bar_id = ui.id().with("accent_bar");
                            let bar_start = ui.cursor().min;
                            let (_, bar_rect) = ui.allocate_space(egui::vec2(3.0, 0.0));
                            let _ = bar_rect; // placeholder, painted below

                            ui.add_space(10.0);

                            // Content column: title row, subtitle, buttons
                            ui.vertical(|ui| {
                                // Title row: icon + title + close button
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(semantic_icons::action::IMPORT)
                                            .color(accent_color)
                                            .size(18.0),
                                    );

                                    ui.add_space(6.0);

                                    ui.label(
                                        RichText::new("Update Available")
                                            .color(text_color)
                                            .strong()
                                            .size(14.0),
                                    );

                                    // Close button (right aligned)
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let close_btn = ui.add(
                                                egui::Button::new(
                                                    RichText::new(semantic_icons::action::CLOSE)
                                                        .color(muted_color)
                                                        .size(14.0),
                                                )
                                                .frame(false),
                                            );
                                            if close_btn.clicked() {
                                                action = UpdateBannerAction::Dismissed(
                                                    self.version.clone(),
                                                );
                                            }
                                        },
                                    );
                                });

                                // Version subtitle
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("v{} is ready", self.version))
                                        .color(muted_color)
                                        .size(typography::MD),
                                );

                                // Buttons row
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    // "See changes" - ghost button
                                    let see_changes_btn = ui.add(
                                        egui::Button::new(
                                            RichText::new("See changes")
                                                .color(muted_color)
                                                .size(typography::MD),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::NONE)
                                        .corner_radius(6.0),
                                    );
                                    if see_changes_btn.hovered() {
                                        ui.painter().rect_filled(
                                            see_changes_btn.rect,
                                            6.0,
                                            theme.bg_hover(),
                                        );
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if see_changes_btn.clicked() {
                                        action = UpdateBannerAction::SeeChanges(
                                            self.release_url.clone(),
                                        );
                                    }

                                    ui.add_space(4.0);

                                    // Primary action - accent filled
                                    if self.has_download {
                                        let label = if self.is_downloading {
                                            "Updating\u{2026}"
                                        } else {
                                            "Restart"
                                        };
                                        let restart_btn = ui.add_enabled(
                                            !self.is_downloading,
                                            egui::Button::new(
                                                RichText::new(label)
                                                    .color(theme.bg_base())
                                                    .size(typography::MD),
                                            )
                                            .fill(accent_color)
                                            .stroke(egui::Stroke::NONE)
                                            .corner_radius(6.0),
                                        );
                                        if restart_btn.clicked() {
                                            action = UpdateBannerAction::Restart;
                                        }
                                        if restart_btn.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                    } else {
                                        let download_btn = ui.add(
                                            egui::Button::new(
                                                RichText::new("Download")
                                                    .color(theme.bg_base())
                                                    .size(typography::MD),
                                            )
                                            .fill(accent_color)
                                            .stroke(egui::Stroke::NONE)
                                            .corner_radius(6.0),
                                        );
                                        if download_btn.clicked() {
                                            action = UpdateBannerAction::SeeChanges(
                                                self.release_url.clone(),
                                            );
                                        }
                                        if download_btn.hovered() {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                    }
                                });
                            });

                            // Paint accent bar spanning the full content height
                            let content_bottom = ui.min_rect().max.y;
                            let bar_rect = egui::Rect::from_min_size(
                                bar_start,
                                egui::vec2(3.0, content_bottom - bar_start.y),
                            );
                            ui.painter().rect_filled(bar_rect, 2.0, accent_color);
                            let _ = bar_id;
                        });
                    });
            });

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_banner() {
        let banner = UpdateBanner::new(
            "0.2.0".to_string(),
            "https://github.com/meldrumlabs/enya/releases/tag/v0.2.0".to_string(),
            String::new(),
            true,
        );
        assert_eq!(banner.version, "0.2.0");
        assert!(banner.has_download);
        assert!(!banner.is_downloading);
    }

    #[test]
    fn test_set_theme() {
        let mut banner =
            UpdateBanner::new("0.2.0".to_string(), String::new(), String::new(), false);
        banner.set_theme(AppTheme::Dark);
        // Doesn't panic
    }
}
