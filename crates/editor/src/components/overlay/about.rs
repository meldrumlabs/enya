//! About overlay - displays information about the Enya project.

use egui::{Color32, Key, RichText};

use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;

/// A modal overlay that displays information about the Enya project
pub struct AboutOverlay {
    /// Whether the overlay is open
    is_open: bool,
    /// Current theme
    theme: AppTheme,
}

impl Default for AboutOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl AboutOverlay {
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
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

        // Handle keyboard input
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));

        if escape {
            should_close = true;
        }

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.4).clamp(400.0, 600.0);

        egui::Area::new(egui::Id::new("about_overlay_popup"))
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
                            RichText::new("About Enya")
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

                    // Content area
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.set_width(popup_width - 32.0);

                            // Tagline
                            ui.label(
                                RichText::new("A keyboard-first observability editor")
                                    .color(value_color)
                                    .size(typography::LG),
                            );

                            ui.add_space(12.0);

                            // Description
                            ui.label(
                                RichText::new(
                                    "Connects metrics, logs, traces, SQL, and git with AI in one \
                                     interface — designed for those who build, ship, and get paged.",
                                )
                                .color(muted_text)
                                .size(typography::MD),
                            );

                            ui.add_space(16.0);

                            // Info grid
                            egui::Grid::new("about_info_grid")
                                .num_columns(2)
                                .spacing([20.0, 8.0])
                                .show(ui, |ui| {
                                    self.info_row(
                                        ui,
                                        "Version",
                                        &format!("v{}", env!("CARGO_PKG_VERSION")),
                                        key_color,
                                        value_color,
                                    );
                                    ui.end_row();

                                    self.info_row(ui, "License", "MIT", key_color, value_color);
                                    ui.end_row();

                                    // Source with clickable link
                                    ui.label(
                                        RichText::new("Source")
                                            .color(key_color)
                                            .font(typography::monospace(typography::MD)),
                                    );
                                    let github_response = ui.add(
                                        egui::Label::new(
                                            RichText::new("github.com/meldrumlabs/enya")
                                                .color(accent_color)
                                                .font(typography::monospace(typography::MD)),
                                        )
                                        .sense(egui::Sense::click()),
                                    );
                                    if github_response.clicked() {
                                        ui.ctx().open_url(egui::OpenUrl::new_tab(
                                            "https://github.com/meldrumlabs/enya",
                                        ));
                                    }
                                    if github_response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    ui.end_row();

                                    // Developer with clickable link
                                    ui.label(
                                        RichText::new("Developer")
                                            .color(key_color)
                                            .font(typography::monospace(typography::MD)),
                                    );
                                    let meldrum_response = ui.add(
                                        egui::Label::new(
                                            RichText::new("Meldrum Labs")
                                                .color(accent_color)
                                                .font(typography::monospace(typography::MD)),
                                        )
                                        .sense(egui::Sense::click()),
                                    );
                                    if meldrum_response.clicked() {
                                        ui.ctx().open_url(egui::OpenUrl::new_tab(
                                            "https://meldrumlabs.com",
                                        ));
                                    }
                                    if meldrum_response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    ui.end_row();
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
                    ui.add_space(12.0);

                    // Memorial dedication (centered)
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("In memory of Enya — the family dog")
                                .color(muted_text.gamma_multiply(0.6))
                                .size(typography::XS)
                                .italics(),
                        );
                    });

                    ui.add_space(8.0);

                    // Minimal footer hint (centered)
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Esc to close")
                                .color(muted_text.gamma_multiply(0.5))
                                .size(typography::XS),
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
                .font(typography::monospace(typography::MD)),
        );
        ui.label(
            RichText::new(value)
                .color(value_color)
                .font(typography::monospace(typography::MD)),
        );
    }
}
