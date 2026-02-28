//! Which-key style overlay component for displaying available keybindings.
//!
//! Inspired by the neovim which-key.nvim plugin, this component displays
//! available keyboard shortcuts in a floating popup when the user presses `?`.
//! Uses a tabbed layout to keep the overlay compact on smaller screens.

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

/// A tab containing one or more keybinding groups
struct Tab {
    /// Display label for the tab
    label: &'static str,
    /// Icon for the tab
    icon: &'static str,
    /// Indices into the groups vec
    group_indices: Vec<usize>,
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
    /// Tab definitions (indices into groups)
    tabs: Vec<Tab>,
    /// Currently active tab index
    active_tab: usize,
}

impl Default for WhichKey {
    fn default() -> Self {
        Self::new()
    }
}

impl WhichKey {
    pub fn new() -> Self {
        let groups = Self::build_keybindings();
        let tabs = Self::build_tabs();
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            groups,
            tabs,
            active_tab: 0,
        }
    }

    /// Build the default keybinding groups
    fn build_keybindings() -> Vec<KeybindingGroup> {
        vec![
            // 0: Navigation
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
            // 1: Panes
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
            // 2: Window
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
            // 3: Editor
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
                    Keybinding {
                        key: "ct",
                        description: "Cycle theme",
                    },
                ],
            },
            // 4: Search
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
            // 5: Go To
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
            // 6: Time Range
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
            // 7: Agent
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
            // 8: Help
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

    /// Build tab definitions that group related keybinding groups together
    fn build_tabs() -> Vec<Tab> {
        vec![
            Tab {
                label: "Navigate",
                icon: semantic_icons::nav::COMPASS,
                group_indices: vec![0, 1, 2], // Navigation, Panes, Window
            },
            Tab {
                label: "Edit",
                icon: semantic_icons::action::EDIT,
                group_indices: vec![3, 4], // Editor, Search
            },
            Tab {
                label: "Go To",
                icon: semantic_icons::action::LINK,
                group_indices: vec![5, 6], // Go To, Time Range
            },
            Tab {
                label: "Agent",
                icon: semantic_icons::action::BRAIN,
                group_indices: vec![7, 8], // Agent, Help
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

    /// Show the overlay.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut should_close = false;
        let tab_count = self.tabs.len();

        // Skip input handling on the first frame after opening
        // This prevents the same key press that opened us from closing us
        if self.just_opened {
            self.just_opened = false;
        } else {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    should_close = true;
                }
                // ? (Shift+/) to toggle off
                if i.consume_key(egui::Modifiers::SHIFT, Key::Slash) {
                    should_close = true;
                }
                // Tab navigation: l/Right = next, h/Left = prev
                if i.consume_key(egui::Modifiers::NONE, Key::L)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(egui::Modifiers::NONE, Key::Tab)
                {
                    self.active_tab = (self.active_tab + 1) % tab_count;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::H)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft)
                {
                    self.active_tab = (self.active_tab + tab_count - 1) % tab_count;
                }
                // Number keys for direct tab access
                if i.consume_key(egui::Modifiers::NONE, Key::Num1) {
                    self.active_tab = 0;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::Num2) && tab_count > 1 {
                    self.active_tab = 1;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::Num3) && tab_count > 2 {
                    self.active_tab = 2;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::Num4) && tab_count > 3 {
                    self.active_tab = 3;
                }
            });
        }

        let content_rect = crate::util::overlay_content_rect(ctx);
        let popup_width = crate::util::overlay_width(ctx, 0.70, 480.0, 700.0);
        // Fixed row count so every tab renders at the same height.
        // The largest tabs (Navigate, Go To) have 9 bindings → 5 rows in 2 columns.
        const GRID_ROWS: usize = 5;

        egui::Area::new(egui::Id::new("which_key_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .constrain_to(content_rect)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = self.theme.border_subtle();
                let text_col = self.theme.text_primary();
                let muted_text = self.theme.text_tertiary();
                let key_bg = self.theme.bg_elevated();
                let accent_color = self.theme.accent_primary();
                let bg_surface = self.theme.bg_surface();

                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::keyboard::KEYBOARD)
                                .color(muted_text)
                                .size(16.0),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("Keyboard Shortcuts")
                                .color(text_col)
                                .size(14.0)
                                .strong(),
                        );
                    });
                    ui.add_space(8.0);

                    // Tab bar
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let tab_labels: Vec<(&str, &str)> =
                            self.tabs.iter().map(|t| (t.icon, t.label)).collect();
                        for (i, (icon, label)) in tab_labels.iter().enumerate() {
                            let is_active = i == self.active_tab;
                            let btn_text = if is_active {
                                RichText::new(format!("{icon} {label}"))
                                    .color(accent_color)
                                    .size(typography::MD)
                                    .strong()
                            } else {
                                RichText::new(format!("{icon} {label}"))
                                    .color(muted_text)
                                    .size(typography::MD)
                            };

                            let btn = egui::Button::new(btn_text)
                                .fill(if is_active {
                                    bg_surface
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(if is_active {
                                    egui::Stroke::new(1.0, separator_color)
                                } else {
                                    egui::Stroke::NONE
                                })
                                .corner_radius(4.0);

                            if ui.add(btn).clicked() {
                                self.active_tab = i;
                            }
                        }
                    });
                    ui.add_space(6.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(10.0);

                    // Content: flat 2-column grid with fixed row count
                    if let Some(tab) = self.tabs.get(self.active_tab) {
                        let bindings: Vec<&Keybinding> = tab
                            .group_indices
                            .iter()
                            .filter_map(|&idx| self.groups.get(idx))
                            .flat_map(|g| g.bindings.iter())
                            .collect();

                        let col_size = GRID_ROWS;

                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.columns(2, |cols| {
                                for row in 0..GRID_ROWS {
                                    // Left column
                                    if let Some(b) = bindings.get(row) {
                                        Self::render_binding(
                                            &mut cols[0],
                                            b,
                                            muted_text,
                                            key_bg,
                                            text_col,
                                        );
                                    } else {
                                        // Empty row placeholder for consistent height
                                        cols[0]
                                            .allocate_space(egui::vec2(1.0, typography::MD + 6.0));
                                    }

                                    // Right column
                                    if let Some(b) = bindings.get(row + col_size) {
                                        Self::render_binding(
                                            &mut cols[1],
                                            b,
                                            muted_text,
                                            key_bg,
                                            text_col,
                                        );
                                    } else {
                                        cols[1]
                                            .allocate_space(egui::vec2(1.0, typography::MD + 6.0));
                                    }
                                }
                            });
                        });
                    }

                    ui.add_space(10.0);

                    // Separator above footer
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(6.0);

                    // Footer
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        render_key_badge(ui, "h/l", key_bg, accent_color);
                        ui.label(
                            RichText::new(" switch tab  ")
                                .color(muted_text)
                                .font(typography::proportional(typography::SM)),
                        );
                        render_key_badge(ui, "Esc", key_bg, accent_color);
                        ui.label(
                            RichText::new(" close")
                                .color(muted_text)
                                .font(typography::proportional(typography::SM)),
                        );
                    });
                    ui.add_space(8.0);
                });
            });

        if should_close {
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
        }
    }

    /// Render a single key → description row
    fn render_binding(
        ui: &mut egui::Ui,
        binding: &Keybinding,
        muted_text: Color32,
        key_bg: Color32,
        text_col: Color32,
    ) {
        ui.horizontal(|ui| {
            render_key_badge(ui, binding.key, key_bg, text_col);
            ui.add_space(6.0);
            ui.label(
                RichText::new(binding.description)
                    .color(muted_text)
                    .font(typography::proportional(typography::MD)),
            );
        });
    }
}
