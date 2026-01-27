//! Which-key style leader popup for Space+X commands.
//!
//! Inspired by neovim's which-key.nvim plugin (included in LazyVim), this
//! component displays available leader key commands when Space is pressed.
//! Unlike the static WhichKey help overlay, this popup appears dynamically
//! and shows only Space+X commands.

use egui::RichText;

use crate::components::util::finder_utils::{OverlayStyle, render_key_badge};
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;
use crate::workspace::LEADER_POPUP_DELAY_MS;

/// Result from showing the leader popup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderPopupResult {
    /// Popup is still visible, no action taken
    None,
    /// User pressed Escape to dismiss
    Dismissed,
    /// Leader key timeout expired
    TimedOut,
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

/// Dynamic popup that shows available Space+X commands when Space is pressed.
///
/// This popup appears after a short delay (to let power users type fast sequences
/// without seeing it) and auto-hides after the leader key timeout.
pub struct LeaderPopup {
    /// Whether the popup is currently visible
    is_visible: bool,
    /// When the popup became visible (for timeout tracking)
    show_time: Option<Instant>,
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
            is_visible: false,
            show_time: None,
            theme: AppTheme::default(),
        }
    }

    /// Set the theme (call with `self.theme()` from Workspace)
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the popup is currently visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Update visibility based on the leader key state.
    ///
    /// The popup shows after `LEADER_POPUP_DELAY_MS` and stays visible until
    /// Space is cleared (by executing a command, pressing Escape, or invalid key).
    /// This matches neovim's which-key.nvim behavior - no auto-timeout.
    pub fn update_visibility(&mut self, space_press_time: Option<Instant>) {
        match space_press_time {
            Some(press_time) => {
                let elapsed = Instant::now().duration_since(press_time).as_millis();

                // Show popup after delay (so power users don't see it)
                if elapsed >= LEADER_POPUP_DELAY_MS && !self.is_visible {
                    self.is_visible = true;
                    self.show_time = Some(Instant::now());
                }
            }
            None => {
                // Space is not active, hide popup
                self.hide();
            }
        }
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.show_time = None;
    }

    /// Build the list of commands to display
    fn build_commands() -> Vec<LeaderCommand> {
        vec![
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
        ]
    }

    /// Show the popup. This is purely visual - keyboard input is handled by keyboard.rs.
    ///
    /// The popup will automatically hide when:
    /// - The space leader key times out (via update_visibility)
    /// - A valid Space+X command is executed (keyboard.rs clears space)
    /// - An invalid key is pressed (keyboard.rs clears space)
    ///
    /// Call this every frame when updating overlays.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context, _is_native: bool) {
        if !self.is_visible {
            return;
        }

        // Extract colors from theme
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let text_col = self.theme.text_primary();
        let muted_text = self.theme.text_tertiary();
        let key_bg = self.theme.bg_elevated();
        let accent_color = self.theme.accent_primary();

        let commands = Self::build_commands();
        let popup_width = 220.0;

        // Render the popup
        // IMPORTANT: interactable(false) prevents the Area from capturing focus,
        // which would block keyboard handling in keyboard.rs
        egui::Area::new(egui::Id::new("leader_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 50.0]) // Slightly below center
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Minimal header - centered space symbol
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("␣").color(muted_text).size(typography::LG));
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
