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

/// Application theme presets
///
/// Each theme is a complete color scheme including backgrounds, accents, and UI colors.
/// The default theme is Dark (Obsidian Glass with Enya Emerald accent).
#[derive(
    Clone,
    Copy,
    Eq,
    PartialEq,
    PartialOrd,
    Hash,
    Default,
    Debug,
    serde::Deserialize,
    serde::Serialize,
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
    /// Nord theme - Arctic blue #88C0D0
    Nord,
    /// Catppuccin Mocha theme - Warm pastel dark with mauve accent #CBA6F7
    Catppuccin,
    /// Ayu Dark theme - Soft amber warmth with orange accent #FFB454
    Ayu,
    /// Bergman theme - Swedish foggy noir with steel silver accent #A8B0C0
    Bergman,
    /// Aurora theme - Northern Lights with aurora teal accent #7EE8B8
    Aurora,
    /// Stockholm theme - Clean Nordic white with slate blue accent #5C7A99
    Stockholm,
    /// Graphite theme - Industrial precision with molten orange accent #E85D04
    Graphite,
    /// Ink theme - Monochrome editorial with pure silver accent #C0C0C8
    Ink,
    /// Midsommar theme - Swedish summer with flag blue accent #2563EB
    Midsommar,
    /// Skärgård theme - Stockholm archipelago with Baltic blue accent #1E4D6B
    Skargard,
}

impl AppTheme {
    /// Returns the display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Midnight => "Midnight",
            Self::Nord => "Nord",
            Self::Catppuccin => "Catppuccin",
            Self::Ayu => "Ayu",
            Self::Bergman => "Bergman",
            Self::Aurora => "Aurora",
            Self::Stockholm => "Stockholm",
            Self::Graphite => "Graphite",
            Self::Ink => "Ink",
            Self::Midsommar => "Midsommar",
            Self::Skargard => "Skärgård",
        }
    }

    /// Returns all available themes
    pub fn all() -> &'static [AppTheme] {
        &[
            Self::Dark,
            Self::Light,
            Self::Midnight,
            Self::Nord,
            Self::Catppuccin,
            Self::Ayu,
            Self::Bergman,
            Self::Aurora,
            Self::Stockholm,
            Self::Graphite,
            Self::Ink,
            Self::Midsommar,
            Self::Skargard,
        ]
    }

    /// Returns true if this is a dark theme
    pub fn is_dark(&self) -> bool {
        !matches!(
            self,
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard
        )
    }

    /// Returns true if this is a light theme
    pub fn is_light(&self) -> bool {
        matches!(
            self,
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard
        )
    }

    /// Cycle to the next theme
    pub fn next(&mut self) {
        let themes = Self::all();
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
            "nord" | "n" => Some(Self::Nord),
            "catppuccin" | "cat" | "mocha" | "c" => Some(Self::Catppuccin),
            "ayu" | "a" | "amber" => Some(Self::Ayu),
            "bergman" | "b" | "noir" | "fog" | "foggy" | "seventh-seal" => Some(Self::Bergman),
            "aurora" | "ar" | "northern" | "lights" | "borealis" => Some(Self::Aurora),
            "stockholm" | "sthlm" | "sto" | "ikea" | "nordic-white" => Some(Self::Stockholm),
            "graphite" | "graph" | "industrial" | "foundry" | "molten" => Some(Self::Graphite),
            "ink" | "i" | "editorial" | "monochrome" | "silver" => Some(Self::Ink),
            "midsommar" | "mid" | "summer" | "swedish-summer" | "flagblue" => Some(Self::Midsommar),
            "skargard" | "sk" | "archipelago" | "baltic" | "coastal" => Some(Self::Skargard),
            _ => None,
        }
    }

    /// Get the egui Visuals for this theme
    pub fn visuals(&self) -> Visuals {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
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
            Self::Light => Color32::from_rgb(250, 248, 245), // Warm cream paper #FAF8F5
            Self::Nord => Color32::from_rgb(46, 52, 64),     // Nord polar night
            Self::Midnight => Color32::from_rgb(10, 11, 16), // Deep space blue #0A0B10
            Self::Catppuccin => Color32::from_rgb(30, 30, 46), // Mocha base #1E1E2E
            Self::Ayu => Color32::from_rgb(10, 14, 20),      // Deep charcoal #0A0E14
            Self::Bergman => Color32::from_rgb(18, 20, 26),  // Foggy charcoal #12141A
            Self::Aurora => Color32::from_rgb(13, 17, 23),   // Deep night sky #0D1117
            Self::Stockholm => Color32::from_rgb(250, 250, 248), // Warm off-white #FAFAF8
            Self::Graphite => Color32::from_rgb(18, 18, 20), // Deep warm charcoal #121214
            Self::Ink => Color32::from_rgb(10, 10, 15),      // Blue-black #0A0A0F
            Self::Midsommar => Color32::from_rgb(254, 254, 245), // Bright summer white #FEFEF5
            Self::Skargard => Color32::from_rgb(248, 251, 252), // Sea mist white #F8FBFC
            Self::Dark => Color32::from_rgb(8, 8, 10),       // Obsidian dark
        }
    }

    /// Surface/panel background color
    pub fn bg_surface(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Nord => Color32::from_rgb(59, 66, 82),
            Self::Midnight => Color32::from_rgb(18, 20, 28), // Deep navy #12141C
            Self::Catppuccin => Color32::from_rgb(49, 50, 68), // Surface0 #313244
            Self::Ayu => Color32::from_rgb(13, 16, 23),      // Dark blue-gray #0D1017
            Self::Bergman => Color32::from_rgb(28, 30, 38),  // Slate fog #1C1E26
            Self::Aurora => Color32::from_rgb(22, 27, 34),   // Night surface #161B22
            Self::Stockholm => Color32::from_rgb(245, 245, 243), // Warm surface
            Self::Graphite => Color32::from_rgb(26, 26, 28), // Surface #1A1A1C
            Self::Ink => Color32::from_rgb(18, 18, 24),      // Surface #121218
            Self::Midsommar => Color32::from_rgb(250, 250, 240), // Summer surface #FAFAF0
            Self::Skargard => Color32::from_rgb(242, 246, 248), // Sea surface #F2F6F8
            Self::Dark => Color32::from_rgb(18, 18, 21),
        }
    }

    /// Elevated elements (cards, dropdowns)
    pub fn bg_elevated(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(240, 236, 230), // Aged paper #F0ECE6
            Self::Nord => Color32::from_rgb(67, 76, 94),
            Self::Midnight => Color32::from_rgb(26, 29, 40), // Lighter navy #1A1D28
            Self::Catppuccin => Color32::from_rgb(69, 71, 90), // Surface1 #45475A
            Self::Ayu => Color32::from_rgb(21, 26, 34),      // Slightly lighter #151A22
            Self::Bergman => Color32::from_rgb(38, 40, 48),  // Elevated fog #262830
            Self::Aurora => Color32::from_rgb(33, 38, 45),   // Elevated night #21262D
            Self::Stockholm => Color32::from_rgb(240, 240, 238), // Elevated warm white
            Self::Graphite => Color32::from_rgb(36, 36, 38), // Elevated #242426
            Self::Ink => Color32::from_rgb(28, 28, 36),      // Elevated #1C1C24
            Self::Midsommar => Color32::from_rgb(245, 245, 234), // Summer elevated #F5F5EA
            Self::Skargard => Color32::from_rgb(236, 241, 244), // Sea elevated #ECF1F4
            Self::Dark => Color32::from_rgb(26, 26, 30),
        }
    }

    /// Hover state background
    pub fn bg_hover(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(232, 228, 220), // Darker paper #E8E4DC
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Midnight => Color32::from_rgb(34, 38, 52), // Hover navy #222634
            Self::Catppuccin => Color32::from_rgb(88, 91, 112), // Surface2 #585B70
            Self::Ayu => Color32::from_rgb(28, 34, 44),      // Hover charcoal #1C222C
            Self::Bergman => Color32::from_rgb(48, 52, 62),  // Hover fog #30343E
            Self::Aurora => Color32::from_rgb(40, 46, 56),   // Hover night #282E38
            Self::Stockholm => Color32::from_rgb(230, 230, 226), // Hover warm white
            Self::Graphite => Color32::from_rgb(46, 46, 50), // Hover #2E2E32
            Self::Ink => Color32::from_rgb(38, 38, 46),      // Hover #26262E
            Self::Midsommar => Color32::from_rgb(239, 239, 228), // Summer hover #EFEFE4
            Self::Skargard => Color32::from_rgb(229, 234, 238), // Sea hover #E5EAEE
            Self::Dark => Color32::from_rgb(36, 36, 40),
        }
    }

    /// Selected item background
    pub fn bg_selected(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(225, 220, 210), // Selected paper #E1DCD2
            Self::Nord => Color32::from_rgb(30, 50, 60),     // Blue tint
            Self::Midnight => Color32::from_rgb(25, 40, 65), // Blue selection #192841
            Self::Catppuccin => Color32::from_rgb(50, 45, 70), // Mauve tint selection
            Self::Ayu => Color32::from_rgb(40, 35, 25),      // Amber tint selection
            Self::Bergman => Color32::from_rgb(40, 48, 60),  // Steel tint selection
            Self::Aurora => Color32::from_rgb(25, 50, 45),   // Teal tint selection
            Self::Stockholm => Color32::from_rgb(210, 220, 230), // Slate blue tint selection
            Self::Graphite => Color32::from_rgb(58, 42, 32), // Orange tint selection #3A2A20
            Self::Ink => Color32::from_rgb(32, 32, 42),      // Silver tint selection #20202A
            Self::Midsommar => Color32::from_rgb(224, 232, 245), // Blue tint selection #E0E8F5
            Self::Skargard => Color32::from_rgb(224, 234, 240), // Blue tint selection #E0EAF0
            Self::Dark => Color32::from_rgb(28, 42, 36),     // Emerald tint
        }
    }

    /// Card background (slightly darker than elevated)
    pub fn bg_card(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Nord => Color32::from_rgb(60, 68, 84),
            Self::Midnight => Color32::from_rgb(20, 22, 32), // Card navy
            Self::Catppuccin => Color32::from_rgb(54, 55, 74), // Between surface0/1
            Self::Ayu => Color32::from_rgb(16, 20, 28),      // Card charcoal
            Self::Bergman => Color32::from_rgb(32, 34, 42),  // Card fog
            Self::Aurora => Color32::from_rgb(27, 32, 40),   // Card night
            Self::Stockholm => Color32::from_rgb(242, 242, 240), // Card warm white
            Self::Graphite => Color32::from_rgb(30, 30, 32), // Card graphite
            Self::Ink => Color32::from_rgb(22, 22, 28),      // Card ink
            Self::Midsommar => Color32::from_rgb(248, 248, 238), // Card summer #F8F8EE
            Self::Skargard => Color32::from_rgb(238, 244, 246), // Card sea #EEF4F6
            Self::Dark => Color32::from_rgb(18, 18, 22),
        }
    }

    /// Inset background (darker than surface, for inputs)
    pub fn bg_inset(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 253, 250), // Bright paper #FFFDF a
            Self::Nord => Color32::from_rgb(52, 58, 72),
            Self::Midnight => Color32::from_rgb(14, 15, 22), // Inset navy
            Self::Catppuccin => Color32::from_rgb(24, 24, 37), // Mantle #181825
            Self::Ayu => Color32::from_rgb(8, 11, 16),       // Inset charcoal
            Self::Bergman => Color32::from_rgb(14, 16, 22),  // Inset fog
            Self::Aurora => Color32::from_rgb(10, 14, 18),   // Inset night
            Self::Stockholm => Color32::from_rgb(255, 255, 253), // Inset warm white
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Inset graphite
            Self::Ink => Color32::from_rgb(8, 8, 12),        // Inset ink
            Self::Midsommar => Color32::from_rgb(255, 255, 252), // Inset summer
            Self::Skargard => Color32::from_rgb(252, 254, 255), // Inset sea
            Self::Dark => Color32::from_rgb(12, 12, 15),
        }
    }

    // =========================================================================
    // Border Colors
    // =========================================================================

    /// Subtle divider color
    pub fn border_subtle(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(220, 215, 205), // Subtle paper edge #DCD7CD
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Midnight => Color32::from_rgb(40, 44, 58), // Subtle navy border
            Self::Catppuccin => Color32::from_rgb(69, 71, 90), // Surface1 #45475A
            Self::Ayu => Color32::from_rgb(35, 42, 52),      // Subtle charcoal border
            Self::Bergman => Color32::from_rgb(50, 54, 65),  // Subtle fog border
            Self::Aurora => Color32::from_rgb(48, 54, 62),   // Subtle night border
            Self::Stockholm => Color32::from_rgb(230, 230, 228), // Subtle light gray
            Self::Graphite => Color32::from_rgb(42, 42, 46), // Subtle border #2A2A2E
            Self::Ink => Color32::from_rgb(30, 30, 40),      // Subtle border #1E1E28
            Self::Midsommar => Color32::from_rgb(232, 232, 224), // Subtle summer border #E8E8E0
            Self::Skargard => Color32::from_rgb(226, 232, 236), // Subtle sea border #E2E8EC
            Self::Dark => Color32::from_rgb(38, 38, 44),
        }
    }

    /// Default border color
    pub fn border_default(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185), // Paper edge #C8C3B9
            Self::Nord => Color32::from_rgb(94, 105, 128),
            Self::Midnight => Color32::from_rgb(55, 60, 78), // Navy border
            Self::Catppuccin => Color32::from_rgb(88, 91, 112), // Surface2 #585B70
            Self::Ayu => Color32::from_rgb(48, 56, 68),      // Charcoal border
            Self::Bergman => Color32::from_rgb(65, 70, 82),  // Fog border
            Self::Aurora => Color32::from_rgb(56, 62, 72),   // Night border
            Self::Stockholm => Color32::from_rgb(224, 224, 224), // Light gray #E0E0E0
            Self::Graphite => Color32::from_rgb(58, 58, 64), // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),      // Default border #2E2E38
            Self::Midsommar => Color32::from_rgb(216, 216, 208), // Default summer border #D8D8D0
            Self::Skargard => Color32::from_rgb(210, 216, 220), // Default sea border #D2D8DC
            Self::Dark => Color32::from_rgb(52, 52, 60),
        }
    }

    /// Focus border color
    pub fn border_focus(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 100, 100), // Dark gray ink #646464
            Self::Nord => Color32::from_rgb(59, 66, 82),
            Self::Midnight => Color32::from_rgb(59, 130, 246), // Electric blue focus
            Self::Catppuccin => Color32::from_rgb(137, 120, 190), // Mauve focus
            Self::Ayu => Color32::from_rgb(180, 120, 60),      // Amber focus
            Self::Bergman => Color32::from_rgb(143, 152, 172), // Steel silver focus
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal focus
            Self::Stockholm => Color32::from_rgb(92, 122, 153), // Slate blue focus #5C7A99
            Self::Graphite => Color32::from_rgb(232, 93, 4),   // Molten orange focus #E85D04
            Self::Ink => Color32::from_rgb(192, 192, 200),     // Silver focus #C0C0C8
            Self::Midsommar => Color32::from_rgb(37, 99, 235), // Swedish flag blue focus #2563EB
            Self::Skargard => Color32::from_rgb(30, 77, 107),  // Baltic blue focus #1E4D6B
            Self::Dark => Color32::from_rgb(55, 80, 72),
        }
    }

    // =========================================================================
    // Text Colors
    // =========================================================================

    /// Primary text color
    pub fn text_primary(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(30, 30, 30), // Rich black ink #1E1E1E
            Self::Nord => Color32::from_rgb(236, 239, 244),
            Self::Midnight => Color32::from_rgb(228, 228, 231), // Off-white #E4E4E7
            Self::Catppuccin => Color32::from_rgb(205, 214, 244), // Text #CDD6F4
            Self::Ayu => Color32::from_rgb(191, 189, 182),      // Off-white #BFBDB6
            Self::Bergman => Color32::from_rgb(216, 218, 224),  // Cool off-white #D8DAE0
            Self::Aurora => Color32::from_rgb(230, 237, 243),   // Crisp white #E6EDF3
            Self::Stockholm => Color32::from_rgb(45, 52, 54),   // Dark charcoal #2D3436
            Self::Graphite => Color32::from_rgb(232, 230, 224), // Warm off-white #E8E6E0
            Self::Ink => Color32::from_rgb(228, 228, 236),      // Cool off-white #E4E4EC
            Self::Midsommar => Color32::from_rgb(26, 26, 26),   // Dark text #1A1A1A
            Self::Skargard => Color32::from_rgb(26, 40, 48),    // Dark maritime text #1A2830
            Self::Dark => Color32::from_rgb(248, 248, 252),
        }
    }

    /// Secondary text color
    pub fn text_secondary(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(80, 80, 80), // Lighter ink #505050
            Self::Nord => Color32::from_rgb(180, 190, 200),
            Self::Midnight => Color32::from_rgb(161, 161, 170), // Silver #A1A1AA
            Self::Catppuccin => Color32::from_rgb(166, 173, 200), // Subtext1 #A6ADC8
            Self::Ayu => Color32::from_rgb(98, 106, 115),       // Muted gray #626A73
            Self::Bergman => Color32::from_rgb(140, 148, 164),  // Misty gray #8C94A4
            Self::Aurora => Color32::from_rgb(139, 148, 158),   // Muted silver #8B949E
            Self::Stockholm => Color32::from_rgb(99, 110, 114), // Secondary gray #636E72
            Self::Graphite => Color32::from_rgb(168, 166, 160), // Secondary text #A8A6A0
            Self::Ink => Color32::from_rgb(152, 152, 168),      // Secondary text #9898A8
            Self::Midsommar => Color32::from_rgb(74, 74, 74),   // Secondary text #4A4A4A
            Self::Skargard => Color32::from_rgb(58, 72, 80),    // Secondary text #3A4850
            Self::Dark => Color32::from_rgb(158, 158, 168),
        }
    }

    /// Tertiary/muted text color
    pub fn text_tertiary(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(120, 115, 110), // Faded ink #78736E
            Self::Nord => Color32::from_rgb(120, 130, 145),
            Self::Midnight => Color32::from_rgb(113, 113, 122), // Darker silver #71717A
            Self::Catppuccin => Color32::from_rgb(127, 132, 156), // Subtext0 #7F849C
            Self::Ayu => Color32::from_rgb(75, 82, 90),         // Darker gray #4B525A
            Self::Bergman => Color32::from_rgb(92, 96, 112),    // Deep fog #5C6070
            Self::Aurora => Color32::from_rgb(110, 118, 129),   // Deep night #6E7681
            Self::Stockholm => Color32::from_rgb(140, 148, 152), // Tertiary gray
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Tertiary text #606070
            Self::Midsommar => Color32::from_rgb(122, 122, 122), // Tertiary text #7A7A7A
            Self::Skargard => Color32::from_rgb(106, 120, 128), // Tertiary text #6A7880
            Self::Dark => Color32::from_rgb(100, 100, 112),
        }
    }

    // =========================================================================
    // Accent Colors
    // =========================================================================

    /// Primary accent color
    pub fn accent_primary(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(16, 185, 129), // #10B981 Enya Emerald
            Self::Nord => Color32::from_rgb(136, 192, 208), // #88C0D0
            Self::Light => Color32::from_rgb(50, 50, 50),  // Charcoal ink #323232
            Self::Midnight => Color32::from_rgb(59, 130, 246), // Electric Blue #3B82F6
            Self::Catppuccin => Color32::from_rgb(203, 166, 247), // Mauve #CBA6F7
            Self::Ayu => Color32::from_rgb(255, 180, 84),  // Warm Orange #FFB454
            Self::Bergman => Color32::from_rgb(168, 176, 192), // Steel Silver #A8B0C0
            Self::Aurora => Color32::from_rgb(126, 232, 184), // Aurora Teal #7EE8B8
            Self::Stockholm => Color32::from_rgb(92, 122, 153), // Slate blue #5C7A99
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Molten orange #E85D04
            Self::Ink => Color32::from_rgb(192, 192, 200), // Pure silver #C0C0C8
            Self::Midsommar => Color32::from_rgb(37, 99, 235), // Swedish flag blue #2563EB
            Self::Skargard => Color32::from_rgb(30, 77, 107), // Baltic blue #1E4D6B
        }
    }

    /// Hover accent color (brighter)
    pub fn accent_hover(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(52, 211, 153),
            Self::Light => Color32::from_rgb(30, 30, 30), // Rich black ink hover #1E1E1E
            Self::Nord => Color32::from_rgb(143, 188, 187),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Brighter Blue #60A5FA
            Self::Catppuccin => Color32::from_rgb(221, 180, 255), // Lighter Mauve #DDB4FF
            Self::Ayu => Color32::from_rgb(255, 204, 128),     // Brighter Orange #FFCC80
            Self::Bergman => Color32::from_rgb(192, 200, 216), // Lighter Silver #C0C8D8
            Self::Aurora => Color32::from_rgb(165, 243, 206),  // Bright Aurora #A5F3CE
            Self::Stockholm => Color32::from_rgb(75, 102, 130), // Darker slate blue
            Self::Graphite => Color32::from_rgb(255, 116, 32), // Brighter orange #FF7420
            Self::Ink => Color32::from_rgb(216, 216, 224),     // Brighter silver #D8D8E0
            Self::Midsommar => Color32::from_rgb(29, 78, 216), // Darker blue #1D4ED8
            Self::Skargard => Color32::from_rgb(24, 61, 85),   // Darker blue #183D55
        }
    }

    /// Muted accent color (for subtle backgrounds)
    pub fn accent_muted(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(20, 40, 34),
            Self::Light => Color32::from_rgb(240, 236, 228), // Light sepia tint #F0ECE4
            Self::Nord => Color32::from_rgb(20, 35, 45),
            Self::Midnight => Color32::from_rgb(20, 30, 50), // Muted blue bg
            Self::Catppuccin => Color32::from_rgb(40, 35, 55), // Muted mauve bg
            Self::Ayu => Color32::from_rgb(30, 25, 18),      // Muted amber bg
            Self::Bergman => Color32::from_rgb(30, 34, 45),  // Muted fog bg
            Self::Aurora => Color32::from_rgb(20, 40, 35),   // Muted aurora bg
            Self::Stockholm => Color32::from_rgb(230, 235, 242), // Muted slate blue bg
            Self::Graphite => Color32::from_rgb(40, 30, 22), // Muted orange bg
            Self::Ink => Color32::from_rgb(28, 28, 35),      // Muted silver bg
            Self::Midsommar => Color32::from_rgb(235, 242, 252), // Muted blue bg
            Self::Skargard => Color32::from_rgb(238, 245, 250), // Muted sea blue bg
        }
    }

    /// Accent glow color (semi-transparent)
    pub fn accent_glow(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgba_premultiplied(16, 185, 129, 30),
            Self::Light => Color32::from_rgba_premultiplied(50, 50, 50, 40),
            Self::Nord => Color32::from_rgba_premultiplied(136, 192, 208, 30),
            Self::Midnight => Color32::from_rgba_premultiplied(59, 130, 246, 30),
            Self::Catppuccin => Color32::from_rgba_premultiplied(203, 166, 247, 30),
            Self::Ayu => Color32::from_rgba_premultiplied(255, 180, 84, 30),
            Self::Bergman => Color32::from_rgba_premultiplied(168, 176, 192, 30),
            Self::Aurora => Color32::from_rgba_premultiplied(126, 232, 184, 30),
            Self::Stockholm => Color32::from_rgba_premultiplied(92, 122, 153, 40),
            Self::Graphite => Color32::from_rgba_premultiplied(232, 93, 4, 30),
            Self::Ink => Color32::from_rgba_premultiplied(192, 192, 200, 30),
            Self::Midsommar => Color32::from_rgba_premultiplied(37, 99, 235, 40),
            Self::Skargard => Color32::from_rgba_premultiplied(30, 77, 107, 40),
        }
    }

    /// Selection background color
    pub fn accent_selection(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(24, 52, 42),
            Self::Nord => Color32::from_rgb(30, 50, 60),
            Self::Light => Color32::from_rgb(230, 225, 215), // Warm sepia selection #E6E1D7
            Self::Midnight => Color32::from_rgb(30, 45, 70), // Blue selection
            Self::Catppuccin => Color32::from_rgb(55, 48, 75), // Mauve selection
            Self::Ayu => Color32::from_rgb(45, 38, 25),      // Amber selection
            Self::Bergman => Color32::from_rgb(42, 48, 62),  // Steel selection
            Self::Aurora => Color32::from_rgb(30, 55, 48),   // Teal selection
            Self::Stockholm => Color32::from_rgb(200, 215, 230), // Slate blue selection
            Self::Graphite => Color32::from_rgb(60, 45, 32), // Orange tint selection
            Self::Ink => Color32::from_rgb(38, 38, 50),      // Silver tint selection
            Self::Midsommar => Color32::from_rgb(218, 230, 248), // Blue tint selection
            Self::Skargard => Color32::from_rgb(218, 232, 242), // Sea blue tint selection
        }
    }

    // =========================================================================
    // Overlay Colors (for modals, dropdowns, popups)
    // =========================================================================

    /// Overlay background (frosted glass)
    pub fn overlay_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(250, 248, 245, 250),
            Self::Nord => Color32::from_rgba_unmultiplied(46, 52, 64, 245),
            Self::Midnight => Color32::from_rgba_unmultiplied(14, 16, 24, 245),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(30, 30, 46, 245),
            Self::Ayu => Color32::from_rgba_unmultiplied(12, 16, 22, 245),
            Self::Bergman => Color32::from_rgba_unmultiplied(20, 22, 28, 245),
            Self::Aurora => Color32::from_rgba_unmultiplied(16, 20, 26, 245),
            Self::Stockholm => Color32::from_rgba_unmultiplied(250, 250, 248, 250),
            Self::Graphite => Color32::from_rgba_unmultiplied(18, 18, 20, 245),
            Self::Ink => Color32::from_rgba_unmultiplied(10, 10, 15, 245),
            Self::Midsommar => Color32::from_rgba_unmultiplied(254, 254, 245, 250),
            Self::Skargard => Color32::from_rgba_unmultiplied(248, 251, 252, 250),
            Self::Dark => Color32::from_rgba_unmultiplied(14, 14, 16, 245),
        }
    }

    /// Overlay background (deep/premium glass)
    pub fn overlay_bg_deep(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(245, 242, 237, 248),
            Self::Nord => Color32::from_rgba_unmultiplied(40, 46, 56, 235),
            Self::Midnight => Color32::from_rgba_unmultiplied(10, 12, 20, 235),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(24, 24, 37, 235),
            Self::Ayu => Color32::from_rgba_unmultiplied(8, 12, 18, 235),
            Self::Bergman => Color32::from_rgba_unmultiplied(16, 18, 24, 235),
            Self::Aurora => Color32::from_rgba_unmultiplied(12, 16, 22, 235),
            Self::Stockholm => Color32::from_rgba_unmultiplied(245, 245, 243, 248),
            Self::Graphite => Color32::from_rgba_unmultiplied(14, 14, 16, 235),
            Self::Ink => Color32::from_rgba_unmultiplied(8, 8, 12, 235),
            Self::Midsommar => Color32::from_rgba_unmultiplied(250, 250, 240, 248),
            Self::Skargard => Color32::from_rgba_unmultiplied(242, 246, 248, 248),
            Self::Dark => Color32::from_rgba_unmultiplied(12, 12, 14, 235),
        }
    }

    /// Overlay border color
    pub fn overlay_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(200, 195, 185, 220),
            Self::Nord => Color32::from_rgba_unmultiplied(76, 86, 106, 160),
            Self::Midnight => Color32::from_rgba_unmultiplied(55, 60, 80, 160),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(69, 71, 90, 160),
            Self::Ayu => Color32::from_rgba_unmultiplied(48, 56, 68, 160),
            Self::Bergman => Color32::from_rgba_unmultiplied(55, 60, 72, 160),
            Self::Aurora => Color32::from_rgba_unmultiplied(50, 58, 68, 160),
            Self::Stockholm => Color32::from_rgba_unmultiplied(224, 224, 224, 220),
            Self::Graphite => Color32::from_rgba_unmultiplied(58, 58, 64, 160),
            Self::Ink => Color32::from_rgba_unmultiplied(46, 46, 56, 160),
            Self::Midsommar => Color32::from_rgba_unmultiplied(216, 216, 208, 220),
            Self::Skargard => Color32::from_rgba_unmultiplied(210, 216, 220, 220),
            Self::Dark => Color32::from_rgba_unmultiplied(45, 45, 48, 160),
        }
    }

    /// Overlay inner highlight (top edge glow for glass effect)
    pub fn overlay_highlight(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
                Color32::from_rgba_unmultiplied(255, 255, 252, 100)
            }
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 12),
        }
    }

    /// Overlay inner highlight (stronger for premium glass)
    pub fn overlay_highlight_strong(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
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
            Self::Light => Color32::from_rgb(245, 242, 237),
            Self::Nord => Color32::from_rgb(46, 52, 64),
            Self::Midnight => Color32::from_rgb(14, 16, 24),
            Self::Catppuccin => Color32::from_rgb(24, 24, 37),
            Self::Ayu => Color32::from_rgb(10, 14, 20),
            Self::Bergman => Color32::from_rgb(16, 18, 24),
            Self::Aurora => Color32::from_rgb(14, 18, 24),
            Self::Stockholm => Color32::from_rgb(245, 245, 243),
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Popup graphite
            Self::Ink => Color32::from_rgb(12, 12, 18),      // Popup ink
            Self::Midsommar => Color32::from_rgb(250, 250, 240), // Popup midsommar
            Self::Skargard => Color32::from_rgb(242, 246, 248), // Popup skargard
            Self::Dark => Color32::from_rgb(16, 16, 20),
        }
    }

    /// Popup border color (subtle accent tint)
    pub fn popup_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Midnight => Color32::from_rgb(50, 60, 85),
            Self::Catppuccin => Color32::from_rgb(65, 60, 85),
            Self::Ayu => Color32::from_rgb(55, 50, 40),
            Self::Bergman => Color32::from_rgb(58, 64, 78),
            Self::Aurora => Color32::from_rgb(45, 70, 62),
            Self::Stockholm => Color32::from_rgb(200, 210, 220), // Slate blue tint
            Self::Graphite => Color32::from_rgb(80, 55, 35),     // Orange tint border
            Self::Ink => Color32::from_rgb(55, 55, 70),          // Silver tint border
            Self::Midsommar => Color32::from_rgb(190, 210, 235), // Blue tint border
            Self::Skargard => Color32::from_rgb(180, 200, 215),  // Sea blue tint border
            Self::Dark => Color32::from_rgb(50, 55, 52),
        }
    }

    // =========================================================================
    // Backdrop Colors (for modal overlays)
    // =========================================================================

    /// Backdrop color (dimming overlay)
    pub fn backdrop_color(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(50, 48, 45, 60),
            Self::Nord => Color32::from_rgba_unmultiplied(25, 30, 40, 200),
            Self::Midnight => Color32::from_rgba_unmultiplied(5, 8, 15, 200),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(17, 17, 27, 200),
            Self::Ayu => Color32::from_rgba_unmultiplied(5, 8, 12, 200),
            Self::Bergman => Color32::from_rgba_unmultiplied(10, 12, 18, 200),
            Self::Aurora => Color32::from_rgba_unmultiplied(8, 12, 16, 200),
            Self::Stockholm => Color32::from_rgba_unmultiplied(45, 52, 54, 60),
            Self::Graphite => Color32::from_rgba_unmultiplied(10, 10, 12, 200),
            Self::Ink => Color32::from_rgba_unmultiplied(5, 5, 10, 200),
            Self::Midsommar => Color32::from_rgba_unmultiplied(26, 26, 26, 60),
            Self::Skargard => Color32::from_rgba_unmultiplied(26, 40, 48, 60),
            Self::Dark => Color32::from_rgba_unmultiplied(4, 4, 6, 200),
        }
    }

    /// Backdrop color (stronger for premium modals)
    pub fn backdrop_color_strong(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(50, 48, 45, 80),
            Self::Nord => Color32::from_rgba_unmultiplied(25, 30, 40, 210),
            Self::Midnight => Color32::from_rgba_unmultiplied(5, 8, 15, 210),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(17, 17, 27, 210),
            Self::Ayu => Color32::from_rgba_unmultiplied(5, 8, 12, 210),
            Self::Bergman => Color32::from_rgba_unmultiplied(10, 12, 18, 210),
            Self::Aurora => Color32::from_rgba_unmultiplied(8, 12, 16, 210),
            Self::Stockholm => Color32::from_rgba_unmultiplied(45, 52, 54, 80),
            Self::Graphite => Color32::from_rgba_unmultiplied(10, 10, 12, 210),
            Self::Ink => Color32::from_rgba_unmultiplied(5, 5, 10, 210),
            Self::Midsommar => Color32::from_rgba_unmultiplied(26, 26, 26, 80),
            Self::Skargard => Color32::from_rgba_unmultiplied(26, 40, 48, 80),
            Self::Dark => Color32::from_rgba_unmultiplied(4, 4, 6, 210),
        }
    }

    /// Backdrop vignette color (edge darkening). Returns None for light themes.
    pub fn backdrop_vignette(&self) -> Option<Color32> {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => None,
            _ => Some(Color32::from_rgba_unmultiplied(0, 0, 0, 40)),
        }
    }

    /// Backdrop accent glow color. Returns None for light themes.
    pub fn backdrop_accent_glow(&self) -> Option<Color32> {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => None,
            Self::Dark => Some(Color32::from_rgba_unmultiplied(16, 185, 129, 8)),
            Self::Nord => Some(Color32::from_rgba_unmultiplied(136, 192, 208, 8)),
            Self::Midnight => Some(Color32::from_rgba_unmultiplied(59, 130, 246, 8)),
            Self::Catppuccin => Some(Color32::from_rgba_unmultiplied(203, 166, 247, 8)),
            Self::Ayu => Some(Color32::from_rgba_unmultiplied(255, 180, 84, 8)),
            Self::Bergman => Some(Color32::from_rgba_unmultiplied(168, 176, 192, 8)),
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
            Self::Light => Color32::from_rgb(255, 245, 180),
            Self::Nord => Color32::from_rgb(40, 60, 80),
            Self::Midnight => Color32::from_rgb(30, 50, 80),
            Self::Catppuccin => Color32::from_rgb(55, 50, 70),
            Self::Ayu => Color32::from_rgb(50, 40, 25),
            Self::Bergman => Color32::from_rgb(45, 50, 65),
            Self::Aurora => Color32::from_rgb(30, 55, 50),
            Self::Stockholm => Color32::from_rgb(210, 225, 245), // Slate blue highlight
            Self::Graphite => Color32::from_rgb(60, 40, 28),     // Orange tint highlight
            Self::Ink => Color32::from_rgb(35, 35, 50),          // Silver tint highlight
            Self::Midsommar => Color32::from_rgb(210, 230, 250), // Blue tint highlight
            Self::Skargard => Color32::from_rgb(210, 235, 250),  // Sea blue tint highlight
            Self::Dark => Color32::from_rgb(16, 60, 48),
        }
    }

    /// Match highlight text color (for fuzzy search result highlighting)
    /// This is a bright, visible color for text foreground use (not background)
    pub fn highlight_match_text(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(180, 100, 0),
            Self::Nord => Color32::from_rgb(235, 203, 139),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Electric blue
            Self::Catppuccin => Color32::from_rgb(249, 226, 175), // Yellow (Rosewater)
            Self::Ayu => Color32::from_rgb(255, 200, 100),     // Gold
            Self::Bergman => Color32::from_rgb(192, 200, 216), // Bright silver
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Stockholm => Color32::from_rgb(65, 95, 130), // Deep slate blue
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(220, 220, 230),     // Bright silver
            Self::Midsommar => Color32::from_rgb(29, 78, 216), // Deep blue
            Self::Skargard => Color32::from_rgb(24, 61, 85),   // Deep sea blue
            Self::Dark => Color32::from_rgb(255, 200, 80),
        }
    }

    /// Line highlight color (for target lines in source preview)
    pub fn highlight_line(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(255, 220, 120, 80),
            Self::Nord => Color32::from_rgba_unmultiplied(235, 203, 139, 30),
            Self::Midnight => Color32::from_rgba_unmultiplied(59, 130, 246, 30),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(203, 166, 247, 30),
            Self::Ayu => Color32::from_rgba_unmultiplied(255, 180, 84, 30),
            Self::Bergman => Color32::from_rgba_unmultiplied(168, 176, 192, 30),
            Self::Aurora => Color32::from_rgba_unmultiplied(126, 232, 184, 30),
            Self::Stockholm => Color32::from_rgba_unmultiplied(92, 122, 153, 60),
            Self::Graphite => Color32::from_rgba_unmultiplied(232, 93, 4, 30),
            Self::Ink => Color32::from_rgba_unmultiplied(192, 192, 200, 30),
            Self::Midsommar => Color32::from_rgba_unmultiplied(37, 99, 235, 60),
            Self::Skargard => Color32::from_rgba_unmultiplied(30, 77, 107, 60),
            Self::Dark => Color32::from_rgba_unmultiplied(255, 220, 0, 30),
        }
    }

    // =========================================================================
    // Badge Colors (status line badges)
    // =========================================================================

    /// Zen mode badge background
    pub fn badge_zen_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 90, 80),
            Self::Nord => Color32::from_rgb(180, 142, 173),
            Self::Midnight => Color32::from_rgb(167, 139, 250), // Violet
            Self::Catppuccin => Color32::from_rgb(203, 166, 247), // Mauve
            Self::Ayu => Color32::from_rgb(210, 180, 140),      // Tan
            Self::Bergman => Color32::from_rgb(168, 176, 192),  // Steel silver
            Self::Aurora => Color32::from_rgb(165, 210, 195),   // Aurora mint
            Self::Stockholm => Color32::from_rgb(92, 122, 153), // Slate blue
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Midsommar => Color32::from_rgb(37, 99, 235),  // Swedish flag blue
            Self::Skargard => Color32::from_rgb(30, 77, 107),   // Baltic blue
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
            Self::Light => Color32::from_rgb(60, 60, 60),
            Self::Nord => Color32::from_rgb(136, 192, 208),
            Self::Midnight => Color32::from_rgb(56, 189, 248), // Sky blue
            Self::Catppuccin => Color32::from_rgb(137, 220, 235), // Sky
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Bergman => Color32::from_rgb(143, 152, 172), // Fog silver
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Stockholm => Color32::from_rgb(110, 145, 180), // Lighter slate blue
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(210, 210, 220),     // Bright silver
            Self::Midsommar => Color32::from_rgb(96, 165, 250), // Lighter blue
            Self::Skargard => Color32::from_rgb(60, 115, 150), // Lighter sea blue
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
            Self::Light => Color32::from_rgb(100, 150, 220),
            Self::Nord => Color32::from_rgb(129, 161, 193),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Sky blue
            Self::Catppuccin => Color32::from_rgb(137, 180, 250), // Blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Bergman => Color32::from_rgb(168, 176, 192), // Steel silver
            Self::Aurora => Color32::from_rgb(139, 198, 198),  // Aurora cyan
            Self::Stockholm => Color32::from_rgb(92, 122, 153), // Slate blue
            Self::Graphite => Color32::from_rgb(232, 93, 4),   // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),     // Pure silver
            Self::Midsommar => Color32::from_rgb(37, 99, 235), // Swedish flag blue
            Self::Skargard => Color32::from_rgb(30, 77, 107),  // Baltic blue
            Self::Dark => Color32::from_rgb(130, 180, 255),
        }
    }

    /// Insert mode color (editing)
    pub fn mode_insert(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 180, 100),
            Self::Nord => Color32::from_rgb(163, 190, 140),
            Self::Midnight => Color32::from_rgb(52, 211, 153), // Green
            Self::Catppuccin => Color32::from_rgb(166, 227, 161), // Green
            Self::Ayu => Color32::from_rgb(170, 210, 120),     // Green
            Self::Bergman => Color32::from_rgb(130, 170, 150), // Misty green
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Stockholm => Color32::from_rgb(75, 140, 110), // Muted green
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),     // Muted green
            Self::Midsommar => Color32::from_rgb(75, 140, 110), // Muted green
            Self::Skargard => Color32::from_rgb(65, 130, 115), // Teal green
            Self::Dark => Color32::from_rgb(150, 220, 120),
        }
    }

    /// Buffer border color (inactive)
    pub fn buffer_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Midnight => Color32::from_rgb(55, 60, 78),
            Self::Catppuccin => Color32::from_rgb(69, 71, 90),
            Self::Ayu => Color32::from_rgb(48, 56, 68),
            Self::Bergman => Color32::from_rgb(50, 54, 65),
            Self::Aurora => Color32::from_rgb(48, 54, 62),
            Self::Stockholm => Color32::from_rgb(224, 224, 224), // Light gray #E0E0E0
            Self::Graphite => Color32::from_rgb(58, 58, 64),     // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),          // Default border #2E2E38
            Self::Midsommar => Color32::from_rgb(216, 216, 208), // Summer border #D8D8D0
            Self::Skargard => Color32::from_rgb(210, 216, 220),  // Sea border #D2D8DC
            Self::Dark => Color32::from_rgb(60, 60, 70),
        }
    }

    /// Buffer background color
    pub fn buffer_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(250, 248, 245),
            Self::Nord => Color32::from_rgb(52, 58, 72),
            Self::Midnight => Color32::from_rgb(16, 18, 26),
            Self::Catppuccin => Color32::from_rgb(36, 36, 54),
            Self::Ayu => Color32::from_rgb(12, 16, 22),
            Self::Bergman => Color32::from_rgb(22, 24, 30),
            Self::Aurora => Color32::from_rgb(18, 22, 28),
            Self::Stockholm => Color32::from_rgb(248, 248, 246), // Warm off-white
            Self::Graphite => Color32::from_rgb(22, 22, 24),     // Buffer graphite
            Self::Ink => Color32::from_rgb(14, 14, 20),          // Buffer ink
            Self::Midsommar => Color32::from_rgb(252, 252, 243), // Buffer summer
            Self::Skargard => Color32::from_rgb(246, 250, 252),  // Buffer sea
            Self::Dark => Color32::from_rgb(25, 25, 30),
        }
    }

    /// Buffer content background (inner area)
    pub fn buffer_content_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 253, 250),
            Self::Nord => Color32::from_rgb(46, 52, 64),
            Self::Midnight => Color32::from_rgb(12, 14, 20),
            Self::Catppuccin => Color32::from_rgb(30, 30, 46),
            Self::Ayu => Color32::from_rgb(10, 14, 20),
            Self::Bergman => Color32::from_rgb(18, 20, 26),
            Self::Aurora => Color32::from_rgb(13, 17, 23),
            Self::Stockholm => Color32::from_rgb(252, 252, 250), // Bright warm white
            Self::Graphite => Color32::from_rgb(18, 18, 20),     // Content bg #121214
            Self::Ink => Color32::from_rgb(10, 10, 15),          // Content bg #0A0A0F
            Self::Midsommar => Color32::from_rgb(254, 254, 245), // Content summer #FEFEF5
            Self::Skargard => Color32::from_rgb(248, 251, 252),  // Content sea #F8FBFC
            Self::Dark => Color32::from_rgb(20, 20, 25),
        }
    }

    // =========================================================================
    // Semantic Colors
    // =========================================================================

    /// Success color
    pub fn semantic_success(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(45, 100, 45),
            Self::Nord => Color32::from_rgb(163, 190, 140),
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Catppuccin => Color32::from_rgb(166, 227, 161), // Green
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Bergman => Color32::from_rgb(130, 180, 150), // Misty green
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Stockholm => Color32::from_rgb(50, 120, 80), // Deep green
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),     // Muted green
            Self::Midsommar => Color32::from_rgb(50, 130, 85), // Success green
            Self::Skargard => Color32::from_rgb(45, 125, 95),  // Teal green
            Self::Dark => Color32::from_rgb(34, 197, 94),
        }
    }

    /// Warning color
    pub fn semantic_warning(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(180, 120, 30),
            Self::Nord => Color32::from_rgb(235, 203, 139),
            Self::Midnight => Color32::from_rgb(251, 191, 36), // Amber
            Self::Catppuccin => Color32::from_rgb(249, 226, 175), // Yellow
            Self::Ayu => Color32::from_rgb(255, 200, 100),
            Self::Bergman => Color32::from_rgb(210, 190, 140), // Muted gold
            Self::Aurora => Color32::from_rgb(255, 200, 120),  // Warm gold
            Self::Stockholm => Color32::from_rgb(180, 130, 45), // Amber
            Self::Graphite => Color32::from_rgb(255, 180, 80), // Warm orange
            Self::Ink => Color32::from_rgb(220, 200, 140),     // Muted gold
            Self::Midsommar => Color32::from_rgb(185, 140, 45), // Amber
            Self::Skargard => Color32::from_rgb(175, 125, 45), // Deep amber
            Self::Dark => Color32::from_rgb(251, 176, 45),
        }
    }

    /// Error color
    pub fn semantic_error(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(180, 40, 40),
            Self::Nord => Color32::from_rgb(191, 97, 106),
            Self::Midnight => Color32::from_rgb(248, 113, 113), // Red
            Self::Catppuccin => Color32::from_rgb(243, 139, 168), // Red
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Bergman => Color32::from_rgb(200, 110, 120), // Muted rose
            Self::Aurora => Color32::from_rgb(248, 113, 113),  // Soft red
            Self::Stockholm => Color32::from_rgb(180, 60, 60), // Deep red
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Soft red
            Self::Ink => Color32::from_rgb(200, 110, 120),     // Muted rose
            Self::Midsommar => Color32::from_rgb(185, 55, 55), // Deep red
            Self::Skargard => Color32::from_rgb(175, 50, 60),  // Dark red
            Self::Dark => Color32::from_rgb(239, 82, 82),
        }
    }

    /// Info color
    pub fn semantic_info(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(50, 80, 140),
            Self::Nord => Color32::from_rgb(129, 161, 193),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Blue
            Self::Catppuccin => Color32::from_rgb(137, 180, 250), // Blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Bergman => Color32::from_rgb(143, 162, 192), // Steel blue
            Self::Aurora => Color32::from_rgb(139, 198, 198),  // Aurora cyan
            Self::Stockholm => Color32::from_rgb(70, 105, 140), // Slate blue
            Self::Graphite => Color32::from_rgb(232, 93, 4),   // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),     // Pure silver
            Self::Midsommar => Color32::from_rgb(37, 99, 235), // Swedish flag blue
            Self::Skargard => Color32::from_rgb(30, 77, 107),  // Baltic blue
            Self::Dark => Color32::from_rgb(82, 146, 255),
        }
    }

    // =========================================================================
    // Syntax Highlighting Colors
    // =========================================================================

    /// Keyword color
    pub fn syntax_keyword(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(30, 30, 30),
            Self::Nord => Color32::from_rgb(180, 142, 173),
            Self::Midnight => Color32::from_rgb(199, 146, 234), // Purple
            Self::Catppuccin => Color32::from_rgb(203, 166, 247), // Mauve
            Self::Ayu => Color32::from_rgb(255, 143, 64),       // Orange
            Self::Bergman => Color32::from_rgb(168, 176, 192),  // Steel silver
            Self::Aurora => Color32::from_rgb(200, 160, 220),   // Aurora violet
            Self::Stockholm => Color32::from_rgb(70, 100, 135), // Deep slate blue
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Midsommar => Color32::from_rgb(37, 99, 235),  // Swedish flag blue
            Self::Skargard => Color32::from_rgb(30, 77, 107),   // Baltic blue
            Self::Dark => Color32::from_rgb(198, 146, 255),
        }
    }

    /// Key/property color
    pub fn syntax_key(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(50, 50, 50),
            Self::Nord => Color32::from_rgb(129, 161, 193),
            Self::Midnight => Color32::from_rgb(96, 165, 250), // Blue
            Self::Catppuccin => Color32::from_rgb(137, 180, 250), // Blue
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Bergman => Color32::from_rgb(143, 162, 192), // Steel blue
            Self::Aurora => Color32::from_rgb(139, 198, 198),  // Aurora cyan
            Self::Stockholm => Color32::from_rgb(60, 90, 125), // Muted slate blue
            Self::Graphite => Color32::from_rgb(255, 160, 100), // Bright orange
            Self::Ink => Color32::from_rgb(160, 160, 180),     // Muted silver
            Self::Midsommar => Color32::from_rgb(29, 78, 216), // Darker blue
            Self::Skargard => Color32::from_rgb(24, 61, 85),   // Darker blue
            Self::Dark => Color32::from_rgb(110, 190, 248),
        }
    }

    /// Value/string color
    pub fn syntax_value(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(70, 70, 70),
            Self::Nord => Color32::from_rgb(163, 190, 140),
            Self::Midnight => Color32::from_rgb(52, 211, 153), // Green
            Self::Catppuccin => Color32::from_rgb(166, 227, 161), // Green
            Self::Ayu => Color32::from_rgb(170, 210, 120),     // Green
            Self::Bergman => Color32::from_rgb(130, 180, 150), // Misty green
            Self::Aurora => Color32::from_rgb(126, 232, 184),  // Aurora teal
            Self::Stockholm => Color32::from_rgb(45, 100, 70), // Deep green
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Sage green
            Self::Ink => Color32::from_rgb(130, 180, 150),     // Muted green
            Self::Midsommar => Color32::from_rgb(60, 130, 85), // Green
            Self::Skargard => Color32::from_rgb(50, 120, 100), // Teal green
            Self::Dark => Color32::from_rgb(52, 211, 153),
        }
    }

    /// Operator/punctuation color
    pub fn syntax_punctuation(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 95, 90),
            Self::Nord => Color32::from_rgb(180, 190, 200),
            Self::Midnight => Color32::from_rgb(148, 163, 184), // Slate
            Self::Catppuccin => Color32::from_rgb(147, 153, 178), // Overlay2
            Self::Ayu => Color32::from_rgb(140, 148, 156),      // Gray
            Self::Bergman => Color32::from_rgb(140, 148, 164),  // Misty gray
            Self::Aurora => Color32::from_rgb(139, 148, 158),   // Muted silver
            Self::Stockholm => Color32::from_rgb(99, 110, 114), // Secondary gray
            Self::Graphite => Color32::from_rgb(168, 166, 160), // Secondary text #A8A6A0
            Self::Ink => Color32::from_rgb(152, 152, 168),      // Secondary text #9898A8
            Self::Midsommar => Color32::from_rgb(74, 74, 74),   // Secondary text
            Self::Skargard => Color32::from_rgb(58, 72, 80),    // Secondary text
            Self::Dark => Color32::from_rgb(140, 140, 155),
        }
    }

    /// Comment color
    pub fn syntax_comment(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(140, 135, 125),
            Self::Nord => Color32::from_rgb(97, 110, 136),
            Self::Midnight => Color32::from_rgb(100, 116, 139), // Slate gray
            Self::Catppuccin => Color32::from_rgb(108, 112, 134), // Overlay0
            Self::Ayu => Color32::from_rgb(90, 100, 110),       // Gray
            Self::Bergman => Color32::from_rgb(92, 96, 112),    // Deep fog
            Self::Aurora => Color32::from_rgb(110, 118, 129),   // Deep night
            Self::Stockholm => Color32::from_rgb(140, 148, 152), // Light gray
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Tertiary text #606070
            Self::Midsommar => Color32::from_rgb(122, 122, 122), // Tertiary text
            Self::Skargard => Color32::from_rgb(106, 120, 128), // Tertiary text
            Self::Dark => Color32::from_rgb(128, 128, 128),
        }
    }

    /// Function color
    pub fn syntax_function(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(40, 40, 40),
            Self::Nord => Color32::from_rgb(136, 192, 208),
            Self::Midnight => Color32::from_rgb(56, 189, 248), // Cyan
            Self::Catppuccin => Color32::from_rgb(137, 220, 235), // Sky
            Self::Ayu => Color32::from_rgb(255, 180, 84),      // Orange
            Self::Bergman => Color32::from_rgb(192, 200, 216), // Bright silver
            Self::Aurora => Color32::from_rgb(165, 243, 206),  // Bright aurora
            Self::Stockholm => Color32::from_rgb(75, 110, 150), // Slate blue
            Self::Graphite => Color32::from_rgb(255, 130, 60), // Bright orange
            Self::Ink => Color32::from_rgb(216, 216, 224),     // Bright silver
            Self::Midsommar => Color32::from_rgb(29, 78, 216), // Darker blue
            Self::Skargard => Color32::from_rgb(20, 65, 95),   // Darker blue
            Self::Dark => Color32::from_rgb(100, 160, 255),
        }
    }

    /// Type/class color
    pub fn syntax_type(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(60, 60, 60),
            Self::Nord => Color32::from_rgb(235, 203, 139),
            Self::Midnight => Color32::from_rgb(251, 191, 36), // Amber
            Self::Catppuccin => Color32::from_rgb(249, 226, 175), // Yellow
            Self::Ayu => Color32::from_rgb(89, 186, 163),      // Teal
            Self::Bergman => Color32::from_rgb(210, 190, 140), // Muted gold
            Self::Aurora => Color32::from_rgb(200, 220, 180),  // Aurora yellow-green
            Self::Stockholm => Color32::from_rgb(55, 85, 115), // Muted slate blue
            Self::Graphite => Color32::from_rgb(200, 170, 120), // Warm tan
            Self::Ink => Color32::from_rgb(180, 180, 190),     // Light silver
            Self::Midsommar => Color32::from_rgb(180, 130, 45), // Amber
            Self::Skargard => Color32::from_rgb(120, 100, 60), // Bronze
            Self::Dark => Color32::from_rgb(220, 160, 100),
        }
    }

    /// Number/constant color
    pub fn syntax_number(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(55, 55, 55),
            Self::Nord => Color32::from_rgb(180, 142, 173),
            Self::Midnight => Color32::from_rgb(248, 113, 113), // Red
            Self::Catppuccin => Color32::from_rgb(250, 179, 135), // Peach
            Self::Ayu => Color32::from_rgb(230, 140, 90),       // Coral
            Self::Bergman => Color32::from_rgb(200, 160, 170),  // Dusty rose
            Self::Aurora => Color32::from_rgb(255, 180, 150),   // Aurora peach
            Self::Stockholm => Color32::from_rgb(145, 95, 100), // Dusty mauve
            Self::Graphite => Color32::from_rgb(255, 140, 80),  // Coral orange
            Self::Ink => Color32::from_rgb(200, 160, 180),      // Dusty rose
            Self::Midsommar => Color32::from_rgb(150, 90, 100), // Dusty rose
            Self::Skargard => Color32::from_rgb(140, 85, 100),  // Muted plum
            Self::Dark => Color32::from_rgb(220, 120, 120),
        }
    }

    /// Variable color
    pub fn syntax_variable(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(45, 45, 45),
            Self::Nord => Color32::from_rgb(236, 239, 244),
            Self::Midnight => Color32::from_rgb(228, 228, 231), // Off-white
            Self::Catppuccin => Color32::from_rgb(205, 214, 244), // Text
            Self::Ayu => Color32::from_rgb(191, 189, 182),      // Fg
            Self::Bergman => Color32::from_rgb(216, 218, 224),  // Cool off-white
            Self::Aurora => Color32::from_rgb(230, 237, 243),   // Crisp white
            Self::Stockholm => Color32::from_rgb(45, 52, 54),   // Dark charcoal
            Self::Graphite => Color32::from_rgb(232, 230, 224), // Text primary #E8E6E0
            Self::Ink => Color32::from_rgb(228, 228, 236),      // Text primary #E4E4EC
            Self::Midsommar => Color32::from_rgb(26, 26, 26),   // Text primary
            Self::Skargard => Color32::from_rgb(26, 40, 48),    // Text primary
            Self::Dark => Color32::from_rgb(220, 220, 220),
        }
    }

    // =========================================================================
    // Scrollbar Colors
    // =========================================================================

    /// Scrollbar track color
    pub fn scrollbar_track(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
                Color32::from_rgba_unmultiplied(80, 75, 70, 15)
            }
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        }
    }

    /// Scrollbar thumb color
    pub fn scrollbar_thumb(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(120, 115, 105, 160),
            Self::Nord => Color32::from_rgba_unmultiplied(129, 161, 193, 120),
            Self::Midnight => Color32::from_rgba_unmultiplied(96, 165, 250, 80),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(147, 153, 178, 120),
            Self::Ayu => Color32::from_rgba_unmultiplied(140, 148, 156, 120),
            Self::Bergman => Color32::from_rgba_unmultiplied(140, 148, 164, 120),
            Self::Aurora => Color32::from_rgba_unmultiplied(139, 148, 158, 120),
            Self::Stockholm => Color32::from_rgba_unmultiplied(99, 110, 114, 160),
            Self::Graphite => Color32::from_rgba_unmultiplied(168, 166, 160, 120),
            Self::Ink => Color32::from_rgba_unmultiplied(152, 152, 168, 120),
            Self::Midsommar => Color32::from_rgba_unmultiplied(74, 74, 74, 160),
            Self::Skargard => Color32::from_rgba_unmultiplied(58, 72, 80, 160),
            Self::Dark => Color32::from_rgba_unmultiplied(140, 140, 160, 120),
        }
    }

    /// Scrollbar thumb highlight color
    pub fn scrollbar_thumb_highlight(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(80, 75, 70, 200),
            Self::Nord => Color32::from_rgba_unmultiplied(143, 188, 187, 160),
            Self::Midnight => Color32::from_rgba_unmultiplied(96, 165, 250, 140),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(203, 166, 247, 140),
            Self::Ayu => Color32::from_rgba_unmultiplied(255, 180, 84, 140),
            Self::Bergman => Color32::from_rgba_unmultiplied(168, 176, 192, 140),
            Self::Aurora => Color32::from_rgba_unmultiplied(126, 232, 184, 140),
            Self::Stockholm => Color32::from_rgba_unmultiplied(92, 122, 153, 200),
            Self::Graphite => Color32::from_rgba_unmultiplied(232, 93, 4, 140),
            Self::Ink => Color32::from_rgba_unmultiplied(192, 192, 200, 140),
            Self::Midsommar => Color32::from_rgba_unmultiplied(37, 99, 235, 200),
            Self::Skargard => Color32::from_rgba_unmultiplied(30, 77, 107, 200),
            Self::Dark => Color32::from_rgba_unmultiplied(180, 180, 200, 160),
        }
    }

    /// Scrollbar cap highlight color
    pub fn scrollbar_cap(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
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
            Self::Light => Color32::from_rgba_unmultiplied(248, 245, 240, 252),
            Self::Nord => Color32::from_rgba_unmultiplied(46, 52, 64, 250),
            Self::Midnight => Color32::from_rgba_unmultiplied(14, 16, 24, 250),
            Self::Catppuccin => Color32::from_rgba_unmultiplied(30, 30, 46, 250),
            Self::Ayu => Color32::from_rgba_unmultiplied(12, 16, 22, 250),
            Self::Bergman => Color32::from_rgba_unmultiplied(18, 20, 26, 250),
            Self::Aurora => Color32::from_rgba_unmultiplied(13, 17, 23, 250),
            Self::Stockholm => Color32::from_rgba_unmultiplied(248, 248, 246, 252),
            Self::Graphite => Color32::from_rgba_unmultiplied(18, 18, 20, 250),
            Self::Ink => Color32::from_rgba_unmultiplied(10, 10, 15, 250),
            Self::Midsommar => Color32::from_rgba_unmultiplied(254, 254, 245, 252),
            Self::Skargard => Color32::from_rgba_unmultiplied(248, 251, 252, 252),
            Self::Dark => Color32::from_rgba_unmultiplied(15, 15, 15, 250),
        }
    }

    /// Agent panel border
    pub fn agent_panel_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Midnight => Color32::from_rgb(55, 60, 78),
            Self::Catppuccin => Color32::from_rgb(69, 71, 90),
            Self::Ayu => Color32::from_rgb(48, 56, 68),
            Self::Bergman => Color32::from_rgb(50, 54, 65),
            Self::Aurora => Color32::from_rgb(48, 54, 62),
            Self::Stockholm => Color32::from_rgb(224, 224, 224),
            Self::Graphite => Color32::from_rgb(58, 58, 64), // Default border #3A3A40
            Self::Ink => Color32::from_rgb(46, 46, 56),      // Default border #2E2E38
            Self::Midsommar => Color32::from_rgb(216, 216, 208), // Summer border
            Self::Skargard => Color32::from_rgb(210, 216, 220), // Sea border
            Self::Dark => Color32::from_rgb(38, 38, 44),
        }
    }

    /// User message background in chat
    pub fn chat_user_msg_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(240, 236, 228),
            Self::Nord => Color32::from_rgb(67, 76, 94),
            Self::Midnight => Color32::from_rgb(26, 29, 40),
            Self::Catppuccin => Color32::from_rgb(69, 71, 90),
            Self::Ayu => Color32::from_rgb(21, 26, 34),
            Self::Bergman => Color32::from_rgb(38, 40, 48),
            Self::Aurora => Color32::from_rgb(33, 38, 45),
            Self::Stockholm => Color32::from_rgb(240, 240, 238),
            Self::Graphite => Color32::from_rgb(30, 30, 32), // Elevated graphite
            Self::Ink => Color32::from_rgb(22, 22, 28),      // Elevated ink
            Self::Midsommar => Color32::from_rgb(245, 245, 234), // Elevated summer
            Self::Skargard => Color32::from_rgb(236, 241, 244), // Elevated sea
            Self::Dark => Color32::from_rgb(26, 26, 30),
        }
    }

    // =========================================================================
    // Diagnostic Background Colors
    // =========================================================================

    /// Error diagnostic background
    pub fn diagnostic_error_bg(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
                Color32::from_rgb(255, 240, 235)
            } // Warm rose-tinted paper
            _ => self.semantic_error().gamma_multiply(0.15),
        }
    }

    /// Warning diagnostic background
    pub fn diagnostic_warning_bg(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
                Color32::from_rgb(255, 248, 230)
            } // Warm amber-tinted paper
            _ => self.semantic_warning().gamma_multiply(0.15),
        }
    }

    /// Info diagnostic background
    pub fn diagnostic_info_bg(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
                Color32::from_rgb(240, 240, 248)
            } // Subtle gray-blue paper
            _ => self.semantic_info().gamma_multiply(0.15),
        }
    }

    /// Hint diagnostic background
    pub fn diagnostic_hint_bg(&self) -> Color32 {
        match self {
            Self::Light | Self::Stockholm | Self::Midsommar | Self::Skargard => {
                Color32::from_rgb(242, 250, 242)
            } // Subtle sage paper
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
            Self::Nord => [
                bg,
                Color32::from_rgb(52, 60, 75),
                Color32::from_rgb(60, 80, 95),
                Color32::from_rgb(75, 110, 130),
                Color32::from_rgb(95, 140, 160),
                Color32::from_rgb(115, 165, 185),
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
            Self::Catppuccin => [
                bg,
                Color32::from_rgb(40, 38, 55),
                Color32::from_rgb(60, 55, 85),
                Color32::from_rgb(90, 80, 130),
                Color32::from_rgb(130, 110, 175),
                Color32::from_rgb(170, 140, 210),
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
            Self::Bergman => [
                bg,
                Color32::from_rgb(28, 32, 42),
                Color32::from_rgb(45, 52, 68),
                Color32::from_rgb(70, 80, 100),
                Color32::from_rgb(100, 112, 135),
                Color32::from_rgb(135, 145, 170),
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
            Self::Stockholm => [
                bg,
                Color32::from_rgb(235, 238, 242),
                Color32::from_rgb(210, 220, 230),
                Color32::from_rgb(170, 190, 210),
                Color32::from_rgb(130, 155, 180),
                Color32::from_rgb(100, 130, 160),
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
            Self::Midsommar => [
                bg,
                Color32::from_rgb(245, 245, 238),
                Color32::from_rgb(225, 232, 245),
                Color32::from_rgb(185, 210, 240),
                Color32::from_rgb(140, 175, 225),
                Color32::from_rgb(90, 140, 210),
                accent,
                accent_hover,
            ],
            Self::Skargard => [
                bg,
                Color32::from_rgb(238, 245, 250),
                Color32::from_rgb(215, 232, 245),
                Color32::from_rgb(175, 210, 232),
                Color32::from_rgb(125, 175, 210),
                Color32::from_rgb(75, 135, 180),
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
            Self::Nord => [
                Color32::from_rgb(136, 192, 208), // Frost cyan (nord8)
                Color32::from_rgb(163, 190, 140), // Aurora green (nord14)
                Color32::from_rgb(235, 203, 139), // Aurora yellow (nord13)
                Color32::from_rgb(180, 142, 173), // Aurora purple (nord15)
                Color32::from_rgb(208, 135, 112), // Aurora orange (nord12)
                Color32::from_rgb(129, 161, 193), // Frost blue (nord9)
                Color32::from_rgb(191, 97, 106),  // Aurora red (nord11)
                Color32::from_rgb(143, 188, 187), // Frost teal (nord7)
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
            Self::Catppuccin => [
                Color32::from_rgb(245, 194, 231), // Pink (accent)
                Color32::from_rgb(137, 180, 250), // Blue
                Color32::from_rgb(166, 227, 161), // Green
                Color32::from_rgb(249, 226, 175), // Yellow
                Color32::from_rgb(203, 166, 247), // Mauve
                Color32::from_rgb(148, 226, 213), // Teal
                Color32::from_rgb(243, 139, 168), // Red
                Color32::from_rgb(250, 179, 135), // Peach
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
            Self::Bergman => [
                Color32::from_rgb(200, 180, 160), // Silver cream (accent)
                Color32::from_rgb(143, 162, 192), // Slate blue
                Color32::from_rgb(180, 160, 140), // Warm gray
                Color32::from_rgb(160, 180, 170), // Sea mist
                Color32::from_rgb(190, 170, 180), // Dusty rose
                Color32::from_rgb(170, 170, 180), // Cool gray
                Color32::from_rgb(200, 160, 150), // Blush
                Color32::from_rgb(150, 170, 170), // Slate teal
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
            Self::Skargard => [
                Color32::from_rgb(100, 160, 190), // Baltic blue (accent)
                Color32::from_rgb(130, 180, 170), // Sea foam
                Color32::from_rgb(80, 140, 160),  // Deep sea
                Color32::from_rgb(160, 190, 180), // Pale aqua
                Color32::from_rgb(110, 150, 140), // Kelp
                Color32::from_rgb(140, 170, 190), // Sky over sea
                Color32::from_rgb(90, 130, 150),  // Stormy blue
                Color32::from_rgb(120, 160, 150), // Fjord
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
            Self::Stockholm => [
                Color32::from_rgb(59, 93, 140),   // Nordic blue (accent)
                Color32::from_rgb(92, 122, 153),  // Slate blue
                Color32::from_rgb(70, 110, 130),  // Steel blue
                Color32::from_rgb(110, 140, 160), // Sky
                Color32::from_rgb(80, 100, 120),  // Charcoal blue
                Color32::from_rgb(100, 130, 150), // Powder blue
                Color32::from_rgb(130, 100, 120), // Dusty mauve
                Color32::from_rgb(90, 120, 140),  // Storm
            ],
            Self::Midsommar => [
                Color32::from_rgb(234, 179, 8),   // Sunflower yellow (accent)
                Color32::from_rgb(52, 211, 153),  // Meadow green
                Color32::from_rgb(244, 114, 182), // Wildflower pink
                Color32::from_rgb(96, 165, 250),  // Sky blue
                Color32::from_rgb(251, 146, 60),  // Marigold
                Color32::from_rgb(168, 85, 247),  // Lavender
                Color32::from_rgb(248, 113, 113), // Poppy red
                Color32::from_rgb(74, 222, 128),  // Grass
            ],
        }
    }

    /// Get a chart color by index (wraps around)
    pub fn chart_color(&self, index: usize) -> Color32 {
        let palette = self.chart_palette();
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
            // === Dark Themes ===
            Self::Dark => [
                Color32::from_rgb(248, 113, 133), // Red - Soft coral
                Color32::from_rgb(52, 211, 153),  // Green - Emerald (accent-inspired)
                Color32::from_rgb(250, 204, 21),  // Yellow - Gold
                Color32::from_rgb(96, 165, 250),  // Blue - Sky blue
                Color32::from_rgb(192, 132, 252), // Magenta - Violet
                Color32::from_rgb(34, 211, 238),  // Cyan - Bright cyan
            ],
            Self::Nord => [
                Color32::from_rgb(191, 97, 106),  // Red - Aurora red (nord11)
                Color32::from_rgb(163, 190, 140), // Green - Aurora green (nord14)
                Color32::from_rgb(235, 203, 139), // Yellow - Aurora yellow (nord13)
                Color32::from_rgb(129, 161, 193), // Blue - Frost blue (nord9)
                Color32::from_rgb(180, 142, 173), // Magenta - Aurora purple (nord15)
                Color32::from_rgb(136, 192, 208), // Cyan - Frost cyan (nord8)
            ],
            Self::Midnight => [
                Color32::from_rgb(248, 113, 113), // Red - Neon red
                Color32::from_rgb(52, 211, 153),  // Green - Cyber teal-green
                Color32::from_rgb(251, 191, 36),  // Yellow - Neon amber
                Color32::from_rgb(96, 165, 250),  // Blue - Electric blue (accent)
                Color32::from_rgb(192, 132, 252), // Magenta - Neon purple
                Color32::from_rgb(34, 211, 238),  // Cyan - Bright cyan
            ],
            Self::Catppuccin => [
                Color32::from_rgb(243, 139, 168), // Red - Catppuccin red
                Color32::from_rgb(166, 227, 161), // Green - Catppuccin green
                Color32::from_rgb(249, 226, 175), // Yellow - Catppuccin yellow
                Color32::from_rgb(137, 180, 250), // Blue - Catppuccin blue
                Color32::from_rgb(203, 166, 247), // Magenta - Catppuccin mauve
                Color32::from_rgb(148, 226, 213), // Cyan - Catppuccin teal
            ],
            Self::Ayu => [
                Color32::from_rgb(255, 102, 102), // Red - Warm red
                Color32::from_rgb(127, 204, 127), // Green - Soft green
                Color32::from_rgb(255, 204, 102), // Yellow - Amber-yellow
                Color32::from_rgb(89, 186, 163),  // Blue - Ayu cyan-blue
                Color32::from_rgb(172, 128, 255), // Magenta - Purple
                Color32::from_rgb(127, 193, 202), // Cyan - Soft cyan
            ],
            Self::Bergman => [
                Color32::from_rgb(180, 120, 120), // Red - Muted dusty red
                Color32::from_rgb(140, 170, 140), // Green - Foggy green
                Color32::from_rgb(200, 180, 130), // Yellow - Sepia gold
                Color32::from_rgb(130, 150, 180), // Blue - Slate blue
                Color32::from_rgb(160, 140, 170), // Magenta - Dusty violet
                Color32::from_rgb(140, 170, 180), // Cyan - Misty cyan
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
            Self::Skargard => [
                Color32::from_rgb(180, 100, 100), // Red - Muted coastal red
                Color32::from_rgb(100, 160, 140), // Green - Sea green
                Color32::from_rgb(200, 180, 120), // Yellow - Sandy gold
                Color32::from_rgb(80, 140, 180),  // Blue - Baltic blue
                Color32::from_rgb(140, 120, 160), // Magenta - Heather purple
                Color32::from_rgb(100, 170, 180), // Cyan - Archipelago cyan
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
            Self::Stockholm => [
                Color32::from_rgb(153, 27, 27),  // Red - Nordic muted red
                Color32::from_rgb(22, 101, 52),  // Green - Nordic forest
                Color32::from_rgb(133, 77, 14),  // Yellow - Nordic amber
                Color32::from_rgb(59, 93, 140),  // Blue - Nordic blue (accent-inspired)
                Color32::from_rgb(107, 33, 168), // Magenta - Nordic purple
                Color32::from_rgb(17, 94, 89),   // Cyan - Nordic teal
            ],
            Self::Midsommar => [
                Color32::from_rgb(220, 38, 38),  // Red - Poppy red
                Color32::from_rgb(22, 163, 74),  // Green - Meadow green
                Color32::from_rgb(202, 138, 4),  // Yellow - Sunflower
                Color32::from_rgb(37, 99, 235),  // Blue - Swedish flag blue (accent)
                Color32::from_rgb(147, 51, 234), // Magenta - Wildflower purple
                Color32::from_rgb(6, 182, 212),  // Cyan - Summer sky cyan
            ],
        }
    }

    /// Commit marker color for git annotations on charts
    pub fn chart_commit_marker(&self) -> Color32 {
        match self {
            // Dark themes - vibrant markers
            Self::Dark => Color32::from_rgb(180, 155, 255), // Violet
            Self::Nord => Color32::from_rgb(180, 142, 173), // Aurora purple
            Self::Midnight => Color32::from_rgb(192, 132, 252), // Neon purple
            Self::Catppuccin => Color32::from_rgb(203, 166, 247), // Mauve
            Self::Ayu => Color32::from_rgb(172, 128, 255),  // Purple
            Self::Bergman => Color32::from_rgb(170, 160, 180), // Muted violet
            Self::Aurora => Color32::from_rgb(180, 150, 180), // Soft purple
            Self::Graphite => Color32::from_rgb(180, 140, 120), // Copper
            Self::Ink => Color32::from_rgb(160, 160, 170),  // Silver
            Self::Skargard => Color32::from_rgb(130, 150, 170), // Slate blue

            // Light themes - muted markers
            Self::Light => Color32::from_rgb(139, 92, 246), // Purple
            Self::Stockholm => Color32::from_rgb(100, 90, 130), // Dusty violet
            Self::Midsommar => Color32::from_rgb(168, 85, 247), // Lavender
        }
    }

    // =========================================================================
    // Annotation Colors (Team collaboration annotations on charts)
    // =========================================================================

    /// Normal priority annotation color (notes/comments)
    pub fn annotation_normal(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(59, 130, 246),
            Self::Nord => Color32::from_rgb(136, 192, 208),
            Self::Midnight => Color32::from_rgb(96, 165, 250),
            Self::Catppuccin => Color32::from_rgb(137, 180, 250),
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Bergman => Color32::from_rgb(143, 162, 192),
            Self::Aurora => Color32::from_rgb(139, 198, 198),
            Self::Stockholm => Color32::from_rgb(92, 122, 153), // Slate blue
            Self::Graphite => Color32::from_rgb(232, 93, 4),    // Molten orange
            Self::Ink => Color32::from_rgb(192, 192, 200),      // Pure silver
            Self::Midsommar => Color32::from_rgb(37, 99, 235),  // Swedish flag blue
            Self::Skargard => Color32::from_rgb(30, 77, 107),   // Baltic blue
            Self::Dark => Color32::from_rgb(100, 149, 237),
        }
    }

    /// Important priority annotation color (highlighted)
    pub fn annotation_important(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(245, 158, 11),
            Self::Nord => Color32::from_rgb(235, 203, 139),
            Self::Midnight => Color32::from_rgb(251, 191, 36),
            Self::Catppuccin => Color32::from_rgb(249, 226, 175),
            Self::Ayu => Color32::from_rgb(255, 180, 84),
            Self::Bergman => Color32::from_rgb(210, 190, 140),
            Self::Aurora => Color32::from_rgb(255, 200, 120),
            Self::Stockholm => Color32::from_rgb(180, 130, 45), // Amber
            Self::Graphite => Color32::from_rgb(255, 180, 80),  // Warm orange
            Self::Ink => Color32::from_rgb(220, 200, 140),      // Muted gold
            Self::Midsommar => Color32::from_rgb(185, 140, 45), // Amber
            Self::Skargard => Color32::from_rgb(175, 125, 45),  // Deep amber
            Self::Dark => Color32::from_rgb(255, 165, 0),
        }
    }

    /// Critical priority annotation color (alert-style)
    pub fn annotation_critical(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(220, 38, 38),
            Self::Nord => Color32::from_rgb(191, 97, 106),
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Catppuccin => Color32::from_rgb(243, 139, 168),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Bergman => Color32::from_rgb(200, 110, 120),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Stockholm => Color32::from_rgb(180, 60, 60), // Deep red
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Soft red
            Self::Ink => Color32::from_rgb(200, 110, 120),     // Muted rose
            Self::Midsommar => Color32::from_rgb(185, 55, 55), // Deep red
            Self::Skargard => Color32::from_rgb(175, 50, 60),  // Dark red
            Self::Dark => Color32::from_rgb(220, 53, 69),
        }
    }

    /// Resolved annotation color (dimmed/inactive)
    pub fn annotation_resolved(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(156, 163, 175),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Midnight => Color32::from_rgb(113, 113, 122),
            Self::Catppuccin => Color32::from_rgb(108, 112, 134),
            Self::Ayu => Color32::from_rgb(90, 100, 110),
            Self::Bergman => Color32::from_rgb(92, 96, 112),
            Self::Aurora => Color32::from_rgb(110, 118, 129),
            Self::Stockholm => Color32::from_rgb(140, 148, 152),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Tertiary text #707068
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Tertiary text #606070
            Self::Midsommar => Color32::from_rgb(122, 122, 122), // Tertiary text
            Self::Skargard => Color32::from_rgb(106, 120, 128), // Tertiary text
            Self::Dark => Color32::GRAY,
        }
    }

    // =========================================================================
    // Diff Colors (Git diff visualization)
    // =========================================================================

    /// Addition line background - subtle tint spanning full line
    pub fn diff_added_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(230, 255, 237),
            Self::Nord => Color32::from_rgb(35, 55, 45),
            Self::Midnight => Color32::from_rgb(18, 35, 30),
            Self::Catppuccin => Color32::from_rgb(30, 45, 35),
            Self::Ayu => Color32::from_rgb(22, 35, 25),
            Self::Bergman => Color32::from_rgb(25, 38, 32),
            Self::Aurora => Color32::from_rgb(20, 40, 35),
            Self::Stockholm => Color32::from_rgb(228, 245, 235),
            Self::Graphite => Color32::from_rgb(22, 35, 25), // Added bg graphite
            Self::Ink => Color32::from_rgb(20, 30, 28),      // Added bg ink
            Self::Midsommar => Color32::from_rgb(230, 248, 238), // Added bg midsommar
            Self::Skargard => Color32::from_rgb(230, 248, 242), // Added bg skargard
            Self::Dark => Color32::from_rgb(19, 35, 26),
        }
    }

    /// Deletion line background - subtle tint spanning full line
    pub fn diff_removed_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 235, 235),
            Self::Nord => Color32::from_rgb(55, 40, 45),
            Self::Midnight => Color32::from_rgb(40, 22, 28),
            Self::Catppuccin => Color32::from_rgb(50, 32, 40),
            Self::Ayu => Color32::from_rgb(40, 25, 25),
            Self::Bergman => Color32::from_rgb(40, 28, 32),
            Self::Aurora => Color32::from_rgb(40, 25, 28),
            Self::Stockholm => Color32::from_rgb(255, 235, 235),
            Self::Graphite => Color32::from_rgb(40, 25, 25), // Removed bg graphite
            Self::Ink => Color32::from_rgb(35, 22, 28),      // Removed bg ink
            Self::Midsommar => Color32::from_rgb(255, 238, 238), // Removed bg midsommar
            Self::Skargard => Color32::from_rgb(255, 240, 242), // Removed bg skargard
            Self::Dark => Color32::from_rgb(40, 22, 24),
        }
    }

    /// Word-level addition highlight - brighter for inline changes
    pub fn diff_added_word_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(172, 242, 189),
            Self::Nord => Color32::from_rgb(55, 90, 70),
            Self::Midnight => Color32::from_rgb(30, 70, 55),
            Self::Catppuccin => Color32::from_rgb(50, 85, 60),
            Self::Ayu => Color32::from_rgb(40, 70, 45),
            Self::Bergman => Color32::from_rgb(40, 75, 55),
            Self::Aurora => Color32::from_rgb(35, 80, 65),
            Self::Stockholm => Color32::from_rgb(170, 230, 190),
            Self::Graphite => Color32::from_rgb(45, 70, 45), // Added word graphite
            Self::Ink => Color32::from_rgb(35, 60, 50),      // Added word ink
            Self::Midsommar => Color32::from_rgb(168, 235, 195), // Added word midsommar
            Self::Skargard => Color32::from_rgb(168, 235, 205), // Added word skargard
            Self::Dark => Color32::from_rgb(35, 70, 50),
        }
    }

    /// Word-level deletion highlight - brighter for inline changes
    pub fn diff_removed_word_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 200, 200),
            Self::Nord => Color32::from_rgb(100, 55, 60),
            Self::Midnight => Color32::from_rgb(80, 40, 45),
            Self::Catppuccin => Color32::from_rgb(100, 55, 70),
            Self::Ayu => Color32::from_rgb(85, 45, 45),
            Self::Bergman => Color32::from_rgb(85, 48, 55),
            Self::Aurora => Color32::from_rgb(90, 45, 50),
            Self::Stockholm => Color32::from_rgb(255, 195, 195),
            Self::Graphite => Color32::from_rgb(85, 45, 45), // Removed word graphite
            Self::Ink => Color32::from_rgb(70, 40, 50),      // Removed word ink
            Self::Midsommar => Color32::from_rgb(255, 198, 198), // Removed word midsommar
            Self::Skargard => Color32::from_rgb(255, 200, 205), // Removed word skargard
            Self::Dark => Color32::from_rgb(75, 35, 38),
        }
    }

    /// Addition text color - high contrast for readability
    pub fn diff_added_text(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(36, 138, 61),
            Self::Nord => Color32::from_rgb(163, 190, 140),
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Catppuccin => Color32::from_rgb(166, 227, 161),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Bergman => Color32::from_rgb(130, 180, 150),
            Self::Aurora => Color32::from_rgb(126, 232, 184),
            Self::Stockholm => Color32::from_rgb(36, 130, 65),
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Added text graphite
            Self::Ink => Color32::from_rgb(130, 180, 150),      // Added text ink
            Self::Midsommar => Color32::from_rgb(36, 135, 70),  // Added text midsommar
            Self::Skargard => Color32::from_rgb(35, 125, 80),   // Added text skargard
            Self::Dark => Color32::from_rgb(126, 231, 135),
        }
    }

    /// Deletion text color - high contrast for readability
    pub fn diff_removed_text(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(207, 34, 46),
            Self::Nord => Color32::from_rgb(191, 97, 106),
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Catppuccin => Color32::from_rgb(243, 139, 168),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Bergman => Color32::from_rgb(200, 110, 120),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Stockholm => Color32::from_rgb(200, 45, 55),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Removed text graphite
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Removed text ink
            Self::Midsommar => Color32::from_rgb(200, 50, 55),  // Removed text midsommar
            Self::Skargard => Color32::from_rgb(190, 45, 55),   // Removed text skargard
            Self::Dark => Color32::from_rgb(255, 123, 114),
        }
    }

    /// Context line text color - dimmed for less visual weight
    pub fn diff_context_text(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(87, 96, 106),
            Self::Nord => Color32::from_rgb(120, 130, 145),
            Self::Midnight => Color32::from_rgb(113, 113, 122),
            Self::Catppuccin => Color32::from_rgb(108, 112, 134),
            Self::Ayu => Color32::from_rgb(90, 100, 110),
            Self::Bergman => Color32::from_rgb(92, 96, 112),
            Self::Aurora => Color32::from_rgb(110, 118, 129),
            Self::Stockholm => Color32::from_rgb(99, 110, 114),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Context text graphite
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Context text ink
            Self::Midsommar => Color32::from_rgb(74, 74, 74),   // Context text midsommar
            Self::Skargard => Color32::from_rgb(58, 72, 80),    // Context text skargard
            Self::Dark => Color32::from_rgb(145, 152, 161),
        }
    }

    /// Addition gutter stripe color
    pub fn diff_added_gutter(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(52, 168, 83),
            Self::Nord => Color32::from_rgb(163, 190, 140),
            Self::Midnight => Color32::from_rgb(52, 211, 153),
            Self::Catppuccin => Color32::from_rgb(166, 227, 161),
            Self::Ayu => Color32::from_rgb(170, 210, 120),
            Self::Bergman => Color32::from_rgb(130, 180, 150),
            Self::Aurora => Color32::from_rgb(126, 232, 184),
            Self::Stockholm => Color32::from_rgb(52, 150, 85),
            Self::Graphite => Color32::from_rgb(140, 190, 110), // Added gutter graphite
            Self::Ink => Color32::from_rgb(130, 180, 150),      // Added gutter ink
            Self::Midsommar => Color32::from_rgb(52, 155, 90),  // Added gutter midsommar
            Self::Skargard => Color32::from_rgb(50, 145, 95),   // Added gutter skargard
            Self::Dark => Color32::from_rgb(63, 185, 80),
        }
    }

    /// Deletion gutter stripe color
    pub fn diff_removed_gutter(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(234, 67, 53),
            Self::Nord => Color32::from_rgb(191, 97, 106),
            Self::Midnight => Color32::from_rgb(248, 113, 113),
            Self::Catppuccin => Color32::from_rgb(243, 139, 168),
            Self::Ayu => Color32::from_rgb(255, 110, 110),
            Self::Bergman => Color32::from_rgb(200, 110, 120),
            Self::Aurora => Color32::from_rgb(248, 113, 113),
            Self::Stockholm => Color32::from_rgb(220, 65, 60),
            Self::Graphite => Color32::from_rgb(240, 100, 100), // Removed gutter graphite
            Self::Ink => Color32::from_rgb(200, 110, 120),      // Removed gutter ink
            Self::Midsommar => Color32::from_rgb(220, 70, 65),  // Removed gutter midsommar
            Self::Skargard => Color32::from_rgb(210, 65, 70),   // Removed gutter skargard
            Self::Dark => Color32::from_rgb(248, 81, 73),
        }
    }

    /// Line number text color
    pub fn diff_line_number(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(140, 150, 160),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Midnight => Color32::from_rgb(70, 80, 100),
            Self::Catppuccin => Color32::from_rgb(80, 82, 104),
            Self::Ayu => Color32::from_rgb(60, 70, 80),
            Self::Bergman => Color32::from_rgb(65, 70, 85),
            Self::Aurora => Color32::from_rgb(70, 78, 88),
            Self::Stockholm => Color32::from_rgb(140, 148, 152),
            Self::Graphite => Color32::from_rgb(112, 112, 104), // Line number graphite
            Self::Ink => Color32::from_rgb(96, 96, 112),        // Line number ink
            Self::Midsommar => Color32::from_rgb(122, 122, 122), // Line number midsommar
            Self::Skargard => Color32::from_rgb(106, 120, 128), // Line number skargard
            Self::Dark => Color32::from_rgb(72, 79, 88),
        }
    }

    /// Line number background color
    pub fn diff_line_number_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(246, 248, 250),
            Self::Nord => Color32::from_rgb(40, 46, 56),
            Self::Midnight => Color32::from_rgb(12, 14, 20),
            Self::Catppuccin => Color32::from_rgb(24, 24, 37),
            Self::Ayu => Color32::from_rgb(8, 11, 16),
            Self::Bergman => Color32::from_rgb(14, 16, 22),
            Self::Aurora => Color32::from_rgb(10, 14, 18),
            Self::Stockholm => Color32::from_rgb(245, 245, 243),
            Self::Graphite => Color32::from_rgb(14, 14, 16), // Line number bg graphite
            Self::Ink => Color32::from_rgb(8, 8, 12),        // Line number bg ink
            Self::Midsommar => Color32::from_rgb(250, 250, 240), // Line number bg midsommar
            Self::Skargard => Color32::from_rgb(242, 246, 248), // Line number bg skargard
            Self::Dark => Color32::from_rgb(13, 17, 23),
        }
    }

    /// Hunk header background
    pub fn diff_hunk_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(240, 245, 255),
            Self::Nord => Color32::from_rgb(40, 50, 70),
            Self::Midnight => Color32::from_rgb(20, 30, 55),
            Self::Catppuccin => Color32::from_rgb(38, 38, 60),
            Self::Ayu => Color32::from_rgb(20, 25, 35),
            Self::Bergman => Color32::from_rgb(28, 32, 45),
            Self::Aurora => Color32::from_rgb(22, 32, 38),
            Self::Stockholm => Color32::from_rgb(235, 240, 250),
            Self::Graphite => Color32::from_rgb(30, 25, 20), // Hunk bg graphite
            Self::Ink => Color32::from_rgb(20, 20, 30),      // Hunk bg ink
            Self::Midsommar => Color32::from_rgb(235, 242, 252), // Hunk bg midsommar
            Self::Skargard => Color32::from_rgb(235, 245, 252), // Hunk bg skargard
            Self::Dark => Color32::from_rgb(22, 27, 46),
        }
    }

    /// Hunk header text color
    pub fn diff_hunk_text(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(47, 93, 158),
            Self::Nord => Color32::from_rgb(129, 161, 193),
            Self::Midnight => Color32::from_rgb(96, 165, 250),
            Self::Catppuccin => Color32::from_rgb(137, 180, 250),
            Self::Ayu => Color32::from_rgb(89, 186, 163),
            Self::Bergman => Color32::from_rgb(143, 162, 192),
            Self::Aurora => Color32::from_rgb(139, 198, 198),
            Self::Stockholm => Color32::from_rgb(70, 100, 135),
            Self::Graphite => Color32::from_rgb(232, 93, 4), // Hunk text graphite
            Self::Ink => Color32::from_rgb(192, 192, 200),   // Hunk text ink
            Self::Midsommar => Color32::from_rgb(37, 99, 235), // Hunk text midsommar
            Self::Skargard => Color32::from_rgb(30, 77, 107), // Hunk text skargard
            Self::Dark => Color32::from_rgb(121, 184, 255),
        }
    }

    /// File header text color
    pub fn diff_file_header(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(36, 41, 47),
            Self::Nord => Color32::from_rgb(236, 239, 244),
            Self::Midnight => Color32::from_rgb(228, 228, 231),
            Self::Catppuccin => Color32::from_rgb(205, 214, 244),
            Self::Ayu => Color32::from_rgb(191, 189, 182),
            Self::Bergman => Color32::from_rgb(216, 218, 224),
            Self::Aurora => Color32::from_rgb(230, 237, 243),
            Self::Stockholm => Color32::from_rgb(45, 52, 54),
            Self::Graphite => Color32::from_rgb(232, 230, 224), // File header graphite
            Self::Ink => Color32::from_rgb(228, 228, 236),      // File header ink
            Self::Midsommar => Color32::from_rgb(26, 26, 26),   // File header midsommar
            Self::Skargard => Color32::from_rgb(26, 40, 48),    // File header skargard
            Self::Dark => Color32::from_rgb(201, 209, 217),
        }
    }

    /// File header background color
    pub fn diff_file_header_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(246, 248, 250),
            Self::Nord => Color32::from_rgb(46, 52, 64),
            Self::Midnight => Color32::from_rgb(16, 18, 26),
            Self::Catppuccin => Color32::from_rgb(36, 36, 54),
            Self::Ayu => Color32::from_rgb(12, 16, 22),
            Self::Bergman => Color32::from_rgb(22, 24, 30),
            Self::Aurora => Color32::from_rgb(18, 22, 28),
            Self::Stockholm => Color32::from_rgb(242, 244, 246),
            Self::Graphite => Color32::from_rgb(22, 22, 24), // File header bg graphite
            Self::Ink => Color32::from_rgb(14, 14, 20),      // File header bg ink
            Self::Midsommar => Color32::from_rgb(250, 250, 240), // File header bg midsommar
            Self::Skargard => Color32::from_rgb(242, 246, 248), // File header bg skargard
            Self::Dark => Color32::from_rgb(22, 27, 34),
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
