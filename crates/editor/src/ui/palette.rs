//! Obsidian Glass Design System
//!
//! A premium dark theme palette with refined depth, subtle warmth, and configurable accents.
//! Designed for a luxurious, high-end developer experience with Departure Mono typography.
//! This module provides a centralized color system for consistent UI styling.
//!
//! The default theme is Obsidian Glass Emerald, but users can switch to other themes
//! like Nord, Gruvbox, Rose, or Amber via `:theme <name>`.

use egui::Color32;

use crate::ui::theme::AppTheme;

/// Core background colors - layered for depth with subtle warmth
pub mod bg {
    use super::*;

    /// Main canvas background - rich obsidian black with subtle warmth
    pub const BASE: Color32 = Color32::from_rgb(8, 8, 10); // Slightly cooler for contrast

    /// Elevated panels, modals, floating elements - subtle lift
    pub const SURFACE: Color32 = Color32::from_rgb(18, 18, 21); // Hint of depth

    /// Cards, dropdowns, nested containers - refined elevation
    pub const ELEVATED: Color32 = Color32::from_rgb(26, 26, 30); // Subtle blue undertone

    /// Interactive hover states - gentle warmth on interaction
    pub const HOVER: Color32 = Color32::from_rgb(36, 36, 40); // Refined hover

    /// Active/pressed states - tactile feedback
    pub const ACTIVE: Color32 = Color32::from_rgb(44, 44, 50); // Premium active

    /// Selected item background - elegant emerald tint
    pub const SELECTED: Color32 = Color32::from_rgb(28, 42, 36); // Richer emerald tint
}

/// Border colors - refined separators with subtle gradation
pub mod border {
    use super::*;

    /// Very subtle dividers - barely perceptible structure
    pub const SUBTLE: Color32 = Color32::from_rgb(38, 38, 44); // Refined divider

    /// Default borders - clean definition
    pub const DEFAULT: Color32 = Color32::from_rgb(52, 52, 60); // Slightly cooler

    /// Focused/highlighted element borders - subtle emerald tint for cohesive theme
    pub const FOCUS: Color32 = Color32::from_rgb(55, 80, 72); // Emerald-tinted focus

    /// Accent border (for primary actions) - signature emerald
    pub const ACCENT: Color32 = Color32::from_rgb(16, 185, 129); // Emerald
}

/// Text colors - refined hierarchy for optimal readability
pub mod text {
    use super::*;

    /// Primary text - crisp, slightly warm white for readability
    pub const PRIMARY: Color32 = Color32::from_rgb(248, 248, 252); // Refined white

    /// Secondary text - balanced mid-tone for supporting content
    pub const SECONDARY: Color32 = Color32::from_rgb(158, 158, 168); // Slightly cooler

    /// Tertiary text - subtle hints and placeholders
    pub const TERTIARY: Color32 = Color32::from_rgb(100, 100, 112); // Refined muted

    /// Disabled text - clearly reduced but still readable
    pub const DISABLED: Color32 = Color32::from_rgb(75, 75, 85); // Consistent with border
}

/// Accent colors - default emerald for interactive elements
/// This module is kept for backwards compatibility; use accent_* functions for theme-aware colors
pub mod accent {
    use super::*;

    /// Primary accent - signature emerald with depth
    pub const PRIMARY: Color32 = Color32::from_rgb(16, 185, 129); // #10B981 - Enya emerald

    /// Hover state - luminous emerald for clear interaction feedback
    pub const HOVER: Color32 = Color32::from_rgb(52, 211, 153); // #34D399 - bright emerald

    /// Muted accent - subtle emerald tint for backgrounds
    pub const MUTED: Color32 = Color32::from_rgb(20, 40, 34); // Richer emerald background

    /// Light theme accent - deeper emerald for light backgrounds
    pub const LIGHT: Color32 = Color32::from_rgb(5, 150, 105); // #059669

    /// Subtle glow color - for premium hover effects
    pub const GLOW: Color32 = Color32::from_rgba_premultiplied(16, 185, 129, 30); // 12% emerald
}

/// Emerald accent colors (default Obsidian Glass theme)
pub mod emerald {
    use super::*;

    pub const PRIMARY: Color32 = Color32::from_rgb(16, 185, 129); // #10B981
    pub const HOVER: Color32 = Color32::from_rgb(52, 211, 153); // #34D399
    pub const MUTED: Color32 = Color32::from_rgb(20, 40, 34);
    pub const LIGHT: Color32 = Color32::from_rgb(5, 150, 105); // #059669
    pub const GLOW: Color32 = Color32::from_rgba_premultiplied(16, 185, 129, 30);
    pub const SELECTION: Color32 = Color32::from_rgb(24, 52, 42);
    pub const FOCUS_BORDER: Color32 = Color32::from_rgb(55, 80, 72);
}

/// Nord accent colors (Arctic, icy blue)
pub mod nord {
    use super::*;

    pub const PRIMARY: Color32 = Color32::from_rgb(136, 192, 208); // #88C0D0 - Nord frost
    pub const HOVER: Color32 = Color32::from_rgb(143, 188, 187); // #8FBCBB - lighter frost
    pub const MUTED: Color32 = Color32::from_rgb(20, 35, 45);
    pub const LIGHT: Color32 = Color32::from_rgb(94, 129, 172); // #5E81AC - darker for light mode
    pub const GLOW: Color32 = Color32::from_rgba_premultiplied(136, 192, 208, 30);
    pub const SELECTION: Color32 = Color32::from_rgb(30, 50, 60);
    pub const FOCUS_BORDER: Color32 = Color32::from_rgb(59, 66, 82); // #3B4252
}

/// Gruvbox accent colors (Warm retro)
pub mod gruvbox {
    use super::*;

    pub const PRIMARY: Color32 = Color32::from_rgb(214, 93, 14); // #D65D0E - Gruvbox orange
    pub const HOVER: Color32 = Color32::from_rgb(254, 128, 25); // #FE8019 - bright orange
    pub const MUTED: Color32 = Color32::from_rgb(40, 30, 20);
    pub const LIGHT: Color32 = Color32::from_rgb(175, 58, 3); // #AF3A03 - darker for light mode
    pub const GLOW: Color32 = Color32::from_rgba_premultiplied(214, 93, 14, 30);
    pub const SELECTION: Color32 = Color32::from_rgb(50, 40, 30);
    pub const FOCUS_BORDER: Color32 = Color32::from_rgb(80, 73, 69); // #504945
}

/// Rose accent colors (Soft pink)
pub mod rose {
    use super::*;

    pub const PRIMARY: Color32 = Color32::from_rgb(244, 114, 182); // #F472B6
    pub const HOVER: Color32 = Color32::from_rgb(251, 146, 201); // #FB92C9
    pub const MUTED: Color32 = Color32::from_rgb(40, 25, 35);
    pub const LIGHT: Color32 = Color32::from_rgb(219, 39, 119); // #DB2777 - darker for light mode
    pub const GLOW: Color32 = Color32::from_rgba_premultiplied(244, 114, 182, 30);
    pub const SELECTION: Color32 = Color32::from_rgb(55, 35, 45);
    pub const FOCUS_BORDER: Color32 = Color32::from_rgb(90, 60, 75);
}

/// Amber accent colors (Warm gold)
pub mod amber {
    use super::*;

    pub const PRIMARY: Color32 = Color32::from_rgb(245, 158, 11); // #F59E0B
    pub const HOVER: Color32 = Color32::from_rgb(252, 191, 73); // #FCBF49
    pub const MUTED: Color32 = Color32::from_rgb(40, 35, 20);
    pub const LIGHT: Color32 = Color32::from_rgb(217, 119, 6); // #D97706 - darker for light mode
    pub const GLOW: Color32 = Color32::from_rgba_premultiplied(245, 158, 11, 30);
    pub const SELECTION: Color32 = Color32::from_rgb(50, 45, 25);
    pub const FOCUS_BORDER: Color32 = Color32::from_rgb(90, 80, 55);
}

/// Highlight colors - premium selections and search matches
pub mod highlight {
    use super::*;

    /// Search match - luminous emerald for high visibility
    pub const MATCH: Color32 = Color32::from_rgb(52, 211, 153); // #34D399 - bright emerald

    /// Selection background - rich emerald tint for selections
    pub const SELECTION: Color32 = Color32::from_rgb(24, 52, 42); // Deeper emerald selection

    /// Text selection - refined emerald tint
    pub const TEXT_SELECTION: Color32 = Color32::from_rgb(22, 55, 45); // Premium selection

    /// Cursor line highlight - subtle row emphasis
    pub const CURSOR_LINE: Color32 = Color32::from_rgb(14, 14, 18); // Barely visible lift
}

/// Semantic colors - refined status indicators
pub mod semantic {
    use super::*;

    /// Success - vibrant green for positive states
    pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94); // #22C55E

    /// Warning - warm amber for attention
    pub const WARNING: Color32 = Color32::from_rgb(251, 176, 45); // Warmer amber

    /// Error - refined coral-red for errors
    pub const ERROR: Color32 = Color32::from_rgb(239, 82, 82); // Slightly warmer

    /// Info - refined blue for information
    pub const INFO: Color32 = Color32::from_rgb(82, 146, 255); // Brighter blue
}

/// Syntax highlighting colors - refined for readability
pub mod syntax {
    use super::*;

    /// Keywords - soft violet for language constructs
    pub const KEYWORD: Color32 = Color32::from_rgb(198, 146, 255); // Richer violet

    /// Tag keys, properties - refined sky blue
    pub const KEY: Color32 = Color32::from_rgb(110, 190, 248); // Brighter blue

    /// Values, strings - signature emerald
    pub const VALUE: Color32 = Color32::from_rgb(52, 211, 153); // #34D399 - bright emerald

    /// Operators, punctuation - subtle neutral
    pub const PUNCTUATION: Color32 = Color32::from_rgb(140, 140, 155); // Slightly cooler

    /// Wildcards, special - warm gold
    pub const SPECIAL: Color32 = Color32::from_rgb(255, 200, 60); // Richer gold

    /// Negation, errors - soft coral
    pub const NEGATION: Color32 = Color32::from_rgb(255, 120, 120); // Refined coral
}

/// Chart colors - premium palette for data visualization
pub mod chart {
    use super::*;

    /// Primary chart color - refined sky blue
    pub const PRIMARY: Color32 = Color32::from_rgb(110, 190, 248); // Brighter blue

    /// Full palette for multi-series charts - harmonious and distinct
    pub const PALETTE: &[Color32] = &[
        Color32::from_rgb(110, 190, 248), // Refined sky blue
        Color32::from_rgb(140, 150, 255), // Refined indigo
        Color32::from_rgb(100, 240, 218), // Vibrant teal
        Color32::from_rgb(198, 146, 255), // Rich violet
        Color32::from_rgb(255, 200, 60),  // Warm gold
        Color32::from_rgb(255, 130, 190), // Soft rose
        Color32::from_rgb(52, 211, 153),  // Signature emerald
        Color32::from_rgb(255, 120, 120), // Soft coral
    ];

    /// Commit marker color - distinguished violet
    pub const COMMIT_MARKER: Color32 = Color32::from_rgb(180, 155, 255); // Richer violet
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
// Theme-aware helper functions (delegate to AppTheme methods)
// ============================================================================

/// Get the appropriate background base color for the current theme
#[inline]
pub fn bg_base(theme: AppTheme) -> Color32 {
    theme.bg_base()
}

/// Get the appropriate surface color for the current theme
#[inline]
pub fn bg_surface(theme: AppTheme) -> Color32 {
    theme.bg_surface()
}

/// Get the appropriate elevated background for the current theme
#[inline]
pub fn bg_elevated(theme: AppTheme) -> Color32 {
    theme.bg_elevated()
}

/// Get the appropriate hover background for the current theme
#[inline]
pub fn bg_hover(theme: AppTheme) -> Color32 {
    theme.bg_hover()
}

/// Get the appropriate selected background for the current theme
#[inline]
pub fn bg_selected(theme: AppTheme) -> Color32 {
    theme.bg_selected()
}

/// Get the appropriate subtle border for the current theme
#[inline]
pub fn border_subtle(theme: AppTheme) -> Color32 {
    theme.border_subtle()
}

/// Get the appropriate default border for the current theme
#[inline]
pub fn border_default(theme: AppTheme) -> Color32 {
    theme.border_default()
}

/// Get the appropriate primary text color for the current theme
#[inline]
pub fn text_primary(theme: AppTheme) -> Color32 {
    theme.text_primary()
}

/// Get the appropriate secondary text color for the current theme
#[inline]
pub fn text_secondary(theme: AppTheme) -> Color32 {
    theme.text_secondary()
}

/// Get the appropriate tertiary text color for the current theme
#[inline]
pub fn text_tertiary(theme: AppTheme) -> Color32 {
    theme.text_tertiary()
}

// ============================================================================
// Accent-aware helper functions (delegate to AppTheme methods)
// ============================================================================

/// Get the primary accent color for the current theme
#[inline]
pub fn accent_primary(theme: AppTheme) -> Color32 {
    theme.accent_primary()
}

/// Get the hover accent color for the current theme
#[inline]
pub fn accent_hover(theme: AppTheme) -> Color32 {
    theme.accent_hover()
}

/// Get the muted accent color for the current theme
#[inline]
pub fn accent_muted(theme: AppTheme) -> Color32 {
    theme.accent_muted()
}

/// Get the glow accent color for the current theme
#[inline]
pub fn accent_glow(theme: AppTheme) -> Color32 {
    theme.accent_glow()
}

/// Get the selection background color for the current theme
#[inline]
pub fn accent_selection(theme: AppTheme) -> Color32 {
    theme.accent_selection()
}

/// Get the focus border color for the current theme
#[inline]
pub fn accent_focus_border(theme: AppTheme) -> Color32 {
    theme.border_focus()
}
