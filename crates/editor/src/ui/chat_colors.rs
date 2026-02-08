//! Shared theme color helpers for chat-like UI components.
//!
//! This module provides consistent colors across chat components
//! (agent panel, etc.) that adapt to the current theme.

use egui::Color32;

use crate::ui::theme::AppTheme;

/// Chat-specific theme colors that adapt to the current app theme.
///
/// Provides semantic colors for chat UI elements like messages,
/// selections, badges, and avatars.
#[derive(Clone, Copy)]
pub struct ChatColors {
    theme: AppTheme,
}

impl ChatColors {
    /// Create a new ChatColors helper for the given theme.
    pub fn new(theme: AppTheme) -> Self {
        Self { theme }
    }

    // =========================================================================
    // Selection & Highlighting
    // =========================================================================

    /// Background color for selected items (channels, threads, messages).
    pub fn selection_bg(&self) -> Color32 {
        self.theme.accent_primary().gamma_multiply(0.15)
    }

    /// Subtle highlight for keyboard navigation focus.
    pub fn nav_highlight_bg(&self) -> Color32 {
        self.theme.accent_primary().gamma_multiply(0.08)
    }

    /// Background color for hovered items.
    pub fn hover_bg(&self) -> Color32 {
        self.theme.bg_hover()
    }

    // =========================================================================
    // Message Backgrounds
    // =========================================================================

    /// Background color for the current user's own messages.
    pub fn own_message_bg(&self) -> Color32 {
        self.theme.accent_primary().gamma_multiply(0.15)
    }

    /// Background color for other users' messages.
    pub fn other_message_bg(&self) -> Color32 {
        self.theme.bg_elevated()
    }

    /// Background color for AI agent messages.
    pub fn agent_message_bg(&self) -> Color32 {
        match self.theme {
            AppTheme::Custom(colors) => colors.bg_elevated,
            AppTheme::Light => Color32::from_rgb(245, 243, 255),
            AppTheme::Nord => Color32::from_rgb(46, 52, 74),
            AppTheme::Midnight => Color32::from_rgb(25, 30, 50),
            AppTheme::Catppuccin => Color32::from_rgb(40, 38, 60),
            AppTheme::Ayu => Color32::from_rgb(22, 26, 35),
            AppTheme::Bergman => Color32::from_rgb(30, 34, 45),
            AppTheme::Aurora => Color32::from_rgb(22, 35, 38),
            AppTheme::Stockholm => Color32::from_rgb(235, 240, 248),
            AppTheme::Graphite => Color32::from_rgb(35, 30, 25), // Orange-tinted dark
            AppTheme::Ink => Color32::from_rgb(22, 22, 32),      // Silver-tinted dark
            AppTheme::Midsommar => Color32::from_rgb(235, 242, 252), // Blue-tinted summer light
            AppTheme::Skargard => Color32::from_rgb(235, 245, 252), // Sea blue-tinted skargard light
            AppTheme::Dark => Color32::from_rgb(30, 27, 45),
        }
    }

    /// Background color for message hover state.
    pub fn message_hover_bg(&self) -> Color32 {
        self.theme.bg_hover()
    }

    // =========================================================================
    // Badges & Indicators
    // =========================================================================

    /// Color for unread count badges.
    pub fn unread_badge(&self) -> Color32 {
        self.theme.accent_primary()
    }

    /// Color for critical/urgent badges.
    pub fn critical_badge(&self) -> Color32 {
        self.theme.semantic_error()
    }

    // =========================================================================
    // Avatars
    // =========================================================================

    /// Glow ring color for AI agent avatars.
    pub fn agent_avatar_ring(&self) -> Color32 {
        self.theme.accent_primary().gamma_multiply(0.3)
    }

    // =========================================================================
    // Borders & Dividers
    // =========================================================================

    /// Color for divider lines between sections.
    pub fn divider(&self) -> Color32 {
        self.theme.border_subtle()
    }

    /// Border color for embedded charts.
    pub fn chart_embed_border(&self) -> Color32 {
        self.theme.accent_primary().gamma_multiply(0.5)
    }

    // =========================================================================
    // Text Colors
    // =========================================================================

    /// Color for system messages (join/leave notifications, etc.).
    pub fn system_message(&self) -> Color32 {
        self.theme.text_tertiary()
    }

    // =========================================================================
    // Trend Indicators (for stat cards)
    // =========================================================================

    /// Color for upward trend (typically bad for latency, errors).
    pub fn trend_up(&self) -> Color32 {
        self.theme.semantic_error()
    }

    /// Color for downward trend (typically good for latency, errors).
    pub fn trend_down(&self) -> Color32 {
        self.theme.semantic_success()
    }

    // =========================================================================
    // Bar Chart Colors (cycling palette)
    // =========================================================================

    /// First bar chart color.
    pub fn chart_color_1(&self) -> Color32 {
        self.theme.chart_color(0)
    }

    /// Second bar chart color.
    pub fn chart_color_2(&self) -> Color32 {
        self.theme.chart_color(1)
    }

    /// Third bar chart color.
    pub fn chart_color_3(&self) -> Color32 {
        self.theme.chart_color(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_colors_creation() {
        let colors = ChatColors::new(AppTheme::Dark);
        // Should not panic
        let _ = colors.selection_bg();
        let _ = colors.hover_bg();
        let _ = colors.own_message_bg();
    }

    #[test]
    fn test_all_themes_have_colors() {
        for theme in AppTheme::all() {
            let colors = ChatColors::new(*theme);
            // Ensure all colors are valid (no panic)
            let _ = colors.selection_bg();
            let _ = colors.nav_highlight_bg();
            let _ = colors.hover_bg();
            let _ = colors.own_message_bg();
            let _ = colors.other_message_bg();
            let _ = colors.agent_message_bg();
            let _ = colors.message_hover_bg();
            let _ = colors.unread_badge();
            let _ = colors.critical_badge();
            let _ = colors.agent_avatar_ring();
            let _ = colors.divider();
            let _ = colors.chart_embed_border();
            let _ = colors.system_message();
        }
    }
}
