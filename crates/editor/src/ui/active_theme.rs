//! Active theme abstraction for unified color access.
//!
//! This module provides `ActiveThemeColors` which can represent colors from
//! either a builtin `AppTheme` or a custom `ResolvedCustomTheme`. Components
//! use this to access theme colors without caring about the source.

use egui::Color32;

use super::custom_theme::ResolvedCustomTheme;
use super::theme::AppTheme;

/// Unified theme colors that can come from either builtin or custom themes.
///
/// This struct holds all the colors needed by UI components, resolved from
/// either an `AppTheme` or a `ResolvedCustomTheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActiveThemeColors {
    /// Whether this is a dark theme
    pub is_dark: bool,

    // Backgrounds
    pub bg_base: Color32,
    pub bg_surface: Color32,
    pub bg_elevated: Color32,
    pub bg_hover: Color32,

    // Text
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,

    // Accents
    pub accent_primary: Color32,
    pub accent_hover: Color32,
    pub accent_muted: Color32,

    // Borders
    pub border_subtle: Color32,
    pub border_default: Color32,
    pub border_focus: Color32,

    // Overlays
    pub overlay_bg: Color32,
    pub overlay_border: Color32,
    pub overlay_highlight: Color32,

    // Semantic
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,

    // Search/highlight
    pub highlight_match: Color32,

    // Charts
    pub chart_palette: [Color32; 8],
}

impl ActiveThemeColors {
    /// Create from a builtin theme.
    pub fn from_builtin(theme: AppTheme) -> Self {
        Self {
            is_dark: theme.is_dark(),
            bg_base: theme.bg_base(),
            bg_surface: theme.bg_surface(),
            bg_elevated: theme.bg_elevated(),
            bg_hover: theme.bg_hover(),
            text_primary: theme.text_primary(),
            text_secondary: theme.text_secondary(),
            text_muted: theme.text_tertiary(),
            accent_primary: theme.accent_primary(),
            accent_hover: theme.accent_hover(),
            accent_muted: theme.accent_muted(),
            border_subtle: theme.border_subtle(),
            border_default: theme.border_default(),
            border_focus: theme.border_focus(),
            overlay_bg: theme.overlay_bg(),
            overlay_border: theme.overlay_border(),
            overlay_highlight: theme.overlay_highlight(),
            success: theme.semantic_success(),
            warning: theme.semantic_warning(),
            error: theme.semantic_error(),
            info: theme.semantic_info(),
            highlight_match: theme.highlight_match_text(),
            chart_palette: theme.chart_palette(),
        }
    }

    /// Create from a resolved custom theme.
    pub fn from_custom(theme: &ResolvedCustomTheme) -> Self {
        // For custom themes, derive some colors that may not be specified
        let bg_hover = if theme.is_dark {
            Color32::from_rgba_unmultiplied(
                theme.bg_surface.r().saturating_add(15),
                theme.bg_surface.g().saturating_add(15),
                theme.bg_surface.b().saturating_add(15),
                theme.bg_surface.a(),
            )
        } else {
            Color32::from_rgba_unmultiplied(
                theme.bg_surface.r().saturating_sub(10),
                theme.bg_surface.g().saturating_sub(10),
                theme.bg_surface.b().saturating_sub(10),
                theme.bg_surface.a(),
            )
        };

        // Overlay colors derived from base colors
        let overlay_bg = Color32::from_rgba_unmultiplied(
            theme.bg_elevated.r(),
            theme.bg_elevated.g(),
            theme.bg_elevated.b(),
            240, // Semi-transparent
        );
        let overlay_border = theme.border_subtle;
        let overlay_highlight = Color32::from_rgba_unmultiplied(
            theme.accent_primary.r(),
            theme.accent_primary.g(),
            theme.accent_primary.b(),
            30, // Subtle highlight
        );

        Self {
            is_dark: theme.is_dark,
            bg_base: theme.bg_base,
            bg_surface: theme.bg_surface,
            bg_elevated: theme.bg_elevated,
            bg_hover,
            text_primary: theme.text_primary,
            text_secondary: theme.text_secondary,
            text_muted: theme.text_muted,
            accent_primary: theme.accent_primary,
            accent_hover: theme.accent_hover,
            accent_muted: theme.accent_muted,
            border_subtle: theme.border_subtle,
            border_default: theme.border_strong,
            border_focus: theme.accent_primary,
            overlay_bg,
            overlay_border,
            overlay_highlight,
            success: theme.success,
            warning: theme.warning,
            error: theme.error,
            info: theme.info,
            // Use accent color for search match highlighting in custom themes
            highlight_match: theme.accent_primary,
            chart_palette: theme.chart_palette,
        }
    }
}

impl From<AppTheme> for ActiveThemeColors {
    fn from(theme: AppTheme) -> Self {
        Self::from_builtin(theme)
    }
}

impl From<&ResolvedCustomTheme> for ActiveThemeColors {
    fn from(theme: &ResolvedCustomTheme) -> Self {
        Self::from_custom(theme)
    }
}
