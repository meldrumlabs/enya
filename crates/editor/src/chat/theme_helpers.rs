//! Shared theme color helpers for the chat module.
//!
//! This module provides consistent colors across chat components
//! (channels panel, chat view, etc.) that adapt to the current theme.

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
            AppTheme::Light => Color32::from_rgb(245, 243, 255), // Purple-50 (soft lavender)
            AppTheme::Dark => Color32::from_rgb(30, 27, 45),     // Dark purple tint
            AppTheme::Nord => Color32::from_rgb(46, 52, 74),     // Nord frost-tinted
            AppTheme::Gruvbox => Color32::from_rgb(50, 40, 50),  // Gruvbox purple-tinted
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
        match self.theme {
            AppTheme::Light => Color32::from_rgb(220, 38, 38), // Red-600
            AppTheme::Dark => Color32::from_rgb(248, 113, 113), // Red-400
            AppTheme::Nord => Color32::from_rgb(191, 97, 106), // Nord aurora red
            AppTheme::Gruvbox => Color32::from_rgb(251, 73, 52), // Gruvbox red
        }
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
        match self.theme {
            AppTheme::Light => Color32::from_rgb(220, 38, 38), // Red-600
            AppTheme::Dark => Color32::from_rgb(248, 113, 113), // Red-400
            AppTheme::Nord => Color32::from_rgb(191, 97, 106), // Nord aurora red
            AppTheme::Gruvbox => Color32::from_rgb(251, 73, 52), // Gruvbox red
        }
    }

    /// Color for downward trend (typically good for latency, errors).
    pub fn trend_down(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(22, 163, 74), // Green-600
            AppTheme::Dark => Color32::from_rgb(74, 222, 128), // Green-400
            AppTheme::Nord => Color32::from_rgb(163, 190, 140), // Nord aurora green
            AppTheme::Gruvbox => Color32::from_rgb(184, 187, 38), // Gruvbox green
        }
    }

    // =========================================================================
    // Bar Chart Colors (cycling palette)
    // =========================================================================

    /// First bar chart color.
    pub fn chart_color_1(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(59, 130, 246), // Blue-500
            AppTheme::Dark => Color32::from_rgb(96, 165, 250),  // Blue-400
            AppTheme::Nord => Color32::from_rgb(136, 192, 208), // Nord frost
            AppTheme::Gruvbox => Color32::from_rgb(131, 165, 152), // Gruvbox aqua
        }
    }

    /// Second bar chart color.
    pub fn chart_color_2(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(168, 85, 247), // Purple-500
            AppTheme::Dark => Color32::from_rgb(192, 132, 252), // Purple-400
            AppTheme::Nord => Color32::from_rgb(180, 142, 173), // Nord aurora purple
            AppTheme::Gruvbox => Color32::from_rgb(211, 134, 155), // Gruvbox purple
        }
    }

    /// Third bar chart color.
    pub fn chart_color_3(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(234, 179, 8), // Yellow-500
            AppTheme::Dark => Color32::from_rgb(250, 204, 21), // Yellow-400
            AppTheme::Nord => Color32::from_rgb(235, 203, 139), // Nord aurora yellow
            AppTheme::Gruvbox => Color32::from_rgb(250, 189, 47), // Gruvbox yellow
        }
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
        for theme in [
            AppTheme::Light,
            AppTheme::Dark,
            AppTheme::Nord,
            AppTheme::Gruvbox,
        ] {
            let colors = ChatColors::new(theme);
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
