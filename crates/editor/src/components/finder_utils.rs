//! Common utilities for finder components (MetricsFinder, QueryFinder)
//!
//! This module provides shared functionality for telescope/fzf-style finder modals,
//! including theme colors, text highlighting, preview data generation, and keyboard handling.

use egui::{Color32, FontId, Key, TextFormat, text::LayoutJob};

use crate::theme::AppTheme;

use super::time_series_chart::DataPoint;

/// Theme-aware colors for finder modals
pub struct FinderColors {
    /// Background color for the modal
    pub bg: Color32,
    /// Border color
    pub border: Color32,
    /// Separator line color
    pub separator: Color32,
    /// Highlight color for matched text
    pub highlight: Color32,
    /// Background for selected row
    pub selected_bg: Color32,
    /// Background for hovered row
    pub hover_bg: Color32,
    /// Preview pane background
    pub preview_bg: Color32,
    /// Panel background (for side-by-side preview)
    pub panel_bg: Color32,
}

impl FinderColors {
    /// Create finder colors for the given theme
    pub fn new(theme: AppTheme) -> Self {
        match theme {
            AppTheme::Light => Self {
                bg: Color32::from_rgb(250, 250, 250),
                border: Color32::from_rgb(200, 200, 200),
                separator: Color32::from_rgb(220, 220, 220),
                highlight: Color32::from_rgb(200, 150, 0),
                selected_bg: Color32::from_rgb(230, 240, 255),
                hover_bg: Color32::from_rgb(240, 245, 250),
                preview_bg: Color32::from_rgb(245, 247, 250),
                panel_bg: Color32::from_rgb(252, 252, 254),
            },
            AppTheme::Dark => Self {
                bg: Color32::from_rgb(30, 30, 35),
                border: Color32::from_rgb(60, 60, 70),
                separator: Color32::from_rgb(50, 50, 55),
                highlight: Color32::from_rgb(255, 200, 50),
                selected_bg: Color32::from_rgb(45, 50, 70),
                hover_bg: Color32::from_rgb(40, 42, 50),
                preview_bg: Color32::from_rgb(25, 27, 32),
                panel_bg: Color32::from_rgb(18, 20, 24),
            },
        }
    }
}

/// Keyboard input state for finder navigation
pub struct FinderKeyboardInput {
    /// Navigate up in the list
    pub navigate_up: bool,
    /// Navigate down in the list
    pub navigate_down: bool,
    /// Confirm selection
    pub confirm: bool,
    /// Close the finder
    pub escape: bool,
    /// Toggle preview pane
    pub toggle_preview: bool,
}

impl FinderKeyboardInput {
    /// Read keyboard input from the egui context
    pub fn read(ctx: &egui::Context) -> Self {
        ctx.input(|i| Self {
            navigate_up: i.key_pressed(Key::ArrowUp) || (i.key_pressed(Key::K) && i.modifiers.ctrl),
            navigate_down: i.key_pressed(Key::ArrowDown)
                || (i.key_pressed(Key::J) && i.modifiers.ctrl)
                || (i.key_pressed(Key::N) && i.modifiers.ctrl),
            confirm: i.key_pressed(Key::Enter),
            escape: i.key_pressed(Key::Escape),
            toggle_preview: i.key_pressed(Key::P) && i.modifiers.ctrl,
        })
    }
}

/// Create a text galley with highlighted match positions
pub fn create_highlighted_text(
    ui: &egui::Ui,
    text: &str,
    positions: &[usize],
    normal_color: Color32,
    highlight_color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::default();
    let font_id = FontId::proportional(14.0);

    for (i, ch) in text.chars().enumerate() {
        let color = if positions.contains(&i) {
            highlight_color
        } else {
            normal_color
        };

        let format = TextFormat {
            font_id: font_id.clone(),
            color,
            ..Default::default()
        };

        job.append(&ch.to_string(), 0.0, format);
    }

    ui.fonts_mut(|f| f.layout_job(job))
}

/// Generate deterministic demo preview data based on an item name
///
/// Returns a vector of `DataPoint`s representing a sine wave pattern
/// that is unique but reproducible for a given item name.
pub fn generate_demo_preview_data(item_name: &str) -> Vec<DataPoint> {
    let seed: u64 = item_name.bytes().map(|b| b as u64).sum();
    let now = 1_700_000_000.0;
    let duration = 3600.0; // 1 hour
    let num_points = 60;

    (0..num_points)
        .map(|i| {
            let t = now + (i as f64 / num_points as f64) * duration;
            // Create a unique but deterministic wave pattern based on seed
            let phase = (seed % 10) as f64 * 0.3;
            let amplitude = 20.0 + (seed % 30) as f64;
            let base = 50.0 + amplitude * ((t / 300.0) + phase).sin();
            let noise = ((t * (17.0 + (seed % 7) as f64)).sin()) * 5.0;
            DataPoint {
                timestamp: t,
                value: base + noise,
            }
        })
        .collect()
}

/// Get the chart line color for the given theme
pub fn chart_color(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Light => Color32::from_rgb(59, 130, 246),
        AppTheme::Dark => Color32::from_rgb(97, 175, 239),
    }
}

/// Render keyboard hints footer for finder modals
pub fn render_keyboard_hints(ui: &mut egui::Ui, hint_color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new("↑↓").color(hint_color).size(11.0));
        ui.label(egui::RichText::new("navigate").color(hint_color).size(11.0));
        ui.add_space(12.0);
        ui.label(egui::RichText::new("↵").color(hint_color).size(11.0));
        ui.label(egui::RichText::new("select").color(hint_color).size(11.0));
        ui.add_space(12.0);
        ui.label(egui::RichText::new("ctrl+p").color(hint_color).size(11.0));
        ui.label(egui::RichText::new("preview").color(hint_color).size(11.0));
        ui.add_space(12.0);
        ui.label(egui::RichText::new("esc").color(hint_color).size(11.0));
        ui.label(egui::RichText::new("close").color(hint_color).size(11.0));
    });
}
