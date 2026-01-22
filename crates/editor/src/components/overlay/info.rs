//! Info overlay component for displaying build information in a terminal-style overlay.

use egui::{Color32, Key, RichText};
use enya_build_info::BuildInfo;

use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;

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
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if !self.is_open {
            return false;
        }

        let mut should_close = false;

        // Handle keyboard input - use consume_key to prevent multiple processing
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                should_close = true;
            }
        });

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.4).clamp(400.0, 600.0);
        let popup_max_height = (screen_rect.height() * 0.5).min(400.0);

        egui::Area::new(egui::Id::new("info_overlay_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = self.theme.border_subtle();
                let muted_text = text_color(self.theme).gamma_multiply(0.6);
                let accent_color = self.theme.accent_hover();
                let key_color = self.theme.text_tertiary();
                let value_color = text_color(self.theme);

                overlay_style.frame().show(ui, |ui| {
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
                                        self.info_row(ui, "Mode", "debug", key_color, value_color);
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
                                .font(typography::proportional(typography::MD)),
                        );
                        ui.label(
                            RichText::new("Esc")
                                .color(key_color)
                                .font(typography::monospace(typography::MD)),
                        );
                        ui.label(
                            RichText::new(" to close")
                                .color(muted_text)
                                .font(typography::proportional(typography::MD)),
                        );
                    });
                    ui.add_space(12.0);
                });
            });

        if should_close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
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
                .font(typography::monospace(typography::XL)),
        );
        ui.label(
            RichText::new(value)
                .color(value_color)
                .font(typography::monospace(typography::XL)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_build_info() -> BuildInfo {
        BuildInfo {
            crate_name: "enya-editor",
            version: enya_build_info::CrateVersion::new(0, 1, 0),
            git_branch: "main",
            git_hash: "abc123",
            datetime: "2024-01-01",
            target_triple: "x86_64-unknown-linux-gnu",
            rustc_version: "1.75.0",
            llvm_version: "17.0",
            features: "default",
            is_in_enya_workspace: true,
        }
    }

    #[test]
    fn test_new_overlay_is_closed() {
        let overlay = InfoOverlay::new(test_build_info());
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_open_close() {
        let mut overlay = InfoOverlay::new(test_build_info());
        overlay.open();
        assert!(overlay.is_open());
        overlay.close();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_theme_can_be_set() {
        let mut overlay = InfoOverlay::new(test_build_info());
        overlay.set_theme(AppTheme::Dark);
        // Theme is stored internally - test that it doesn't panic
    }

    // Note: Testing surrender_focus behavior requires egui::Context.
    // The surrender_focus pattern is verified through code review and
    // manual testing. Key invariant: When show() returns true (close requested),
    // the overlay must call ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL))
    // BEFORE calling self.close() to ensure vim navigation works immediately.
}
