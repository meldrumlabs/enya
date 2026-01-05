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
    /// Nord theme - Arctic blue #88C0D0
    Nord,
    /// Gruvbox theme - Warm retro orange #D65D0E
    Gruvbox,
    /// Light theme - Paper/Ink aesthetic with warm cream backgrounds and rich black text
    Light,
}

impl AppTheme {
    /// Returns the display name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Nord => "Nord",
            Self::Gruvbox => "Gruvbox",
            Self::Light => "Light",
        }
    }

    /// Returns all available themes
    pub fn all() -> &'static [AppTheme] {
        &[Self::Dark, Self::Nord, Self::Gruvbox, Self::Light]
    }

    /// Returns true if this is a dark theme
    pub fn is_dark(&self) -> bool {
        !matches!(self, Self::Light)
    }

    /// Returns true if this is a light theme
    pub fn is_light(&self) -> bool {
        matches!(self, Self::Light)
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
            "nord" | "n" => Some(Self::Nord),
            "gruvbox" | "g" => Some(Self::Gruvbox),
            "light" | "l" => Some(Self::Light),
            _ => None,
        }
    }

    /// Get the egui Visuals for this theme
    pub fn visuals(&self) -> Visuals {
        match self {
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
            Self::Light => Color32::from_rgb(250, 248, 245), // Warm cream paper #FAF8F5
            Self::Gruvbox => Color32::from_rgb(29, 32, 33),  // Gruvbox dark bg
            Self::Nord => Color32::from_rgb(46, 52, 64),     // Nord polar night
            _ => Color32::from_rgb(8, 8, 10),                // Obsidian dark
        }
    }

    /// Surface/panel background color
    pub fn bg_surface(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Gruvbox => Color32::from_rgb(40, 40, 40),
            Self::Nord => Color32::from_rgb(59, 66, 82),
            _ => Color32::from_rgb(18, 18, 21),
        }
    }

    /// Elevated elements (cards, dropdowns)
    pub fn bg_elevated(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(240, 236, 230), // Aged paper #F0ECE6
            Self::Gruvbox => Color32::from_rgb(50, 48, 47),
            Self::Nord => Color32::from_rgb(67, 76, 94),
            _ => Color32::from_rgb(26, 26, 30),
        }
    }

    /// Hover state background
    pub fn bg_hover(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(232, 228, 220), // Darker paper #E8E4DC
            Self::Gruvbox => Color32::from_rgb(60, 56, 54),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            _ => Color32::from_rgb(36, 36, 40),
        }
    }

    /// Selected item background
    pub fn bg_selected(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(225, 220, 210), // Selected paper #E1DCD2
            Self::Gruvbox => Color32::from_rgb(50, 40, 30),  // Orange tint
            Self::Nord => Color32::from_rgb(30, 50, 60),     // Blue tint
            Self::Dark => Color32::from_rgb(28, 42, 36),     // Emerald tint
        }
    }

    /// Card background (slightly darker than elevated)
    pub fn bg_card(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(245, 242, 237), // Parchment #F5F2ED
            Self::Gruvbox => Color32::from_rgb(45, 45, 43),
            Self::Nord => Color32::from_rgb(60, 68, 84),
            _ => Color32::from_rgb(18, 18, 22),
        }
    }

    /// Inset background (darker than surface, for inputs)
    pub fn bg_inset(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 253, 250), // Bright paper #FFFDF a
            Self::Gruvbox => Color32::from_rgb(32, 32, 32),
            Self::Nord => Color32::from_rgb(52, 58, 72),
            _ => Color32::from_rgb(12, 12, 15),
        }
    }

    // =========================================================================
    // Border Colors
    // =========================================================================

    /// Subtle divider color
    pub fn border_subtle(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(220, 215, 205), // Subtle paper edge #DCD7CD
            Self::Gruvbox => Color32::from_rgb(80, 73, 69),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            _ => Color32::from_rgb(38, 38, 44),
        }
    }

    /// Default border color
    pub fn border_default(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185), // Paper edge #C8C3B9
            Self::Gruvbox => Color32::from_rgb(102, 92, 84),
            Self::Nord => Color32::from_rgb(94, 105, 128),
            _ => Color32::from_rgb(52, 52, 60),
        }
    }

    /// Focus border color
    pub fn border_focus(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 100, 100), // Dark gray ink #646464
            Self::Dark => Color32::from_rgb(55, 80, 72),
            Self::Nord => Color32::from_rgb(59, 66, 82),
            Self::Gruvbox => Color32::from_rgb(80, 73, 69),
        }
    }

    // =========================================================================
    // Text Colors
    // =========================================================================

    /// Primary text color
    pub fn text_primary(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(30, 30, 30), // Rich black ink #1E1E1E
            Self::Gruvbox => Color32::from_rgb(235, 219, 178),
            Self::Nord => Color32::from_rgb(236, 239, 244),
            _ => Color32::from_rgb(248, 248, 252),
        }
    }

    /// Secondary text color
    pub fn text_secondary(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(80, 80, 80), // Lighter ink #505050
            Self::Gruvbox => Color32::from_rgb(189, 174, 147),
            Self::Nord => Color32::from_rgb(180, 190, 200),
            _ => Color32::from_rgb(158, 158, 168),
        }
    }

    /// Tertiary/muted text color
    pub fn text_tertiary(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(120, 115, 110), // Faded ink #78736E
            Self::Gruvbox => Color32::from_rgb(146, 131, 116),
            Self::Nord => Color32::from_rgb(120, 130, 145),
            _ => Color32::from_rgb(100, 100, 112),
        }
    }

    // =========================================================================
    // Accent Colors
    // =========================================================================

    /// Primary accent color
    pub fn accent_primary(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(16, 185, 129), // #10B981
            Self::Nord => Color32::from_rgb(136, 192, 208), // #88C0D0
            Self::Gruvbox => Color32::from_rgb(214, 93, 14), // #D65D0E
            Self::Light => Color32::from_rgb(50, 50, 50),  // Charcoal ink #323232
        }
    }

    /// Hover accent color (brighter)
    pub fn accent_hover(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(52, 211, 153),
            Self::Light => Color32::from_rgb(30, 30, 30), // Rich black ink hover #1E1E1E
            Self::Nord => Color32::from_rgb(143, 188, 187),
            Self::Gruvbox => Color32::from_rgb(254, 128, 25),
        }
    }

    /// Muted accent color (for subtle backgrounds)
    pub fn accent_muted(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(20, 40, 34),
            Self::Light => Color32::from_rgb(240, 236, 228), // Light sepia tint #F0ECE4
            Self::Nord => Color32::from_rgb(20, 35, 45),
            Self::Gruvbox => Color32::from_rgb(40, 30, 20),
        }
    }

    /// Accent glow color (semi-transparent)
    pub fn accent_glow(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgba_premultiplied(16, 185, 129, 30),
            Self::Light => Color32::from_rgba_premultiplied(50, 50, 50, 40), // Subtle ink glow
            Self::Nord => Color32::from_rgba_premultiplied(136, 192, 208, 30),
            Self::Gruvbox => Color32::from_rgba_premultiplied(214, 93, 14, 30),
        }
    }

    /// Selection background color
    pub fn accent_selection(&self) -> Color32 {
        match self {
            Self::Dark => Color32::from_rgb(24, 52, 42),
            Self::Nord => Color32::from_rgb(30, 50, 60),
            Self::Gruvbox => Color32::from_rgb(50, 40, 30),
            Self::Light => Color32::from_rgb(230, 225, 215), // Warm sepia selection #E6E1D7
        }
    }

    // =========================================================================
    // Overlay Colors (for modals, dropdowns, popups)
    // =========================================================================

    /// Overlay background (frosted glass)
    pub fn overlay_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(250, 248, 245, 250), // Paper
            Self::Gruvbox => Color32::from_rgba_unmultiplied(29, 32, 33, 245),
            Self::Nord => Color32::from_rgba_unmultiplied(46, 52, 64, 245),
            _ => Color32::from_rgba_unmultiplied(14, 14, 16, 245),
        }
    }

    /// Overlay background (deep/premium glass)
    pub fn overlay_bg_deep(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(245, 242, 237, 248), // Parchment
            Self::Gruvbox => Color32::from_rgba_unmultiplied(24, 27, 28, 235),
            Self::Nord => Color32::from_rgba_unmultiplied(40, 46, 56, 235),
            _ => Color32::from_rgba_unmultiplied(12, 12, 14, 235),
        }
    }

    /// Overlay border color
    pub fn overlay_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(200, 195, 185, 220), // Paper edge
            Self::Gruvbox => Color32::from_rgba_unmultiplied(80, 73, 69, 160),
            Self::Nord => Color32::from_rgba_unmultiplied(76, 86, 106, 160),
            _ => Color32::from_rgba_unmultiplied(45, 45, 48, 160),
        }
    }

    /// Overlay inner highlight (top edge glow for glass effect)
    pub fn overlay_highlight(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(255, 255, 252, 100),
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 12),
        }
    }

    /// Overlay inner highlight (stronger for premium glass)
    pub fn overlay_highlight_strong(&self) -> Color32 {
        match self {
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
            Self::Light => Color32::from_rgb(245, 242, 237), // Parchment
            Self::Gruvbox => Color32::from_rgb(24, 24, 24),
            Self::Nord => Color32::from_rgb(46, 52, 64),
            Self::Dark => Color32::from_rgb(16, 16, 20),
        }
    }

    /// Popup border color (subtle accent tint)
    pub fn popup_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185), // Paper edge
            Self::Gruvbox => Color32::from_rgb(80, 73, 69),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            Self::Dark => Color32::from_rgb(50, 55, 52),
        }
    }

    // =========================================================================
    // Backdrop Colors (for modal overlays)
    // =========================================================================

    /// Backdrop color (dimming overlay)
    pub fn backdrop_color(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(50, 48, 45, 60), // Warm sepia veil
            Self::Gruvbox => Color32::from_rgba_unmultiplied(15, 15, 15, 200),
            Self::Nord => Color32::from_rgba_unmultiplied(25, 30, 40, 200),
            _ => Color32::from_rgba_unmultiplied(4, 4, 6, 200),
        }
    }

    /// Backdrop color (stronger for premium modals)
    pub fn backdrop_color_strong(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(50, 48, 45, 80), // Stronger sepia veil
            Self::Gruvbox => Color32::from_rgba_unmultiplied(15, 15, 15, 210),
            Self::Nord => Color32::from_rgba_unmultiplied(25, 30, 40, 210),
            _ => Color32::from_rgba_unmultiplied(4, 4, 6, 210),
        }
    }

    /// Backdrop vignette color (edge darkening). Returns None for light themes.
    pub fn backdrop_vignette(&self) -> Option<Color32> {
        match self {
            Self::Light => None,
            _ => Some(Color32::from_rgba_unmultiplied(0, 0, 0, 40)),
        }
    }

    /// Backdrop accent glow color. Returns None for light themes.
    pub fn backdrop_accent_glow(&self) -> Option<Color32> {
        match self {
            Self::Light => None,
            Self::Dark => Some(Color32::from_rgba_unmultiplied(16, 185, 129, 8)),
            Self::Nord => Some(Color32::from_rgba_unmultiplied(136, 192, 208, 8)),
            Self::Gruvbox => Some(Color32::from_rgba_unmultiplied(214, 93, 14, 8)),
        }
    }

    // =========================================================================
    // Highlight Colors
    // =========================================================================

    /// Match highlight color (for search results)
    pub fn highlight_match(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 245, 180), // Warm yellow highlighter #FFF5B4
            Self::Gruvbox => Color32::from_rgb(60, 50, 30),
            Self::Nord => Color32::from_rgb(40, 60, 80),
            Self::Dark => Color32::from_rgb(16, 60, 48),
        }
    }

    /// Line highlight color (for target lines in source preview)
    pub fn highlight_line(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(255, 220, 120, 80), // Warm yellow line
            Self::Gruvbox => Color32::from_rgba_unmultiplied(250, 189, 47, 30),
            Self::Nord => Color32::from_rgba_unmultiplied(235, 203, 139, 30),
            Self::Dark => Color32::from_rgba_unmultiplied(255, 220, 0, 30),
        }
    }

    // =========================================================================
    // Badge Colors (status line badges)
    // =========================================================================

    /// Zen mode badge background
    pub fn badge_zen_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 90, 80), // Warm sepia badge #645A50
            Self::Nord => Color32::from_rgb(180, 142, 173), // Nord aurora purple
            Self::Gruvbox => Color32::from_rgb(211, 134, 155), // Gruvbox purple
            Self::Dark => Color32::from_rgb(180, 150, 220), // Light purple
        }
    }

    /// Zen mode badge foreground
    pub fn badge_zen_fg(&self) -> Color32 {
        Color32::from_rgb(40, 44, 52) // Dark text for all themes
    }

    /// Fullscreen badge background
    pub fn badge_fullscreen_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(60, 60, 60), // Charcoal badge #3C3C3C
            Self::Nord => Color32::from_rgb(136, 192, 208), // Nord frost
            Self::Gruvbox => Color32::from_rgb(131, 165, 152), // Gruvbox aqua
            Self::Dark => Color32::from_rgb(120, 200, 220), // Bright cyan
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
            Self::Light => Color32::from_rgb(100, 150, 220), // Visible sky blue #6496DC
            Self::Nord => Color32::from_rgb(129, 161, 193),  // Nord frost
            Self::Gruvbox => Color32::from_rgb(131, 165, 152), // Gruvbox aqua
            Self::Dark => Color32::from_rgb(130, 180, 255),
        }
    }

    /// Insert mode color (editing)
    pub fn mode_insert(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 180, 100), // Visible green #64B464
            Self::Nord => Color32::from_rgb(163, 190, 140),  // Nord aurora green
            Self::Gruvbox => Color32::from_rgb(184, 187, 38), // Gruvbox green
            Self::Dark => Color32::from_rgb(150, 220, 120),
        }
    }

    /// Buffer border color (inactive)
    pub fn buffer_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185), // Paper edge
            Self::Gruvbox => Color32::from_rgb(80, 73, 69),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            _ => Color32::from_rgb(60, 60, 70),
        }
    }

    /// Buffer background color
    pub fn buffer_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(250, 248, 245), // Paper
            Self::Gruvbox => Color32::from_rgb(32, 32, 30),
            Self::Nord => Color32::from_rgb(52, 58, 72),
            _ => Color32::from_rgb(25, 25, 30),
        }
    }

    /// Buffer content background (inner area)
    pub fn buffer_content_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 253, 250), // Bright paper
            Self::Gruvbox => Color32::from_rgb(29, 32, 33),
            Self::Nord => Color32::from_rgb(46, 52, 64),
            _ => Color32::from_rgb(20, 20, 25),
        }
    }

    // =========================================================================
    // Semantic Colors
    // =========================================================================

    /// Success color
    pub fn semantic_success(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(45, 100, 45), // Dark green ink #2D642D
            Self::Gruvbox => Color32::from_rgb(184, 187, 38), // Gruvbox green
            Self::Nord => Color32::from_rgb(163, 190, 140), // Nord aurora green
            _ => Color32::from_rgb(34, 197, 94),
        }
    }

    /// Warning color
    pub fn semantic_warning(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(180, 120, 30), // Dark amber ink #B4781E
            Self::Gruvbox => Color32::from_rgb(250, 189, 47), // Gruvbox yellow
            Self::Nord => Color32::from_rgb(235, 203, 139), // Nord aurora yellow
            _ => Color32::from_rgb(251, 176, 45),
        }
    }

    /// Error color
    pub fn semantic_error(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(180, 40, 40), // Dark red ink #B42828
            Self::Gruvbox => Color32::from_rgb(251, 73, 52), // Gruvbox red
            Self::Nord => Color32::from_rgb(191, 97, 106), // Nord aurora red
            _ => Color32::from_rgb(239, 82, 82),
        }
    }

    /// Info color
    pub fn semantic_info(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(50, 80, 140), // Dark blue ink #32508C
            Self::Gruvbox => Color32::from_rgb(131, 165, 152), // Gruvbox aqua
            Self::Nord => Color32::from_rgb(129, 161, 193), // Nord frost blue
            _ => Color32::from_rgb(82, 146, 255),
        }
    }

    // =========================================================================
    // Syntax Highlighting Colors
    // =========================================================================

    /// Keyword color
    pub fn syntax_keyword(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(30, 30, 30), // Rich black ink #1E1E1E
            Self::Gruvbox => Color32::from_rgb(211, 134, 155), // Gruvbox purple
            Self::Nord => Color32::from_rgb(180, 142, 173), // Nord aurora purple
            _ => Color32::from_rgb(198, 146, 255),
        }
    }

    /// Key/property color
    pub fn syntax_key(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(50, 50, 50), // Charcoal ink #323232
            Self::Gruvbox => Color32::from_rgb(131, 165, 152), // Gruvbox aqua
            Self::Nord => Color32::from_rgb(129, 161, 193),
            _ => Color32::from_rgb(110, 190, 248),
        }
    }

    /// Value/string color
    pub fn syntax_value(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(70, 70, 70), // Medium gray ink #464646
            Self::Gruvbox => Color32::from_rgb(184, 187, 38), // Gruvbox green
            Self::Nord => Color32::from_rgb(163, 190, 140),
            _ => Color32::from_rgb(52, 211, 153),
        }
    }

    /// Operator/punctuation color
    pub fn syntax_punctuation(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(100, 95, 90), // Faded sepia ink #645F5A
            Self::Gruvbox => Color32::from_rgb(189, 174, 147),
            Self::Nord => Color32::from_rgb(180, 190, 200),
            _ => Color32::from_rgb(140, 140, 155),
        }
    }

    /// Comment color
    pub fn syntax_comment(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(140, 135, 125), // Light sepia ink #8C877D
            Self::Gruvbox => Color32::from_rgb(146, 131, 116),
            Self::Nord => Color32::from_rgb(97, 110, 136),
            _ => Color32::from_rgb(128, 128, 128),
        }
    }

    /// Function color
    pub fn syntax_function(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(40, 40, 40), // Dark charcoal ink #282828
            Self::Gruvbox => Color32::from_rgb(250, 189, 47),
            Self::Nord => Color32::from_rgb(136, 192, 208),
            _ => Color32::from_rgb(100, 160, 255),
        }
    }

    /// Type/class color
    pub fn syntax_type(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(60, 60, 60), // Gray ink #3C3C3C
            Self::Gruvbox => Color32::from_rgb(254, 128, 25),
            Self::Nord => Color32::from_rgb(235, 203, 139),
            _ => Color32::from_rgb(220, 160, 100),
        }
    }

    /// Number/constant color
    pub fn syntax_number(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(55, 55, 55), // Dark gray ink #373737
            Self::Gruvbox => Color32::from_rgb(211, 134, 155),
            Self::Nord => Color32::from_rgb(180, 142, 173),
            _ => Color32::from_rgb(220, 120, 120),
        }
    }

    /// Variable color
    pub fn syntax_variable(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(45, 45, 45), // Near-black ink #2D2D2D
            Self::Gruvbox => Color32::from_rgb(235, 219, 178),
            Self::Nord => Color32::from_rgb(236, 239, 244),
            _ => Color32::from_rgb(220, 220, 220),
        }
    }

    // =========================================================================
    // Scrollbar Colors
    // =========================================================================

    /// Scrollbar track color
    pub fn scrollbar_track(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(80, 75, 70, 15), // Sepia ink track
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        }
    }

    /// Scrollbar thumb color
    pub fn scrollbar_thumb(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(120, 115, 105, 160), // Warm gray ink
            Self::Gruvbox => Color32::from_rgba_unmultiplied(146, 131, 116, 120),
            Self::Nord => Color32::from_rgba_unmultiplied(129, 161, 193, 120),
            _ => Color32::from_rgba_unmultiplied(140, 140, 160, 120),
        }
    }

    /// Scrollbar thumb highlight color
    pub fn scrollbar_thumb_highlight(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(80, 75, 70, 200), // Dark sepia ink hover
            Self::Gruvbox => Color32::from_rgba_unmultiplied(189, 174, 147, 160),
            Self::Nord => Color32::from_rgba_unmultiplied(143, 188, 187, 160),
            _ => Color32::from_rgba_unmultiplied(180, 180, 200, 160),
        }
    }

    /// Scrollbar cap highlight color
    pub fn scrollbar_cap(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(255, 252, 245, 80), // Warm paper cap
            _ => Color32::from_rgba_unmultiplied(255, 255, 255, 25),
        }
    }

    // =========================================================================
    // Agent/Panel Colors
    // =========================================================================

    /// Agent panel background
    pub fn agent_panel_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgba_unmultiplied(248, 245, 240, 252), // Warm paper panel
            Self::Gruvbox => Color32::from_rgba_unmultiplied(29, 32, 33, 250),
            Self::Nord => Color32::from_rgba_unmultiplied(46, 52, 64, 250),
            _ => Color32::from_rgba_unmultiplied(15, 15, 15, 250),
        }
    }

    /// Agent panel border
    pub fn agent_panel_border(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(200, 195, 185), // Paper edge border
            Self::Gruvbox => Color32::from_rgb(80, 73, 69),
            Self::Nord => Color32::from_rgb(76, 86, 106),
            _ => Color32::from_rgb(38, 38, 44),
        }
    }

    /// User message background in chat
    pub fn chat_user_msg_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(240, 236, 228), // Aged parchment for user msgs
            Self::Gruvbox => Color32::from_rgb(50, 48, 47),
            Self::Nord => Color32::from_rgb(67, 76, 94),
            _ => Color32::from_rgb(26, 26, 30),
        }
    }

    // =========================================================================
    // Diagnostic Background Colors
    // =========================================================================

    /// Error diagnostic background
    pub fn diagnostic_error_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 240, 235), // Warm rose-tinted paper
            _ => self.semantic_error().gamma_multiply(0.15),
        }
    }

    /// Warning diagnostic background
    pub fn diagnostic_warning_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(255, 248, 230), // Warm amber-tinted paper
            _ => self.semantic_warning().gamma_multiply(0.15),
        }
    }

    /// Info diagnostic background
    pub fn diagnostic_info_bg(&self) -> Color32 {
        match self {
            Self::Light => Color32::from_rgb(240, 240, 248), // Subtle gray-blue paper
            _ => self.semantic_info().gamma_multiply(0.15),
        }
    }

    /// Hint diagnostic background
    pub fn diagnostic_hint_bg(&self) -> Color32 {
        match self {
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
            Self::Light => [
                Color32::from_rgb(250, 248, 245), // Paper base
                Color32::from_rgb(235, 230, 220), // Warm parchment
                Color32::from_rgb(210, 200, 185), // Aged paper
                Color32::from_rgb(170, 165, 155), // Warm gray
                Color32::from_rgb(130, 125, 115), // Sepia
                Color32::from_rgb(90, 85, 80),    // Dark sepia
                accent,                           // Charcoal ink #323232
                accent_hover,                     // Rich black ink #1E1E1E
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
            Self::Gruvbox => [
                bg,
                Color32::from_rgb(40, 35, 30),
                Color32::from_rgb(60, 45, 30),
                Color32::from_rgb(90, 60, 25),
                Color32::from_rgb(130, 75, 20),
                Color32::from_rgb(170, 85, 15),
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
        // The chart palette is theme-independent for now
        // to maintain consistent visualization across themes
        [
            Color32::from_rgb(110, 190, 248), // Refined sky blue
            Color32::from_rgb(140, 150, 255), // Refined indigo
            Color32::from_rgb(100, 240, 218), // Vibrant teal
            Color32::from_rgb(198, 146, 255), // Rich violet
            Color32::from_rgb(255, 200, 60),  // Warm gold
            Color32::from_rgb(255, 130, 190), // Soft rose
            Color32::from_rgb(52, 211, 153),  // Signature emerald
            Color32::from_rgb(255, 120, 120), // Soft coral
        ]
    }

    /// Get a chart color by index (wraps around)
    pub fn chart_color(&self, index: usize) -> Color32 {
        let palette = self.chart_palette();
        palette[index % palette.len()]
    }

    /// Commit marker color for git annotations on charts
    pub fn chart_commit_marker(&self) -> Color32 {
        // Distinguished violet that works across themes
        Color32::from_rgb(180, 155, 255)
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
