//! Which-key style overlay component for displaying available keybindings.
//!
//! Inspired by the neovim which-key.nvim plugin, this component displays
//! available keyboard shortcuts in a floating popup when the user presses `?`.

use egui::{Color32, Key, RichText};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, render_key_badge};

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
    /// Current theme (can be Custom with plugin colors)
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
                        key: "h/j/k/l",
                        description: "Move focus (vim-style)",
                    },
                    Keybinding {
                        key: "←/↓/↑/→",
                        description: "Move focus (arrows)",
                    },
                ],
            },
            KeybindingGroup {
                name: "Panes",
                icon: semantic_icons::nav::PANES,
                bindings: vec![
                    Keybinding {
                        key: "x",
                        description: "Close pane",
                    },
                    Keybinding {
                        key: "f",
                        description: "Fullscreen",
                    },
                    Keybinding {
                        key: "z",
                        description: "Zen mode",
                    },
                    Keybinding {
                        key: "gf",
                        description: "Float pane",
                    },
                    Keybinding {
                        key: "Ctrl+V",
                        description: "Multi-select",
                    },
                ],
            },
            KeybindingGroup {
                name: "Window",
                icon: semantic_icons::nav::EXPAND_ALL,
                bindings: vec![
                    Keybinding {
                        key: "Ctrl+W h/j/k/l",
                        description: "Move pane",
                    },
                    Keybinding {
                        key: "Ctrl+W t h/j/k/l",
                        description: "Merge into tab",
                    },
                ],
            },
            KeybindingGroup {
                name: "Editor",
                icon: semantic_icons::action::EDIT,
                bindings: vec![
                    Keybinding {
                        key: "e",
                        description: "Edit query",
                    },
                    Keybinding {
                        key: "yy",
                        description: "Share/yank URL",
                    },
                    Keybinding {
                        key: "cv",
                        description: "Cycle viz type",
                    },
                ],
            },
            KeybindingGroup {
                name: "Search",
                icon: semantic_icons::action::SEARCH,
                bindings: vec![
                    Keybinding {
                        key: ":",
                        description: "Commands",
                    },
                    Keybinding {
                        key: "/",
                        description: "Filter panes",
                    },
                    Keybinding {
                        key: "Space+f",
                        description: "Find anything",
                    },
                    Keybinding {
                        key: "Space+w",
                        description: "Find workspace",
                    },
                ],
            },
            KeybindingGroup {
                name: "Go To",
                icon: semantic_icons::action::LINK,
                bindings: vec![
                    Keybinding {
                        key: "Space+h",
                        description: "Home",
                    },
                    Keybinding {
                        key: "Space+d",
                        description: "Diagnostics",
                    },
                    Keybinding {
                        key: "Space+p",
                        description: "Plugins",
                    },
                    Keybinding {
                        key: "gd",
                        description: "Definition",
                    },
                    Keybinding {
                        key: "ga",
                        description: "Alert",
                    },
                ],
            },
            KeybindingGroup {
                name: "Time Range",
                icon: semantic_icons::time::CLOCK,
                bindings: vec![
                    Keybinding {
                        key: "Space+t",
                        description: "Time picker",
                    },
                    Keybinding {
                        key: "t5/t1/t3",
                        description: "5/15/30 min",
                    },
                    Keybinding {
                        key: "th/t6",
                        description: "1h/6h",
                    },
                    Keybinding {
                        key: "td/tw",
                        description: "Day/week",
                    },
                ],
            },
            KeybindingGroup {
                name: "Agent",
                icon: semantic_icons::action::BRAIN,
                bindings: vec![
                    Keybinding {
                        key: "Space+a",
                        description: "Agent panel",
                    },
                    Keybinding {
                        key: "aa",
                        description: "Ask agent",
                    },
                    Keybinding {
                        key: "aw/ae/ay",
                        description: "What/Explain/Why",
                    },
                ],
            },
            KeybindingGroup {
                name: "Help",
                icon: semantic_icons::status::INFO,
                bindings: vec![Keybinding {
                    key: "?",
                    description: "This help",
                }],
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
    #[profiling::function]
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

        // Calculate popup dimensions - wide for 3-column layout
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.8).clamp(700.0, 1000.0);
        let popup_max_height = (screen_rect.height() * 0.6).clamp(300.0, 450.0);

        egui::Area::new(egui::Id::new("which_key_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Extract colors from theme (handles both builtin and custom themes)
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = self.theme.border_subtle();
                let text_col = self.theme.text_primary();
                let muted_text = self.theme.text_tertiary();
                let key_bg = self.theme.bg_elevated();
                let accent_color = self.theme.accent_primary();

                overlay_style.frame().show(ui, |ui| {
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
                                .color(text_col)
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

                            // Split groups into three columns for compact layout
                            let groups = &self.groups;
                            let col_size = groups.len().div_ceil(3);

                            ui.columns(3, |columns| {
                                for (i, group) in groups.iter().enumerate() {
                                    let col = i / col_size;
                                    let col = col.min(2); // Ensure we don't overflow
                                    Self::render_group(
                                        &mut columns[col],
                                        group,
                                        accent_color,
                                        muted_text,
                                        key_bg,
                                        text_col,
                                    );
                                    columns[col].add_space(12.0);
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
                                .font(typography::proportional(typography::MD)),
                        );
                        render_key_badge(ui, "Esc", key_bg, accent_color);
                        ui.label(
                            RichText::new(" or ")
                                .color(muted_text)
                                .font(typography::proportional(typography::MD)),
                        );
                        render_key_badge(ui, "?", key_bg, accent_color);
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

    /// Render a group of keybindings
    fn render_group(
        ui: &mut egui::Ui,
        group: &KeybindingGroup,
        accent_color: Color32,
        muted_text: Color32,
        key_bg: Color32,
        text_col: Color32,
    ) {
        // Group header with icon
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(group.icon)
                    .color(accent_color)
                    .size(typography::LG),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(group.name)
                    .color(accent_color)
                    .size(typography::MD)
                    .strong(),
            );
        });
        ui.add_space(4.0);

        // Keybindings
        for binding in &group.bindings {
            ui.horizontal(|ui| {
                ui.add_space(16.0); // Indent under group header

                // Key badge
                render_key_badge(ui, binding.key, key_bg, text_col);

                ui.add_space(6.0);

                // Description
                ui.label(
                    RichText::new(binding.description)
                        .color(muted_text)
                        .font(typography::proportional(typography::MD)),
                );
            });
            ui.add_space(1.0);
        }
    }
}
