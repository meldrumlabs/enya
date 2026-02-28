//! Native app promo overlay for WASM builds.
//!
//! This overlay is shown on the landing page in WASM builds to inform users
//! about features that are only available in the native desktop app, such as
//! git integration and AI agents.

use egui::{Key, RichText};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;

/// Detect the user's operating system from the browser's user agent
fn detect_os() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(navigator) = window.navigator().user_agent() {
                let ua = navigator.to_lowercase();
                if ua.contains("mac") {
                    return "macOS";
                } else if ua.contains("win") {
                    return "Windows";
                } else if ua.contains("linux") {
                    return "Linux";
                }
            }
        }
    }
    // Default fallback
    "Desktop"
}

/// A feature item for the native app promo
struct NativeFeature {
    /// Icon for the feature
    icon: &'static str,
    /// Feature title
    title: &'static str,
    /// Feature description
    description: &'static str,
}

/// Features only available in native app
const NATIVE_FEATURES: &[NativeFeature] = &[
    NativeFeature {
        icon: semantic_icons::git::BRANCH,
        title: "Git Integration",
        description: "Let Enya integrate and analyze your codebase together with metrics",
    },
    NativeFeature {
        icon: semantic_icons::action::BRAIN,
        title: "AI Agents",
        description: "Run Codex or Claude to create, understand and debug metrics",
    },
    NativeFeature {
        icon: semantic_icons::language::LUA,
        title: "Plugins",
        description: "Create your own Lua-based plugins with custom panes, charts, and integrations",
    },
];

/// Overlay to promote the native desktop app on WASM builds
pub struct NativePromoOverlay {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip input on the first frame after opening
    just_opened: bool,
    /// Current theme
    theme: AppTheme,
    /// Whether the user has dismissed this overlay (persists for session)
    dismissed: bool,
}

impl Default for NativePromoOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePromoOverlay {
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            dismissed: false,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay (if not previously dismissed this session)
    pub fn open(&mut self) {
        if !self.dismissed && !self.is_open {
            self.is_open = true;
            self.just_opened = true;
        }
    }

    /// Open the overlay and force show even if dismissed
    pub fn open_force(&mut self) {
        self.is_open = true;
        self.just_opened = true;
    }

    /// Close the overlay
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Dismiss the overlay for this session
    pub fn dismiss(&mut self) {
        self.is_open = false;
        self.dismissed = true;
    }

    /// Check if the overlay is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the overlay.
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut should_dismiss = false;

        // Skip input handling on the first frame after opening
        if self.just_opened {
            self.just_opened = false;
        } else {
            // Handle keyboard input
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape)
                    || i.consume_key(egui::Modifiers::NONE, Key::Enter)
                {
                    should_dismiss = true;
                }
            });
        }

        // Calculate popup dimensions
        let popup_width = crate::util::overlay_width(ctx, 0.50, 500.0, 620.0);
        let content_width = popup_width - 48.0; // 24px padding on each side

        // Extract colors from theme
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let separator_color = self.theme.border_subtle();
        let muted_text = self.theme.text_primary().gamma_multiply(0.6);
        let accent_color = self.theme.accent_primary();
        let text_col = self.theme.text_primary();

        egui::Area::new(egui::Id::new("native_promo_overlay_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .constrain_to(crate::util::overlay_content_rect(ctx))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header section with icon and title
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new(semantic_icons::action::IMPORT)
                                .color(accent_color)
                                .size(28.0),
                        );
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("Web Preview")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Get the Full Experience")
                                    .color(text_col)
                                    .size(typography::HEADING)
                                    .strong(),
                            );
                        });
                    });
                    ui.add_space(16.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(20.0);

                    // Description text
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.vertical(|ui| {
                            ui.set_width(content_width);
                            ui.label(
                                RichText::new(
                                    "You're using the web version of Enya. Download the native desktop app to access features including:",
                                )
                                .color(text_col)
                                .size(typography::MD),
                            );
                        });
                    });
                    ui.add_space(20.0);

                    // Feature list
                    for feature in NATIVE_FEATURES {
                        ui.horizontal(|ui| {
                            ui.add_space(24.0);
                            ui.label(
                                RichText::new(feature.icon)
                                    .color(accent_color)
                                    .size(typography::XL),
                            );
                            ui.add_space(14.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(feature.title)
                                        .color(text_col)
                                        .size(typography::MD)
                                        .strong(),
                                );
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(feature.description)
                                        .color(muted_text)
                                        .size(typography::SM),
                                );
                            });
                        });
                        ui.add_space(14.0);
                    }
                    ui.add_space(4.0);

                    // Download link - centered
                    let os_name = detect_os();
                    ui.vertical_centered(|ui| {
                        let download_response = ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "{}  Download for {}",
                                    semantic_icons::action::IMPORT,
                                    os_name
                                ))
                                .color(accent_color)
                                .size(typography::LG)
                                .strong(),
                            )
                            .sense(egui::Sense::click()),
                        );

                        if download_response.clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab("https://enya.build/download"));
                        }

                        if download_response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    });
                    ui.add_space(20.0);

                    // Separator above footer
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(12.0);

                    // Footer - minimal
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Esc to close")
                                .color(muted_text.gamma_multiply(0.7))
                                .size(typography::XS),
                        );
                    });
                    ui.add_space(12.0);
                });
            });

        if should_dismiss {
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.dismiss();
        }
    }
}
