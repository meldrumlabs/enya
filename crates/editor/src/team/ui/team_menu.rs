//! Team menu overlay component.
//!
//! A sleek, centered overlay for team collaboration, styled similarly to the
//! unified finder for a cohesive UX. Shows team members, presence status,
//! and collaboration actions.

use egui::{Align2, Color32, Id, RichText, Vec2};
use enya_team_api::{User, UserId};

use crate::components::util::finder_utils::OverlayStyle;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Actions that can be triggered from the team menu.
#[derive(Debug, Clone, PartialEq)]
pub enum TeamMenuAction {
    /// No action
    None,
    /// Add annotation to focused chart
    AddAnnotation,
    /// Share current workspace view
    ShareView,
    /// Start war room (incident mode)
    StartWarRoom,
    /// Open team settings
    OpenSettings,
    /// Switch to a different team
    SwitchTeam,
    /// Sign out
    SignOut,
}

/// Member presence status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberPresence {
    /// Currently active in the app.
    Online,
    /// Connected but idle.
    Idle,
    /// Not connected.
    Offline,
}

impl MemberPresence {
    /// Get the display color for this presence status.
    pub fn color(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::Online => palette::semantic::SUCCESS,
            Self::Idle => palette::semantic::WARNING,
            Self::Offline => theme.text_tertiary(),
        }
    }

    /// Get the status label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Idle => "idle",
            Self::Offline => "offline",
        }
    }
}

/// A team member with presence info.
#[derive(Debug, Clone)]
pub struct TeamMember {
    pub user: User,
    pub presence: MemberPresence,
    /// What they're currently viewing (if any).
    pub viewing: Option<String>,
    /// Whether this is the current user.
    pub is_self: bool,
}

/// Menu item for rendering.
struct MenuItem {
    icon: &'static str,
    label: &'static str,
    action: TeamMenuAction,
}

const MENU_ITEMS: &[MenuItem] = &[
    MenuItem {
        icon: semantic_icons::social::COMMENT,
        label: "Add Annotation",
        action: TeamMenuAction::AddAnnotation,
    },
    MenuItem {
        icon: semantic_icons::action::SHARE,
        label: "Share current view",
        action: TeamMenuAction::ShareView,
    },
    MenuItem {
        icon: semantic_icons::status::ALERT,
        label: "Start War Room",
        action: TeamMenuAction::StartWarRoom,
    },
    MenuItem {
        icon: semantic_icons::action::SETTINGS,
        label: "Team Settings",
        action: TeamMenuAction::OpenSettings,
    },
    MenuItem {
        icon: semantic_icons::nav::SWITCH,
        label: "Switch Team...",
        action: TeamMenuAction::SwitchTeam,
    },
    MenuItem {
        icon: semantic_icons::action::LOGOUT,
        label: "Sign Out",
        action: TeamMenuAction::SignOut,
    },
];

/// Team menu overlay state and rendering.
pub struct TeamMenu {
    /// Whether the menu is open.
    is_open: bool,
    /// Current theme.
    theme: AppTheme,
    /// Selected index for keyboard navigation.
    selected_index: usize,
    /// Whether we're in the members section (true) or actions section (false).
    in_members_section: bool,
}

impl Default for TeamMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl TeamMenu {
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
            selected_index: 0,
            in_members_section: true,
        }
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Toggle menu open/closed.
    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Open the menu.
    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_index = 0;
        self.in_members_section = true;
    }

    /// Close the menu.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if menu is open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the team menu as a centered overlay (like the unified finder).
    /// Returns the action if any menu item was selected.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        members: &[TeamMember],
        team_name: Option<&str>,
        current_user_id: Option<UserId>,
    ) -> TeamMenuAction {
        if !self.is_open {
            return TeamMenuAction::None;
        }

        let mut action = TeamMenuAction::None;
        let mut should_close = false;

        // Handle keyboard navigation
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                should_close = true;
            }

            // Navigate up
            if input.key_pressed(egui::Key::ArrowUp)
                || input.key_pressed(egui::Key::K)
                || (input.modifiers.ctrl && input.key_pressed(egui::Key::P))
            {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else if !self.in_members_section {
                    // Switch to members section
                    self.in_members_section = true;
                    self.selected_index = members.len().saturating_sub(1);
                }
            }

            // Navigate down
            if input.key_pressed(egui::Key::ArrowDown)
                || input.key_pressed(egui::Key::J)
                || (input.modifiers.ctrl && input.key_pressed(egui::Key::N))
            {
                if self.in_members_section {
                    if self.selected_index + 1 < members.len() {
                        self.selected_index += 1;
                    } else {
                        // Switch to actions section
                        self.in_members_section = false;
                        self.selected_index = 0;
                    }
                } else if self.selected_index + 1 < MENU_ITEMS.len() {
                    self.selected_index += 1;
                }
            }

            // Tab to switch sections
            if input.key_pressed(egui::Key::Tab) {
                self.in_members_section = !self.in_members_section;
                self.selected_index = 0;
            }

            // Enter to select
            if input.key_pressed(egui::Key::Enter) && !self.in_members_section {
                if let Some(item) = MENU_ITEMS.get(self.selected_index) {
                    action = item.action.clone();
                    should_close = true;
                }
            }
        });

        if should_close {
            self.close();
            return action;
        }

        // Calculate dimensions - similar to unified finder but smaller
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.4).clamp(350.0, 450.0);
        let popup_max_height = (screen_rect.height() * 0.6).clamp(300.0, 500.0);

        let overlay_style = OverlayStyle::frosted_glass(self.theme);

        egui::Area::new(Id::new("team_menu_overlay"))
            .anchor(Align2::CENTER_CENTER, [0.0, -30.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Allocate fixed size
                let (area_rect, _) = ui.allocate_exact_size(
                    egui::vec2(popup_width, popup_max_height),
                    egui::Sense::hover(),
                );

                ui.set_clip_rect(area_rect);

                let mut child_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(area_rect)
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );

                // Premium glass frame
                let frame = overlay_style
                    .frame()
                    .inner_margin(egui::Margin::symmetric(0, 12))
                    .corner_radius(14.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 32,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    });

                frame.show(&mut child_ui, |ui| {
                    ui.set_width(popup_width - 2.0);

                    // Header with team name
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::social::TEAM)
                                .size(typography::LG)
                                .color(self.theme.accent_primary()),
                        );
                        ui.add_space(8.0);
                        let title = team_name.unwrap_or("Team");
                        ui.label(
                            RichText::new(title)
                                .size(typography::LG)
                                .color(self.theme.text_primary())
                                .strong(),
                        );
                    });

                    ui.add_space(4.0);

                    // Hint text
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(
                                "↑↓ navigate • Tab switch section • Enter select • Esc close",
                            )
                            .size(typography::XS)
                            .color(self.theme.text_tertiary()),
                        );
                    });

                    ui.add_space(12.0);

                    // Divider
                    self.render_divider(ui);

                    // Members section
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        let section_color = if self.in_members_section {
                            self.theme.accent_primary()
                        } else {
                            self.theme.text_tertiary()
                        };
                        ui.label(
                            RichText::new("MEMBERS")
                                .size(typography::XS)
                                .color(section_color),
                        );
                    });
                    ui.add_space(4.0);

                    // Member list in a scroll area
                    let member_height = members.len().min(5) as f32 * 44.0;
                    egui::ScrollArea::vertical()
                        .max_height(member_height.max(100.0))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (idx, member) in members.iter().enumerate() {
                                let is_selected =
                                    self.in_members_section && idx == self.selected_index;
                                let is_current = current_user_id
                                    .map(|id| id == member.user.id)
                                    .unwrap_or(false);

                                self.render_member_row(ui, member, is_selected, is_current);
                            }
                        });

                    ui.add_space(8.0);
                    self.render_divider(ui);
                    ui.add_space(8.0);

                    // Actions section
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        let section_color = if !self.in_members_section {
                            self.theme.accent_primary()
                        } else {
                            self.theme.text_tertiary()
                        };
                        ui.label(
                            RichText::new("ACTIONS")
                                .size(typography::XS)
                                .color(section_color),
                        );
                    });
                    ui.add_space(4.0);

                    // Action items
                    for (idx, item) in MENU_ITEMS.iter().enumerate() {
                        let is_selected = !self.in_members_section && idx == self.selected_index;
                        if self.render_action_row(ui, item, is_selected) {
                            action = item.action.clone();
                            should_close = true;
                        }
                    }

                    ui.add_space(8.0);
                });
            });

        // Close on click outside
        if ctx.input(|i| i.pointer.any_click()) {
            let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
            if let Some(pos) = pointer_pos {
                let center = ctx.available_rect().center();
                let overlay_rect = egui::Rect::from_center_size(
                    center + egui::vec2(0.0, -30.0),
                    egui::vec2(popup_width, popup_max_height),
                );
                if !overlay_rect.contains(pos) {
                    should_close = true;
                }
            }
        }

        if should_close {
            self.close();
        }

        action
    }

    /// Render a horizontal divider.
    fn render_divider(&self, ui: &mut egui::Ui) {
        let available = ui.available_rect_before_wrap();
        ui.painter().hline(
            (available.left() + 16.0)..=(available.right() - 16.0),
            available.top(),
            egui::Stroke::new(1.0, self.theme.border_subtle()),
        );
    }

    /// Render a member row.
    fn render_member_row(
        &self,
        ui: &mut egui::Ui,
        member: &TeamMember,
        is_selected: bool,
        is_current: bool,
    ) {
        let row_height = 40.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_height),
            egui::Sense::hover(),
        );

        let is_hovered = response.hovered();
        let highlight = is_selected || is_hovered;

        // Background highlight
        if highlight {
            let highlight_color = if is_selected {
                self.theme.accent_primary().gamma_multiply(0.15)
            } else {
                self.theme.bg_surface()
            };
            ui.painter()
                .rect_filled(rect.shrink2(egui::vec2(8.0, 1.0)), 6.0, highlight_color);
        }

        // Presence dot
        let dot_center = rect.left_center() + Vec2::new(24.0, 0.0);
        ui.painter()
            .circle_filled(dot_center, 4.0, member.presence.color(self.theme));

        // Name
        let name_text = if is_current || member.is_self {
            format!("{} (you)", member.user.display_name)
        } else {
            member.user.display_name.clone()
        };

        let name_color = if highlight {
            self.theme.text_primary()
        } else {
            self.theme.text_secondary()
        };

        ui.painter().text(
            rect.left_center() + Vec2::new(40.0, -6.0),
            Align2::LEFT_CENTER,
            &name_text,
            typography::proportional(typography::SM),
            name_color,
        );

        // Viewing info or presence label
        let subtitle = if let Some(ref viewing) = member.viewing {
            format!("viewing \"{viewing}\"")
        } else {
            member.presence.label().to_string()
        };

        ui.painter().text(
            rect.left_center() + Vec2::new(40.0, 8.0),
            Align2::LEFT_CENTER,
            &subtitle,
            typography::proportional(typography::XS),
            self.theme.text_tertiary(),
        );
    }

    /// Render an action row. Returns true if clicked.
    fn render_action_row(&self, ui: &mut egui::Ui, item: &MenuItem, is_selected: bool) -> bool {
        let row_height = 36.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        let is_hovered = response.hovered();
        let highlight = is_selected || is_hovered;

        // Background highlight
        if highlight {
            let highlight_color = if is_selected {
                self.theme.accent_primary().gamma_multiply(0.15)
            } else {
                self.theme.bg_surface()
            };
            ui.painter()
                .rect_filled(rect.shrink2(egui::vec2(8.0, 1.0)), 6.0, highlight_color);
        }

        // Icon
        let icon_color = if highlight {
            self.theme.accent_primary()
        } else {
            self.theme.text_tertiary()
        };

        ui.painter().text(
            rect.left_center() + Vec2::new(24.0, 0.0),
            Align2::LEFT_CENTER,
            item.icon,
            typography::proportional(typography::MD),
            icon_color,
        );

        // Label
        let label_color = if highlight {
            self.theme.text_primary()
        } else {
            self.theme.text_secondary()
        };

        ui.painter().text(
            rect.left_center() + Vec2::new(48.0, 0.0),
            Align2::LEFT_CENTER,
            item.label,
            typography::proportional(typography::SM),
            label_color,
        );

        // Keyboard shortcut hint for selected item
        if is_selected {
            ui.painter().text(
                rect.right_center() + Vec2::new(-16.0, 0.0),
                Align2::RIGHT_CENTER,
                "↵",
                typography::proportional(typography::SM),
                self.theme.text_tertiary(),
            );
        }

        response.clicked()
    }

    // Keep old methods for backward compatibility but they're no longer primary

    /// Show the team menu button (legacy, for toolbar use).
    pub fn show_button(
        &mut self,
        ui: &mut egui::Ui,
        team_name: Option<&str>,
        online_count: usize,
    ) -> egui::Response {
        let text = match team_name {
            Some(name) => {
                let truncated = if name.len() > 12 {
                    format!("{}...", &name[..9])
                } else {
                    name.to_string()
                };
                format!(
                    "{} {} ({})",
                    semantic_icons::social::TEAM,
                    truncated,
                    online_count
                )
            }
            None => format!("{} Team", semantic_icons::social::TEAM),
        };

        let button = egui::Button::new(
            RichText::new(&text)
                .size(typography::MD)
                .color(self.theme.text_secondary()),
        )
        .fill(Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE);

        let response = ui.add(button);

        if response.clicked() {
            self.toggle();
        }

        response
    }

    /// Show dropdown (legacy API - now routes to show()).
    pub fn show_dropdown(
        &mut self,
        ctx: &egui::Context,
        _button_rect: egui::Rect,
        members: &[TeamMember],
        current_user_id: Option<UserId>,
    ) -> TeamMenuAction {
        self.show(ctx, members, None, current_user_id)
    }
}
