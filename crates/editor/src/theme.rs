use egui::Color32;
use egui::Shadow;
use egui::Stroke;
use egui::Visuals;
use egui::style::Selection;
use egui::style::TextCursorStyle;
use egui::style::Widgets;

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
pub enum AppTheme {
    /// Light theme
    #[default]
    Light,
    /// Dark theme
    Dark,
}

impl AppTheme {
    pub fn name(&self) -> &str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
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

        // All background-related colors are set to white:
        faint_bg_color: Color32::from_rgb(255, 255, 255), // previously barely visible background
        extreme_bg_color: Color32::from_rgb(245, 245, 245), // e.g. TextEdit background
        code_bg_color: Color32::from_rgb(245, 245, 245),  // code background is now light grey

        warn_fg_color: Color32::from_rgb(255, 100, 0), // warning text remains as is
        error_fg_color: Color32::from_rgb(255, 0, 0),  // error text remains as is

        window_shadow: Shadow {
            offset: [10, 20],
            blur: 15,
            spread: 0,
            color: Color32::from_black_alpha(25),
        },
        // Set window fill to pure white:
        window_fill: Color32::from_rgb(255, 255, 255),
        // Window stroke is now white; this might make borders invisible:
        window_stroke: Stroke::new(1.0, Color32::from_rgb(255, 255, 255)),

        // Panel fill is pure white:
        panel_fill: Color32::from_rgb(255, 255, 255),

        popup_shadow: Shadow {
            offset: [6, 10],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(25),
        },

        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, Color32::from_rgb(0, 83, 125)),
            ..Default::default()
        },
        ..Visuals::dark()
    }
}
