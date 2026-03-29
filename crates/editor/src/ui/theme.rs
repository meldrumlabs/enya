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
    /// Light theme - White Obsidian Glass with Enya Emerald accent #10B981
    Light,
    /// Parchment theme - Paper/Ink aesthetic with warm cream backgrounds and rich black text
    Parchment,
    /// Stockholm theme - Cool Nordic light with steel blue accent #4A6FA5
    Stockholm,
    /// Copenhagen theme - Danish hygge with muted sage green accent #6B8F71
    Copenhagen,
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
    /// Void theme - OLED black with electric violet accent #7C3AED
    Void,
    /// Neon theme - Deep black with hot magenta accent #E040A0
    Neon,
    /// Onyx theme - True dark with gold accent #D4AF37
    Onyx,
    /// System theme - follows OS light/dark preference, resolves to Light or Dark at runtime
    System,
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
            Self::Parchment => "Parchment",
            Self::Stockholm => "Stockholm",
            Self::Copenhagen => "Copenhagen",
            Self::Midnight => "Midnight",
            Self::Ayu => "Ayu",
            Self::Aurora => "Aurora",
            Self::Graphite => "Graphite",
            Self::Ink => "Ink",
            Self::Void => "Void",
            Self::Neon => "Neon",
            Self::Onyx => "Onyx",
            Self::System => "System",
            Self::Custom(_) => "Custom",
        }
    }

    /// Returns all available themes
    pub fn all() -> &'static [AppTheme] {
        &[
            Self::System,
            Self::Dark,
            Self::Light,
            Self::Parchment,
            Self::Stockholm,
            Self::Copenhagen,
            Self::Midnight,
            Self::Ayu,
            Self::Aurora,
            Self::Graphite,
            Self::Ink,
            Self::Void,
            Self::Neon,
            Self::Onyx,
        ]
    }

    /// Returns true if this is a dark theme
    pub fn is_dark(&self) -> bool {
        match self {
            Self::Custom(colors) => colors.is_dark,
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => false,
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
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "parchment" => Some(Self::Parchment),
            "stockholm" => Some(Self::Stockholm),
            "copenhagen" => Some(Self::Copenhagen),
            "midnight" => Some(Self::Midnight),
            "ayu" => Some(Self::Ayu),
            "aurora" => Some(Self::Aurora),
            "graphite" => Some(Self::Graphite),
            "ink" => Some(Self::Ink),
            "void" => Some(Self::Void),
            "neon" => Some(Self::Neon),
            "onyx" => Some(Self::Onyx),
            "system" => Some(Self::System),
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
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => {
                super::design::light_theme(*self)
            }
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
            Self::Parchment => Color32::from_rgb(250, 248, 245), // Warm cream paper #FAF8F5
            Self::Stockholm => Color32::from_rgb(250, 251, 252), // Cool white #FAFBFC
            Self::Copenhagen => Color32::from_rgb(250, 250, 248), // Warm white #FAFAF8
            Self::Light => Color32::from_rgb(251, 251, 252),     // Cool neutral white #FBFBFC
            Self::Midnight => Color32::from_rgb(10, 11, 16),     // Deep space blue #0A0B10
            Self::Ayu => Color32::from_rgb(10, 14, 20),          // Deep charcoal #0A0E14
            Self::Aurora => Color32::from_rgb(13, 17, 23),       // Deep night sky #0D1117
            Self::Graphite => Color32::from_rgb(18, 18, 20),     // Deep warm charcoal #121214
            Self::Ink => Color32::from_rgb(10, 10, 15),          // Blue-black #0A0A0F
            Self::Void => Color32::from_rgb(0, 0, 0),            // True OLED black
            Self::Neon => Color32::from_rgb(5, 5, 8),            // Deep near-black
            Self::Onyx => Color32::from_rgb(12, 12, 12),         // True dark neutral
            Self::System | Self::Dark => Color32::from_rgb(8, 8, 10), // Obsidian dark
        }
    }

    /// Surface/panel background color
    pub fn bg_surface(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Parchment => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Stockholm => Color32::from_rgb(244, 245, 248), // Cool gray surface #F4F5F8
            Self::Copenhagen => Color32::from_rgb(245, 244, 240), // Warm gray surface #F5F4F0
            Self::Light => Color32::from_rgb(242, 243, 245),     // Neutral gray surface #F2F3F5
            Self::Midnight => Color32::from_rgb(18, 20, 28),     // Deep navy #12141C
            Self::Ayu => Color32::from_rgb(13, 16, 23),          // Dark blue-gray #0D1017
            Self::Aurora => Color32::from_rgb(22, 27, 34),       // Night surface #161B22
            Self::Graphite => Color32::from_rgb(26, 26, 28),     // Surface #1A1A1C
            Self::Ink => Color32::from_rgb(18, 18, 24),          // Surface #121218
            Self::Void => Color32::from_rgb(8, 8, 14),           // Near-black with violet tint
            Self::Neon => Color32::from_rgb(12, 10, 16),         // Dark with magenta tint
            Self::Onyx => Color32::from_rgb(18, 18, 16),         // Warm dark surface
            Self::System | Self::Dark => Color32::from_rgb(18, 18, 21),
        }
    }

    /// Elevated elements (cards, dropdowns)
    pub fn bg_elevated(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_elevated,
            Self::Parchment => Color32::from_rgb(240, 236, 230), // Aged paper #F0ECE6
            Self::Stockholm => Color32::from_rgb(237, 239, 243), // Elevated blue-gray #EDEFF3
            Self::Copenhagen => Color32::from_rgb(238, 236, 230), // Warm elevated #EEECE6
            Self::Light => Color32::from_rgb(234, 235, 238),     // Neutral elevated #EAEBEE
            Self::Midnight => Color32::from_rgb(26, 29, 40),     // Lighter navy #1A1D28
            Self::Ayu => Color32::from_rgb(21, 26, 34),          // Slightly lighter #151A22
            Self::Aurora => Color32::from_rgb(33, 38, 45),       // Elevated night #21262D
            Self::Graphite => Color32::from_rgb(36, 36, 38),     // Elevated #242426
            Self::Ink => Color32::from_rgb(28, 28, 36),          // Elevated #1C1C24
            Self::Void => Color32::from_rgb(16, 16, 26),         // Dark violet
            Self::Neon => Color32::from_rgb(22, 18, 28),         // Dark magenta
            Self::Onyx => Color32::from_rgb(28, 28, 24),         // Warm elevated
            Self::System | Self::Dark => Color32::from_rgb(26, 26, 30),
        }
    }

    /// Hover state background
    pub fn bg_hover(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_hover,
            Self::Parchment => Color32::from_rgb(232, 228, 220), // Darker paper #E8E4DC
            Self::Stockholm => Color32::from_rgb(228, 231, 237), // Hover cool gray #E4E7ED
            Self::Copenhagen => Color32::from_rgb(230, 227, 220), // Warm hover #E6E3DC
            Self::Light => Color32::from_rgb(226, 228, 232),     // Neutral hover #E2E4E8
            Self::Midnight => Color32::from_rgb(34, 38, 52),     // Hover navy #222634
            Self::Ayu => Color32::from_rgb(28, 34, 44),          // Hover charcoal #1C222C
            Self::Aurora => Color32::from_rgb(40, 46, 56),       // Hover night #282E38
            Self::Graphite => Color32::from_rgb(46, 46, 50),     // Hover #2E2E32
            Self::Ink => Color32::from_rgb(38, 38, 46),          // Hover #26262E
            Self::Void => Color32::from_rgb(24, 24, 38),         // Subtle violet lift
            Self::Neon => Color32::from_rgb(32, 26, 40),         // Subtle magenta lift
            Self::Onyx => Color32::from_rgb(38, 38, 32),         // Warm hover
            Self::System | Self::Dark => Color32::from_rgb(36, 36, 40),
        }
    }

    /// Selected item background
    pub fn bg_selected(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Parchment => Color32::from_rgb(225, 220, 210), // Selected paper #E1DCD2
            Self::Stockholm => Color32::from_rgb(218, 224, 232), // Blue tint selected #DAE0E8
            Self::Copenhagen => Color32::from_rgb(220, 228, 222), // Sage tint selected #DCE4DE
            Self::Light => Color32::from_rgb(220, 238, 230),     // Emerald tint selected #DCEEE6
            Self::Midnight => Color32::from_rgb(25, 40, 65),     // Blue selection #192841
            Self::Ayu => Color32::from_rgb(40, 35, 25),          // Amber tint selection
            Self::Aurora => Color32::from_rgb(25, 50, 45),       // Teal tint selection
            Self::Graphite => Color32::from_rgb(58, 42, 32),     // Orange tint selection #3A2A20
            Self::Ink => Color32::from_rgb(32, 32, 42),          // Silver tint selection #20202A
            Self::Void => Color32::from_rgb(30, 20, 55),         // Violet tint selected
            Self::Neon => Color32::from_rgb(45, 20, 40),         // Magenta tint selected
            Self::Onyx => Color32::from_rgb(40, 35, 22),         // Gold tint selected
            Self::System | Self::Dark => Color32::from_rgb(28, 42, 36), // Emerald tint
        }
    }

    /// Card background (slightly darker than elevated)
    pub fn bg_card(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Parchment => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Stockholm => Color32::from_rgb(244, 245, 248), // Card cool gray #F4F5F8
            Self::Copenhagen => Color32::from_rgb(245, 244, 240), // Card warm gray #F5F4F0
            Self::Light => Color32::from_rgb(242, 243, 245),     // Card = surface #F2F3F5
            Self::Midnight => Color32::from_rgb(20, 22, 32),     // Card navy
            Self::Ayu => Color32::from_rgb(16, 20, 28),          // Card charcoal
            Self::Aurora => Color32::from_rgb(27, 32, 40),       // Card night
            Self::Graphite => Color32::from_rgb(30, 30, 32),     // Card graphite
            Self::Ink => Color32::from_rgb(22, 22, 28),          // Card ink
            Self::Void => Color32::from_rgb(10, 10, 18),         // Deep card
            Self::Neon => Color32::from_rgb(15, 12, 20),         // Deep card
            Self::Onyx => Color32::from_rgb(22, 22, 18),         // Warm card
            Self::System | Self::Dark => Color32::from_rgb(18, 18, 22),
        }
    }

    /// Inset background (darker than surface, for inputs)
    pub fn bg_inset(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_base,
            Self::Parchment => Color32::from_rgb(255, 253, 250), // Bright paper #FFFDF a
            Self::Stockholm => Color32::from_rgb(252, 253, 254), // Bright inset #FCFDFE
            Self::Copenhagen => Color32::from_rgb(253, 253, 251), // Bright warm inset #FDFDFB
            Self::Light => Color32::from_rgb(253, 253, 254),     // Bright inset #FDFDFE
            Self::Midnight => Color32::from_rgb(14, 15, 22),     // Inset navy
            Self::Ayu => Color32::from_rgb(8, 11, 16),           // Inset charcoal
            Self::Aurora => Color32::from_rgb(10, 14, 18),       // Inset night
            Self::Graphite => Color32::from_rgb(14, 14, 16),     // Inset graphite
            Self::Ink => Color32::from_rgb(8, 8, 12),            // Inset ink
            Self::Void => Color32::from_rgb(4, 4, 8),            // Deeper than base
            Self::Neon => Color32::from_rgb(3, 3, 6),            // Deeper than base
            Self::Onyx => Color32::from_rgb(8, 8, 6),            // Deeper than base
            Self::System | Self::Dark => Color32::from_rgb(12, 12, 15),
        }
    }

    // =========================================================================
    // Border Colors
    // =========================================================================

    /// Subtle divider color
    pub fn border_subtle(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_subtle,
            Self::Parchment => Color32::from_rgb(220, 215, 205), // Subtle paper edge #DCD7CD
            Self::Stockholm => Color32::from_rgb(222, 226, 232), // Subtle cool border #DEE2E8
            Self::Copenhagen => Color32::from_rgb(225, 222, 215), // Warm subtle border #E1DED7
            Self::Light => Color32::from_rgb(226, 228, 232),     // Neutral subtle border #E2E4E8
            Self::Midnight => Color32::from_rgb(40, 44, 58),     // Subtle navy border
            Self::Ayu => Color32::from_rgb(35, 42, 52),          // Subtle charcoal border
            Self::Aurora => Color32::from_rgb(48, 54, 62),       // Subtle night border
            Self::Graphite => Color32::from_rgb(42, 42, 46),     // Subtle border #2A2A2E
            Self::Ink => Color32::from_rgb(30, 30, 40),          // Subtle border #1E1E28
            Self::Void => Color32::from_rgb(28, 28, 42),         // Subtle violet border
            Self::Neon => Color32::from_rgb(30, 26, 38),         // Subtle magenta border
            Self::Onyx => Color32::from_rgb(35, 35, 30),         // Warm subtle border
            Self::System | Self::Dark => Color32::from_rgb(38, 38, 44),
        }
    }

    /// Default border color
    pub fn border_default(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Parchment => Color32::from_rgb(200, 195, 185), // Paper edge #C8C3B9
            Self::Stockholm => Color32::from_rgb(205, 210, 218), // Cool border #CDD2DA
            Self::Copenhagen => Color32::from_rgb(208, 204, 196), // Warm default border #D0CCC4
            Self::Light => Color32::from_rgb(210, 212, 218),     // Neutral default border #D2D4DA
            Self::Midnight => Color32::from_rgb(55, 60, 78),     // Navy border
            Self::Ayu => Color32::from_rgb(48, 56, 68),          // Charcoal border
            Self::Aurora => Color32::from_rgb(56, 62, 72),       // Night border
            Self::Graphite => Color32::from_rgb(58, 58, 64),     // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),          // Default border #2E2E38
            Self::Void => Color32::from_rgb(42, 42, 60),         // Default violet border
            Self::Neon => Color32::from_rgb(48, 40, 58),         // Default magenta border
            Self::Onyx => Color32::from_rgb(52, 50, 42),         // Warm default border
            Self::System | Self::Dark => Color32::from_rgb(52, 52, 60),
        }
    }

    /// Focus border color
    pub fn border_focus(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_focus,
            Self::Parchment => Color32::from_rgb(100, 100, 100), // Dark gray ink #646464
            Self::Stockholm => Color32::from_rgb(74, 111, 165),  // Steel blue focus #4A6FA5
            Self::Copenhagen => Color32::from_rgb(107, 143, 113), // Sage green focus #6B8F71
            Self::Light => Color32::from_rgb(16, 185, 129),      // Enya emerald focus #10B981
            Self::Midnight => Color32::from_rgb(59, 130, 246),   // Electric blue focus
            Self::Ayu => Color32::from_rgb(180, 120, 60),        // Amber focus
            Self::Aurora => Color32::from_rgb(126, 232, 184),    // Aurora teal focus
            Self::Graphite => Color32::from_rgb(232, 93, 4),     // Molten orange focus #E85D04
            Self::Ink => Color32::from_rgb(192, 192, 200),       // Silver focus #C0C0C8
            Self::Void => Color32::from_rgb(124, 58, 237),       // Electric violet focus #7C3AED
            Self::Neon => Color32::from_rgb(224, 64, 160),       // Hot magenta focus #E040A0
            Self::Onyx => Color32::from_rgb(212, 175, 55),       // Gold focus #D4AF37
            Self::System | Self::Dark => Color32::from_rgb(55, 80, 72),
        }
    }

    // =========================================================================
    // Text Colors
    // =========================================================================

    /// Primary text color
    pub fn text_primary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_primary,
            Self::Parchment => Color32::from_rgb(30, 30, 30), // Rich black ink #1E1E1E
            Self::Stockholm => Color32::from_rgb(28, 32, 38), // Cool near-black #1C2026
            Self::Copenhagen => Color32::from_rgb(32, 30, 28), // Warm near-black #201E1C
            Self::Light => Color32::from_rgb(17, 19, 24),     // Near-black #111318
            Self::Midnight => Color32::from_rgb(228, 228, 231), // Off-white #E4E4E7
            Self::Ayu => Color32::from_rgb(191, 189, 182),    // Off-white #BFBDB6
            Self::Aurora => Color32::from_rgb(230, 237, 243), // Crisp white #E6EDF3
            Self::Graphite => Color32::from_rgb(232, 230, 224), // Warm off-white #E8E6E0
            Self::Ink => Color32::from_rgb(228, 228, 236),    // Cool off-white #E4E4EC
            Self::Void => Color32::from_rgb(232, 232, 240),   // Cool off-white
            Self::Neon => Color32::from_rgb(232, 232, 240),   // Cool off-white
            Self::Onyx => Color32::from_rgb(220, 216, 204),   // Warm off-white
            Self::System | Self::Dark => Color32::from_rgb(248, 248, 252),
        }
    }

    /// Secondary text color
    pub fn text_secondary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_secondary,
            Self::Parchment => Color32::from_rgb(80, 80, 80), // Lighter ink #505050
            Self::Stockholm => Color32::from_rgb(72, 80, 92), // Blue-gray secondary #48505C
            Self::Copenhagen => Color32::from_rgb(82, 78, 72), // Warm gray secondary #524E48
            Self::Light => Color32::from_rgb(75, 80, 92),     // Neutral gray #4B505C
            Self::Midnight => Color32::from_rgb(161, 161, 170), // Silver #A1A1AA
            Self::Ayu => Color32::from_rgb(98, 106, 115),     // Muted gray #626A73
            Self::Aurora => Color32::from_rgb(139, 148, 158), // Muted silver #8B949E
            Self::Graphite => Color32::from_rgb(168, 166, 160), // Secondary text #A8A6A0
            Self::Ink => Color32::from_rgb(152, 152, 168),    // Secondary text #9898A8
            Self::Void => Color32::from_rgb(148, 148, 168),   // Muted lavender
            Self::Neon => Color32::from_rgb(155, 148, 168),   // Muted pink-gray
            Self::Onyx => Color32::from_rgb(160, 156, 140),   // Warm gray
            Self::System | Self::Dark => Color32::from_rgb(158, 158, 168),
        }
    }

    /// Tertiary/muted text color
    pub fn text_tertiary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Parchment => Color32::from_rgb(120, 115, 110), // Faded ink #78736E
            Self::Stockholm => Color32::from_rgb(108, 118, 132), // Muted tertiary #6C7684
            Self::Copenhagen => Color32::from_rgb(118, 114, 106), // Muted warm tertiary #76726A
            Self::Light => Color32::from_rgb(112, 118, 130),     // Muted gray #707682
            Self::Midnight => Color32::from_rgb(113, 113, 122),  // Darker silver #71717A
            Self::Ayu => Color32::from_rgb(75, 82, 90),          // Darker gray #4B525A
            Self::Aurora => Color32::from_rgb(110, 118, 129),    // Deep night #6E7681
            Self::Graphite => Color32::from_rgb(112, 112, 104),  // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),         // Tertiary text #606070
            Self::Void => Color32::from_rgb(92, 92, 112),        // Deep muted
            Self::Neon => Color32::from_rgb(100, 92, 112),       // Deep muted
            Self::Onyx => Color32::from_rgb(100, 98, 88),        // Dark warm gray
            Self::System | Self::Dark => Color32::from_rgb(100, 100, 112),
        }
    }

    // =========================================================================
    // Accent Colors
    // =========================================================================

    /// Primary accent color
    pub fn accent_primary(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            Self::System | Self::Dark => Color32::from_rgb(16, 185, 129), // #10B981 Enya Emerald
            Self::Light => Color32::from_rgb(16, 185, 129),               // #10B981 Enya Emerald
            Self::Parchment => Color32::from_rgb(50, 50, 50),             // Charcoal ink #323232
            Self::Stockholm => Color32::from_rgb(74, 111, 165),           // Steel blue #4A6FA5
            Self::Copenhagen => Color32::from_rgb(107, 143, 113), // Muted sage green #6B8F71
            Self::Midnight => Color32::from_rgb(59, 130, 246),    // Electric Blue #3B82F6
            Self::Ayu => Color32::from_rgb(255, 180, 84),         // Warm Orange #FFB454
            Self::Aurora => Color32::from_rgb(126, 232, 184),     // Aurora Teal #7EE8B8
            Self::Graphite => Color32::from_rgb(232, 93, 4),      // Molten orange #E85D04
            Self::Ink => Color32::from_rgb(192, 192, 200),        // Pure silver #C0C0C8
            Self::Void => Color32::from_rgb(124, 58, 237),        // Electric violet #7C3AED
            Self::Neon => Color32::from_rgb(224, 64, 160),        // Hot magenta #E040A0
            Self::Onyx => Color32::from_rgb(212, 175, 55),        // Gold #D4AF37
        }
    }

    /// Hover accent color (brighter)
    pub fn accent_hover(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_hover,
            Self::System | Self::Dark => Color32::from_rgb(52, 211, 153),
            Self::Light => Color32::from_rgb(5, 150, 105), // Darker emerald #059669
            Self::Parchment => Color32::from_rgb(30, 30, 30), // Rich black ink hover #1E1E1E
            Self::Stockholm => Color32::from_rgb(56, 92, 145), // Darker steel blue #385C91
            Self::Copenhagen => Color32::from_rgb(90, 122, 96), // Darker sage #5A7A60
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Brighter Blue #60A5FA
            Self::Ayu => Color32::from_rgb(255, 204, 128), // Brighter Orange #FFCC80
            Self::Aurora => Color32::from_rgb(165, 243, 206), // Bright Aurora #A5F3CE
            Self::Graphite => Color32::from_rgb(255, 116, 32), // Brighter orange #FF7420
            Self::Ink => Color32::from_rgb(216, 216, 224), // Brighter silver #D8D8E0
            Self::Void => Color32::from_rgb(167, 139, 250), // Brighter violet #A78BFA
            Self::Neon => Color32::from_rgb(255, 110, 199), // Brighter magenta #FF6EC7
            Self::Onyx => Color32::from_rgb(232, 200, 80), // Brighter gold #E8C850
        }
    }

    /// Muted accent color (for subtle backgrounds)
    pub fn accent_muted(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::System | Self::Dark => Color32::from_rgb(20, 40, 34),
            Self::Light => Color32::from_rgb(236, 253, 245), // Light emerald tint #ECFDF5
            Self::Parchment => Color32::from_rgb(240, 236, 228), // Light sepia tint #F0ECE4
            Self::Stockholm => Color32::from_rgb(236, 241, 248), // Light blue tint #ECF1F8
            Self::Copenhagen => Color32::from_rgb(238, 245, 240), // Light sage tint #EEF5F0
            Self::Midnight => Color32::from_rgb(20, 30, 50), // Muted blue bg
            Self::Ayu => Color32::from_rgb(30, 25, 18),      // Muted amber bg
            Self::Aurora => Color32::from_rgb(20, 40, 35),   // Muted aurora bg
            Self::Graphite => Color32::from_rgb(40, 30, 22), // Muted orange bg
            Self::Ink => Color32::from_rgb(28, 28, 35),      // Muted silver bg
            Self::Void => Color32::from_rgb(22, 15, 45),     // Deep violet bg
            Self::Neon => Color32::from_rgb(35, 15, 30),     // Deep magenta bg
            Self::Onyx => Color32::from_rgb(30, 28, 18),     // Dark gold bg
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
            Self::System | Self::Dark => Color32::from_rgba_premultiplied(16, 185, 129, 30),
            Self::Light => Color32::from_rgba_premultiplied(16, 185, 129, 35),
            Self::Parchment => Color32::from_rgba_premultiplied(50, 50, 50, 40),
            Self::Stockholm => Color32::from_rgba_premultiplied(74, 111, 165, 35),
            Self::Copenhagen => Color32::from_rgba_premultiplied(107, 143, 113, 35),
            Self::Midnight => Color32::from_rgba_premultiplied(59, 130, 246, 30),
            Self::Ayu => Color32::from_rgba_premultiplied(255, 180, 84, 30),
            Self::Aurora => Color32::from_rgba_premultiplied(126, 232, 184, 30),
            Self::Graphite => Color32::from_rgba_premultiplied(232, 93, 4, 30),
            Self::Ink => Color32::from_rgba_premultiplied(192, 192, 200, 30),
            Self::Void => Color32::from_rgba_premultiplied(124, 58, 237, 30),
            Self::Neon => Color32::from_rgba_premultiplied(224, 64, 160, 30),
            Self::Onyx => Color32::from_rgba_premultiplied(212, 175, 55, 30),
        }
    }

    /// Selection background color
    pub fn accent_selection(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::System | Self::Dark => Color32::from_rgb(24, 52, 42),
            Self::Light => Color32::from_rgb(220, 252, 240), // Emerald selection #DCFCF0
            Self::Parchment => Color32::from_rgb(230, 225, 215), // Warm sepia selection #E6E1D7
            Self::Stockholm => Color32::from_rgb(225, 234, 245), // Blue selection #E1EAF5
            Self::Copenhagen => Color32::from_rgb(228, 240, 232), // Sage selection #E4F0E8
            Self::Midnight => Color32::from_rgb(30, 45, 70), // Blue selection
            Self::Ayu => Color32::from_rgb(45, 38, 25),      // Amber selection
            Self::Aurora => Color32::from_rgb(30, 55, 48),   // Teal selection
            Self::Graphite => Color32::from_rgb(60, 45, 32), // Orange tint selection
            Self::Ink => Color32::from_rgb(38, 38, 50),      // Silver tint selection
            Self::Void => Color32::from_rgb(30, 22, 60),     // Violet selection
            Self::Neon => Color32::from_rgb(45, 20, 40),     // Magenta selection
            Self::Onyx => Color32::from_rgb(38, 34, 22),     // Gold selection
        }
    }

    // =========================================================================
    // Overlay Colors (for modals, dropdowns, popups)
    // =========================================================================

    /// Overlay background (frosted glass)
    pub fn overlay_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_bg,
            Self::Parchment => Color32::from_rgba_unmultiplied(250, 248, 245, 250),
            Self::Stockholm => Color32::from_rgba_unmultiplied(250, 251, 252, 250),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(250, 250, 248, 250),
            Self::Light => Color32::from_rgba_unmultiplied(251, 251, 252, 250),
            Self::Midnight => Color32::from_rgba_unmultiplied(14, 16, 24, 245),
            Self::Ayu => Color32::from_rgba_unmultiplied(12, 16, 22, 245),
            Self::Aurora => Color32::from_rgba_unmultiplied(16, 20, 26, 245),
            Self::Graphite => Color32::from_rgba_unmultiplied(18, 18, 20, 245),
            Self::Ink => Color32::from_rgba_unmultiplied(10, 10, 15, 245),
            Self::Void => Color32::from_rgba_unmultiplied(0, 0, 0, 245),
            Self::Neon => Color32::from_rgba_unmultiplied(5, 5, 8, 245),
            Self::Onyx => Color32::from_rgba_unmultiplied(12, 12, 12, 245),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(14, 14, 16, 245),
        }
    }

    /// Overlay background (deep/premium glass)
    pub fn overlay_bg_deep(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_bg,
            Self::Parchment => Color32::from_rgba_unmultiplied(245, 242, 237, 248),
            Self::Stockholm => Color32::from_rgba_unmultiplied(244, 245, 248, 248),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(245, 244, 240, 248),
            Self::Light => Color32::from_rgba_unmultiplied(242, 243, 245, 248),
            Self::Midnight => Color32::from_rgba_unmultiplied(10, 12, 20, 235),
            Self::Ayu => Color32::from_rgba_unmultiplied(8, 12, 18, 235),
            Self::Aurora => Color32::from_rgba_unmultiplied(12, 16, 22, 235),
            Self::Graphite => Color32::from_rgba_unmultiplied(14, 14, 16, 235),
            Self::Ink => Color32::from_rgba_unmultiplied(8, 8, 12, 235),
            Self::Void => Color32::from_rgba_unmultiplied(0, 0, 0, 235),
            Self::Neon => Color32::from_rgba_unmultiplied(3, 3, 6, 235),
            Self::Onyx => Color32::from_rgba_unmultiplied(8, 8, 6, 235),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(12, 12, 14, 235),
        }
    }

    /// Overlay border color
    pub fn overlay_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_border,
            Self::Parchment => Color32::from_rgba_unmultiplied(200, 195, 185, 220),
            Self::Stockholm => Color32::from_rgba_unmultiplied(205, 210, 218, 220),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(208, 204, 196, 220),
            Self::Light => Color32::from_rgba_unmultiplied(210, 212, 218, 220),
            Self::Midnight => Color32::from_rgba_unmultiplied(55, 60, 80, 160),
            Self::Ayu => Color32::from_rgba_unmultiplied(48, 56, 68, 160),
            Self::Aurora => Color32::from_rgba_unmultiplied(50, 58, 68, 160),
            Self::Graphite => Color32::from_rgba_unmultiplied(58, 58, 64, 160),
            Self::Ink => Color32::from_rgba_unmultiplied(46, 46, 56, 160),
            Self::Void => Color32::from_rgba_unmultiplied(42, 42, 60, 160),
            Self::Neon => Color32::from_rgba_unmultiplied(48, 40, 58, 160),
            Self::Onyx => Color32::from_rgba_unmultiplied(52, 50, 42, 160),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(45, 45, 48, 160),
        }
    }

    /// Overlay inner highlight (top edge glow for glass effect)
    pub fn overlay_highlight(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_highlight,
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => {
                Color32::from_rgba_unmultiplied(255, 255, 252, 100)
            }
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 12),
        }
    }

    /// Overlay inner highlight (stronger for premium glass)
    pub fn overlay_highlight_strong(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.overlay_highlight,
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => {
                Color32::from_rgba_unmultiplied(255, 255, 252, 150)
            }
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
            Self::Parchment => Color32::from_rgb(245, 242, 237),
            Self::Stockholm => Color32::from_rgb(244, 245, 248),
            Self::Copenhagen => Color32::from_rgb(245, 244, 240),
            Self::Light => Color32::from_rgb(242, 243, 245),
            Self::Midnight => Color32::from_rgb(14, 16, 24),
            Self::Ayu => Color32::from_rgb(10, 14, 20),
            Self::Aurora => Color32::from_rgb(14, 18, 24),
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Popup graphite
            Self::Ink => Color32::from_rgb(12, 12, 18),      // Popup ink
            Self::Void => Color32::from_rgb(4, 4, 8),        // Popup void
            Self::Neon => Color32::from_rgb(5, 5, 8),        // Popup neon
            Self::Onyx => Color32::from_rgb(8, 8, 6),        // Popup onyx
            Self::System | Self::Dark => Color32::from_rgb(16, 16, 20),
        }
    }

    /// Popup border color (subtle accent tint)
    pub fn popup_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Parchment => Color32::from_rgb(200, 195, 185),
            Self::Stockholm => Color32::from_rgb(185, 195, 210), // Cool blue popup border
            Self::Copenhagen => Color32::from_rgb(190, 200, 192), // Sage-tinted border
            Self::Light => Color32::from_rgb(180, 195, 188),     // Slight emerald tint
            Self::Midnight => Color32::from_rgb(50, 60, 85),
            Self::Ayu => Color32::from_rgb(55, 50, 40),
            Self::Aurora => Color32::from_rgb(45, 70, 62),
            Self::Graphite => Color32::from_rgb(80, 55, 35), // Orange tint border
            Self::Ink => Color32::from_rgb(55, 55, 70),      // Silver tint border
            Self::Void => Color32::from_rgb(60, 40, 90),     // Violet tint border
            Self::Neon => Color32::from_rgb(70, 40, 65),     // Magenta tint border
            Self::Onyx => Color32::from_rgb(65, 58, 35),     // Gold tint border
            Self::System | Self::Dark => Color32::from_rgb(50, 55, 52),
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
            Self::Parchment => Color32::from_rgba_unmultiplied(50, 48, 45, 60),
            Self::Stockholm => Color32::from_rgba_unmultiplied(40, 48, 60, 60),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(45, 42, 38, 60),
            Self::Light => Color32::from_rgba_unmultiplied(17, 19, 24, 60),
            Self::Midnight => Color32::from_rgba_unmultiplied(5, 8, 15, 200),
            Self::Ayu => Color32::from_rgba_unmultiplied(5, 8, 12, 200),
            Self::Aurora => Color32::from_rgba_unmultiplied(8, 12, 16, 200),
            Self::Graphite => Color32::from_rgba_unmultiplied(10, 10, 12, 200),
            Self::Ink => Color32::from_rgba_unmultiplied(5, 5, 10, 200),
            Self::Void => Color32::from_rgba_unmultiplied(0, 0, 0, 200),
            Self::Neon => Color32::from_rgba_unmultiplied(3, 3, 5, 200),
            Self::Onyx => Color32::from_rgba_unmultiplied(6, 6, 6, 200),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(4, 4, 6, 200),
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
            Self::Parchment => Color32::from_rgba_unmultiplied(50, 48, 45, 80),
            Self::Stockholm => Color32::from_rgba_unmultiplied(40, 48, 60, 80),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(45, 42, 38, 80),
            Self::Light => Color32::from_rgba_unmultiplied(17, 19, 24, 80),
            Self::Midnight => Color32::from_rgba_unmultiplied(5, 8, 15, 210),
            Self::Ayu => Color32::from_rgba_unmultiplied(5, 8, 12, 210),
            Self::Aurora => Color32::from_rgba_unmultiplied(8, 12, 16, 210),
            Self::Graphite => Color32::from_rgba_unmultiplied(10, 10, 12, 210),
            Self::Ink => Color32::from_rgba_unmultiplied(5, 5, 10, 210),
            Self::Void => Color32::from_rgba_unmultiplied(0, 0, 0, 210),
            Self::Neon => Color32::from_rgba_unmultiplied(3, 3, 5, 210),
            Self::Onyx => Color32::from_rgba_unmultiplied(6, 6, 6, 210),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(4, 4, 6, 210),
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
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => None,
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
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => None,
            Self::System | Self::Dark => Some(Color32::from_rgba_unmultiplied(16, 185, 129, 8)),
            Self::Midnight => Some(Color32::from_rgba_unmultiplied(59, 130, 246, 8)),
            Self::Ayu => Some(Color32::from_rgba_unmultiplied(255, 180, 84, 8)),
            Self::Aurora => Some(Color32::from_rgba_unmultiplied(126, 232, 184, 8)),
            Self::Graphite => Some(Color32::from_rgba_unmultiplied(232, 93, 4, 8)),
            Self::Ink => Some(Color32::from_rgba_unmultiplied(192, 192, 200, 8)),
            Self::Void => Some(Color32::from_rgba_unmultiplied(124, 58, 237, 8)),
            Self::Neon => Some(Color32::from_rgba_unmultiplied(224, 64, 160, 8)),
            Self::Onyx => Some(Color32::from_rgba_unmultiplied(212, 175, 55, 8)),
        }
    }

    // =========================================================================
    // Highlight Colors
    // =========================================================================

    /// Match highlight color (for search results)
    pub fn highlight_match(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Parchment => Color32::from_rgb(255, 245, 180),
            Self::Stockholm => Color32::from_rgb(225, 238, 255), // Blue highlight #E1EEFF
            Self::Copenhagen => Color32::from_rgb(228, 245, 232), // Green highlight #E4F5E8
            Self::Light => Color32::from_rgb(220, 252, 240),     // Emerald highlight #DCFCF0
            Self::Midnight => Color32::from_rgb(30, 50, 80),
            Self::Ayu => Color32::from_rgb(50, 40, 25),
            Self::Aurora => Color32::from_rgb(30, 55, 50),
            Self::Graphite => Color32::from_rgb(60, 40, 28), // Orange tint highlight
            Self::Ink => Color32::from_rgb(35, 35, 50),      // Silver tint highlight
            Self::Void => Color32::from_rgb(35, 25, 65),     // Violet tint highlight
            Self::Neon => Color32::from_rgb(45, 20, 40),     // Magenta tint highlight
            Self::Onyx => Color32::from_rgb(35, 30, 18),     // Gold tint highlight
            Self::System | Self::Dark => Color32::from_rgb(16, 60, 48),
        }
    }

    /// Match highlight text color (for fuzzy search result highlighting)
    /// This is a bright, visible color for text foreground use (not background)
    pub fn highlight_match_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.highlight_match,
            Self::Parchment => Color32::from_rgb(180, 100, 0),
            Self::Stockholm => Color32::from_rgb(56, 92, 145), // Steel blue match text
            Self::Copenhagen => Color32::from_rgb(60, 100, 65), // Dark sage match text
            Self::Light => Color32::from_rgb(4, 120, 87),      // Dark emerald #047857
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Electric blue
            Self::Ayu => Color32::from_rgb(255, 200, 100),     // Gold
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(220, 220, 230),     // Bright silver
            Self::Void => Color32::from_rgb(167, 139, 250),    // Bright violet
            Self::Neon => Color32::from_rgb(255, 110, 199),    // Bright magenta
            Self::Onyx => Color32::from_rgb(232, 200, 80),     // Bright gold
            Self::System | Self::Dark => Color32::from_rgb(255, 200, 80),
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
            Self::Parchment => Color32::from_rgba_unmultiplied(255, 220, 120, 80),
            Self::Stockholm => Color32::from_rgba_unmultiplied(74, 111, 165, 60),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(107, 143, 113, 60),
            Self::Light => Color32::from_rgba_unmultiplied(16, 185, 129, 60),
            Self::Midnight => Color32::from_rgba_unmultiplied(59, 130, 246, 30),
            Self::Ayu => Color32::from_rgba_unmultiplied(255, 180, 84, 30),
            Self::Aurora => Color32::from_rgba_unmultiplied(126, 232, 184, 30),
            Self::Graphite => Color32::from_rgba_unmultiplied(232, 93, 4, 30),
            Self::Ink => Color32::from_rgba_unmultiplied(192, 192, 200, 30),
            Self::Void => Color32::from_rgba_unmultiplied(124, 58, 237, 30),
            Self::Neon => Color32::from_rgba_unmultiplied(224, 64, 160, 30),
            Self::Onyx => Color32::from_rgba_unmultiplied(212, 175, 55, 30),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(255, 220, 0, 30),
        }
    }

    // =========================================================================
    // Badge Colors (status line badges)
    // =========================================================================

    /// Zen mode badge background
    pub fn badge_zen_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            Self::Parchment => Color32::from_rgb(100, 90, 80),
            Self::Stockholm => Color32::from_rgb(74, 111, 165), // Steel blue
            Self::Copenhagen => Color32::from_rgb(107, 143, 113), // Sage green
            Self::Light => Color32::from_rgb(16, 185, 129),     // Enya emerald
            Self::Midnight => Color32::from_rgb(167, 139, 250), // Violet
            Self::Ayu => Color32::from_rgb(210, 180, 140),      // Tan
            Self::Aurora => Color32::from_rgb(165, 210, 195),   // Aurora mint
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Void => Color32::from_rgb(167, 139, 250),     // Bright violet
            Self::Neon => Color32::from_rgb(255, 110, 199),     // Bright magenta
            Self::Onyx => Color32::from_rgb(232, 200, 80),      // Bright gold
            Self::System | Self::Dark => Color32::from_rgb(180, 150, 220),
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
            Self::Parchment => Color32::from_rgb(60, 60, 60),
            Self::Stockholm => Color32::from_rgb(56, 92, 145), // Steel blue
            Self::Copenhagen => Color32::from_rgb(55, 95, 145), // Warm blue
            Self::Light => Color32::from_rgb(5, 150, 105),     // Darker emerald
            Self::Midnight => Color32::from_rgb(56, 189, 248), // Sky blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(210, 210, 220),     // Bright silver
            Self::Void => Color32::from_rgb(96, 165, 250),     // Blue
            Self::Neon => Color32::from_rgb(96, 165, 250),     // Blue
            Self::Onyx => Color32::from_rgb(130, 160, 210),    // Warm blue
            Self::System | Self::Dark => Color32::from_rgb(120, 200, 220),
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
            Self::Parchment => Color32::from_rgb(100, 150, 220),
            Self::Stockholm => Color32::from_rgb(74, 111, 165), // Steel blue
            Self::Copenhagen => Color32::from_rgb(80, 130, 170), // Warm blue
            Self::Light => Color32::from_rgb(37, 99, 235),      // Blue
            Self::Midnight => Color32::from_rgb(96, 165, 250),  // Sky blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),       // Teal
            Self::Aurora => Color32::from_rgb(139, 198, 198),   // Aurora cyan
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Void => Color32::from_rgb(96, 165, 250),      // Sky blue
            Self::Neon => Color32::from_rgb(96, 165, 250),      // Sky blue
            Self::Onyx => Color32::from_rgb(130, 160, 210),     // Warm blue
            Self::System | Self::Dark => Color32::from_rgb(130, 180, 255),
        }
    }

    /// Insert mode color (editing)
    pub fn mode_insert(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Parchment => Color32::from_rgb(100, 180, 100),
            Self::Stockholm => Color32::from_rgb(40, 140, 80), // Nordic green
            Self::Copenhagen => Color32::from_rgb(60, 140, 80), // Forest green
            Self::Light => Color32::from_rgb(22, 163, 74),     // Green
            Self::Midnight => Color32::from_rgb(52, 211, 153), // Green
            Self::Ayu => Color32::from_rgb(170, 210, 120),     // Green
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),     // Muted green
            Self::Void => Color32::from_rgb(52, 211, 153),     // Green
            Self::Neon => Color32::from_rgb(52, 211, 153),     // Green
            Self::Onyx => Color32::from_rgb(52, 185, 100),     // Green
            Self::System | Self::Dark => Color32::from_rgb(150, 220, 120),
        }
    }

    /// Buffer border color (inactive)
    pub fn buffer_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Parchment => Color32::from_rgb(200, 195, 185),
            Self::Stockholm => Color32::from_rgb(205, 210, 218), // Cool border
            Self::Copenhagen => Color32::from_rgb(208, 204, 196), // Warm border
            Self::Light => Color32::from_rgb(210, 212, 218),     // Neutral border #D2D4DA
            Self::Midnight => Color32::from_rgb(55, 60, 78),
            Self::Ayu => Color32::from_rgb(48, 56, 68),
            Self::Aurora => Color32::from_rgb(48, 54, 62),
            Self::Graphite => Color32::from_rgb(58, 58, 64), // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),      // Default border #2E2E38
            Self::Void => Color32::from_rgb(42, 42, 60),     // Default violet border
            Self::Neon => Color32::from_rgb(48, 40, 58),     // Default magenta border
            Self::Onyx => Color32::from_rgb(52, 50, 42),     // Warm default border
            Self::System | Self::Dark => Color32::from_rgb(60, 60, 70),
        }
    }

    /// Buffer background color
    pub fn buffer_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Parchment => Color32::from_rgb(250, 248, 245),
            Self::Stockholm => Color32::from_rgb(250, 251, 252), // Cool white
            Self::Copenhagen => Color32::from_rgb(250, 250, 248), // Warm white
            Self::Light => Color32::from_rgb(251, 251, 252),     // Cool neutral white #FBFBFC
            Self::Midnight => Color32::from_rgb(16, 18, 26),
            Self::Ayu => Color32::from_rgb(12, 16, 22),
            Self::Aurora => Color32::from_rgb(18, 22, 28),
            Self::Graphite => Color32::from_rgb(22, 22, 24), // Buffer graphite
            Self::Ink => Color32::from_rgb(14, 14, 20),      // Buffer ink
            Self::Void => Color32::from_rgb(8, 8, 14),       // Buffer void
            Self::Neon => Color32::from_rgb(12, 10, 16),     // Buffer neon
            Self::Onyx => Color32::from_rgb(18, 18, 16),     // Buffer onyx
            Self::System | Self::Dark => Color32::from_rgb(25, 25, 30),
        }
    }

    /// Buffer content background (inner area)
    pub fn buffer_content_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_base,
            Self::Parchment => Color32::from_rgb(255, 253, 250),
            Self::Stockholm => Color32::from_rgb(252, 253, 254), // Bright inset
            Self::Copenhagen => Color32::from_rgb(253, 253, 251), // Bright inset
            Self::Light => Color32::from_rgb(253, 253, 254),     // Bright inset #FDFDFE
            Self::Midnight => Color32::from_rgb(12, 14, 20),
            Self::Ayu => Color32::from_rgb(10, 14, 20),
            Self::Aurora => Color32::from_rgb(13, 17, 23),
            Self::Graphite => Color32::from_rgb(18, 18, 20), // Content bg #121214
            Self::Ink => Color32::from_rgb(10, 10, 15),      // Content bg #0A0A0F
            Self::Void => Color32::from_rgb(0, 0, 0),        // Content bg void
            Self::Neon => Color32::from_rgb(5, 5, 8),        // Content bg neon
            Self::Onyx => Color32::from_rgb(12, 12, 12),     // Content bg onyx
            Self::System | Self::Dark => Color32::from_rgb(20, 20, 25),
        }
    }

    // =========================================================================
    // Semantic Colors
    // =========================================================================

    /// Success color
    pub fn semantic_success(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Parchment => Color32::from_rgb(45, 100, 45),
            Self::Stockholm => Color32::from_rgb(30, 110, 60), // Forest green
            Self::Copenhagen => Color32::from_rgb(40, 115, 55), // Forest green
            Self::Light => Color32::from_rgb(22, 163, 74),     // Green #16A34A
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Aurora => Color32::from_rgb(126, 232, 184), // Aurora teal
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),    // Muted green
            Self::Void => Color32::from_rgb(52, 211, 153),    // Green
            Self::Neon => Color32::from_rgb(52, 211, 153),    // Green
            Self::Onyx => Color32::from_rgb(52, 185, 100),    // Green
            Self::System | Self::Dark => Color32::from_rgb(34, 197, 94),
        }
    }

    /// Warning color
    pub fn semantic_warning(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.warning,
            Self::Parchment => Color32::from_rgb(180, 120, 30),
            Self::Stockholm => Color32::from_rgb(170, 110, 20), // Amber
            Self::Copenhagen => Color32::from_rgb(175, 115, 25), // Warm amber
            Self::Light => Color32::from_rgb(202, 138, 4),      // Amber #CA8A04
            Self::Midnight => Color32::from_rgb(251, 191, 36),  // Amber
            Self::Ayu => Color32::from_rgb(255, 200, 100),
            Self::Aurora => Color32::from_rgb(255, 200, 120), // Warm gold
            Self::Graphite => Color32::from_rgb(255, 180, 80), // Warm orange
            Self::Ink => Color32::from_rgb(220, 200, 140),    // Muted gold
            Self::Void => Color32::from_rgb(251, 191, 36),    // Amber
            Self::Neon => Color32::from_rgb(251, 191, 36),    // Amber
            Self::Onyx => Color32::from_rgb(232, 180, 60),    // Warm amber
            Self::System | Self::Dark => Color32::from_rgb(251, 176, 45),
        }
    }

    /// Error color
    pub fn semantic_error(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Parchment => Color32::from_rgb(180, 40, 40),
            Self::Stockholm => Color32::from_rgb(190, 40, 40), // Red
            Self::Copenhagen => Color32::from_rgb(185, 45, 45), // Warm red
            Self::Light => Color32::from_rgb(220, 38, 38),     // Red #DC2626
            Self::Midnight => Color32::from_rgb(248, 113, 113), // Red
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113), // Soft red
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Soft red
            Self::Ink => Color32::from_rgb(200, 110, 120),    // Muted rose
            Self::Void => Color32::from_rgb(248, 113, 113),   // Red
            Self::Neon => Color32::from_rgb(248, 113, 113),   // Red
            Self::Onyx => Color32::from_rgb(230, 100, 100),   // Soft red
            Self::System | Self::Dark => Color32::from_rgb(239, 82, 82),
        }
    }

    /// Info color
    pub fn semantic_info(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Parchment => Color32::from_rgb(50, 80, 140),
            Self::Stockholm => Color32::from_rgb(50, 90, 150), // Deep blue
            Self::Copenhagen => Color32::from_rgb(55, 95, 145), // Warm blue
            Self::Light => Color32::from_rgb(37, 99, 235),     // Blue #2563EB
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Aurora => Color32::from_rgb(139, 198, 198), // Aurora cyan
            Self::Graphite => Color32::from_rgb(232, 93, 4),  // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),    // Pure silver
            Self::Void => Color32::from_rgb(96, 165, 250),    // Blue
            Self::Neon => Color32::from_rgb(96, 165, 250),    // Blue
            Self::Onyx => Color32::from_rgb(130, 160, 210),   // Warm blue
            Self::System | Self::Dark => Color32::from_rgb(82, 146, 255),
        }
    }

    // =========================================================================
    // Syntax Highlighting Colors
    // =========================================================================

    /// Keyword color
    pub fn syntax_keyword(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            Self::Parchment => Color32::from_rgb(30, 30, 30),
            Self::Stockholm => Color32::from_rgb(74, 111, 165), // Steel blue keywords
            Self::Copenhagen => Color32::from_rgb(107, 143, 113), // Sage green keywords
            Self::Light => Color32::from_rgb(5, 150, 105),      // Emerald keywords #059669
            Self::Midnight => Color32::from_rgb(199, 146, 234), // Purple
            Self::Ayu => Color32::from_rgb(255, 143, 64),       // Orange
            Self::Aurora => Color32::from_rgb(200, 160, 220),   // Aurora violet
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Void => Color32::from_rgb(124, 58, 237),      // Electric violet
            Self::Neon => Color32::from_rgb(224, 64, 160),      // Hot magenta
            Self::Onyx => Color32::from_rgb(212, 175, 55),      // Gold
            Self::System | Self::Dark => Color32::from_rgb(198, 146, 255),
        }
    }

    /// Key/property color
    pub fn syntax_key(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Parchment => Color32::from_rgb(50, 50, 50),
            Self::Stockholm => Color32::from_rgb(50, 90, 150), // Deep blue keys
            Self::Copenhagen => Color32::from_rgb(55, 95, 145), // Warm blue keys
            Self::Light => Color32::from_rgb(37, 99, 235),     // Blue keys #2563EB
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Aurora => Color32::from_rgb(139, 198, 198),  // Aurora cyan
            Self::Graphite => Color32::from_rgb(255, 160, 100), // Bright orange
            Self::Ink => Color32::from_rgb(160, 160, 180),     // Muted silver
            Self::Void => Color32::from_rgb(96, 165, 250),     // Blue
            Self::Neon => Color32::from_rgb(96, 165, 250),     // Blue
            Self::Onyx => Color32::from_rgb(130, 160, 210),    // Warm blue
            Self::System | Self::Dark => Color32::from_rgb(110, 190, 248),
        }
    }

    /// Value/string color
    pub fn syntax_value(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Parchment => Color32::from_rgb(70, 70, 70),
            Self::Stockholm => Color32::from_rgb(30, 110, 60), // Nordic green values
            Self::Copenhagen => Color32::from_rgb(40, 115, 55), // Forest green values
            Self::Light => Color32::from_rgb(22, 163, 74),     // Green values #16A34A
            Self::Midnight => Color32::from_rgb(52, 211, 153), // Green
            Self::Ayu => Color32::from_rgb(170, 210, 120),     // Green
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),     // Muted green
            Self::Void => Color32::from_rgb(52, 211, 153),     // Green
            Self::Neon => Color32::from_rgb(52, 211, 153),     // Green
            Self::Onyx => Color32::from_rgb(52, 185, 100),     // Green
            Self::System | Self::Dark => Color32::from_rgb(52, 211, 153),
        }
    }

    /// Operator/punctuation color
    pub fn syntax_punctuation(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_secondary,
            Self::Parchment => Color32::from_rgb(100, 95, 90),
            Self::Stockholm => Color32::from_rgb(108, 118, 132), // Blue-gray punctuation
            Self::Copenhagen => Color32::from_rgb(118, 114, 106), // Warm gray
            Self::Light => Color32::from_rgb(112, 118, 130),     // Muted gray #707682
            Self::Midnight => Color32::from_rgb(148, 163, 184),  // Slate
            Self::Ayu => Color32::from_rgb(140, 148, 156),       // Gray
            Self::Aurora => Color32::from_rgb(139, 148, 158),    // Muted silver
            Self::Graphite => Color32::from_rgb(168, 166, 160),  // Secondary text #A8A6A0
            Self::Ink => Color32::from_rgb(152, 152, 168),       // Secondary text #9898A8
            Self::Void => Color32::from_rgb(148, 148, 168),      // Muted
            Self::Neon => Color32::from_rgb(155, 148, 168),      // Muted
            Self::Onyx => Color32::from_rgb(160, 156, 140),      // Warm muted
            Self::System | Self::Dark => Color32::from_rgb(140, 140, 155),
        }
    }

    /// Comment color
    pub fn syntax_comment(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Parchment => Color32::from_rgb(140, 135, 125),
            Self::Stockholm => Color32::from_rgb(140, 150, 165), // Light gray-blue comments
            Self::Copenhagen => Color32::from_rgb(148, 144, 135), // Warm gray comments
            Self::Light => Color32::from_rgb(148, 152, 163),     // Neutral gray comments #9498A3
            Self::Midnight => Color32::from_rgb(100, 116, 139),  // Slate gray
            Self::Ayu => Color32::from_rgb(90, 100, 110),        // Gray
            Self::Aurora => Color32::from_rgb(110, 118, 129),    // Deep night
            Self::Graphite => Color32::from_rgb(112, 112, 104),  // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),         // Tertiary text #606070
            Self::Void => Color32::from_rgb(85, 85, 104),        // Dark muted
            Self::Neon => Color32::from_rgb(85, 80, 96),         // Dark muted
            Self::Onyx => Color32::from_rgb(88, 85, 72),         // Dark warm
            Self::System | Self::Dark => Color32::from_rgb(128, 128, 128),
        }
    }

    /// Function color
    pub fn syntax_function(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_hover,
            Self::Parchment => Color32::from_rgb(40, 40, 40),
            Self::Stockholm => Color32::from_rgb(40, 50, 65), // Dark blue-gray functions
            Self::Copenhagen => Color32::from_rgb(45, 50, 42), // Dark warm-green
            Self::Light => Color32::from_rgb(4, 120, 87),     // Dark emerald functions #047857
            Self::Midnight => Color32::from_rgb(56, 189, 248), // Cyan
            Self::Ayu => Color32::from_rgb(255, 180, 84),     // Orange
            Self::Aurora => Color32::from_rgb(165, 243, 206), // Bright aurora
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(216, 216, 224),    // Bright silver
            Self::Void => Color32::from_rgb(167, 139, 250),   // Bright violet
            Self::Neon => Color32::from_rgb(255, 110, 199),   // Bright magenta
            Self::Onyx => Color32::from_rgb(232, 200, 80),    // Bright gold
            Self::System | Self::Dark => Color32::from_rgb(100, 160, 255),
        }
    }

    /// Type/class color
    pub fn syntax_type(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.warning,
            Self::Parchment => Color32::from_rgb(60, 60, 60),
            Self::Stockholm => Color32::from_rgb(85, 95, 115), // Medium blue-gray types
            Self::Copenhagen => Color32::from_rgb(90, 95, 80), // Warm olive-gray
            Self::Light => Color32::from_rgb(55, 65, 81),      // Neutral dark type #374151
            Self::Midnight => Color32::from_rgb(251, 191, 36), // Amber
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Aurora => Color32::from_rgb(200, 220, 180),  // Aurora yellow-green
            Self::Graphite => Color32::from_rgb(200, 170, 120), // Warm tan
            Self::Ink => Color32::from_rgb(180, 180, 190),     // Light silver
            Self::Void => Color32::from_rgb(184, 160, 232),    // Lighter violet
            Self::Neon => Color32::from_rgb(244, 114, 182),    // Pink
            Self::Onyx => Color32::from_rgb(200, 184, 128),    // Pale gold
            Self::System | Self::Dark => Color32::from_rgb(220, 160, 100),
        }
    }

    /// Number/constant color
    pub fn syntax_number(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Parchment => Color32::from_rgb(55, 55, 55),
            Self::Stockholm => Color32::from_rgb(130, 80, 50), // Warm brown numbers
            Self::Copenhagen => Color32::from_rgb(140, 85, 55), // Warm brown
            Self::Light => Color32::from_rgb(180, 83, 9),      // Amber numbers #B45309
            Self::Midnight => Color32::from_rgb(248, 113, 113), // Red
            Self::Ayu => Color32::from_rgb(230, 140, 90),      // Coral
            Self::Aurora => Color32::from_rgb(255, 180, 150),  // Aurora peach
            Self::Graphite => Color32::from_rgb(255, 140, 80), // Coral orange
            Self::Ink => Color32::from_rgb(200, 160, 180),     // Dusty rose
            Self::Void => Color32::from_rgb(248, 113, 113),    // Coral red
            Self::Neon => Color32::from_rgb(251, 146, 60),     // Orange
            Self::Onyx => Color32::from_rgb(184, 152, 112),    // Warm bronze
            Self::System | Self::Dark => Color32::from_rgb(220, 120, 120),
        }
    }

    /// Variable color
    pub fn syntax_variable(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_primary,
            Self::Parchment => Color32::from_rgb(45, 45, 45),
            Self::Stockholm => Color32::from_rgb(50, 58, 70), // Dark cool gray variables
            Self::Copenhagen => Color32::from_rgb(52, 50, 45), // Dark warm gray
            Self::Light => Color32::from_rgb(24, 24, 27),     // Near-black variables #18181B
            Self::Midnight => Color32::from_rgb(228, 228, 231), // Off-white
            Self::Ayu => Color32::from_rgb(191, 189, 182),    // Fg
            Self::Aurora => Color32::from_rgb(230, 237, 243), // Crisp white
            Self::Graphite => Color32::from_rgb(232, 230, 224), // Text primary #E8E6E0
            Self::Ink => Color32::from_rgb(228, 228, 236),    // Text primary #E4E4EC
            Self::Void => Color32::from_rgb(224, 224, 232),   // Near-white
            Self::Neon => Color32::from_rgb(224, 224, 232),   // Near-white
            Self::Onyx => Color32::from_rgb(212, 208, 196),   // Warm near-white
            Self::System | Self::Dark => Color32::from_rgb(220, 220, 220),
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
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => {
                Color32::from_rgba_unmultiplied(80, 75, 70, 15)
            }
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
            Self::Parchment => Color32::from_rgba_unmultiplied(120, 115, 105, 160),
            Self::Stockholm => Color32::from_rgba_unmultiplied(108, 118, 132, 150),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(118, 114, 106, 150),
            Self::Light => Color32::from_rgba_unmultiplied(112, 118, 130, 150),
            Self::Midnight => Color32::from_rgba_unmultiplied(96, 165, 250, 80),
            Self::Ayu => Color32::from_rgba_unmultiplied(140, 148, 156, 120),
            Self::Aurora => Color32::from_rgba_unmultiplied(139, 148, 158, 120),
            Self::Graphite => Color32::from_rgba_unmultiplied(168, 166, 160, 120),
            Self::Ink => Color32::from_rgba_unmultiplied(152, 152, 168, 120),
            Self::Void => Color32::from_rgba_unmultiplied(148, 148, 168, 120),
            Self::Neon => Color32::from_rgba_unmultiplied(155, 148, 168, 120),
            Self::Onyx => Color32::from_rgba_unmultiplied(160, 156, 140, 120),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(140, 140, 160, 120),
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
            Self::Parchment => Color32::from_rgba_unmultiplied(80, 75, 70, 200),
            Self::Stockholm => Color32::from_rgba_unmultiplied(74, 111, 165, 160),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(107, 143, 113, 160),
            Self::Light => Color32::from_rgba_unmultiplied(16, 185, 129, 160),
            Self::Midnight => Color32::from_rgba_unmultiplied(96, 165, 250, 140),
            Self::Ayu => Color32::from_rgba_unmultiplied(255, 180, 84, 140),
            Self::Aurora => Color32::from_rgba_unmultiplied(126, 232, 184, 140),
            Self::Graphite => Color32::from_rgba_unmultiplied(232, 93, 4, 140),
            Self::Ink => Color32::from_rgba_unmultiplied(192, 192, 200, 140),
            Self::Void => Color32::from_rgba_unmultiplied(124, 58, 237, 160),
            Self::Neon => Color32::from_rgba_unmultiplied(224, 64, 160, 160),
            Self::Onyx => Color32::from_rgba_unmultiplied(212, 175, 55, 160),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(180, 180, 200, 160),
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
            Self::Light | Self::Parchment | Self::Stockholm | Self::Copenhagen => {
                Color32::from_rgba_unmultiplied(255, 252, 245, 80)
            }
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
            Self::Parchment => Color32::from_rgba_unmultiplied(248, 245, 240, 252),
            Self::Stockholm => Color32::from_rgba_unmultiplied(248, 249, 252, 252),
            Self::Copenhagen => Color32::from_rgba_unmultiplied(248, 248, 245, 252),
            Self::Light => Color32::from_rgba_unmultiplied(249, 250, 252, 252),
            Self::Midnight => Color32::from_rgba_unmultiplied(14, 16, 24, 250),
            Self::Ayu => Color32::from_rgba_unmultiplied(12, 16, 22, 250),
            Self::Aurora => Color32::from_rgba_unmultiplied(13, 17, 23, 250),
            Self::Graphite => Color32::from_rgba_unmultiplied(18, 18, 20, 250),
            Self::Ink => Color32::from_rgba_unmultiplied(10, 10, 15, 250),
            Self::Void => Color32::from_rgba_unmultiplied(0, 0, 0, 250),
            Self::Neon => Color32::from_rgba_unmultiplied(5, 5, 8, 250),
            Self::Onyx => Color32::from_rgba_unmultiplied(12, 12, 12, 250),
            Self::System | Self::Dark => Color32::from_rgba_unmultiplied(15, 15, 15, 250),
        }
    }

    /// Agent panel border
    pub fn agent_panel_border(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.border_default,
            Self::Parchment => Color32::from_rgb(200, 195, 185),
            Self::Stockholm => Color32::from_rgb(205, 210, 218),
            Self::Copenhagen => Color32::from_rgb(208, 204, 196),
            Self::Light => Color32::from_rgb(210, 212, 218),
            Self::Midnight => Color32::from_rgb(55, 60, 78),
            Self::Ayu => Color32::from_rgb(48, 56, 68),
            Self::Aurora => Color32::from_rgb(48, 54, 62),
            Self::Graphite => Color32::from_rgb(58, 58, 64), // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),      // Default border #2E2E38
            Self::Void => Color32::from_rgb(42, 42, 60),     // Default violet border
            Self::Neon => Color32::from_rgb(48, 40, 58),     // Default magenta border
            Self::Onyx => Color32::from_rgb(52, 50, 42),     // Warm default border
            Self::System | Self::Dark => Color32::from_rgb(38, 38, 44),
        }
    }

    /// User message background in chat
    pub fn chat_user_msg_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_elevated,
            Self::Parchment => Color32::from_rgb(240, 236, 228),
            Self::Stockholm => Color32::from_rgb(237, 239, 243), // Elevated blue-gray
            Self::Copenhagen => Color32::from_rgb(238, 236, 230), // Warm elevated
            Self::Light => Color32::from_rgb(234, 235, 238),
            Self::Midnight => Color32::from_rgb(26, 29, 40),
            Self::Ayu => Color32::from_rgb(21, 26, 34),
            Self::Aurora => Color32::from_rgb(33, 38, 45),
            Self::Graphite => Color32::from_rgb(30, 30, 32), // Elevated graphite
            Self::Ink => Color32::from_rgb(22, 22, 28),      // Elevated ink
            Self::Void => Color32::from_rgb(16, 16, 26),     // Elevated void
            Self::Neon => Color32::from_rgb(22, 18, 28),     // Elevated neon
            Self::Onyx => Color32::from_rgb(28, 28, 24),     // Elevated onyx
            Self::System | Self::Dark => Color32::from_rgb(26, 26, 30),
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
            Self::Parchment => Color32::from_rgb(255, 240, 235), // Warm rose-tinted paper
            Self::Stockholm => Color32::from_rgb(255, 238, 238), // Cool rose tint
            Self::Copenhagen => Color32::from_rgb(255, 242, 238), // Warm rose tint
            Self::Light => Color32::from_rgb(255, 240, 238),     // Neutral rose tint
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
            Self::Parchment => Color32::from_rgb(255, 248, 230), // Warm amber-tinted paper
            Self::Stockholm => Color32::from_rgb(255, 248, 235), // Cool amber tint
            Self::Copenhagen => Color32::from_rgb(255, 250, 232), // Warm amber tint
            Self::Light => Color32::from_rgb(255, 250, 235),     // Neutral amber tint
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
            Self::Parchment => Color32::from_rgb(240, 240, 248), // Subtle gray-blue paper
            Self::Stockholm => Color32::from_rgb(235, 242, 252), // Blue-tinted info
            Self::Copenhagen => Color32::from_rgb(240, 245, 250), // Warm blue tint
            Self::Light => Color32::from_rgb(238, 242, 252),     // Neutral blue tint
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
            Self::Parchment => Color32::from_rgb(242, 250, 242), // Subtle sage paper
            Self::Stockholm => Color32::from_rgb(238, 250, 242), // Cool sage tint
            Self::Copenhagen => Color32::from_rgb(238, 250, 240), // Sage tint
            Self::Light => Color32::from_rgb(236, 253, 245),     // Neutral emerald tint
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
            Self::Parchment => [
                Color32::from_rgb(250, 248, 245),
                Color32::from_rgb(235, 230, 220),
                Color32::from_rgb(210, 200, 185),
                Color32::from_rgb(170, 165, 155),
                Color32::from_rgb(130, 125, 115),
                Color32::from_rgb(90, 85, 80),
                accent,
                accent_hover,
            ],
            Self::Stockholm => [
                Color32::from_rgb(250, 251, 252),
                Color32::from_rgb(230, 235, 242),
                Color32::from_rgb(200, 210, 225),
                Color32::from_rgb(165, 180, 200),
                Color32::from_rgb(125, 145, 175),
                Color32::from_rgb(90, 110, 145),
                accent,
                accent_hover,
            ],
            Self::Copenhagen => [
                Color32::from_rgb(250, 250, 248),
                Color32::from_rgb(235, 238, 230),
                Color32::from_rgb(210, 218, 205),
                Color32::from_rgb(175, 195, 178),
                Color32::from_rgb(140, 165, 145),
                Color32::from_rgb(110, 140, 115),
                accent,
                accent_hover,
            ],
            Self::Light => [
                Color32::from_rgb(251, 251, 252),
                Color32::from_rgb(228, 240, 235),
                Color32::from_rgb(195, 225, 212),
                Color32::from_rgb(155, 205, 185),
                Color32::from_rgb(110, 185, 158),
                Color32::from_rgb(65, 165, 135),
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
            Self::Void => [
                bg,
                Color32::from_rgb(10, 8, 20),
                Color32::from_rgb(22, 15, 45),
                Color32::from_rgb(45, 30, 85),
                Color32::from_rgb(75, 45, 140),
                Color32::from_rgb(100, 55, 190),
                accent,
                accent_hover,
            ],
            Self::Neon => [
                bg,
                Color32::from_rgb(15, 8, 15),
                Color32::from_rgb(35, 15, 30),
                Color32::from_rgb(70, 25, 55),
                Color32::from_rgb(120, 40, 90),
                Color32::from_rgb(175, 55, 130),
                accent,
                accent_hover,
            ],
            Self::Onyx => [
                bg,
                Color32::from_rgb(20, 18, 12),
                Color32::from_rgb(35, 30, 18),
                Color32::from_rgb(70, 58, 30),
                Color32::from_rgb(120, 100, 40),
                Color32::from_rgb(170, 140, 48),
                accent,
                accent_hover,
            ],
            Self::System | Self::Dark => [
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
            Self::System | Self::Dark => [
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
            Self::Void => [
                Color32::from_rgb(124, 58, 237),  // Electric violet (accent) #7C3AED
                Color32::from_rgb(167, 139, 250), // Bright violet #A78BFA
                Color32::from_rgb(56, 189, 248),  // Sky blue #38BDF8
                Color32::from_rgb(244, 114, 182), // Pink #F472B6
                Color32::from_rgb(52, 211, 153),  // Green #34D399
                Color32::from_rgb(251, 191, 36),  // Amber #FBBF24
                Color32::from_rgb(248, 113, 113), // Red #F87171
                Color32::from_rgb(129, 140, 248), // Indigo #818CF8
            ],
            Self::Neon => [
                Color32::from_rgb(224, 64, 160),  // Hot magenta (accent) #E040A0
                Color32::from_rgb(168, 85, 247),  // Purple #A855F7
                Color32::from_rgb(56, 189, 248),  // Sky blue #38BDF8
                Color32::from_rgb(52, 211, 153),  // Green #34D399
                Color32::from_rgb(251, 191, 36),  // Amber #FBBF24
                Color32::from_rgb(248, 113, 113), // Red #F87171
                Color32::from_rgb(255, 110, 199), // Bright magenta #FF6EC7
                Color32::from_rgb(129, 140, 248), // Indigo #818CF8
            ],
            Self::Onyx => [
                Color32::from_rgb(212, 175, 55),  // Gold (accent) #D4AF37
                Color32::from_rgb(232, 200, 80),  // Bright gold #E8C850
                Color32::from_rgb(184, 152, 112), // Bronze #B89870
                Color32::from_rgb(160, 160, 144), // Warm gray #A0A090
                Color32::from_rgb(200, 168, 96),  // Amber gold #C8A860
                Color32::from_rgb(140, 136, 128), // Stone #8C8880
                Color32::from_rgb(208, 184, 112), // Pale gold #D0B870
                Color32::from_rgb(152, 144, 120), // Sand #989078
            ],

            // === Light Themes ===
            Self::Parchment => [
                Color32::from_rgb(16, 163, 127), // Muted emerald
                Color32::from_rgb(59, 130, 246), // Classic blue
                Color32::from_rgb(139, 92, 246), // Purple
                Color32::from_rgb(245, 158, 11), // Amber
                Color32::from_rgb(236, 72, 153), // Pink
                Color32::from_rgb(20, 184, 166), // Teal
                Color32::from_rgb(239, 68, 68),  // Red
                Color32::from_rgb(99, 102, 241), // Indigo
            ],
            Self::Stockholm => [
                Color32::from_rgb(74, 111, 165), // Steel blue (accent)
                Color32::from_rgb(45, 140, 100), // Nordic green
                Color32::from_rgb(120, 85, 195), // Muted purple
                Color32::from_rgb(200, 140, 30), // Warm amber
                Color32::from_rgb(190, 70, 130), // Rose
                Color32::from_rgb(30, 160, 150), // Teal
                Color32::from_rgb(200, 65, 65),  // Muted red
                Color32::from_rgb(90, 95, 200),  // Indigo
            ],
            Self::Copenhagen => [
                Color32::from_rgb(107, 143, 113), // Sage green (accent)
                Color32::from_rgb(55, 120, 165),  // Warm blue
                Color32::from_rgb(130, 90, 180),  // Warm purple
                Color32::from_rgb(195, 140, 35),  // Warm amber
                Color32::from_rgb(180, 75, 120),  // Warm rose
                Color32::from_rgb(35, 150, 140),  // Warm teal
                Color32::from_rgb(195, 70, 70),   // Warm red
                Color32::from_rgb(100, 100, 190), // Warm indigo
            ],
            Self::Light => [
                Color32::from_rgb(16, 185, 129), // Emerald (accent)
                Color32::from_rgb(59, 130, 246), // Blue
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
            Self::System | Self::Dark => [
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
            Self::Void => [
                Color32::from_rgb(96, 165, 250),  // 0: Scan - Sky blue
                Color32::from_rgb(52, 211, 153),  // 1: Filter - Green
                Color32::from_rgb(251, 146, 60),  // 2: Join - Orange
                Color32::from_rgb(167, 139, 250), // 3: Aggregate - Bright violet
                Color32::from_rgb(248, 113, 113), // 4: Sort - Red
                Color32::from_rgb(56, 189, 248),  // 5: Project - Cyan
                Color32::from_rgb(251, 191, 36),  // 6: Hash - Amber
                Color32::from_rgb(124, 58, 237),  // 7: Remote - Electric violet
                Color32::from_rgb(244, 114, 182), // 8: Union - Pink
                Color32::from_rgb(163, 230, 53),  // 9: Cooperative - Lime
                Color32::from_rgb(184, 160, 232), // 10: Other Exec - Light violet
                Color32::from_rgb(92, 92, 112),   // 11: Reserved - Muted
            ],
            Self::Neon => [
                Color32::from_rgb(96, 165, 250),  // 0: Scan - Sky blue
                Color32::from_rgb(52, 211, 153),  // 1: Filter - Green
                Color32::from_rgb(251, 146, 60),  // 2: Join - Orange
                Color32::from_rgb(168, 85, 247),  // 3: Aggregate - Purple
                Color32::from_rgb(248, 113, 113), // 4: Sort - Red
                Color32::from_rgb(56, 189, 248),  // 5: Project - Cyan
                Color32::from_rgb(251, 191, 36),  // 6: Hash - Amber
                Color32::from_rgb(224, 64, 160),  // 7: Remote - Hot magenta
                Color32::from_rgb(255, 110, 199), // 8: Union - Bright magenta
                Color32::from_rgb(163, 230, 53),  // 9: Cooperative - Lime
                Color32::from_rgb(244, 114, 182), // 10: Other Exec - Pink
                Color32::from_rgb(100, 92, 112),  // 11: Reserved - Muted
            ],
            Self::Onyx => [
                Color32::from_rgb(130, 160, 210), // 0: Scan - Warm blue
                Color32::from_rgb(52, 185, 100),  // 1: Filter - Green
                Color32::from_rgb(232, 180, 60),  // 2: Join - Warm amber
                Color32::from_rgb(184, 152, 112), // 3: Aggregate - Bronze
                Color32::from_rgb(230, 100, 100), // 4: Sort - Soft red
                Color32::from_rgb(160, 180, 200), // 5: Project - Steel blue
                Color32::from_rgb(212, 175, 55),  // 6: Hash - Gold
                Color32::from_rgb(100, 160, 190), // 7: Remote - Teal
                Color32::from_rgb(200, 160, 140), // 8: Union - Warm rose
                Color32::from_rgb(160, 190, 120), // 9: Cooperative - Olive
                Color32::from_rgb(232, 200, 80),  // 10: Other Exec - Bright gold
                Color32::from_rgb(100, 98, 88),   // 11: Reserved - Warm gray
            ],

            // === Light Themes ===
            Self::Parchment => [
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
            Self::Stockholm => [
                Color32::from_rgb(50, 90, 150),   // 0: Scan - Deep blue
                Color32::from_rgb(30, 130, 70),   // 1: Filter - Green
                Color32::from_rgb(210, 100, 25),  // 2: Join - Warm orange
                Color32::from_rgb(120, 60, 200),  // 3: Aggregate - Purple
                Color32::from_rgb(190, 45, 45),   // 4: Sort - Red
                Color32::from_rgb(25, 160, 145),  // 5: Project - Teal
                Color32::from_rgb(180, 125, 10),  // 6: Hash - Gold
                Color32::from_rgb(15, 155, 185),  // 7: Remote - Cyan
                Color32::from_rgb(190, 50, 110),  // 8: Union - Pink
                Color32::from_rgb(110, 175, 30),  // 9: Cooperative - Lime
                Color32::from_rgb(210, 140, 20),  // 10: Other Exec - Amber
                Color32::from_rgb(108, 118, 132), // 11: Reserved - Gray
            ],
            Self::Copenhagen => [
                Color32::from_rgb(55, 95, 145),   // 0: Scan - Warm blue
                Color32::from_rgb(40, 130, 65),   // 1: Filter - Green
                Color32::from_rgb(200, 105, 30),  // 2: Join - Warm orange
                Color32::from_rgb(115, 65, 190),  // 3: Aggregate - Purple
                Color32::from_rgb(185, 50, 50),   // 4: Sort - Red
                Color32::from_rgb(30, 155, 140),  // 5: Project - Teal
                Color32::from_rgb(175, 130, 15),  // 6: Hash - Gold
                Color32::from_rgb(20, 145, 175),  // 7: Remote - Cyan
                Color32::from_rgb(180, 55, 105),  // 8: Union - Pink
                Color32::from_rgb(115, 170, 35),  // 9: Cooperative - Lime
                Color32::from_rgb(205, 145, 25),  // 10: Other Exec - Amber
                Color32::from_rgb(118, 114, 106), // 11: Reserved - Warm gray
            ],
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
            Self::System | Self::Dark => [
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
            Self::Void => [
                Color32::from_rgb(248, 113, 113), // Red - Coral red
                Color32::from_rgb(52, 211, 153),  // Green - Green
                Color32::from_rgb(251, 191, 36),  // Yellow - Amber
                Color32::from_rgb(96, 165, 250),  // Blue - Sky blue
                Color32::from_rgb(124, 58, 237),  // Magenta - Electric violet (accent)
                Color32::from_rgb(167, 139, 250), // Cyan - Bright violet
            ],
            Self::Neon => [
                Color32::from_rgb(248, 113, 113), // Red - Coral red
                Color32::from_rgb(52, 211, 153),  // Green - Green
                Color32::from_rgb(251, 191, 36),  // Yellow - Amber
                Color32::from_rgb(96, 165, 250),  // Blue - Sky blue
                Color32::from_rgb(224, 64, 160),  // Magenta - Hot magenta (accent)
                Color32::from_rgb(255, 110, 199), // Cyan - Bright magenta
            ],
            Self::Onyx => [
                Color32::from_rgb(230, 100, 100), // Red - Soft red
                Color32::from_rgb(52, 185, 100),  // Green - Green
                Color32::from_rgb(232, 180, 60),  // Yellow - Warm amber
                Color32::from_rgb(130, 160, 210), // Blue - Warm blue
                Color32::from_rgb(212, 175, 55),  // Magenta - Gold (accent)
                Color32::from_rgb(232, 200, 80),  // Cyan - Bright gold
            ],

            // === Light Themes ===
            Self::Parchment => [
                Color32::from_rgb(185, 28, 28),  // Red - Deep ink red
                Color32::from_rgb(21, 128, 61),  // Green - Forest green
                Color32::from_rgb(161, 98, 7),   // Yellow - Amber-brown
                Color32::from_rgb(29, 78, 216),  // Blue - Classic blue
                Color32::from_rgb(126, 34, 206), // Magenta - Purple
                Color32::from_rgb(14, 116, 144), // Cyan - Teal
            ],
            Self::Stockholm => [
                Color32::from_rgb(175, 35, 35),  // Red - Nordic red
                Color32::from_rgb(25, 120, 60),  // Green - Forest green
                Color32::from_rgb(155, 100, 10), // Yellow - Warm amber
                Color32::from_rgb(50, 90, 180),  // Blue - Steel blue
                Color32::from_rgb(115, 40, 190), // Magenta - Cool purple
                Color32::from_rgb(20, 120, 140), // Cyan - Nordic teal
            ],
            Self::Copenhagen => [
                Color32::from_rgb(170, 40, 40),  // Red - Warm red
                Color32::from_rgb(30, 115, 55),  // Green - Forest green
                Color32::from_rgb(160, 105, 15), // Yellow - Warm amber
                Color32::from_rgb(45, 85, 170),  // Blue - Warm blue
                Color32::from_rgb(110, 45, 180), // Magenta - Warm purple
                Color32::from_rgb(25, 125, 135), // Cyan - Warm teal
            ],
            Self::Light => [
                Color32::from_rgb(220, 38, 38),  // Red
                Color32::from_rgb(22, 163, 74),  // Green
                Color32::from_rgb(202, 138, 4),  // Yellow
                Color32::from_rgb(37, 99, 235),  // Blue
                Color32::from_rgb(147, 51, 234), // Magenta
                Color32::from_rgb(14, 116, 144), // Cyan
            ],
        }
    }

    /// Commit marker color for git annotations on charts
    pub fn chart_commit_marker(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_primary,
            // Dark themes - vibrant markers
            Self::System | Self::Dark => Color32::from_rgb(180, 155, 255), // Violet
            Self::Midnight => Color32::from_rgb(192, 132, 252),            // Neon purple
            Self::Ayu => Color32::from_rgb(172, 128, 255),                 // Purple
            Self::Aurora => Color32::from_rgb(180, 150, 180),              // Soft purple
            Self::Graphite => Color32::from_rgb(180, 140, 120),            // Copper
            Self::Ink => Color32::from_rgb(160, 160, 170),                 // Silver
            Self::Void => Color32::from_rgb(167, 139, 250),                // Bright violet
            Self::Neon => Color32::from_rgb(168, 85, 247),                 // Purple
            Self::Onyx => Color32::from_rgb(184, 152, 112),                // Bronze

            // Light themes - muted markers
            Self::Parchment => Color32::from_rgb(139, 92, 246), // Purple
            Self::Stockholm => Color32::from_rgb(120, 85, 195), // Muted purple
            Self::Copenhagen => Color32::from_rgb(130, 90, 180), // Warm purple
            Self::Light => Color32::from_rgb(139, 92, 246),     // Purple
        }
    }

    // =========================================================================
    // Annotation Colors
    // =========================================================================

    /// Normal priority annotation color (notes/comments)
    pub fn annotation_normal(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Parchment => Color32::from_rgb(59, 130, 246),
            Self::Stockholm => Color32::from_rgb(74, 111, 165), // Steel blue
            Self::Copenhagen => Color32::from_rgb(55, 95, 145), // Warm blue
            Self::Light => Color32::from_rgb(37, 99, 235),      // Blue
            Self::Midnight => Color32::from_rgb(96, 165, 250),
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Aurora => Color32::from_rgb(139, 198, 198),
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),   // Pure silver
            Self::Void => Color32::from_rgb(96, 165, 250),   // Blue
            Self::Neon => Color32::from_rgb(96, 165, 250),   // Blue
            Self::Onyx => Color32::from_rgb(130, 160, 210),  // Warm blue
            Self::System | Self::Dark => Color32::from_rgb(100, 149, 237),
        }
    }

    /// Important priority annotation color (highlighted)
    pub fn annotation_important(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.warning,
            Self::Parchment => Color32::from_rgb(245, 158, 11),
            Self::Stockholm => Color32::from_rgb(200, 140, 30), // Warm amber
            Self::Copenhagen => Color32::from_rgb(195, 140, 35), // Warm amber
            Self::Light => Color32::from_rgb(245, 158, 11),     // Amber
            Self::Midnight => Color32::from_rgb(251, 191, 36),
            Self::Ayu => Color32::from_rgb(255, 180, 84),
            Self::Aurora => Color32::from_rgb(255, 200, 120),
            Self::Graphite => Color32::from_rgb(255, 180, 80), // Warm orange
            Self::Ink => Color32::from_rgb(220, 200, 140),     // Muted gold
            Self::Void => Color32::from_rgb(251, 191, 36),     // Amber
            Self::Neon => Color32::from_rgb(251, 191, 36),     // Amber
            Self::Onyx => Color32::from_rgb(232, 180, 60),     // Warm amber
            Self::System | Self::Dark => Color32::from_rgb(255, 165, 0),
        }
    }

    /// Critical priority annotation color (alert-style)
    pub fn annotation_critical(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Parchment => Color32::from_rgb(220, 38, 38),
            Self::Stockholm => Color32::from_rgb(190, 40, 40), // Nordic red
            Self::Copenhagen => Color32::from_rgb(185, 45, 45), // Warm red
            Self::Light => Color32::from_rgb(220, 38, 38),     // Red
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Soft red
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Muted rose
            Self::Void => Color32::from_rgb(248, 113, 113),     // Red
            Self::Neon => Color32::from_rgb(248, 113, 113),     // Red
            Self::Onyx => Color32::from_rgb(230, 100, 100),     // Soft red
            Self::System | Self::Dark => Color32::from_rgb(220, 53, 69),
        }
    }

    /// Resolved annotation color (dimmed/inactive)
    pub fn annotation_resolved(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Parchment => Color32::from_rgb(156, 163, 175),
            Self::Stockholm => Color32::from_rgb(140, 150, 165), // Muted blue-gray
            Self::Copenhagen => Color32::from_rgb(148, 144, 135), // Warm gray
            Self::Light => Color32::from_rgb(148, 152, 163),     // Gray
            Self::Midnight => Color32::from_rgb(113, 113, 122),
            Self::Ayu => Color32::from_rgb(90, 100, 110),
            Self::Aurora => Color32::from_rgb(110, 118, 129),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Tertiary text #606070
            Self::Void => Color32::from_rgb(92, 92, 112),       // Deep muted
            Self::Neon => Color32::from_rgb(100, 92, 112),      // Deep muted
            Self::Onyx => Color32::from_rgb(100, 98, 88),       // Dark warm gray
            Self::System | Self::Dark => Color32::GRAY,
        }
    }

    // =========================================================================
    // Diff Colors (Git diff visualization)
    // =========================================================================

    /// Addition line background - subtle tint spanning full line
    pub fn diff_added_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success.gamma_multiply(0.15),
            Self::Parchment => Color32::from_rgb(230, 255, 237),
            Self::Stockholm => Color32::from_rgb(232, 248, 238), // Cool green tint
            Self::Copenhagen => Color32::from_rgb(232, 250, 238), // Warm green tint
            Self::Light => Color32::from_rgb(230, 255, 237),     // Green tint
            Self::Midnight => Color32::from_rgb(18, 35, 30),
            Self::Ayu => Color32::from_rgb(22, 35, 25),
            Self::Aurora => Color32::from_rgb(20, 40, 35),
            Self::Graphite => Color32::from_rgb(22, 35, 25), // Added bg graphite
            Self::Ink => Color32::from_rgb(20, 30, 28),      // Added bg ink
            Self::Void => Color32::from_rgb(15, 30, 28),     // Added bg void
            Self::Neon => Color32::from_rgb(15, 30, 28),     // Added bg neon
            Self::Onyx => Color32::from_rgb(18, 30, 20),     // Added bg onyx
            Self::System | Self::Dark => Color32::from_rgb(19, 35, 26),
        }
    }

    /// Deletion line background - subtle tint spanning full line
    pub fn diff_removed_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error.gamma_multiply(0.15),
            Self::Parchment => Color32::from_rgb(255, 235, 235),
            Self::Stockholm => Color32::from_rgb(252, 236, 238), // Cool red tint
            Self::Copenhagen => Color32::from_rgb(252, 238, 236), // Warm red tint
            Self::Light => Color32::from_rgb(255, 235, 235),     // Red tint
            Self::Midnight => Color32::from_rgb(40, 22, 28),
            Self::Ayu => Color32::from_rgb(40, 25, 25),
            Self::Aurora => Color32::from_rgb(40, 25, 28),
            Self::Graphite => Color32::from_rgb(40, 25, 25), // Removed bg graphite
            Self::Ink => Color32::from_rgb(35, 22, 28),      // Removed bg ink
            Self::Void => Color32::from_rgb(35, 18, 22),     // Removed bg void
            Self::Neon => Color32::from_rgb(35, 18, 22),     // Removed bg neon
            Self::Onyx => Color32::from_rgb(35, 20, 18),     // Removed bg onyx
            Self::System | Self::Dark => Color32::from_rgb(40, 22, 24),
        }
    }

    /// Word-level addition highlight - prominent for inline changes
    pub fn diff_added_word_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success.gamma_multiply(0.45),
            Self::Parchment => Color32::from_rgb(150, 235, 170),
            Self::Stockholm => Color32::from_rgb(160, 228, 185), // Cool green word
            Self::Copenhagen => Color32::from_rgb(165, 230, 185), // Warm green word
            Self::Light => Color32::from_rgb(150, 235, 170),     // Green word
            Self::Midnight => Color32::from_rgb(38, 95, 70),
            Self::Ayu => Color32::from_rgb(48, 95, 58),
            Self::Aurora => Color32::from_rgb(42, 105, 82),
            Self::Graphite => Color32::from_rgb(52, 95, 58), // Added word graphite
            Self::Ink => Color32::from_rgb(42, 82, 65),      // Added word ink
            Self::Void => Color32::from_rgb(32, 82, 62),     // Added word void
            Self::Neon => Color32::from_rgb(32, 82, 62),     // Added word neon
            Self::Onyx => Color32::from_rgb(38, 75, 48),     // Added word onyx
            Self::System | Self::Dark => Color32::from_rgb(42, 95, 65),
        }
    }

    /// Word-level deletion highlight - prominent for inline changes
    pub fn diff_removed_word_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error.gamma_multiply(0.45),
            Self::Parchment => Color32::from_rgb(255, 178, 178),
            Self::Stockholm => Color32::from_rgb(245, 185, 190), // Cool red word
            Self::Copenhagen => Color32::from_rgb(248, 188, 185), // Warm red word
            Self::Light => Color32::from_rgb(255, 178, 178),     // Red word
            Self::Midnight => Color32::from_rgb(108, 48, 55),
            Self::Ayu => Color32::from_rgb(112, 52, 52),
            Self::Aurora => Color32::from_rgb(115, 52, 58),
            Self::Graphite => Color32::from_rgb(112, 52, 52), // Removed word graphite
            Self::Ink => Color32::from_rgb(92, 48, 58),       // Removed word ink
            Self::Void => Color32::from_rgb(92, 42, 48),      // Removed word void
            Self::Neon => Color32::from_rgb(92, 42, 48),      // Removed word neon
            Self::Onyx => Color32::from_rgb(92, 45, 42),      // Removed word onyx
            Self::System | Self::Dark => Color32::from_rgb(100, 42, 46),
        }
    }

    /// Addition text color - high contrast for readability
    pub fn diff_added_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Parchment => Color32::from_rgb(36, 138, 61),
            Self::Stockholm => Color32::from_rgb(30, 120, 60), // Nordic green text
            Self::Copenhagen => Color32::from_rgb(35, 125, 55), // Forest green text
            Self::Light => Color32::from_rgb(22, 163, 74),     // Green text
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Aurora => Color32::from_rgb(126, 232, 184),
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Added text graphite
            Self::Ink => Color32::from_rgb(130, 180, 150),      // Added text ink
            Self::Void => Color32::from_rgb(52, 211, 153),      // Added text void
            Self::Neon => Color32::from_rgb(52, 211, 153),      // Added text neon
            Self::Onyx => Color32::from_rgb(52, 185, 100),      // Added text onyx
            Self::System | Self::Dark => Color32::from_rgb(126, 231, 135),
        }
    }

    /// Deletion text color - high contrast for readability
    pub fn diff_removed_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Parchment => Color32::from_rgb(207, 34, 46),
            Self::Stockholm => Color32::from_rgb(190, 40, 50), // Nordic red text
            Self::Copenhagen => Color32::from_rgb(185, 40, 50), // Warm red text
            Self::Light => Color32::from_rgb(220, 38, 38),     // Red text
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Removed text graphite
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Removed text ink
            Self::Void => Color32::from_rgb(248, 113, 113),     // Removed text void
            Self::Neon => Color32::from_rgb(248, 113, 113),     // Removed text neon
            Self::Onyx => Color32::from_rgb(230, 100, 100),     // Removed text onyx
            Self::System | Self::Dark => Color32::from_rgb(255, 123, 114),
        }
    }

    /// Context line text color - dimmed for less visual weight
    pub fn diff_context_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Parchment => Color32::from_rgb(87, 96, 106),
            Self::Stockholm => Color32::from_rgb(108, 118, 132), // Blue-gray context
            Self::Copenhagen => Color32::from_rgb(118, 114, 106), // Warm gray context
            Self::Light => Color32::from_rgb(112, 118, 130),     // Gray
            Self::Midnight => Color32::from_rgb(113, 113, 122),
            Self::Ayu => Color32::from_rgb(90, 100, 110),
            Self::Aurora => Color32::from_rgb(110, 118, 129),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Context text graphite
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Context text ink
            Self::Void => Color32::from_rgb(92, 92, 112),       // Context text void
            Self::Neon => Color32::from_rgb(100, 92, 112),      // Context text neon
            Self::Onyx => Color32::from_rgb(100, 98, 88),       // Context text onyx
            Self::System | Self::Dark => Color32::from_rgb(145, 152, 161),
        }
    }

    /// Addition gutter stripe color
    pub fn diff_added_gutter(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.success,
            Self::Parchment => Color32::from_rgb(52, 168, 83),
            Self::Stockholm => Color32::from_rgb(40, 150, 75), // Nordic green gutter
            Self::Copenhagen => Color32::from_rgb(45, 155, 75), // Green gutter
            Self::Light => Color32::from_rgb(22, 163, 74),     // Green
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Aurora => Color32::from_rgb(126, 232, 184),
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Added gutter graphite
            Self::Ink => Color32::from_rgb(130, 180, 150),      // Added gutter ink
            Self::Void => Color32::from_rgb(52, 211, 153),      // Added gutter void
            Self::Neon => Color32::from_rgb(52, 211, 153),      // Added gutter neon
            Self::Onyx => Color32::from_rgb(52, 185, 100),      // Added gutter onyx
            Self::System | Self::Dark => Color32::from_rgb(63, 185, 80),
        }
    }

    /// Deletion gutter stripe color
    pub fn diff_removed_gutter(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.error,
            Self::Parchment => Color32::from_rgb(234, 67, 53),
            Self::Stockholm => Color32::from_rgb(200, 55, 55), // Nordic red gutter
            Self::Copenhagen => Color32::from_rgb(210, 60, 55), // Red gutter
            Self::Light => Color32::from_rgb(220, 38, 38),     // Red
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Removed gutter graphite
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Removed gutter ink
            Self::Void => Color32::from_rgb(248, 113, 113),     // Removed gutter void
            Self::Neon => Color32::from_rgb(248, 113, 113),     // Removed gutter neon
            Self::Onyx => Color32::from_rgb(230, 100, 100),     // Removed gutter onyx
            Self::System | Self::Dark => Color32::from_rgb(248, 81, 73),
        }
    }

    /// Line number text color
    pub fn diff_line_number(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_muted,
            Self::Parchment => Color32::from_rgb(140, 150, 160),
            Self::Stockholm => Color32::from_rgb(140, 150, 168), // Blue-gray line numbers
            Self::Copenhagen => Color32::from_rgb(148, 144, 135), // Warm gray line numbers
            Self::Light => Color32::from_rgb(148, 152, 163),     // Gray
            Self::Midnight => Color32::from_rgb(70, 80, 100),
            Self::Ayu => Color32::from_rgb(60, 70, 80),
            Self::Aurora => Color32::from_rgb(70, 78, 88),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Line number graphite
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Line number ink
            Self::Void => Color32::from_rgb(85, 85, 104),       // Line number void
            Self::Neon => Color32::from_rgb(85, 80, 96),        // Line number neon
            Self::Onyx => Color32::from_rgb(88, 85, 72),        // Line number onyx
            Self::System | Self::Dark => Color32::from_rgb(72, 79, 88),
        }
    }

    /// Line number background color
    pub fn diff_line_number_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Parchment => Color32::from_rgb(246, 248, 250),
            Self::Stockholm => Color32::from_rgb(246, 248, 252), // Cool line number bg
            Self::Copenhagen => Color32::from_rgb(248, 248, 245), // Warm line number bg
            Self::Light => Color32::from_rgb(246, 247, 249),     // Light bg
            Self::Midnight => Color32::from_rgb(12, 14, 20),
            Self::Ayu => Color32::from_rgb(8, 11, 16),
            Self::Aurora => Color32::from_rgb(10, 14, 18),
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Line number bg graphite
            Self::Ink => Color32::from_rgb(8, 8, 12),        // Line number bg ink
            Self::Void => Color32::from_rgb(4, 4, 8),        // Line number bg void
            Self::Neon => Color32::from_rgb(3, 3, 6),        // Line number bg neon
            Self::Onyx => Color32::from_rgb(8, 8, 6),        // Line number bg onyx
            Self::System | Self::Dark => Color32::from_rgb(13, 17, 23),
        }
    }

    /// Hunk header background
    pub fn diff_hunk_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.accent_muted,
            Self::Parchment => Color32::from_rgb(240, 245, 255),
            Self::Stockholm => Color32::from_rgb(236, 241, 248), // Blue hunk bg
            Self::Copenhagen => Color32::from_rgb(238, 245, 240), // Sage hunk bg
            Self::Light => Color32::from_rgb(236, 253, 245),     // Emerald tint
            Self::Midnight => Color32::from_rgb(20, 30, 55),
            Self::Ayu => Color32::from_rgb(20, 25, 35),
            Self::Aurora => Color32::from_rgb(22, 32, 38),
            Self::Graphite => Color32::from_rgb(30, 25, 20), // Hunk bg graphite
            Self::Ink => Color32::from_rgb(20, 20, 30),      // Hunk bg ink
            Self::Void => Color32::from_rgb(22, 15, 45),     // Hunk bg void
            Self::Neon => Color32::from_rgb(35, 15, 30),     // Hunk bg neon
            Self::Onyx => Color32::from_rgb(30, 28, 18),     // Hunk bg onyx
            Self::System | Self::Dark => Color32::from_rgb(22, 27, 46),
        }
    }

    /// Hunk header text color
    pub fn diff_hunk_text(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.info,
            Self::Parchment => Color32::from_rgb(47, 93, 158),
            Self::Stockholm => Color32::from_rgb(56, 92, 145), // Steel blue hunk text
            Self::Copenhagen => Color32::from_rgb(60, 100, 65), // Dark sage hunk text
            Self::Light => Color32::from_rgb(4, 120, 87),      // Dark emerald
            Self::Midnight => Color32::from_rgb(96, 165, 250),
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Aurora => Color32::from_rgb(139, 198, 198),
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Hunk text graphite
            Self::Ink => Color32::from_rgb(192, 192, 200),   // Hunk text ink
            Self::Void => Color32::from_rgb(124, 58, 237),   // Hunk text void
            Self::Neon => Color32::from_rgb(224, 64, 160),   // Hunk text neon
            Self::Onyx => Color32::from_rgb(212, 175, 55),   // Hunk text onyx
            Self::System | Self::Dark => Color32::from_rgb(121, 184, 255),
        }
    }

    /// File header text color
    pub fn diff_file_header(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.text_primary,
            Self::Parchment => Color32::from_rgb(36, 41, 47),
            Self::Stockholm => Color32::from_rgb(28, 32, 38), // Cool near-black header
            Self::Copenhagen => Color32::from_rgb(32, 30, 28), // Warm near-black header
            Self::Light => Color32::from_rgb(17, 19, 24),     // Near-black
            Self::Midnight => Color32::from_rgb(228, 228, 231),
            Self::Ayu => Color32::from_rgb(191, 189, 182),
            Self::Aurora => Color32::from_rgb(230, 237, 243),
            Self::Graphite => Color32::from_rgb(232, 230, 224), // File header graphite
            Self::Ink => Color32::from_rgb(228, 228, 236),      // File header ink
            Self::Void => Color32::from_rgb(232, 232, 240),     // File header void
            Self::Neon => Color32::from_rgb(232, 232, 240),     // File header neon
            Self::Onyx => Color32::from_rgb(220, 216, 204),     // File header onyx
            Self::System | Self::Dark => Color32::from_rgb(201, 209, 217),
        }
    }

    /// File header background color
    pub fn diff_file_header_bg(&self) -> Color32 {
        match self {
            Self::Custom(colors) => colors.bg_surface,
            Self::Parchment => Color32::from_rgb(246, 248, 250),
            Self::Stockholm => Color32::from_rgb(244, 245, 248), // Cool file header bg
            Self::Copenhagen => Color32::from_rgb(245, 244, 240), // Warm surface
            Self::Light => Color32::from_rgb(242, 243, 245),     // Surface
            Self::Midnight => Color32::from_rgb(16, 18, 26),
            Self::Ayu => Color32::from_rgb(12, 16, 22),
            Self::Aurora => Color32::from_rgb(18, 22, 28),
            Self::Graphite => Color32::from_rgb(22, 22, 24), // File header bg graphite
            Self::Ink => Color32::from_rgb(14, 14, 20),      // File header bg ink
            Self::Void => Color32::from_rgb(8, 8, 14),       // File header bg void
            Self::Neon => Color32::from_rgb(12, 10, 16),     // File header bg neon
            Self::Onyx => Color32::from_rgb(18, 18, 16),     // File header bg onyx
            Self::System | Self::Dark => Color32::from_rgb(22, 27, 34),
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
