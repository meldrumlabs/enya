//! Which-key style leader popup for leader key commands.
//!
//! Inspired by neovim's which-key.nvim plugin (included in LazyVim), this
//! component displays available leader key commands when a leader key is pressed.
//! Unlike the static WhichKey help overlay, this popup appears dynamically
//! and shows commands for the active leader key (Space, g, etc.).

use egui::RichText;

use crate::components::util::finder_utils::{OverlayStyle, render_key_badge};
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;
use crate::workspace::LEADER_POPUP_DELAY_MS;

/// Identifies which leader key triggered the popup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeaderKey {
    /// Space leader key (Space+X commands)
    Space,
    /// Go-to leader key (gd, ga, gf commands)
    G,
}

impl LeaderKey {
    /// Display symbol for the popup header
    pub fn display_symbol(&self) -> &'static str {
        match self {
            Self::Space => "␣",
            Self::G => "g",
        }
    }

    /// Unique egui ID for this popup
    fn popup_id(&self) -> &'static str {
        match self {
            Self::Space => "leader_popup_space",
            Self::G => "leader_popup_g",
        }
    }
}

/// A leader key command to display in the popup
struct LeaderCommand {
    /// The key to press (e.g., "f", "w")
    key: &'static str,
    /// Nerd font icon
    icon: &'static str,
    /// Description of the command
    label: &'static str,
}

/// Dynamic popup that shows available commands when a leader key is pressed.
///
/// This popup appears after a short delay (to let power users type fast sequences
/// without seeing it) and auto-hides when the leader key is cleared.
pub struct LeaderPopup {
    /// Whether the Space popup is currently visible
    space_visible: bool,
    /// When the Space popup became visible
    space_show_time: Option<Instant>,
    /// Whether the G popup is currently visible
    g_visible: bool,
    /// When the G popup became visible
    g_show_time: Option<Instant>,
    /// Current theme (supports custom plugin themes)
    theme: AppTheme,
}

impl Default for LeaderPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl LeaderPopup {
    /// Create a new leader popup
    pub fn new() -> Self {
        Self {
            space_visible: false,
            space_show_time: None,
            g_visible: false,
            g_show_time: None,
            theme: AppTheme::default(),
        }
    }

    /// Set the theme (call with `self.theme()` from Workspace)
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if a specific leader key popup is currently visible
    pub fn is_visible(&self, leader_key: LeaderKey) -> bool {
        match leader_key {
            LeaderKey::Space => self.space_visible,
            LeaderKey::G => self.g_visible,
        }
    }

    /// Update visibility based on the leader key state.
    ///
    /// The popup shows after `LEADER_POPUP_DELAY_MS` and stays visible until
    /// the leader key is cleared (by executing a command, pressing Escape, or invalid key).
    pub fn update_visibility(&mut self, leader_key: LeaderKey, press_time: Option<Instant>) {
        match leader_key {
            LeaderKey::Space => self.update_space_visibility(press_time),
            LeaderKey::G => self.update_g_visibility(press_time),
        }
    }

    /// Update Space leader key visibility
    fn update_space_visibility(&mut self, press_time: Option<Instant>) {
        match press_time {
            Some(press_time) => {
                let elapsed = Instant::now().duration_since(press_time).as_millis();

                // Show popup after delay (so power users don't see it)
                if elapsed >= LEADER_POPUP_DELAY_MS && !self.space_visible {
                    self.space_visible = true;
                    self.space_show_time = Some(Instant::now());
                }
            }
            None => {
                // Space is not active, hide popup
                self.hide(LeaderKey::Space);
            }
        }
    }

    /// Update G leader key visibility
    fn update_g_visibility(&mut self, press_time: Option<Instant>) {
        match press_time {
            Some(press_time) => {
                let elapsed = Instant::now().duration_since(press_time).as_millis();

                // Show popup after delay (so power users don't see it)
                if elapsed >= LEADER_POPUP_DELAY_MS && !self.g_visible {
                    self.g_visible = true;
                    self.g_show_time = Some(Instant::now());
                }
            }
            None => {
                // G is not active, hide popup
                self.hide(LeaderKey::G);
            }
        }
    }

    /// Hide a specific leader key popup
    pub fn hide(&mut self, leader_key: LeaderKey) {
        match leader_key {
            LeaderKey::Space => {
                self.space_visible = false;
                self.space_show_time = None;
            }
            LeaderKey::G => {
                self.g_visible = false;
                self.g_show_time = None;
            }
        }
    }

    /// Build the list of commands to display for a leader key
    fn build_commands(leader_key: LeaderKey) -> Vec<LeaderCommand> {
        match leader_key {
            LeaderKey::Space => vec![
                LeaderCommand {
                    key: "f",
                    icon: semantic_icons::action::SEARCH,
                    label: "Find anything",
                },
                LeaderCommand {
                    key: "w",
                    icon: semantic_icons::file::FOLDER,
                    label: "Workspace",
                },
                LeaderCommand {
                    key: "h",
                    icon: semantic_icons::nav::HOME,
                    label: "Home",
                },
                LeaderCommand {
                    key: "d",
                    icon: semantic_icons::diagnostic::WARNING,
                    label: "Diagnostics",
                },
                LeaderCommand {
                    key: "t",
                    icon: semantic_icons::time::CLOCK,
                    label: "Time picker",
                },
                LeaderCommand {
                    key: "a",
                    icon: semantic_icons::action::BRAIN,
                    label: "Agent",
                },
                LeaderCommand {
                    key: "p",
                    icon: semantic_icons::action::TOOL,
                    label: "Plugins",
                },
            ],
            LeaderKey::G => vec![
                LeaderCommand {
                    key: "d",
                    icon: semantic_icons::action::LINK,
                    label: "Definition",
                },
                LeaderCommand {
                    key: "a",
                    icon: semantic_icons::status::ALERT,
                    label: "Alert",
                },
                LeaderCommand {
                    key: "f",
                    icon: semantic_icons::nav::FULLSCREEN,
                    label: "Float pane",
                },
            ],
        }
    }

    /// Show all visible leader key popups.
    ///
    /// Call this every frame when updating overlays.
    #[profiling::function]
    pub fn show_all(&mut self, ctx: &egui::Context, is_native: bool) {
        self.show(ctx, LeaderKey::Space, is_native);
        self.show(ctx, LeaderKey::G, is_native);
    }

    /// Show a specific leader key popup. This is purely visual - keyboard input is handled by keyboard.rs.
    ///
    /// The popup will automatically hide when:
    /// - The leader key is cleared (via update_visibility)
    /// - A valid command is executed (keyboard.rs clears the leader key)
    /// - An invalid key is pressed (keyboard.rs clears the leader key)
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context, leader_key: LeaderKey, _is_native: bool) {
        if !self.is_visible(leader_key) {
            return;
        }

        // Extract colors from theme
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let text_col = self.theme.text_primary();
        let muted_text = self.theme.text_tertiary();
        let key_bg = self.theme.bg_elevated();
        let accent_color = self.theme.accent_primary();

        let commands = Self::build_commands(leader_key);
        let popup_width = 220.0;

        // Render the popup
        // IMPORTANT: interactable(false) prevents the Area from capturing focus,
        // which would block keyboard handling in keyboard.rs
        egui::Area::new(egui::Id::new(leader_key.popup_id()))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 50.0]) // Slightly below center
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Minimal header - centered leader key symbol
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(leader_key.display_symbol())
                                .color(muted_text)
                                .size(typography::LG),
                        );
                    });
                    ui.add_space(6.0);

                    // Command list with keys on the right
                    for cmd in &commands {
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);

                            // Icon
                            ui.label(
                                RichText::new(cmd.icon)
                                    .color(accent_color)
                                    .size(semantic_icons::SIZE_ITEM),
                            );
                            ui.add_space(8.0);

                            // Label (left-aligned, takes remaining space)
                            ui.label(
                                RichText::new(cmd.label)
                                    .color(muted_text)
                                    .font(typography::proportional(typography::MD)),
                            );

                            // Flexible space to push key badge to the right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(14.0);
                                    render_key_badge(ui, cmd.key, key_bg, text_col);
                                },
                            );
                        });
                        ui.add_space(6.0);
                    }

                    // Final padding (4.0 + 6.0 from last row = 10.0, matching top)
                    ui.add_space(4.0);
                });
            });
    }
}
