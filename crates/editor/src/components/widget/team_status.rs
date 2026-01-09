//! Team collaboration status widget for the status line.
//!
//! Shows team connection status only when connected. Invisible for non-team users.

use egui::{Color32, Response, Ui};
use enya_team_api::{TeamConnectionStatus, WsConnectionState};

use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Team status display info extracted from TeamConnectionStatus.
#[derive(Debug, Clone, Default)]
pub struct TeamStatusInfo {
    /// Whether connected to team server.
    pub is_connected: bool,
    /// Number of online team members (including self).
    pub online_count: usize,
    /// Number of unread notifications/mentions.
    pub unread_count: usize,
    /// Current team name (if connected).
    pub team_name: Option<String>,
    /// Current user display name (if connected).
    pub user_name: Option<String>,
    /// WebSocket connection state for real-time updates.
    pub ws_state: WsState,
}

/// Simplified WebSocket state for UI display.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum WsState {
    /// Not connected to WebSocket.
    #[default]
    Disconnected,
    /// WebSocket is connecting.
    Connecting,
    /// WebSocket is connected and ready.
    Connected,
    /// WebSocket connection failed.
    Failed,
    /// WebSocket is reconnecting.
    Reconnecting,
}

impl From<&WsConnectionState> for WsState {
    fn from(state: &WsConnectionState) -> Self {
        match state {
            WsConnectionState::Disconnected => WsState::Disconnected,
            WsConnectionState::Connecting => WsState::Connecting,
            WsConnectionState::Connected => WsState::Connected,
            WsConnectionState::Failed { .. } => WsState::Failed,
            WsConnectionState::Reconnecting { .. } => WsState::Reconnecting,
        }
    }
}

impl TeamStatusInfo {
    /// Create status info from connection status.
    pub fn from_status(
        status: &TeamConnectionStatus,
        ws_state: &WsConnectionState,
        online_count: usize,
        unread: usize,
    ) -> Self {
        match status {
            TeamConnectionStatus::Connected { user, team } => Self {
                is_connected: true,
                online_count,
                unread_count: unread,
                team_name: Some(team.name.clone()),
                user_name: Some(user.display_name.clone()),
                ws_state: WsState::from(ws_state),
            },
            _ => Self::default(),
        }
    }

    /// Check if we should show team UI (only when connected).
    pub fn should_show(&self) -> bool {
        self.is_connected
    }
}

/// Team status segment for the status line.
///
/// Renders a compact team indicator:
/// - When disconnected: nothing (invisible)
/// - When connected: "Team Name | 3 online | 2 unread"
pub struct TeamStatusWidget {
    theme: AppTheme,
}

impl TeamStatusWidget {
    pub fn new(theme: AppTheme) -> Self {
        Self { theme }
    }

    /// Render the team status segment (returns None if not connected).
    /// Call this in the right section of status line, before connection status.
    pub fn show(
        &self,
        ui: &mut Ui,
        info: &TeamStatusInfo,
        height: f32,
        padding: f32,
    ) -> Option<Response> {
        if !info.should_show() {
            return None;
        }

        // Build the status text
        let mut parts = Vec::new();

        // Team name (truncated if too long)
        if let Some(ref name) = info.team_name {
            let display_name = if name.len() > 15 {
                format!("{}...", &name[..12])
            } else {
                name.clone()
            };
            parts.push(display_name);
        }

        // Online count
        parts.push(format!("{} online", info.online_count));

        // Unread notifications (with accent color)
        let has_unread = info.unread_count > 0;

        let status_text = parts.join(" | ");

        // Icon based on state - show WebSocket indicator
        let (icon, ws_indicator) = self.ws_icon_and_indicator(info, has_unread);

        // Color based on unread
        let fg_color = if has_unread {
            self.theme.accent_primary()
        } else {
            palette::text::SECONDARY
        };

        // Render segment with WebSocket indicator
        let content = format!("{icon}{ws_indicator} {status_text}");

        // Add unread badge if any
        let unread = info.unread_count;
        let content = if has_unread {
            format!("{content} ({unread})")
        } else {
            content
        };

        // Calculate width
        let galley = ui.painter().layout_no_wrap(
            content.clone(),
            typography::proportional(typography::MD),
            fg_color,
        );
        let text_width = galley.size().x + padding * 2.0;

        // Allocate and render
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(text_width, height), egui::Sense::click());

        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, 0.0, Color32::TRANSPARENT);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &content,
                typography::proportional(typography::MD),
                fg_color,
            );
        }

        // Tooltip with WebSocket status
        let ws_status = match info.ws_state {
            WsState::Connected => "Real-time: connected",
            WsState::Connecting => "Real-time: connecting...",
            WsState::Reconnecting => "Real-time: reconnecting...",
            WsState::Failed => "Real-time: connection failed",
            WsState::Disconnected => "Real-time: disconnected",
        };
        let tooltip = format!("Team collaboration (Space+t)\n{ws_status}");

        if response.hovered() {
            response.clone().on_hover_text(tooltip);
        }

        Some(response)
    }

    /// Get the icon and WebSocket indicator based on state.
    fn ws_icon_and_indicator(
        &self,
        info: &TeamStatusInfo,
        has_unread: bool,
    ) -> (&'static str, &'static str) {
        let icon = if has_unread {
            semantic_icons::status::NOTIFICATION // Bell with dot
        } else {
            semantic_icons::social::TEAM // Users icon
        };

        // WebSocket indicator - small dot/symbol after icon
        let ws_indicator = match info.ws_state {
            WsState::Connected => "",     // No indicator when connected (clean look)
            WsState::Connecting => "...", // Ellipsis for connecting
            WsState::Reconnecting => "~", // Tilde for reconnecting
            WsState::Failed => "!",       // Exclamation for failed
            WsState::Disconnected => "-", // Dash for disconnected
        };

        (icon, ws_indicator)
    }
}
