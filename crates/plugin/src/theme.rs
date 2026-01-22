//! Custom theme definitions for plugins.
//!
//! This module provides types for plugins to define custom color themes
//! that integrate with the editor's theme system.

/// A custom theme definition from a plugin.
#[derive(Debug, Clone)]
pub struct ThemeDefinition {
    /// Unique identifier (e.g., "tokyo-night")
    pub name: String,
    /// Display name for UI (e.g., "Tokyo Night")
    pub display_name: String,
    /// Base theme to inherit from ("dark" or "light")
    pub base: ThemeBase,
    /// Color palette
    pub colors: ThemeColors,
}

impl ThemeDefinition {
    /// Create a new theme definition with default colors.
    pub fn new(name: impl Into<String>, display_name: impl Into<String>, base: ThemeBase) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            base,
            colors: ThemeColors::default(),
        }
    }

    /// Set background colors.
    pub fn with_backgrounds(
        mut self,
        base: Option<u32>,
        surface: Option<u32>,
        elevated: Option<u32>,
    ) -> Self {
        self.colors.bg_base = base;
        self.colors.bg_surface = surface;
        self.colors.bg_elevated = elevated;
        self
    }

    /// Set text colors.
    pub fn with_text(
        mut self,
        primary: Option<u32>,
        secondary: Option<u32>,
        muted: Option<u32>,
    ) -> Self {
        self.colors.text_primary = primary;
        self.colors.text_secondary = secondary;
        self.colors.text_muted = muted;
        self
    }

    /// Set accent colors.
    pub fn with_accents(
        mut self,
        primary: Option<u32>,
        hover: Option<u32>,
        muted: Option<u32>,
    ) -> Self {
        self.colors.accent_primary = primary;
        self.colors.accent_hover = hover;
        self.colors.accent_muted = muted;
        self
    }

    /// Set border colors.
    pub fn with_borders(mut self, subtle: Option<u32>, strong: Option<u32>) -> Self {
        self.colors.border_subtle = subtle;
        self.colors.border_strong = strong;
        self
    }

    /// Set semantic colors.
    pub fn with_semantic(
        mut self,
        success: Option<u32>,
        warning: Option<u32>,
        error: Option<u32>,
        info: Option<u32>,
    ) -> Self {
        self.colors.success = success;
        self.colors.warning = warning;
        self.colors.error = error;
        self.colors.info = info;
        self
    }

    /// Set chart palette.
    pub fn with_chart_palette(mut self, palette: Vec<u32>) -> Self {
        self.colors.chart_palette = palette;
        self
    }
}

/// Base theme to inherit missing colors from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeBase {
    /// Inherit from the dark theme
    #[default]
    Dark,
    /// Inherit from the light theme
    Light,
}

impl ThemeBase {
    /// Parse from string.
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "light" | "l" => Self::Light,
            _ => Self::Dark,
        }
    }
}

/// Color palette for a custom theme.
///
/// All colors are optional - missing colors are inherited from the base theme.
/// Colors are stored as RGB hex values (e.g., 0x1a1b26 for #1a1b26).
#[derive(Debug, Clone, Default)]
pub struct ThemeColors {
    // Backgrounds
    /// Main canvas background (e.g., 0x1a1b26)
    pub bg_base: Option<u32>,
    /// Surface/panel background
    pub bg_surface: Option<u32>,
    /// Elevated elements (cards, dropdowns)
    pub bg_elevated: Option<u32>,

    // Text
    /// Primary text color
    pub text_primary: Option<u32>,
    /// Secondary text color
    pub text_secondary: Option<u32>,
    /// Muted/disabled text color
    pub text_muted: Option<u32>,

    // Accents
    /// Primary accent color
    pub accent_primary: Option<u32>,
    /// Hover accent color (brighter)
    pub accent_hover: Option<u32>,
    /// Muted accent color (for subtle backgrounds)
    pub accent_muted: Option<u32>,

    // Borders
    /// Subtle border color
    pub border_subtle: Option<u32>,
    /// Strong border color
    pub border_strong: Option<u32>,

    // Semantic colors
    /// Success color (green-ish)
    pub success: Option<u32>,
    /// Warning color (yellow/orange-ish)
    pub warning: Option<u32>,
    /// Error color (red-ish)
    pub error: Option<u32>,
    /// Info color (blue-ish)
    pub info: Option<u32>,

    // Chart palette
    /// Colors for chart series (up to 8)
    pub chart_palette: Vec<u32>,
}

impl ThemeColors {
    /// Parse a hex color string (e.g., "#1a1b26" or "1a1b26") to u32.
    pub fn parse_hex(s: &str) -> Option<u32> {
        let hex = s.trim().trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        u32::from_str_radix(hex, 16).ok()
    }

    /// Convert u32 RGB to (r, g, b) tuple.
    pub fn to_rgb(color: u32) -> (u8, u8, u8) {
        let r = ((color >> 16) & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = (color & 0xFF) as u8;
        (r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_base_parse() {
        assert_eq!(ThemeBase::parse("dark"), ThemeBase::Dark);
        assert_eq!(ThemeBase::parse("light"), ThemeBase::Light);
        assert_eq!(ThemeBase::parse("l"), ThemeBase::Light);
        assert_eq!(ThemeBase::parse("LIGHT"), ThemeBase::Light);
        assert_eq!(ThemeBase::parse("unknown"), ThemeBase::Dark); // default
    }

    #[test]
    fn test_parse_hex() {
        assert_eq!(ThemeColors::parse_hex("#1a1b26"), Some(0x1a1b26));
        assert_eq!(ThemeColors::parse_hex("1a1b26"), Some(0x1a1b26));
        assert_eq!(ThemeColors::parse_hex("#FFFFFF"), Some(0xFFFFFF));
        assert_eq!(ThemeColors::parse_hex("invalid"), None);
        assert_eq!(ThemeColors::parse_hex("#12345"), None); // too short
    }

    #[test]
    fn test_to_rgb() {
        assert_eq!(ThemeColors::to_rgb(0xFF0000), (255, 0, 0));
        assert_eq!(ThemeColors::to_rgb(0x00FF00), (0, 255, 0));
        assert_eq!(ThemeColors::to_rgb(0x0000FF), (0, 0, 255));
        assert_eq!(ThemeColors::to_rgb(0x1a1b26), (26, 27, 38));
    }

    #[test]
    fn test_theme_definition_builder() {
        let theme = ThemeDefinition::new("tokyo-night", "Tokyo Night", ThemeBase::Dark)
            .with_backgrounds(Some(0x1a1b26), Some(0x24283b), Some(0x414868))
            .with_accents(Some(0x7aa2f7), Some(0x89b4fa), None)
            .with_chart_palette(vec![0x7aa2f7, 0x9ece6a, 0xe0af68]);

        assert_eq!(theme.name, "tokyo-night");
        assert_eq!(theme.display_name, "Tokyo Night");
        assert_eq!(theme.base, ThemeBase::Dark);
        assert_eq!(theme.colors.bg_base, Some(0x1a1b26));
        assert_eq!(theme.colors.accent_primary, Some(0x7aa2f7));
        assert_eq!(theme.colors.chart_palette.len(), 3);
    }

    #[test]
    fn test_theme_default() {
        assert_eq!(ThemeBase::default(), ThemeBase::Dark);
    }
}
