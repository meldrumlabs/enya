//! Info overlay component for displaying build information in a terminal-style overlay.

use egui::{Color32, FontId, Key, RichText};
use enya_build_info::BuildInfo;

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

/// A modal overlay that displays build and version information
pub struct InfoOverlay {
    /// Whether the overlay is open
    is_open: bool,
    /// Current theme
    theme: AppTheme,
    /// Build info to display
    build_info: BuildInfo,
}

impl InfoOverlay {
    pub fn new(build_info: BuildInfo) -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
            build_info,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay
    pub fn open(&mut self) {
        self.is_open = true;
    }

    /// Close the overlay
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if the overlay is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the overlay. Returns true if it should be closed.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if !self.is_open {
            return false;
        }

        let mut should_close = false;

        // Handle keyboard input
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));

        if escape {
            should_close = true;
        }

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.4).clamp(400.0, 600.0);
        let popup_max_height = (screen_rect.height() * 0.5).min(400.0);

        egui::Area::new(egui::Id::new("info_overlay_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let bg_color = match self.theme {
                    AppTheme::Light => palette::light_bg::SURFACE,
                    AppTheme::Dark => palette::bg::SURFACE,
                };
                let border_color = match self.theme {
                    AppTheme::Light => palette::light_border::DEFAULT,
                    AppTheme::Dark => palette::border::SUBTLE,
                };
                let separator_color = match self.theme {
                    AppTheme::Light => palette::light_border::SUBTLE,
                    AppTheme::Dark => palette::border::SUBTLE,
                };
                let muted_text = text_color(self.theme).gamma_multiply(0.6);
                let accent_color = match self.theme {
                    AppTheme::Light => palette::accent::LIGHT,
                    AppTheme::Dark => palette::accent::HOVER,
                };
                let key_color = match self.theme {
                    AppTheme::Light => palette::light_text::TERTIARY,
                    AppTheme::Dark => palette::text::TERTIARY,
                };
                let value_color = text_color(self.theme);

                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .corner_radius(8.0)
                    .inner_margin(0.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);
                        ui.set_max_height(popup_max_height);

                        // Header section
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(semantic_icons::status::INFO)
                                    .color(accent_color)
                                    .size(20.0),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Build Info")
                                    .color(accent_color)
                                    .size(18.0)
                                    .strong(),
                            );
                        });
                        ui.add_space(12.0);

                        // Separator below header
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(12.0);

                        // Content area with build info
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.vertical(|ui| {
                                ui.set_width(popup_width - 32.0);

                                // Display build info in a terminal-style grid
                                egui::Grid::new("build_info_grid")
                                    .num_columns(2)
                                    .spacing([20.0, 8.0])
                                    .show(ui, |ui| {
                                        self.info_row(
                                            ui,
                                            "Version",
                                            &self.build_info.version.to_string(),
                                            key_color,
                                            value_color,
                                        );
                                        ui.end_row();

                                        if !self.build_info.git_branch.is_empty() {
                                            self.info_row(
                                                ui,
                                                "Branch",
                                                self.build_info.git_branch,
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }

                                        if !self.build_info.git_hash.is_empty() {
                                            self.info_row(
                                                ui,
                                                "Commit",
                                                self.build_info.short_git_hash(),
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }

                                        if !self.build_info.target_triple.is_empty() {
                                            self.info_row(
                                                ui,
                                                "Target",
                                                self.build_info.target_triple,
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }

                                        if !self.build_info.rustc_version.is_empty() {
                                            self.info_row(
                                                ui,
                                                "Rustc",
                                                self.build_info.rustc_version,
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }

                                        if !self.build_info.llvm_version.is_empty() {
                                            self.info_row(
                                                ui,
                                                "LLVM",
                                                self.build_info.llvm_version,
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }

                                        if !self.build_info.datetime.is_empty() {
                                            self.info_row(
                                                ui,
                                                "Built",
                                                self.build_info.datetime,
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }

                                        if !self.build_info.features.is_empty() {
                                            self.info_row(
                                                ui,
                                                "Features",
                                                self.build_info.features,
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }

                                        // Debug mode indicator
                                        if cfg!(debug_assertions) {
                                            self.info_row(
                                                ui,
                                                "Mode",
                                                "debug",
                                                key_color,
                                                value_color,
                                            );
                                            ui.end_row();
                                        }
                                    });
                            });
                        });

                        ui.add_space(16.0);

                        // Separator above footer
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(8.0);

                        // Footer with keyboard hints
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("Press ")
                                    .color(muted_text)
                                    .font(FontId::proportional(12.0)),
                            );
                            ui.label(
                                RichText::new("Esc")
                                    .color(key_color)
                                    .font(FontId::monospace(12.0)),
                            );
                            ui.label(
                                RichText::new(" to close")
                                    .color(muted_text)
                                    .font(FontId::proportional(12.0)),
                            );
                        });
                        ui.add_space(12.0);
                    });
            });

        if should_close {
            self.close();
        }

        should_close
    }

    fn info_row(
        &self,
        ui: &mut egui::Ui,
        key: &str,
        value: &str,
        key_color: Color32,
        value_color: Color32,
    ) {
        ui.label(
            RichText::new(key)
                .color(key_color)
                .font(FontId::monospace(14.0)),
        );
        ui.label(
            RichText::new(value)
                .color(value_color)
                .font(FontId::monospace(14.0)),
        );
    }
}
