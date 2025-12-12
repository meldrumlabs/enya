//! Which-key style overlay component for displaying available keybindings.
//!
//! Inspired by the neovim which-key.nvim plugin, this component displays
//! available keyboard shortcuts in a floating popup when the user presses `?`.

use egui::{Color32, FontId, Key, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

/// A keybinding with its key and description
#[derive(Clone)]
pub struct Keybinding {
    /// The key(s) to press (e.g., "h", "Ctrl+k", "yy")
    pub key: &'static str,
    /// Description of what the keybinding does
    pub description: &'static str,
}

/// A group of related keybindings
#[derive(Clone)]
pub struct KeybindingGroup {
    /// Name of the group (e.g., "Navigation", "Panes")
    pub name: &'static str,
    /// Icon for the group (phosphor icon)
    pub icon: &'static str,
    /// The keybindings in this group
    pub bindings: Vec<Keybinding>,
}

/// A modal overlay that displays available keybindings in a which-key style
pub struct WhichKey {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip input on the first frame after opening (to avoid immediate close)
    just_opened: bool,
    /// Current theme
    theme: AppTheme,
    /// Keybinding groups
    groups: Vec<KeybindingGroup>,
}

impl Default for WhichKey {
    fn default() -> Self {
        Self::new()
    }
}

impl WhichKey {
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            groups: Self::build_keybindings(),
        }
    }

    /// Build the default keybinding groups
    fn build_keybindings() -> Vec<KeybindingGroup> {
        vec![
            KeybindingGroup {
                name: "Navigation",
                icon: semantic_icons::nav::COMPASS,
                bindings: vec![
                    Keybinding {
                        key: "h / ←",
                        description: "Move focus left",
                    },
                    Keybinding {
                        key: "j / ↓",
                        description: "Move focus down",
                    },
                    Keybinding {
                        key: "k / ↑",
                        description: "Move focus up",
                    },
                    Keybinding {
                        key: "l / →",
                        description: "Move focus right",
                    },
                ],
            },
            KeybindingGroup {
                name: "Panes",
                icon: semantic_icons::nav::PANES,
                bindings: vec![
                    Keybinding {
                        key: "x",
                        description: "Close focused pane",
                    },
                    Keybinding {
                        key: "f",
                        description: "Toggle fullscreen",
                    },
                ],
            },
            KeybindingGroup {
                name: "Editor",
                icon: semantic_icons::action::EDIT,
                bindings: vec![
                    Keybinding {
                        key: "e",
                        description: "Edit focused pane query",
                    },
                    Keybinding {
                        key: "yy",
                        description: "Share/yank pane URL",
                    },
                ],
            },
            KeybindingGroup {
                name: "View",
                icon: semantic_icons::mode::VIEW,
                bindings: vec![
                    Keybinding {
                        key: "z",
                        description: "Toggle zen mode",
                    },
                    Keybinding {
                        key: "t",
                        description: "Toggle theme",
                    },
                    Keybinding {
                        key: ":diff",
                        description: "Compare time periods",
                    },
                ],
            },
            KeybindingGroup {
                name: "Search & Commands",
                icon: semantic_icons::action::SEARCH,
                bindings: vec![
                    Keybinding {
                        key: ":",
                        description: "Open command palette",
                    },
                    Keybinding {
                        key: "m",
                        description: "Open metrics finder",
                    },
                    Keybinding {
                        key: "q",
                        description: "Open query finder",
                    },
                    Keybinding {
                        key: "Ctrl+k / Ctrl+p",
                        description: "Open metrics finder",
                    },
                    Keybinding {
                        key: "?",
                        description: "Show this help",
                    },
                ],
            },
            KeybindingGroup {
                name: "Chart Controls",
                icon: semantic_icons::action::CHART,
                bindings: vec![
                    Keybinding {
                        key: "+ / =",
                        description: "Zoom in time range",
                    },
                    Keybinding {
                        key: "- / _",
                        description: "Zoom out time range",
                    },
                    Keybinding {
                        key: "[ / {",
                        description: "Pan time left",
                    },
                    Keybinding {
                        key: "] / }",
                        description: "Pan time right",
                    },
                    Keybinding {
                        key: "0",
                        description: "Reset to default range",
                    },
                ],
            },
        ]
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay
    pub fn open(&mut self) {
        self.is_open = true;
        self.just_opened = true;
    }

    /// Close the overlay
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if the overlay is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the overlay. Returns true if it was closed this frame.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if !self.is_open {
            return false;
        }

        let mut should_close = false;

        // Skip input handling on the first frame after opening
        // This prevents the same key press that opened us from closing us
        if self.just_opened {
            self.just_opened = false;
        } else {
            // Handle keyboard input - close on Escape or ?
            // Use consume_key to prevent the key from being processed multiple times
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    should_close = true;
                }
                // Check for ? (Shift+/) - consume it to toggle off
                if i.consume_key(egui::Modifiers::SHIFT, Key::Slash) {
                    should_close = true;
                }
            });
        }

        // Calculate popup dimensions - wider to fit content in columns
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.7).clamp(600.0, 900.0);
        let popup_max_height = (screen_rect.height() * 0.75).clamp(400.0, 600.0);

        egui::Area::new(egui::Id::new("which_key_popup"))
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
                let key_bg = match self.theme {
                    AppTheme::Light => palette::light_bg::ELEVATED,
                    AppTheme::Dark => palette::bg::ELEVATED,
                };
                let accent_color = match self.theme {
                    AppTheme::Light => palette::accent::LIGHT,
                    AppTheme::Dark => palette::accent::PRIMARY,
                };

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
                                RichText::new(semantic_icons::keyboard::KEYBOARD)
                                    .color(muted_text)
                                    .size(20.0),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Keyboard Shortcuts")
                                    .color(text_color(self.theme))
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
                        ui.add_space(16.0);

                        // Content area with keybinding groups in a 2-column layout
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.vertical(|ui| {
                                ui.set_width(popup_width - 32.0);

                                // Split groups into two columns
                                let groups = &self.groups;
                                let mid = groups.len().div_ceil(2);

                                ui.columns(2, |columns| {
                                    // Left column
                                    for group in groups.iter().take(mid) {
                                        Self::render_group(
                                            &mut columns[0],
                                            group,
                                            self.theme,
                                            accent_color,
                                            muted_text,
                                            key_bg,
                                        );
                                        columns[0].add_space(16.0);
                                    }

                                    // Right column
                                    for group in groups.iter().skip(mid) {
                                        Self::render_group(
                                            &mut columns[1],
                                            group,
                                            self.theme,
                                            accent_color,
                                            muted_text,
                                            key_bg,
                                        );
                                        columns[1].add_space(16.0);
                                    }
                                });
                            });
                        });

                        ui.add_space(8.0);

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
                            Self::render_key_badge(ui, "Esc", key_bg, accent_color);
                            ui.label(
                                RichText::new(" or ")
                                    .color(muted_text)
                                    .font(FontId::proportional(12.0)),
                            );
                            Self::render_key_badge(ui, "?", key_bg, accent_color);
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

    /// Render a group of keybindings
    fn render_group(
        ui: &mut egui::Ui,
        group: &KeybindingGroup,
        theme: AppTheme,
        accent_color: Color32,
        muted_text: Color32,
        key_bg: Color32,
    ) {
        // Group header with icon
        ui.horizontal(|ui| {
            ui.label(RichText::new(group.icon).color(accent_color).size(14.0));
            ui.add_space(4.0);
            ui.label(
                RichText::new(group.name)
                    .color(accent_color)
                    .size(13.0)
                    .strong(),
            );
        });
        ui.add_space(6.0);

        // Keybindings
        for binding in &group.bindings {
            ui.horizontal(|ui| {
                ui.add_space(20.0); // Indent under group header

                // Key badge
                Self::render_key_badge(ui, binding.key, key_bg, text_color(theme));

                ui.add_space(8.0);

                // Description
                ui.label(
                    RichText::new(binding.description)
                        .color(muted_text)
                        .font(FontId::proportional(13.0)),
                );
            });
            ui.add_space(2.0);
        }
    }

    /// Render a keyboard key badge (like a physical key cap)
    fn render_key_badge(ui: &mut egui::Ui, key: &str, bg_color: Color32, text_color: Color32) {
        let font = FontId::monospace(12.0);
        let text = RichText::new(key).color(text_color).font(font);

        egui::Frame::new()
            .fill(bg_color)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .stroke(egui::Stroke::new(1.0, text_color.gamma_multiply(0.2)))
            .show(ui, |ui| {
                ui.label(text);
            });
    }
}
