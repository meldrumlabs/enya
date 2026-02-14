//! Application theme system
//!
//! This module defines the extensible theme system for the editor.
//! The default theme is "Dark" (Obsidian Glass with Enya Emerald accent).

use egui::Color32;
use egui::Shadow;
use egui::Stroke;
use egui::Visuals;
use egui::style::Selection;
use egui::style::TextCursorStyle;
use egui::style::Widgets;

use super::active_theme::ActiveThemeColors;

/// Application theme presets
///
/// Each theme is a complete color scheme including backgrounds, accents, and UI colors.
/// The default theme is Dark (Obsidian Glass with Enya Emerald accent).
#[derive(
    Clone, Copy, Eq, PartialEq, Hash, Default, Debug, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum AppTheme {
    /// Dark theme (Obsidian Glass) - signature Enya green #10B981
    #[default]
    Dark,
    /// Light theme - Paper/Ink aesthetic with warm cream backgrounds and rich black text
    Light,
    /// Midnight theme - Deep space blue with electric blue accent #3B82F6
    Midnight,
    /// Ayu Dark theme - Soft amber warmth with orange accent #FFB454
    Ayu,
    /// Aurora theme - Northern Lights with aurora teal accent #7EE8B8
    Aurora,
    /// Graphite theme - Industrial precision with molten orange accent #E85D04
    Graphite,
    /// Ink theme - Monochrome editorial with pure silver accent #C0C0C8
    Ink,
    /// Custom theme from plugin - carries resolved colors directly
    #[serde(skip)]
    Custom(ActiveThemeColors),
}

impl AppTheme {
    /// Returns the display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Midnight => "Midnight",
            Self::Ayu => "Ayu",
            Self::Aurora => "Aurora",
            Self::Graphite => "Graphite",
            Self::Ink => "Ink",
            Self::Custom(_) => "Custom",
        }
    }

    /// Returns all available themes
    pub fn all() -> &'static [AppTheme] {
        &[
            Self::Dark,
            Self::Light,
            Self::Midnight,
            Self::Ayu,
            Self::Aurora,
            Self::Graphite,
            Self::Ink,
        ]
    }

    /// Returns true if this is a dark theme
    pub fn is_dark(&self) -> bool {
        match self {
            Self::Custom(colors) => colors.is_dark,
            Self::Light => false,
            _ => true,
        }
    }

    /// Returns true if this is a light theme
    pub fn is_light(&self) -> bool {
        !self.is_dark()
    }

    /// Cycle to the next theme (Custom themes cycle back to Dark)
    pub fn next(&mut self) {
        let themes = Self::all();
        // Custom themes aren't in the list, so start from 0 (Dark)
        let current_idx = themes.iter().position(|t| *t == *self).unwrap_or(0);
        let next_idx = (current_idx + 1) % themes.len();
        *self = themes[next_idx];
    }

    /// Parse a theme name (case-insensitive)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dark" | "d" | "default" | "emerald" => Some(Self::Dark),
            "light" | "l" => Some(Self::Light),
            "midnight" | "m" | "space" => Some(Self::Midnight),
            "ayu" | "a" | "amber" => Some(Self::Ayu),
            "aurora" | "ar" | "northern" | "lights" | "borealis" => Some(Self::Aurora),
            "graphite" | "graph" | "industrial" | "foundry" | "molten" => Some(Self::Graphite),
            "ink" | "i" | "editorial" | "monochrome" | "silver" => Some(Self::Ink),
            _ => None,
        }
    }

    /// Get the egui Visuals for this theme
    pub fn visuals(&self) -> Visuals {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    super::design::dark_theme(*self)
                } else {
                    super::design::light_theme(*self)
                }
            }
            Self::Light => super::design::light_theme(*self),
            _ => super::design::dark_theme(*self),
        }
    }

    // =========================================================================
    // Background Colors
    // =========================================================================

    /// Main canvas background color
    pub fn bg_base(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_base,
            Self::Light => Color32::from_rgb(250, 248, 245), // Warm cream paper #FAF8F5
            Self::Midnight => Color32::from_rgb(10, 11, 16), // Deep space blue #0A0B10
            Self::Ayu => Color32::from_rgb(10, 14, 20),      // Deep charcoal #0A0E14
            Self::Aurora => Color32::from_rgb(13, 17, 23),   // Deep night sky #0D1117
            Self::Graphite => Color32::from_rgb(18, 18, 20), // Deep warm charcoal #121214
            Self::Ink => Color32::from_rgb(10, 10, 15),      // Blue-black #0A0A0F
            Self::Dark => Color32::from_rgb(8, 8, 10),       // Obsidian dark
        }
    }

    /// Surface/panel background color
    pub fn bg_surface(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Light => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Midnight => Color32::from_rgb(18, 20, 28), // Deep navy #12141C
            Self::Ayu => Color32::from_rgb(13, 16, 23),      // Dark blue-gray #0D1017
            Self::Aurora => Color32::from_rgb(22, 27, 34),   // Night surface #161B22
            Self::Graphite => Color32::from_rgb(26, 26, 28), // Surface #1A1A1C
            Self::Ink => Color32::from_rgb(18, 18, 24),      // Surface #121218
            Self::Dark => Color32::from_rgb(18, 18, 21),
        }
    }

    /// Elevated elements (cards, dropdowns)
    pub fn bg_elevated(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_elevated,
            Self::Light => Color32::from_rgb(240, 236, 230), // Aged paper #F0ECE6
            Self::Midnight => Color32::from_rgb(26, 29, 40), // Lighter navy #1A1D28
            Self::Ayu => Color32::from_rgb(21, 26, 34),      // Slightly lighter #151A22
            Self::Aurora => Color32::from_rgb(33, 38, 45),   // Elevated night #21262D
            Self::Graphite => Color32::from_rgb(36, 36, 38), // Elevated #242426
            Self::Ink => Color32::from_rgb(28, 28, 36),      // Elevated #1C1C24
            Self::Dark => Color32::from_rgb(26, 26, 30),
        }
    }

    /// Hover state background
    pub fn bg_hover(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_hover,
            Self::Light => Color32::from_rgb(232, 228, 220), // Darker paper #E8E4DC
            Self::Midnight => Color32::from_rgb(34, 38, 52), // Hover navy #222634
            Self::Ayu => Color32::from_rgb(28, 34, 44),      // Hover charcoal #1C222C
            Self::Aurora => Color32::from_rgb(40, 46, 56),   // Hover night #282E38
            Self::Graphite => Color32::from_rgb(46, 46, 50), // Hover #2E2E32
            Self::Ink => Color32::from_rgb(38, 38, 46),      // Hover #26262E
            Self::Dark => Color32::from_rgb(36, 36, 40),
        }
    }

    /// Selected item background
    pub fn bg_selected(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Light => Color32::from_rgb(225, 220, 210), // Selected paper #E1DCD2
            Self::Midnight => Color32::from_rgb(25, 40, 65), // Blue selection #192841
            Self::Ayu => Color32::from_rgb(40, 35, 25),      // Amber tint selection
            Self::Aurora => Color32::from_rgb(25, 50, 45),   // Teal tint selection
            Self::Graphite => Color32::from_rgb(58, 42, 32), // Orange tint selection #3A2A20
            Self::Ink => Color32::from_rgb(32, 32, 42),      // Silver tint selection #20202A
            Self::Dark => Color32::from_rgb(28, 42, 36),     // Emerald tint
        }
    }

    /// Card background (slightly darker than elevated)
    pub fn bg_card(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Light => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Midnight => Color32::from_rgb(20, 22, 32), // Card navy
            Self::Ayu => Color32::from_rgb(16, 20, 28),      // Card charcoal
            Self::Aurora => Color32::from_rgb(27, 32, 40),   // Card night
            Self::Graphite => Color32::from_rgb(30, 30, 32), // Card graphite
            Self::Ink => Color32::from_rgb(22, 22, 28),      // Card ink
            Self::Dark => Color32::from_rgb(18, 18, 22),
        }
    }

    /// Inset background (darker than surface, for inputs)
    pub fn bg_inset(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_base,
            Self::Light => Color32::from_rgb(255, 253, 250), // Bright paper #FFFDF a
            Self::Midnight => Color32::from_rgb(14, 15, 22), // Inset navy
            Self::Ayu => Color32::from_rgb(8, 11, 16),       // Inset charcoal
            Self::Aurora => Color32::from_rgb(10, 14, 18),   // Inset night
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Inset graphite
            Self::Ink => Color32::from_rgb(8, 8, 12),        // Inset ink
            Self::Dark => Color32::from_rgb(12, 12, 15),
        }
    }

    // =========================================================================
    // Border Colors
    // =========================================================================

    /// Subtle divider color
    pub fn border_subtle(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_subtle,
            Self::Light => Color32::from_rgb(220, 215, 205), // Subtle paper edge #DCD7CD
            Self::Midnight => Color32::from_rgb(40, 44, 58), // Subtle navy border
            Self::Ayu => Color32::from_rgb(35, 42, 52),      // Subtle charcoal border
            Self::Aurora => Color32::from_rgb(48, 54, 62),   // Subtle night border
            Self::Graphite => Color32::from_rgb(42, 42, 46), // Subtle border #2A2A2E
            Self::Ink => Color32::from_rgb(30, 30, 40),      // Subtle border #1E1E28
            Self::Dark => Color32::from_rgb(38, 38, 44),
        }
    }

    /// Default border color
    pub fn border_default(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Light => Color32::from_rgb(200, 195, 185), // Paper edge #C8C3B9
            Self::Midnight => Color32::from_rgb(55, 60, 78), // Navy border
            Self::Ayu => Color32::from_rgb(48, 56, 68),      // Charcoal border
            Self::Aurora => Color32::from_rgb(56, 62, 72),   // Night border
            Self::Graphite => Color32::from_rgb(58, 58, 64), // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),      // Default border #2E2E38
            Self::Dark => Color32::from_rgb(52, 52, 60),
        }
    }

    /// Focus border color
    pub fn border_focus(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_focus,
            Self::Light => Color32::from_rgb(100, 100, 100), // Dark gray ink #646464
            Self::Midnight => Color32::from_rgb(59, 130, 246), // Electric blue focus
            Self::Ayu => Color32::from_rgb(180, 120, 60),    // Amber focus
            Self::Aurora => Color32::from_rgb(126, 232, 184), // Aurora teal focus
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Molten orange focus #E85D04
            Self::Ink => Color32::from_rgb(192, 192, 200),   // Silver focus #C0C0C8
            Self::Dark => Color32::from_rgb(55, 80, 72),
        }
    }

    // =========================================================================
    // Text Colors
    // =========================================================================

    /// Primary text color
    pub fn text_primary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_primary,
            Self::Light => Color32::from_rgb(30, 30, 30), // Rich black ink #1E1E1E
            Self::Midnight => Color32::from_rgb(228, 228, 231), // Off-white #E4E4E7
            Self::Ayu => Color32::from_rgb(191, 189, 182), // Off-white #BFBDB6
            Self::Aurora => Color32::from_rgb(230, 237, 243), // Crisp white #E6EDF3
            Self::Graphite => Color32::from_rgb(232, 230, 224), // Warm off-white #E8E6E0
            Self::Ink => Color32::from_rgb(228, 228, 236), // Cool off-white #E4E4EC
            Self::Dark => Color32::from_rgb(248, 248, 252),
        }
    }

    /// Secondary text color
    pub fn text_secondary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_secondary,
            Self::Light => Color32::from_rgb(80, 80, 80), // Lighter ink #505050
            Self::Midnight => Color32::from_rgb(161, 161, 170), // Silver #A1A1AA
            Self::Ayu => Color32::from_rgb(98, 106, 115), // Muted gray #626A73
            Self::Aurora => Color32::from_rgb(139, 148, 158), // Muted silver #8B949E
            Self::Graphite => Color32::from_rgb(168, 166, 160), // Secondary text #A8A6A0
            Self::Ink => Color32::from_rgb(152, 152, 168), // Secondary text #9898A8
            Self::Dark => Color32::from_rgb(158, 158, 168),
        }
    }

    /// Tertiary/muted text color
    pub fn text_tertiary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Light => Color32::from_rgb(120, 115, 110), // Faded ink #78736E
            Self::Midnight => Color32::from_rgb(113, 113, 122), // Darker silver #71717A
            Self::Ayu => Color32::from_rgb(75, 82, 90),      // Darker gray #4B525A
            Self::Aurora => Color32::from_rgb(110, 118, 129), // Deep night #6E7681
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),     // Tertiary text #606070
            Self::Dark => Color32::from_rgb(100, 100, 112),
        }
    }

    // =========================================================================
    // Accent Colors
    // =========================================================================

    /// Primary accent color
    pub fn accent_primary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            Self::Dark => Color32::from_rgb(16, 185, 129), // #10B981 Enya Emerald
            Self::Light => Color32::from_rgb(50, 50, 50),  // Charcoal ink #323232
            Self::Midnight => Color32::from_rgb(59, 130, 246), // Electric Blue #3B82F6
            Self::Ayu => Color32::from_rgb(255, 180, 84),  // Warm Orange #FFB454
            Self::Aurora => Color32::from_rgb(126, 232, 184), // Aurora Teal #7EE8B8
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Molten orange #E85D04
            Self::Ink => Color32::from_rgb(192, 192, 200), // Pure silver #C0C0C8
        }
    }

    /// Hover accent color (brighter)
    pub fn accent_hover(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_hover,
            Self::Dark => Color32::from_rgb(52, 211, 153),
            Self::Light => Color32::from_rgb(30, 30, 30), // Rich black ink hover #1E1E1E
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Brighter Blue #60A5FA
            Self::Ayu => Color32::from_rgb(255, 204, 128), // Brighter Orange #FFCC80
            Self::Aurora => Color32::from_rgb(165, 243, 206), // Bright Aurora #A5F3CE
            Self::Graphite => Color32::from_rgb(255, 116, 32), // Brighter orange #FF7420
            Self::Ink => Color32::from_rgb(216, 216, 224), // Brighter silver #D8D8E0
        }
    }

    /// Muted accent color (for subtle backgrounds)
    pub fn accent_muted(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Dark => Color32::from_rgb(20, 40, 34),
            Self::Light => Color32::from_rgb(240, 236, 228), // Light sepia tint #F0ECE4
            Self::Midnight => Color32::from_rgb(20, 30, 50), // Muted blue bg
            Self::Ayu => Color32::from_rgb(30, 25, 18),      // Muted amber bg
            Self::Aurora => Color32::from_rgb(20, 40, 35),   // Muted aurora bg
            Self::Graphite => Color32::from_rgb(40, 30, 22), // Muted orange bg
            Self::Ink => Color32::from_rgb(28, 28, 35),      // Muted silver bg
        }
    }

    /// Accent glow color (semi-transparent)
    pub fn accent_glow(&self) -> Color32 {
        match self {
            Self::Custom(colors) => Color32::from_rgba_premultiplied(
                colors.accent_primary.r(),
                colors.accent_primary.g(),
                colors.accent_primary.b(),
                30,
            ),
            Self::Dark => Color32::from_rgba_premultiplied(16, 185, 129, 30),
            Self::Light => Color32::from_rgba_premultiplied(50, 50, 50, 40),
            Self::Midnight => Color32::from_rgba_premultiplied(59, 130, 246, 30),
            Self::Ayu => Color32::from_rgba_premultiplied(255, 180, 84, 30),
            Self::Aurora => Color32::from_rgba_premultiplied(126, 232, 184, 30),
            Self::Graphite => Color32::from_rgba_premultiplied(232, 93, 4, 30),
            Self::Ink => Color32::from_rgba_premultiplied(192, 192, 200, 30),
        }
    }

    /// Selection background color
    pub fn accent_selection(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Dark => Color32::from_rgb(24, 52, 42),
            Self::Light => Color32::from_rgb(230, 225, 215), // Warm sepia selection #E6E1D7
            Self::Midnight => Color32::from_rgb(30, 45, 70), // Blue selection
            Self::Ayu => Color32::from_rgb(45, 38, 25),      // Amber selection
            Self::Aurora => Color32::from_rgb(30, 55, 48),   // Teal selection
            Self::Graphite => Color32::from_rgb(60, 45, 32), // Orange tint selection
            Self::Ink => Color32::from_rgb(38, 38, 50),      // Silver tint selection
        }
    }

    // =========================================================================
    // Overlay Colors (for modals, dropdowns, popups)
    // =========================================================================

    /// Overlay background (frosted glass)
    pub fn overlay_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_bg,
            Self::Light => Color32::from_rgba_unmultiplied(250, 248, 245, 250),
            Self::Midnight => Color32::from_rgba_unmultiplied(14, 16, 24, 245),
            Self::Ayu => Color32::from_rgba_unmultiplied(12, 16, 22, 245),
            Self::Aurora => Color32::from_rgba_unmultiplied(16, 20, 26, 245),
            Self::Graphite => Color32::from_rgba_unmultiplied(18, 18, 20, 245),
            Self::Ink => Color32::from_rgba_unmultiplied(10, 10, 15, 245),
            Self::Dark => Color32::from_rgba_unmultiplied(14, 14, 16, 245),
        }
    }

    /// Overlay background (deep/premium glass)
    pub fn overlay_bg_deep(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_bg,
            Self::Light => Color32::from_rgba_unmultiplied(245, 242, 237, 248),
            Self::Midnight => Color32::from_rgba_unmultiplied(10, 12, 20, 235),
            Self::Ayu => Color32::from_rgba_unmultiplied(8, 12, 18, 235),
            Self::Aurora => Color32::from_rgba_unmultiplied(12, 16, 22, 235),
            Self::Graphite => Color32::from_rgba_unmultiplied(14, 14, 16, 235),
            Self::Ink => Color32::from_rgba_unmultiplied(8, 8, 12, 235),
            Self::Dark => Color32::from_rgba_unmultiplied(12, 12, 14, 235),
        }
    }

    /// Overlay border color
    pub fn overlay_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_border,
            Self::Light => Color32::from_rgba_unmultiplied(200, 195, 185, 220),
            Self::Midnight => Color32::from_rgba_unmultiplied(55, 60, 80, 160),
            Self::Ayu => Color32::from_rgba_unmultiplied(48, 56, 68, 160),
            Self::Aurora => Color32::from_rgba_unmultiplied(50, 58, 68, 160),
            Self::Graphite => Color32::from_rgba_unmultiplied(58, 58, 64, 160),
            Self::Ink => Color32::from_rgba_unmultiplied(46, 46, 56, 160),
            Self::Dark => Color32::from_rgba_unmultiplied(45, 45, 48, 160),
        }
    }

    /// Overlay inner highlight (top edge glow for glass effect)
    pub fn overlay_highlight(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_highlight,
            Self::Light => Color32::from_rgba_unmultiplied(255, 255, 252, 100),
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 12),
        }
    }

    /// Overlay inner highlight (stronger for premium glass)
    pub fn overlay_highlight_strong(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_highlight,
            Self::Light => Color32::from_rgba_unmultiplied(255, 255, 252, 150),
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 18),
        }
    }

    // =========================================================================
    // Popup Colors (for completion menus, tooltips)
    // =========================================================================

    /// Popup background (darker for distinction from main UI)
    pub fn popup_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_elevated,
            Self::Light => Color32::from_rgb(245, 242, 237),
            Self::Midnight => Color32::from_rgb(14, 16, 24),
            Self::Ayu => Color32::from_rgb(10, 14, 20),
            Self::Aurora => Color32::from_rgb(14, 18, 24),
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Popup graphite
            Self::Ink => Color32::from_rgb(12, 12, 18),      // Popup ink
            Self::Dark => Color32::from_rgb(16, 16, 20),
        }
    }

    /// Popup border color (subtle accent tint)
    pub fn popup_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Light => Color32::from_rgb(200, 195, 185),
            Self::Midnight => Color32::from_rgb(50, 60, 85),
            Self::Ayu => Color32::from_rgb(55, 50, 40),
            Self::Aurora => Color32::from_rgb(45, 70, 62),
            Self::Graphite => Color32::from_rgb(80, 55, 35), // Orange tint border
            Self::Ink => Color32::from_rgb(55, 55, 70),      // Silver tint border
            Self::Dark => Color32::from_rgb(50, 55, 52),
        }
    }

    // =========================================================================
    // Backdrop Colors (for modal overlays)
    // =========================================================================

    /// Backdrop color (dimming overlay)
    pub fn backdrop_color(&self) -> Color32 {
        match self {
            Self::Custom(colors) => Color32::from_rgba_unmultiplied(
                colors.bg_base.r(),
                colors.bg_base.g(),
                colors.bg_base.b(),
                if colors.is_dark { 200 } else { 60 },
            ),
            Self::Light => Color32::from_rgba_unmultiplied(50, 48, 45, 60),
            Self::Midnight => Color32::from_rgba_unmultiplied(5, 8, 15, 200),
            Self::Ayu => Color32::from_rgba_unmultiplied(5, 8, 12, 200),
            Self::Aurora => Color32::from_rgba_unmultiplied(8, 12, 16, 200),
            Self::Graphite => Color32::from_rgba_unmultiplied(10, 10, 12, 200),
            Self::Ink => Color32::from_rgba_unmultiplied(5, 5, 10, 200),
            Self::Dark => Color32::from_rgba_unmultiplied(4, 4, 6, 200),
        }
    }

    /// Backdrop color (stronger for premium modals)
    pub fn backdrop_color_strong(&self) -> Color32 {
        match self {
            Self::Custom(colors) => Color32::from_rgba_unmultiplied(
                colors.bg_base.r(),
                colors.bg_base.g(),
                colors.bg_base.b(),
                if colors.is_dark { 210 } else { 80 },
            ),
            Self::Light => Color32::from_rgba_unmultiplied(50, 48, 45, 80),
            Self::Midnight => Color32::from_rgba_unmultiplied(5, 8, 15, 210),
            Self::Ayu => Color32::from_rgba_unmultiplied(5, 8, 12, 210),
            Self::Aurora => Color32::from_rgba_unmultiplied(8, 12, 16, 210),
            Self::Graphite => Color32::from_rgba_unmultiplied(10, 10, 12, 210),
            Self::Ink => Color32::from_rgba_unmultiplied(5, 5, 10, 210),
            Self::Dark => Color32::from_rgba_unmultiplied(4, 4, 6, 210),
        }
    }

    /// Backdrop vignette color (edge darkening). Returns None for light themes.
    pub fn backdrop_vignette(&self) -> Option<Color32> {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    Some(Color32::from_rgba_unmultiplied(0, 0, 0, 40))
                } else {
                    None
                }
            }
            Self::Light => None,
            _ => Some(Color32::from_rgba_unmultiplied(0, 0, 0, 40)),
        }
    }

    /// Backdrop accent glow color. Returns None for light themes.
    pub fn backdrop_accent_glow(&self) -> Option<Color32> {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    Some(Color32::from_rgba_unmultiplied(
                        colors.accent_primary.r(),
                        colors.accent_primary.g(),
                        colors.accent_primary.b(),
                        8,
                    ))
                } else {
                    None
                }
            }
            Self::Light => None,
            Self::Dark => Some(Color32::from_rgba_unmultiplied(16, 185, 129, 8)),
            Self::Midnight => Some(Color32::from_rgba_unmultiplied(59, 130, 246, 8)),
            Self::Ayu => Some(Color32::from_rgba_unmultiplied(255, 180, 84, 8)),
            Self::Aurora => Some(Color32::from_rgba_unmultiplied(126, 232, 184, 8)),
            Self::Graphite => Some(Color32::from_rgba_unmultiplied(232, 93, 4, 8)),
            Self::Ink => Some(Color32::from_rgba_unmultiplied(192, 192, 200, 8)),
        }
    }

    // =========================================================================
    // Highlight Colors
    // =========================================================================

    /// Match highlight color (for search results)
    pub fn highlight_match(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Light => Color32::from_rgb(255, 245, 180),
            Self::Midnight => Color32::from_rgb(30, 50, 80),
            Self::Ayu => Color32::from_rgb(50, 40, 25),
            Self::Aurora => Color32::from_rgb(30, 55, 50),
            Self::Graphite => Color32::from_rgb(60, 40, 28), // Orange tint highlight
            Self::Ink => Color32::from_rgb(35, 35, 50),      // Silver tint highlight
            Self::Dark => Color32::from_rgb(16, 60, 48),
        }
    }

    /// Match highlight text color (for fuzzy search result highlighting)
    /// This is a bright, visible color for text foreground use (not background)
    pub fn highlight_match_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.highlight_match,
            Self::Light => Color32::from_rgb(180, 100, 0),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Electric blue
            Self::Ayu => Color32::from_rgb(255, 200, 100),     // Gold
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(220, 220, 230),     // Bright silver
            Self::Dark => Color32::from_rgb(255, 200, 80),
        }
    }

    /// Line highlight color (for target lines in source preview)
    pub fn highlight_line(&self) -> Color32 {
        match self {
            Self::Custom(colors) => Color32::from_rgba_unmultiplied(
                colors.accent_primary.r(),
                colors.accent_primary.g(),
                colors.accent_primary.b(),
                if colors.is_dark { 30 } else { 60 },
            ),
            Self::Light => Color32::from_rgba_unmultiplied(255, 220, 120, 80),
            Self::Midnight => Color32::from_rgba_unmultiplied(59, 130, 246, 30),
            Self::Ayu => Color32::from_rgba_unmultiplied(255, 180, 84, 30),
            Self::Aurora => Color32::from_rgba_unmultiplied(126, 232, 184, 30),
            Self::Graphite => Color32::from_rgba_unmultiplied(232, 93, 4, 30),
            Self::Ink => Color32::from_rgba_unmultiplied(192, 192, 200, 30),
            Self::Dark => Color32::from_rgba_unmultiplied(255, 220, 0, 30),
        }
    }

    // =========================================================================
    // Badge Colors (status line badges)
    // =========================================================================

    /// Zen mode badge background
    pub fn badge_zen_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            Self::Light => Color32::from_rgb(100, 90, 80),
            Self::Midnight => Color32::from_rgb(167, 139, 250), // Violet
            Self::Ayu => Color32::from_rgb(210, 180, 140),      // Tan
            Self::Aurora => Color32::from_rgb(165, 210, 195),   // Aurora mint
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Dark => Color32::from_rgb(180, 150, 220),
        }
    }

    /// Zen mode badge foreground
    pub fn badge_zen_fg(&self) -> Color32 {
        Color32::from_rgb(40, 44, 52) // Dark text for all themes
    }

    /// Fullscreen badge background
    pub fn badge_fullscreen_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Light => Color32::from_rgb(60, 60, 60),
            Self::Midnight => Color32::from_rgb(56, 189, 248), // Sky blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(210, 210, 220),     // Bright silver
            Self::Dark => Color32::from_rgb(120, 200, 220),
        }
    }

    /// Fullscreen badge foreground
    pub fn badge_fullscreen_fg(&self) -> Color32 {
        Color32::from_rgb(40, 44, 52) // Dark text for all themes
    }

    // =========================================================================
    // Buffer/Editor Mode Colors
    // =========================================================================

    /// Normal mode color (viewing/navigating)
    pub fn mode_normal(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Light => Color32::from_rgb(100, 150, 220),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Sky blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Aurora => Color32::from_rgb(139, 198, 198),  // Aurora cyan
            Self::Graphite => Color32::from_rgb(232, 93, 4),   // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),     // Pure silver
            Self::Dark => Color32::from_rgb(130, 180, 255),
        }
    }

    /// Insert mode color (editing)
    pub fn mode_insert(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Light => Color32::from_rgb(100, 180, 100),
            Self::Midnight => Color32::from_rgb(52, 211, 153), // Green
            Self::Ayu => Color32::from_rgb(170, 210, 120),     // Green
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),     // Muted green
            Self::Dark => Color32::from_rgb(150, 220, 120),
        }
    }

    /// Buffer border color (inactive)
    pub fn buffer_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Light => Color32::from_rgb(200, 195, 185),
            Self::Midnight => Color32::from_rgb(55, 60, 78),
            Self::Ayu => Color32::from_rgb(48, 56, 68),
            Self::Aurora => Color32::from_rgb(48, 54, 62),
            Self::Graphite => Color32::from_rgb(58, 58, 64), // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),      // Default border #2E2E38
            Self::Dark => Color32::from_rgb(60, 60, 70),
        }
    }

    /// Buffer background color
    pub fn buffer_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Light => Color32::from_rgb(250, 248, 245),
            Self::Midnight => Color32::from_rgb(16, 18, 26),
            Self::Ayu => Color32::from_rgb(12, 16, 22),
            Self::Aurora => Color32::from_rgb(18, 22, 28),
            Self::Graphite => Color32::from_rgb(22, 22, 24), // Buffer graphite
            Self::Ink => Color32::from_rgb(14, 14, 20),      // Buffer ink
            Self::Dark => Color32::from_rgb(25, 25, 30),
        }
    }

    /// Buffer content background (inner area)
    pub fn buffer_content_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_base,
            Self::Light => Color32::from_rgb(255, 253, 250),
            Self::Midnight => Color32::from_rgb(12, 14, 20),
            Self::Ayu => Color32::from_rgb(10, 14, 20),
            Self::Aurora => Color32::from_rgb(13, 17, 23),
            Self::Graphite => Color32::from_rgb(18, 18, 20), // Content bg #121214
            Self::Ink => Color32::from_rgb(10, 10, 15),      // Content bg #0A0A0F
            Self::Dark => Color32::from_rgb(20, 20, 25),
        }
    }

    // =========================================================================
    // Semantic Colors
    // =========================================================================

    /// Success color
    pub fn semantic_success(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Light => Color32::from_rgb(45, 100, 45),
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Aurora => Color32::from_rgb(126, 232, 184), // Aurora teal
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),    // Muted green
            Self::Dark => Color32::from_rgb(34, 197, 94),
        }
    }

    /// Warning color
    pub fn semantic_warning(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.warning,
            Self::Light => Color32::from_rgb(180, 120, 30),
            Self::Midnight => Color32::from_rgb(251, 191, 36), // Amber
            Self::Ayu => Color32::from_rgb(255, 200, 100),
            Self::Aurora => Color32::from_rgb(255, 200, 120), // Warm gold
            Self::Graphite => Color32::from_rgb(255, 180, 80), // Warm orange
            Self::Ink => Color32::from_rgb(220, 200, 140),    // Muted gold
            Self::Dark => Color32::from_rgb(251, 176, 45),
        }
    }

    /// Error color
    pub fn semantic_error(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Light => Color32::from_rgb(180, 40, 40),
            Self::Midnight => Color32::from_rgb(248, 113, 113), // Red
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113), // Soft red
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Soft red
            Self::Ink => Color32::from_rgb(200, 110, 120),    // Muted rose
            Self::Dark => Color32::from_rgb(239, 82, 82),
        }
    }

    /// Info color
    pub fn semantic_info(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Light => Color32::from_rgb(50, 80, 140),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Aurora => Color32::from_rgb(139, 198, 198), // Aurora cyan
            Self::Graphite => Color32::from_rgb(232, 93, 4),  // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),    // Pure silver
            Self::Dark => Color32::from_rgb(82, 146, 255),
        }
    }

    // =========================================================================
    // Syntax Highlighting Colors
    // =========================================================================

    /// Keyword color
    pub fn syntax_keyword(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            Self::Light => Color32::from_rgb(30, 30, 30),
            Self::Midnight => Color32::from_rgb(199, 146, 234), // Purple
            Self::Ayu => Color32::from_rgb(255, 143, 64),       // Orange
            Self::Aurora => Color32::from_rgb(200, 160, 220),   // Aurora violet
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Dark => Color32::from_rgb(198, 146, 255),
        }
    }

    /// Key/property color
    pub fn syntax_key(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Light => Color32::from_rgb(50, 50, 50),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Aurora => Color32::from_rgb(139, 198, 198),  // Aurora cyan
            Self::Graphite => Color32::from_rgb(255, 160, 100), // Bright orange
            Self::Ink => Color32::from_rgb(160, 160, 180),     // Muted silver
            Self::Dark => Color32::from_rgb(110, 190, 248),
        }
    }

    /// Value/string color
    pub fn syntax_value(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Light => Color32::from_rgb(70, 70, 70),
            Self::Midnight => Color32::from_rgb(52, 211, 153), // Green
            Self::Ayu => Color32::from_rgb(170, 210, 120),     // Green
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),     // Muted green
            Self::Dark => Color32::from_rgb(52, 211, 153),
        }
    }

    /// Operator/punctuation color
    pub fn syntax_punctuation(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_secondary,
            Self::Light => Color32::from_rgb(100, 95, 90),
            Self::Midnight => Color32::from_rgb(148, 163, 184), // Slate
            Self::Ayu => Color32::from_rgb(140, 148, 156),      // Gray
            Self::Aurora => Color32::from_rgb(139, 148, 158),   // Muted silver
            Self::Graphite => Color32::from_rgb(168, 166, 160), // Secondary text #A8A6A0
            Self::Ink => Color32::from_rgb(152, 152, 168),      // Secondary text #9898A8
            Self::Dark => Color32::from_rgb(140, 140, 155),
        }
    }

    /// Comment color
    pub fn syntax_comment(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Light => Color32::from_rgb(140, 135, 125),
            Self::Midnight => Color32::from_rgb(100, 116, 139), // Slate gray
            Self::Ayu => Color32::from_rgb(90, 100, 110),       // Gray
            Self::Aurora => Color32::from_rgb(110, 118, 129),   // Deep night
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Tertiary text #606070
            Self::Dark => Color32::from_rgb(128, 128, 128),
        }
    }

    /// Function color
    pub fn syntax_function(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_hover,
            Self::Light => Color32::from_rgb(40, 40, 40),
            Self::Midnight => Color32::from_rgb(56, 189, 248), // Cyan
            Self::Ayu => Color32::from_rgb(255, 180, 84),      // Orange
            Self::Aurora => Color32::from_rgb(165, 243, 206),  // Bright aurora
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(216, 216, 224),     // Bright silver
            Self::Dark => Color32::from_rgb(100, 160, 255),
        }
    }

    /// Type/class color
    pub fn syntax_type(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.warning,
            Self::Light => Color32::from_rgb(60, 60, 60),
            Self::Midnight => Color32::from_rgb(251, 191, 36), // Amber
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Aurora => Color32::from_rgb(200, 220, 180),  // Aurora yellow-green
            Self::Graphite => Color32::from_rgb(200, 170, 120), // Warm tan
            Self::Ink => Color32::from_rgb(180, 180, 190),     // Light silver
            Self::Dark => Color32::from_rgb(220, 160, 100),
        }
    }

    /// Number/constant color
    pub fn syntax_number(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Light => Color32::from_rgb(55, 55, 55),
            Self::Midnight => Color32::from_rgb(248, 113, 113), // Red
            Self::Ayu => Color32::from_rgb(230, 140, 90),       // Coral
            Self::Aurora => Color32::from_rgb(255, 180, 150),   // Aurora peach
            Self::Graphite => Color32::from_rgb(255, 140, 80),  // Coral orange
            Self::Ink => Color32::from_rgb(200, 160, 180),      // Dusty rose
            Self::Dark => Color32::from_rgb(220, 120, 120),
        }
    }

    /// Variable color
    pub fn syntax_variable(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_primary,
            Self::Light => Color32::from_rgb(45, 45, 45),
            Self::Midnight => Color32::from_rgb(228, 228, 231), // Off-white
            Self::Ayu => Color32::from_rgb(191, 189, 182),      // Fg
            Self::Aurora => Color32::from_rgb(230, 237, 243),   // Crisp white
            Self::Graphite => Color32::from_rgb(232, 230, 224), // Text primary #E8E6E0
            Self::Ink => Color32::from_rgb(228, 228, 236),      // Text primary #E4E4EC
            Self::Dark => Color32::from_rgb(220, 220, 220),
        }
    }

    // =========================================================================
    // Scrollbar Colors
    // =========================================================================

    /// Scrollbar track color
    pub fn scrollbar_track(&self) -> Color32 {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 8)
                } else {
                    Color32::from_rgba_unmultiplied(80, 75, 70, 15)
                }
            }
            Self::Light => Color32::from_rgba_unmultiplied(80, 75, 70, 15),
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        }
    }

    /// Scrollbar thumb color
    pub fn scrollbar_thumb(&self) -> Color32 {
        match self {
            Self::Custom(colors) => Color32::from_rgba_unmultiplied(
                colors.text_secondary.r(),
                colors.text_secondary.g(),
                colors.text_secondary.b(),
                120,
            ),
            Self::Light => Color32::from_rgba_unmultiplied(120, 115, 105, 160),
            Self::Midnight => Color32::from_rgba_unmultiplied(96, 165, 250, 80),
            Self::Ayu => Color32::from_rgba_unmultiplied(140, 148, 156, 120),
            Self::Aurora => Color32::from_rgba_unmultiplied(139, 148, 158, 120),
            Self::Graphite => Color32::from_rgba_unmultiplied(168, 166, 160, 120),
            Self::Ink => Color32::from_rgba_unmultiplied(152, 152, 168, 120),
            Self::Dark => Color32::from_rgba_unmultiplied(140, 140, 160, 120),
        }
    }

    /// Scrollbar thumb highlight color
    pub fn scrollbar_thumb_highlight(&self) -> Color32 {
        match self {
            Self::Custom(colors) => Color32::from_rgba_unmultiplied(
                colors.accent_primary.r(),
                colors.accent_primary.g(),
                colors.accent_primary.b(),
                160,
            ),
            Self::Light => Color32::from_rgba_unmultiplied(80, 75, 70, 200),
            Self::Midnight => Color32::from_rgba_unmultiplied(96, 165, 250, 140),
            Self::Ayu => Color32::from_rgba_unmultiplied(255, 180, 84, 140),
            Self::Aurora => Color32::from_rgba_unmultiplied(126, 232, 184, 140),
            Self::Graphite => Color32::from_rgba_unmultiplied(232, 93, 4, 140),
            Self::Ink => Color32::from_rgba_unmultiplied(192, 192, 200, 140),
            Self::Dark => Color32::from_rgba_unmultiplied(180, 180, 200, 160),
        }
    }

    /// Scrollbar cap highlight color
    pub fn scrollbar_cap(&self) -> Color32 {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 25)
                } else {
                    Color32::from_rgba_unmultiplied(255, 252, 245, 80)
                }
            }
            Self::Light => Color32::from_rgba_unmultiplied(255, 252, 245, 80),
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 25),
        }
    }

    // =========================================================================
    // Agent/Panel Colors
    // =========================================================================

    /// Agent panel background
    pub fn agent_panel_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_bg,
            Self::Light => Color32::from_rgba_unmultiplied(248, 245, 240, 252),
            Self::Midnight => Color32::from_rgba_unmultiplied(14, 16, 24, 250),
            Self::Ayu => Color32::from_rgba_unmultiplied(12, 16, 22, 250),
            Self::Aurora => Color32::from_rgba_unmultiplied(13, 17, 23, 250),
            Self::Graphite => Color32::from_rgba_unmultiplied(18, 18, 20, 250),
            Self::Ink => Color32::from_rgba_unmultiplied(10, 10, 15, 250),
            Self::Dark => Color32::from_rgba_unmultiplied(15, 15, 15, 250),
        }
    }

    /// Agent panel border
    pub fn agent_panel_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Light => Color32::from_rgb(200, 195, 185),
            Self::Midnight => Color32::from_rgb(55, 60, 78),
            Self::Ayu => Color32::from_rgb(48, 56, 68),
            Self::Aurora => Color32::from_rgb(48, 54, 62),
            Self::Graphite => Color32::from_rgb(58, 58, 64), // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),      // Default border #2E2E38
            Self::Dark => Color32::from_rgb(38, 38, 44),
        }
    }

    /// User message background in chat
    pub fn chat_user_msg_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_elevated,
            Self::Light => Color32::from_rgb(240, 236, 228),
            Self::Midnight => Color32::from_rgb(26, 29, 40),
            Self::Ayu => Color32::from_rgb(21, 26, 34),
            Self::Aurora => Color32::from_rgb(33, 38, 45),
            Self::Graphite => Color32::from_rgb(30, 30, 32), // Elevated graphite
            Self::Ink => Color32::from_rgb(22, 22, 28),      // Elevated ink
            Self::Dark => Color32::from_rgb(26, 26, 30),
        }
    }

    // =========================================================================
    // Diagnostic Background Colors
    // =========================================================================

    /// Error diagnostic background
    pub fn diagnostic_error_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    colors.error.gamma_multiply(0.15)
                } else {
                    Color32::from_rgb(255, 240, 235)
                }
            }
            Self::Light => Color32::from_rgb(255, 240, 235), // Warm rose-tinted paper
            _ => self.semantic_error().gamma_multiply(0.15),
        }
    }

    /// Warning diagnostic background
    pub fn diagnostic_warning_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    colors.warning.gamma_multiply(0.15)
                } else {
                    Color32::from_rgb(255, 248, 230)
                }
            }
            Self::Light => Color32::from_rgb(255, 248, 230), // Warm amber-tinted paper
            _ => self.semantic_warning().gamma_multiply(0.15),
        }
    }

    /// Info diagnostic background
    pub fn diagnostic_info_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    colors.info.gamma_multiply(0.15)
                } else {
                    Color32::from_rgb(240, 240, 248)
                }
            }
            Self::Light => Color32::from_rgb(240, 240, 248), // Subtle gray-blue paper
            _ => self.semantic_info().gamma_multiply(0.15),
        }
    }

    /// Hint diagnostic background
    pub fn diagnostic_hint_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => {
                if colors.is_dark {
                    colors.success.gamma_multiply(0.15)
                } else {
                    Color32::from_rgb(242, 250, 242)
                }
            }
            Self::Light => Color32::from_rgb(242, 250, 242), // Subtle sage paper
            _ => self.semantic_success().gamma_multiply(0.15),
        }
    }

    // =========================================================================
    // Visualization Colors
    // =========================================================================

    /// Heatmap gradient colors (8 colors from low to high intensity)
    pub fn heatmap_gradient(&self) -> [Color32; 8] {
        let accent = self.accent_primary();
        let accent_hover = self.accent_hover();
        let bg = self.bg_base();

        // Build gradient from background through muted accent to bright accent
        match self {
            Self::Custom(colors) => {
                // Simple gradient using available colors
                [
                    bg,
                    colors.bg_surface,
                    colors.bg_elevated,
                    colors.bg_hover,
                    colors.accent_muted,
                    colors.accent_muted,
                    accent,
                    accent_hover,
                ]
            }
            Self::Light => [
                Color32::from_rgb(250, 248, 245),
                Color32::from_rgb(235, 230, 220),
                Color32::from_rgb(210, 200, 185),
                Color32::from_rgb(170, 165, 155),
                Color32::from_rgb(130, 125, 115),
                Color32::from_rgb(90, 85, 80),
                accent,
                accent_hover,
            ],
            Self::Midnight => [
                bg,
                Color32::from_rgb(15, 25, 45),
                Color32::from_rgb(25, 45, 80),
                Color32::from_rgb(35, 70, 120),
                Color32::from_rgb(50, 100, 170),
                Color32::from_rgb(70, 130, 210),
                accent,
                accent_hover,
            ],
            Self::Ayu => [
                bg,
                Color32::from_rgb(25, 22, 18),
                Color32::from_rgb(50, 40, 25),
                Color32::from_rgb(90, 65, 35),
                Color32::from_rgb(140, 95, 50),
                Color32::from_rgb(200, 140, 70),
                accent,
                accent_hover,
            ],
            Self::Aurora => [
                bg,
                Color32::from_rgb(20, 30, 28),
                Color32::from_rgb(30, 55, 50),
                Color32::from_rgb(50, 95, 85),
                Color32::from_rgb(75, 140, 125),
                Color32::from_rgb(100, 190, 160),
                accent,
                accent_hover,
            ],
            Self::Graphite => [
                bg,
                Color32::from_rgb(30, 28, 25),
                Color32::from_rgb(55, 40, 30),
                Color32::from_rgb(95, 60, 35),
                Color32::from_rgb(145, 85, 30),
                Color32::from_rgb(195, 110, 25),
                accent,
                accent_hover,
            ],
            Self::Ink => [
                bg,
                Color32::from_rgb(18, 18, 25),
                Color32::from_rgb(32, 32, 45),
                Color32::from_rgb(65, 65, 85),
                Color32::from_rgb(110, 110, 130),
                Color32::from_rgb(155, 155, 170),
                accent,
                accent_hover,
            ],
            Self::Dark => [
                bg,
                Color32::from_rgb(20, 28, 25),
                Color32::from_rgb(18, 38, 32),
                Color32::from_rgb(20, 60, 50),
                Color32::from_rgb(32, 100, 85),
                Color32::from_rgb(16, 140, 100),
                accent,
                accent_hover,
            ],
        }
    }

    // =========================================================================
    // Chart Colors
    // =========================================================================

    /// Chart palette for multi-series data visualization
    /// These colors are designed to be distinct and harmonious
    pub fn chart_palette(&self) -> [Color32; 8] {
        match self {
            Self::Custom(colors) => colors.chart_palette,
            // === Dark Themes ===
            Self::Dark => [
                Color32::from_rgb(16, 185, 129),  // Emerald (accent)
                Color32::from_rgb(110, 190, 248), // Sky blue
                Color32::from_rgb(198, 146, 255), // Violet
                Color32::from_rgb(255, 200, 60),  // Gold
                Color32::from_rgb(255, 130, 190), // Rose
                Color32::from_rgb(100, 240, 218), // Teal
                Color32::from_rgb(255, 120, 120), // Coral
                Color32::from_rgb(140, 150, 255), // Indigo
            ],
            Self::Midnight => [
                Color32::from_rgb(96, 165, 250),  // Electric blue (accent)
                Color32::from_rgb(192, 132, 252), // Neon purple
                Color32::from_rgb(52, 211, 153),  // Cyber teal
                Color32::from_rgb(251, 191, 36),  // Neon amber
                Color32::from_rgb(244, 114, 182), // Hot pink
                Color32::from_rgb(34, 211, 238),  // Cyan
                Color32::from_rgb(248, 113, 113), // Neon red
                Color32::from_rgb(167, 139, 250), // Lavender
            ],
            Self::Ayu => [
                Color32::from_rgb(255, 180, 84),  // Orange (accent)
                Color32::from_rgb(89, 186, 163),  // Cyan
                Color32::from_rgb(172, 128, 255), // Purple
                Color32::from_rgb(255, 238, 153), // Yellow
                Color32::from_rgb(247, 118, 142), // Magenta
                Color32::from_rgb(127, 193, 202), // Blue
                Color32::from_rgb(212, 184, 255), // Lavender
                Color32::from_rgb(149, 230, 203), // Mint
            ],
            Self::Aurora => [
                Color32::from_rgb(139, 198, 198), // Teal (accent)
                Color32::from_rgb(130, 200, 160), // Aurora green
                Color32::from_rgb(180, 180, 220), // Lavender
                Color32::from_rgb(200, 190, 140), // Pale gold
                Color32::from_rgb(200, 160, 180), // Pink
                Color32::from_rgb(100, 180, 200), // Sky blue
                Color32::from_rgb(180, 140, 160), // Mauve
                Color32::from_rgb(160, 200, 180), // Mint
            ],
            Self::Graphite => [
                Color32::from_rgb(255, 149, 0),   // Industrial orange (accent)
                Color32::from_rgb(160, 170, 180), // Steel
                Color32::from_rgb(200, 160, 100), // Brass
                Color32::from_rgb(140, 160, 180), // Gunmetal
                Color32::from_rgb(180, 140, 120), // Copper
                Color32::from_rgb(120, 150, 170), // Slate
                Color32::from_rgb(190, 130, 100), // Rust
                Color32::from_rgb(150, 160, 150), // Patina
            ],
            Self::Ink => [
                Color32::from_rgb(180, 180, 190), // Silver (accent)
                Color32::from_rgb(140, 145, 155), // Charcoal
                Color32::from_rgb(160, 165, 175), // Graphite
                Color32::from_rgb(150, 155, 165), // Slate
                Color32::from_rgb(170, 170, 180), // Pewter
                Color32::from_rgb(130, 140, 150), // Iron
                Color32::from_rgb(175, 175, 185), // Chrome
                Color32::from_rgb(145, 150, 160), // Lead
            ],

            // === Light Themes ===
            Self::Light => [
                Color32::from_rgb(16, 163, 127), // Muted emerald
                Color32::from_rgb(59, 130, 246), // Classic blue
                Color32::from_rgb(139, 92, 246), // Purple
                Color32::from_rgb(245, 158, 11), // Amber
                Color32::from_rgb(236, 72, 153), // Pink
                Color32::from_rgb(20, 184, 166), // Teal
                Color32::from_rgb(239, 68, 68),  // Red
                Color32::from_rgb(99, 102, 241), // Indigo
            ],
        }
    }

    /// Get a chart color by index (wraps around)
    pub fn chart_color(&self, index: usize) -> Color32 {
        let palette = self.chart_palette();
        palette[index % palette.len()]
    }

    /// Execution plan palette for query plan visualization.
    ///
    /// Returns 12 distinct colors optimized for execution plan operators:
    /// 0: Scan/Read (I/O), 1: Filter/Limit, 2: Join, 3: Aggregate/Group,
    /// 4: Sort/Order, 5: Project, 6: Hash, 7: Remote/Exchange,
    /// 8: Union/Interleave, 9: Cooperative/Yield, 10: Other Exec, 11: Reserved
    ///
    /// These colors are designed to be maximally distinct within each theme.
    pub fn plan_palette(&self) -> [Color32; 12] {
        match self {
            Self::Custom(colors) => {
                // Derive 12 colors from chart_palette (8) + semantic colors
                let c = colors.chart_palette;
                [
                    colors.info,         // 0: Scan
                    colors.success,      // 1: Filter
                    colors.warning,      // 2: Join
                    c[2],                // 3: Aggregate
                    colors.error,        // 4: Sort
                    c[4],                // 5: Project
                    c[3],                // 6: Hash
                    c[5],                // 7: Remote
                    c[6],                // 8: Union
                    c[7],                // 9: Cooperative
                    colors.accent_hover, // 10: Other Exec
                    colors.text_muted,   // 11: Reserved
                ]
            }
            // === Dark Themes ===
            Self::Dark => [
                Color32::from_rgb(96, 165, 250),  // 0: Scan - Sky blue
                Color32::from_rgb(52, 211, 153),  // 1: Filter - Emerald
                Color32::from_rgb(251, 146, 60),  // 2: Join - Orange
                Color32::from_rgb(192, 132, 252), // 3: Aggregate - Violet
                Color32::from_rgb(248, 113, 113), // 4: Sort - Red
                Color32::from_rgb(45, 212, 191),  // 5: Project - Teal
                Color32::from_rgb(250, 204, 21),  // 6: Hash - Yellow
                Color32::from_rgb(34, 211, 238),  // 7: Remote - Cyan
                Color32::from_rgb(244, 114, 182), // 8: Union - Pink
                Color32::from_rgb(163, 230, 53),  // 9: Cooperative - Lime
                Color32::from_rgb(251, 191, 36),  // 10: Other Exec - Amber
                Color32::from_rgb(156, 163, 175), // 11: Reserved - Gray
            ],
            Self::Midnight => [
                Color32::from_rgb(96, 165, 250),  // 0: Scan - Electric blue
                Color32::from_rgb(52, 211, 153),  // 1: Filter - Cyber teal
                Color32::from_rgb(251, 146, 60),  // 2: Join - Neon orange
                Color32::from_rgb(192, 132, 252), // 3: Aggregate - Neon purple
                Color32::from_rgb(248, 113, 113), // 4: Sort - Neon red
                Color32::from_rgb(34, 197, 194),  // 5: Project - Deep teal
                Color32::from_rgb(251, 191, 36),  // 6: Hash - Neon amber
                Color32::from_rgb(34, 211, 238),  // 7: Remote - Bright cyan
                Color32::from_rgb(244, 114, 182), // 8: Union - Hot pink
                Color32::from_rgb(190, 242, 100), // 9: Cooperative - Neon lime
                Color32::from_rgb(253, 224, 71),  // 10: Other Exec - Bright yellow
                Color32::from_rgb(148, 163, 184), // 11: Reserved - Slate
            ],
            Self::Ayu => [
                Color32::from_rgb(127, 193, 202), // 0: Scan - Blue
                Color32::from_rgb(149, 230, 203), // 1: Filter - Mint
                Color32::from_rgb(255, 180, 84),  // 2: Join - Orange
                Color32::from_rgb(172, 128, 255), // 3: Aggregate - Purple
                Color32::from_rgb(247, 118, 142), // 4: Sort - Magenta
                Color32::from_rgb(89, 186, 163),  // 5: Project - Cyan
                Color32::from_rgb(255, 238, 153), // 6: Hash - Yellow
                Color32::from_rgb(95, 215, 255),  // 7: Remote - Bright cyan
                Color32::from_rgb(255, 150, 200), // 8: Union - Pink
                Color32::from_rgb(200, 240, 130), // 9: Cooperative - Lime
                Color32::from_rgb(255, 200, 120), // 10: Other Exec - Light orange
                Color32::from_rgb(140, 150, 165), // 11: Reserved - Gray
            ],
            Self::Aurora => [
                Color32::from_rgb(100, 180, 200), // 0: Scan - Sky blue
                Color32::from_rgb(130, 200, 160), // 1: Filter - Aurora green
                Color32::from_rgb(220, 170, 130), // 2: Join - Warm amber
                Color32::from_rgb(175, 160, 210), // 3: Aggregate - Soft purple
                Color32::from_rgb(210, 140, 150), // 4: Sort - Dusty rose
                Color32::from_rgb(139, 198, 198), // 5: Project - Teal
                Color32::from_rgb(210, 200, 140), // 6: Hash - Pale gold
                Color32::from_rgb(120, 190, 210), // 7: Remote - Light cyan
                Color32::from_rgb(200, 160, 180), // 8: Union - Pink
                Color32::from_rgb(170, 210, 150), // 9: Cooperative - Light green
                Color32::from_rgb(230, 185, 130), // 10: Other Exec - Peach
                Color32::from_rgb(160, 165, 175), // 11: Reserved - Cool gray
            ],
            Self::Graphite => [
                Color32::from_rgb(140, 160, 180), // 0: Scan - Gunmetal
                Color32::from_rgb(145, 175, 145), // 1: Filter - Patina green
                Color32::from_rgb(255, 149, 0),   // 2: Join - Industrial orange
                Color32::from_rgb(160, 140, 170), // 3: Aggregate - Steel purple
                Color32::from_rgb(200, 130, 120), // 4: Sort - Rust
                Color32::from_rgb(120, 155, 165), // 5: Project - Slate teal
                Color32::from_rgb(200, 175, 110), // 6: Hash - Brass
                Color32::from_rgb(130, 165, 190), // 7: Remote - Steel blue
                Color32::from_rgb(180, 145, 160), // 8: Union - Dusty pink
                Color32::from_rgb(165, 185, 130), // 9: Cooperative - Olive
                Color32::from_rgb(220, 160, 100), // 10: Other Exec - Copper
                Color32::from_rgb(150, 155, 160), // 11: Reserved - Graphite
            ],
            Self::Ink => [
                Color32::from_rgb(130, 145, 170), // 0: Scan - Slate
                Color32::from_rgb(140, 160, 145), // 1: Filter - Sage gray
                Color32::from_rgb(175, 150, 130), // 2: Join - Warm gray
                Color32::from_rgb(155, 145, 165), // 3: Aggregate - Lavender gray
                Color32::from_rgb(170, 135, 140), // 4: Sort - Dusty rose
                Color32::from_rgb(135, 155, 160), // 5: Project - Cool teal
                Color32::from_rgb(175, 170, 140), // 6: Hash - Khaki
                Color32::from_rgb(140, 160, 175), // 7: Remote - Steel
                Color32::from_rgb(165, 145, 160), // 8: Union - Mauve gray
                Color32::from_rgb(155, 170, 145), // 9: Cooperative - Moss
                Color32::from_rgb(180, 160, 135), // 10: Other Exec - Sand
                Color32::from_rgb(145, 150, 155), // 11: Reserved - Charcoal
            ],

            // === Light Themes ===
            Self::Light => [
                Color32::from_rgb(37, 99, 235),   // 0: Scan - Blue
                Color32::from_rgb(22, 163, 74),   // 1: Filter - Green
                Color32::from_rgb(234, 88, 12),   // 2: Join - Orange
                Color32::from_rgb(147, 51, 234),  // 3: Aggregate - Purple
                Color32::from_rgb(220, 38, 38),   // 4: Sort - Red
                Color32::from_rgb(20, 184, 166),  // 5: Project - Teal
                Color32::from_rgb(202, 138, 4),   // 6: Hash - Yellow
                Color32::from_rgb(6, 182, 212),   // 7: Remote - Cyan
                Color32::from_rgb(219, 39, 119),  // 8: Union - Pink
                Color32::from_rgb(132, 204, 22),  // 9: Cooperative - Lime
                Color32::from_rgb(245, 158, 11),  // 10: Other Exec - Amber
                Color32::from_rgb(107, 114, 128), // 11: Reserved - Gray
            ],
        }
    }

    /// Get a plan operator color by index (wraps around)
    pub fn plan_color(&self, index: usize) -> Color32 {
        let palette = self.plan_palette();
        palette[index % palette.len()]
    }

    /// Terminal palette for ANSI color mapping in the embedded terminal.
    ///
    /// Returns 6 colors for the 6 chromatic ANSI colors: Red, Green, Yellow, Blue, Magenta, Cyan.
    /// These colors are semantically meaningful (red is reddish, green is greenish) while still
    /// fitting the theme's aesthetic. Black/White are derived from theme bg/text colors.
    ///
    /// This is separate from chart_palette because terminal colors need semantic meaning
    /// (errors are red, success is green) while chart colors just need to be distinct.
    pub fn terminal_palette(&self) -> [Color32; 6] {
        match self {
            Self::Custom(colors) => [
                colors.error,          // Red
                colors.success,        // Green
                colors.warning,        // Yellow
                colors.info,           // Blue
                colors.accent_primary, // Magenta (using accent as substitute)
                colors.accent_hover,   // Cyan (using accent hover as substitute)
            ],
            // === Dark Themes ===
            Self::Dark => [
                Color32::from_rgb(248, 113, 133), // Red - Soft coral
                Color32::from_rgb(52, 211, 153),  // Green - Emerald (accent-inspired)
                Color32::from_rgb(250, 204, 21),  // Yellow - Gold
                Color32::from_rgb(96, 165, 250),  // Blue - Sky blue
                Color32::from_rgb(192, 132, 252), // Magenta - Violet
                Color32::from_rgb(34, 211, 238),  // Cyan - Bright cyan
            ],
            Self::Midnight => [
                Color32::from_rgb(248, 113, 113), // Red - Neon red
                Color32::from_rgb(52, 211, 153),  // Green - Cyber teal-green
                Color32::from_rgb(251, 191, 36),  // Yellow - Neon amber
                Color32::from_rgb(96, 165, 250),  // Blue - Electric blue (accent)
                Color32::from_rgb(192, 132, 252), // Magenta - Neon purple
                Color32::from_rgb(34, 211, 238),  // Cyan - Bright cyan
            ],
            Self::Ayu => [
                Color32::from_rgb(255, 102, 102), // Red - Warm red
                Color32::from_rgb(127, 204, 127), // Green - Soft green
                Color32::from_rgb(255, 204, 102), // Yellow - Amber-yellow
                Color32::from_rgb(89, 186, 163),  // Blue - Ayu cyan-blue
                Color32::from_rgb(172, 128, 255), // Magenta - Purple
                Color32::from_rgb(127, 193, 202), // Cyan - Soft cyan
            ],
            Self::Aurora => [
                Color32::from_rgb(240, 120, 140), // Red - Aurora red glow
                Color32::from_rgb(126, 232, 184), // Green - Aurora teal (accent)
                Color32::from_rgb(250, 220, 130), // Yellow - Soft aurora gold
                Color32::from_rgb(130, 180, 230), // Blue - Night sky blue
                Color32::from_rgb(200, 160, 220), // Magenta - Aurora violet
                Color32::from_rgb(100, 210, 220), // Cyan - Aurora cyan
            ],
            Self::Graphite => [
                Color32::from_rgb(232, 93, 4), // Red - Molten orange-red (accent-inspired)
                Color32::from_rgb(140, 180, 120), // Green - Industrial sage
                Color32::from_rgb(230, 180, 80), // Yellow - Brass gold
                Color32::from_rgb(120, 150, 190), // Blue - Steel blue
                Color32::from_rgb(180, 140, 160), // Magenta - Tarnished rose
                Color32::from_rgb(120, 180, 190), // Cyan - Oxidized cyan
            ],
            Self::Ink => [
                Color32::from_rgb(180, 100, 110), // Red - Ink red (muted)
                Color32::from_rgb(130, 160, 140), // Green - Ink green (muted)
                Color32::from_rgb(180, 170, 130), // Yellow - Parchment gold
                Color32::from_rgb(130, 150, 180), // Blue - Steel blue
                Color32::from_rgb(160, 140, 170), // Magenta - Silver violet
                Color32::from_rgb(140, 170, 180), // Cyan - Silver cyan
            ],

            // === Light Themes ===
            Self::Light => [
                Color32::from_rgb(185, 28, 28),  // Red - Deep ink red
                Color32::from_rgb(21, 128, 61),  // Green - Forest green
                Color32::from_rgb(161, 98, 7),   // Yellow - Amber-brown
                Color32::from_rgb(29, 78, 216),  // Blue - Classic blue
                Color32::from_rgb(126, 34, 206), // Magenta - Purple
                Color32::from_rgb(14, 116, 144), // Cyan - Teal
            ],
        }
    }

    /// Commit marker color for git annotations on charts
    pub fn chart_commit_marker(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            // Dark themes - vibrant markers
            Self::Dark => Color32::from_rgb(180, 155, 255), // Violet
            Self::Midnight => Color32::from_rgb(192, 132, 252), // Neon purple
            Self::Ayu => Color32::from_rgb(172, 128, 255),  // Purple
            Self::Aurora => Color32::from_rgb(180, 150, 180), // Soft purple
            Self::Graphite => Color32::from_rgb(180, 140, 120), // Copper
            Self::Ink => Color32::from_rgb(160, 160, 170),  // Silver

            // Light themes - muted markers
            Self::Light => Color32::from_rgb(139, 92, 246), // Purple
        }
    }

    // =========================================================================
    // Annotation Colors
    // =========================================================================

    /// Normal priority annotation color (notes/comments)
    pub fn annotation_normal(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Light => Color32::from_rgb(59, 130, 246),
            Self::Midnight => Color32::from_rgb(96, 165, 250),
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Aurora => Color32::from_rgb(139, 198, 198),
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),   // Pure silver
            Self::Dark => Color32::from_rgb(100, 149, 237),
        }
    }

    /// Important priority annotation color (highlighted)
    pub fn annotation_important(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.warning,
            Self::Light => Color32::from_rgb(245, 158, 11),
            Self::Midnight => Color32::from_rgb(251, 191, 36),
            Self::Ayu => Color32::from_rgb(255, 180, 84),
            Self::Aurora => Color32::from_rgb(255, 200, 120),
            Self::Graphite => Color32::from_rgb(255, 180, 80), // Warm orange
            Self::Ink => Color32::from_rgb(220, 200, 140),     // Muted gold
            Self::Dark => Color32::from_rgb(255, 165, 0),
        }
    }

    /// Critical priority annotation color (alert-style)
    pub fn annotation_critical(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Light => Color32::from_rgb(220, 38, 38),
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Soft red
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Muted rose
            Self::Dark => Color32::from_rgb(220, 53, 69),
        }
    }

    /// Resolved annotation color (dimmed/inactive)
    pub fn annotation_resolved(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Light => Color32::from_rgb(156, 163, 175),
            Self::Midnight => Color32::from_rgb(113, 113, 122),
            Self::Ayu => Color32::from_rgb(90, 100, 110),
            Self::Aurora => Color32::from_rgb(110, 118, 129),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Tertiary text #606070
            Self::Dark => Color32::GRAY,
        }
    }

    // =========================================================================
    // Diff Colors (Git diff visualization)
    // =========================================================================

    /// Addition line background - subtle tint spanning full line
    pub fn diff_added_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success.gamma_multiply(0.15),
            Self::Light => Color32::from_rgb(230, 255, 237),
            Self::Midnight => Color32::from_rgb(18, 35, 30),
            Self::Ayu => Color32::from_rgb(22, 35, 25),
            Self::Aurora => Color32::from_rgb(20, 40, 35),
            Self::Graphite => Color32::from_rgb(22, 35, 25), // Added bg graphite
            Self::Ink => Color32::from_rgb(20, 30, 28),      // Added bg ink
            Self::Dark => Color32::from_rgb(19, 35, 26),
        }
    }

    /// Deletion line background - subtle tint spanning full line
    pub fn diff_removed_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error.gamma_multiply(0.15),
            Self::Light => Color32::from_rgb(255, 235, 235),
            Self::Midnight => Color32::from_rgb(40, 22, 28),
            Self::Ayu => Color32::from_rgb(40, 25, 25),
            Self::Aurora => Color32::from_rgb(40, 25, 28),
            Self::Graphite => Color32::from_rgb(40, 25, 25), // Removed bg graphite
            Self::Ink => Color32::from_rgb(35, 22, 28),      // Removed bg ink
            Self::Dark => Color32::from_rgb(40, 22, 24),
        }
    }

    /// Word-level addition highlight - brighter for inline changes
    pub fn diff_added_word_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success.gamma_multiply(0.35),
            Self::Light => Color32::from_rgb(172, 242, 189),
            Self::Midnight => Color32::from_rgb(30, 70, 55),
            Self::Ayu => Color32::from_rgb(40, 70, 45),
            Self::Aurora => Color32::from_rgb(35, 80, 65),
            Self::Graphite => Color32::from_rgb(45, 70, 45), // Added word graphite
            Self::Ink => Color32::from_rgb(35, 60, 50),      // Added word ink
            Self::Dark => Color32::from_rgb(35, 70, 50),
        }
    }

    /// Word-level deletion highlight - brighter for inline changes
    pub fn diff_removed_word_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error.gamma_multiply(0.35),
            Self::Light => Color32::from_rgb(255, 200, 200),
            Self::Midnight => Color32::from_rgb(80, 40, 45),
            Self::Ayu => Color32::from_rgb(85, 45, 45),
            Self::Aurora => Color32::from_rgb(90, 45, 50),
            Self::Graphite => Color32::from_rgb(85, 45, 45), // Removed word graphite
            Self::Ink => Color32::from_rgb(70, 40, 50),      // Removed word ink
            Self::Dark => Color32::from_rgb(75, 35, 38),
        }
    }

    /// Addition text color - high contrast for readability
    pub fn diff_added_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Light => Color32::from_rgb(36, 138, 61),
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Aurora => Color32::from_rgb(126, 232, 184),
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Added text graphite
            Self::Ink => Color32::from_rgb(130, 180, 150),      // Added text ink
            Self::Dark => Color32::from_rgb(126, 231, 135),
        }
    }

    /// Deletion text color - high contrast for readability
    pub fn diff_removed_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Light => Color32::from_rgb(207, 34, 46),
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Removed text graphite
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Removed text ink
            Self::Dark => Color32::from_rgb(255, 123, 114),
        }
    }

    /// Context line text color - dimmed for less visual weight
    pub fn diff_context_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Light => Color32::from_rgb(87, 96, 106),
            Self::Midnight => Color32::from_rgb(113, 113, 122),
            Self::Ayu => Color32::from_rgb(90, 100, 110),
            Self::Aurora => Color32::from_rgb(110, 118, 129),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Context text graphite
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Context text ink
            Self::Dark => Color32::from_rgb(145, 152, 161),
        }
    }

    /// Addition gutter stripe color
    pub fn diff_added_gutter(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Light => Color32::from_rgb(52, 168, 83),
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Aurora => Color32::from_rgb(126, 232, 184),
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Added gutter graphite
            Self::Ink => Color32::from_rgb(130, 180, 150),      // Added gutter ink
            Self::Dark => Color32::from_rgb(63, 185, 80),
        }
    }

    /// Deletion gutter stripe color
    pub fn diff_removed_gutter(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Light => Color32::from_rgb(234, 67, 53),
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Removed gutter graphite
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Removed gutter ink
            Self::Dark => Color32::from_rgb(248, 81, 73),
        }
    }

    /// Line number text color
    pub fn diff_line_number(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Light => Color32::from_rgb(140, 150, 160),
            Self::Midnight => Color32::from_rgb(70, 80, 100),
            Self::Ayu => Color32::from_rgb(60, 70, 80),
            Self::Aurora => Color32::from_rgb(70, 78, 88),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Line number graphite
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Line number ink
            Self::Dark => Color32::from_rgb(72, 79, 88),
        }
    }

    /// Line number background color
    pub fn diff_line_number_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Light => Color32::from_rgb(246, 248, 250),
            Self::Midnight => Color32::from_rgb(12, 14, 20),
            Self::Ayu => Color32::from_rgb(8, 11, 16),
            Self::Aurora => Color32::from_rgb(10, 14, 18),
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Line number bg graphite
            Self::Ink => Color32::from_rgb(8, 8, 12),        // Line number bg ink
            Self::Dark => Color32::from_rgb(13, 17, 23),
        }
    }

    /// Hunk header background
    pub fn diff_hunk_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Light => Color32::from_rgb(240, 245, 255),
            Self::Midnight => Color32::from_rgb(20, 30, 55),
            Self::Ayu => Color32::from_rgb(20, 25, 35),
            Self::Aurora => Color32::from_rgb(22, 32, 38),
            Self::Graphite => Color32::from_rgb(30, 25, 20), // Hunk bg graphite
            Self::Ink => Color32::from_rgb(20, 20, 30),      // Hunk bg ink
            Self::Dark => Color32::from_rgb(22, 27, 46),
        }
    }

    /// Hunk header text color
    pub fn diff_hunk_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Light => Color32::from_rgb(47, 93, 158),
            Self::Midnight => Color32::from_rgb(96, 165, 250),
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Aurora => Color32::from_rgb(139, 198, 198),
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Hunk text graphite
            Self::Ink => Color32::from_rgb(192, 192, 200),   // Hunk text ink
            Self::Dark => Color32::from_rgb(121, 184, 255),
        }
    }

    /// File header text color
    pub fn diff_file_header(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_primary,
            Self::Light => Color32::from_rgb(36, 41, 47),
            Self::Midnight => Color32::from_rgb(228, 228, 231),
            Self::Ayu => Color32::from_rgb(191, 189, 182),
            Self::Aurora => Color32::from_rgb(230, 237, 243),
            Self::Graphite => Color32::from_rgb(232, 230, 224), // File header graphite
            Self::Ink => Color32::from_rgb(228, 228, 236),      // File header ink
            Self::Dark => Color32::from_rgb(201, 209, 217),
        }
    }

    /// File header background color
    pub fn diff_file_header_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Light => Color32::from_rgb(246, 248, 250),
            Self::Midnight => Color32::from_rgb(16, 18, 26),
            Self::Ayu => Color32::from_rgb(12, 16, 22),
            Self::Aurora => Color32::from_rgb(18, 22, 28),
            Self::Graphite => Color32::from_rgb(22, 22, 24), // File header bg graphite
            Self::Ink => Color32::from_rgb(14, 14, 20),      // File header bg ink
            Self::Dark => Color32::from_rgb(22, 27, 34),
        }
    }

    /// Get the active theme colors for this theme.
    ///
    /// This is a convenience method that extracts `ActiveThemeColors` from
    /// either a custom theme (directly carried) or a builtin theme (computed).
    pub fn active_colors(&self) -> super::ActiveThemeColors {
        match self {
            Self::Custom(colors) => *colors,
            _ => super::ActiveThemeColors::from_builtin(*self),
        }
    }
}

pub fn light() -> Visuals {
    Visuals {
        dark_mode: false,
        widgets: Widgets::light(),
        selection: Selection {
            bg_fill: Color32::from_rgb(144, 209, 255),
            stroke: Stroke::new(1.0, Color32::from_rgb(0, 83, 125)),
        },

        hyperlink_color: Color32::from_rgb(0, 155, 255),

        faint_bg_color: Color32::from_gray(245),
        extreme_bg_color: Color32::from_gray(255),
        code_bg_color: Color32::from_gray(230),

        warn_fg_color: Color32::from_rgb(255, 143, 0),
        error_fg_color: Color32::from_rgb(255, 0, 0),

        window_shadow: egui::epaint::Shadow {
            offset: [10, 20],
            blur: 15,
            spread: 0,
            color: Color32::from_black_alpha(25),
        },
        window_fill: Color32::from_gray(248),
        window_stroke: Stroke::new(1.0, Color32::from_gray(190)),

        panel_fill: Color32::from_gray(248),

        popup_shadow: Shadow {
            offset: [6, 10],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(25),
        },

        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, Color32::from_rgb(0, 83, 125)),
            blink: true,
            on_duration: 0.5,
            off_duration: 0.5,
            ..Default::default()
        },
        ..Visuals::light()
    }
}
