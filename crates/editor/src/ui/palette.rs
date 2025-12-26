//! Obsidian Glass Design System
//!
//! A cohesive dark theme palette with layered depth and muted accents.
//! This module provides a centralized color system for consistent UI styling.

use egui::Color32;

use crate::ui::theme::AppTheme;

/// Core background colors - layered for depth
pub mod bg {
    use super::*;

    /// Main canvas background (almost black)
    pub const BASE: Color32 = Color32::from_rgb(10, 10, 10); // #0A0A0A

    /// Elevated panels, modals, floating elements
    pub const SURFACE: Color32 = Color32::from_rgb(20, 20, 20); // #141414

    /// Cards, dropdowns, nested containers
    pub const ELEVATED: Color32 = Color32::from_rgb(28, 28, 28); // #1C1C1C

    /// Interactive hover states
    pub const HOVER: Color32 = Color32::from_rgb(38, 38, 38); // #262626

    /// Active/pressed states
    pub const ACTIVE: Color32 = Color32::from_rgb(45, 45, 45); // #2D2D2D

    /// Selected item background
    pub const SELECTED: Color32 = Color32::from_rgb(35, 45, 40); // Subtle green tint
}

/// Border colors - subtle separators
pub mod border {
    use super::*;

    /// Very subtle dividers (almost invisible)
    pub const SUBTLE: Color32 = Color32::from_rgb(42, 42, 42); // #2A2A2A

    /// Default borders
    pub const DEFAULT: Color32 = Color32::from_rgb(58, 58, 58); // #3A3A3A

    /// Focused/highlighted element borders
    pub const FOCUS: Color32 = Color32::from_rgb(82, 82, 82); // #525252

    /// Accent border (for primary actions)
    pub const ACCENT: Color32 = Color32::from_rgb(16, 185, 129); // Emerald
}

/// Text colors - hierarchical for readability
pub mod text {
    use super::*;

    /// Primary text (high contrast)
    pub const PRIMARY: Color32 = Color32::from_rgb(250, 250, 250); // #FAFAFA

    /// Secondary text (medium contrast)
    pub const SECONDARY: Color32 = Color32::from_rgb(161, 161, 161); // #A1A1A1

    /// Tertiary text (low contrast - hints, placeholders)
    pub const TERTIARY: Color32 = Color32::from_rgb(107, 107, 107); // #6B6B6B

    /// Disabled text
    pub const DISABLED: Color32 = Color32::from_rgb(82, 82, 82); // #525252
}

/// Accent colors - for interactive elements
/// Using emerald green as the unified brand color
pub mod accent {
    use super::*;

    /// Primary accent (emerald green) - the Enya brand color
    pub const PRIMARY: Color32 = Color32::from_rgb(16, 185, 129); // #10B981

    /// Hover state for accent
    pub const HOVER: Color32 = Color32::from_rgb(52, 211, 153); // #34D399

    /// Muted accent for backgrounds (approximation of 15% opacity emerald on dark bg)
    pub const MUTED: Color32 = Color32::from_rgb(18, 38, 32); // Blended manually

    /// Light theme accent
    pub const LIGHT: Color32 = Color32::from_rgb(5, 150, 105); // #059669 - slightly darker for light bg
}

/// Highlight colors - for search matches, selections
pub mod highlight {
    use super::*;

    /// Search match highlight (emerald - matches brand)
    pub const MATCH: Color32 = Color32::from_rgb(52, 211, 153); // #34D399 - bright emerald for visibility

    /// Selection background (approximation of 20% opacity emerald on dark bg)
    pub const SELECTION: Color32 = Color32::from_rgb(20, 47, 38); // Blended manually

    /// Text selection (emerald tint)
    pub const TEXT_SELECTION: Color32 = Color32::from_rgb(20, 50, 42); // Emerald tint
}

/// Semantic colors - for status indicators
pub mod semantic {
    use super::*;

    /// Success (green)
    pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94); // #22C55E

    /// Warning (amber)
    pub const WARNING: Color32 = Color32::from_rgb(245, 158, 11); // #F59E0B

    /// Error (red)
    pub const ERROR: Color32 = Color32::from_rgb(239, 68, 68); // #EF4444

    /// Info (blue)
    pub const INFO: Color32 = Color32::from_rgb(59, 130, 246); // #3B82F6
}

/// Syntax highlighting colors
pub mod syntax {
    use super::*;

    /// Keywords (purple)
    pub const KEYWORD: Color32 = Color32::from_rgb(192, 132, 252); // #C084FC

    /// Tag keys, properties (sky blue)
    pub const KEY: Color32 = Color32::from_rgb(99, 179, 237); // #63B3ED

    /// Values, strings (emerald)
    pub const VALUE: Color32 = Color32::from_rgb(52, 211, 153); // #34D399

    /// Operators, punctuation (neutral)
    pub const PUNCTUATION: Color32 = Color32::from_rgb(148, 148, 148); // #949494

    /// Wildcards, special (amber)
    pub const SPECIAL: Color32 = Color32::from_rgb(251, 191, 36); // #FBBF24

    /// Negation, errors (coral)
    pub const NEGATION: Color32 = Color32::from_rgb(248, 113, 113); // #F87171
}

/// Chart colors - modern muted palette
pub mod chart {
    use super::*;

    /// Primary chart color (sky blue)
    pub const PRIMARY: Color32 = Color32::from_rgb(99, 179, 237); // #63B3ED

    /// Full palette for multi-series charts
    pub const PALETTE: &[Color32] = &[
        Color32::from_rgb(99, 179, 237),  // Soft sky blue
        Color32::from_rgb(129, 140, 248), // Soft indigo
        Color32::from_rgb(94, 234, 212),  // Soft teal
        Color32::from_rgb(192, 132, 252), // Soft purple
        Color32::from_rgb(251, 191, 36),  // Soft amber
        Color32::from_rgb(244, 114, 182), // Soft pink
        Color32::from_rgb(52, 211, 153),  // Soft emerald
        Color32::from_rgb(248, 113, 113), // Soft coral
    ];

    /// Commit marker color (violet 400 - contrasts well with green plots)
    pub const COMMIT_MARKER: Color32 = Color32::from_rgb(167, 139, 250); // #A78BFA
}

// ============================================================================
// Light theme palette (for completeness)
// ============================================================================

/// Light theme backgrounds
pub mod light_bg {
    use super::*;

    pub const BASE: Color32 = Color32::from_rgb(255, 255, 255); // #FFFFFF
    pub const SURFACE: Color32 = Color32::from_rgb(250, 250, 250); // #FAFAFA
    pub const ELEVATED: Color32 = Color32::from_rgb(245, 245, 245); // #F5F5F5
    pub const HOVER: Color32 = Color32::from_rgb(240, 240, 240); // #F0F0F0
    pub const SELECTED: Color32 = Color32::from_rgb(236, 253, 245); // Green tint
}

/// Light theme borders
pub mod light_border {
    use super::*;

    pub const SUBTLE: Color32 = Color32::from_rgb(229, 229, 229); // #E5E5E5
    pub const DEFAULT: Color32 = Color32::from_rgb(212, 212, 212); // #D4D4D4
    pub const FOCUS: Color32 = Color32::from_rgb(163, 163, 163); // #A3A3A3
}

/// Light theme text
pub mod light_text {
    use super::*;

    pub const PRIMARY: Color32 = Color32::from_rgb(23, 23, 23); // #171717
    pub const SECONDARY: Color32 = Color32::from_rgb(82, 82, 82); // #525252
    pub const TERTIARY: Color32 = Color32::from_rgb(115, 115, 115); // #737373
}

// ============================================================================
// Theme-aware helper functions
// ============================================================================

/// Get the appropriate background base color for the current theme
#[inline]
pub fn bg_base(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => bg::BASE,
        AppTheme::Light => light_bg::BASE,
    }
}

/// Get the appropriate surface color for the current theme
#[inline]
pub fn bg_surface(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => bg::SURFACE,
        AppTheme::Light => light_bg::SURFACE,
    }
}

/// Get the appropriate elevated background for the current theme
#[inline]
pub fn bg_elevated(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => bg::ELEVATED,
        AppTheme::Light => light_bg::ELEVATED,
    }
}

/// Get the appropriate hover background for the current theme
#[inline]
pub fn bg_hover(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => bg::HOVER,
        AppTheme::Light => light_bg::HOVER,
    }
}

/// Get the appropriate selected background for the current theme
#[inline]
pub fn bg_selected(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => bg::SELECTED,
        AppTheme::Light => light_bg::SELECTED,
    }
}

/// Get the appropriate subtle border for the current theme
#[inline]
pub fn border_subtle(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => border::SUBTLE,
        AppTheme::Light => light_border::SUBTLE,
    }
}

/// Get the appropriate default border for the current theme
#[inline]
pub fn border_default(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => border::DEFAULT,
        AppTheme::Light => light_border::DEFAULT,
    }
}

/// Get the appropriate primary text color for the current theme
#[inline]
pub fn text_primary(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => text::PRIMARY,
        AppTheme::Light => light_text::PRIMARY,
    }
}

/// Get the appropriate secondary text color for the current theme
#[inline]
pub fn text_secondary(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => text::SECONDARY,
        AppTheme::Light => light_text::SECONDARY,
    }
}

/// Get the appropriate tertiary text color for the current theme
#[inline]
pub fn text_tertiary(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Dark => text::TERTIARY,
        AppTheme::Light => light_text::TERTIARY,
    }
}
