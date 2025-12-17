//! Common utilities for finder components (MetricsFinder, QueryFinder)
//!
//! This module provides shared functionality for telescope/fzf-style finder modals,
//! including theme colors, text highlighting, preview data generation, and keyboard handling.

use egui::{Color32, Key, TextFormat, text::LayoutJob};

use crate::theme::AppTheme;
use crate::ui::palette;
use crate::ui::typography;

use super::time_series_chart::DataPoint;

/// Overlay style variants for modal/popup components
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum OverlayStyleVariant {
    /// Frosted glass: transparent background with soft edges (default)
    #[default]
    FrostedGlass,
    /// Minimal flat: solid background, no shadows
    MinimalFlat,
    /// Subtle neon: solid background with glowing accent border
    SubtleNeon,
}

/// Styling configuration for overlay/modal components
pub struct OverlayStyle {
    /// Background color
    pub bg: Color32,
    /// Border color
    pub border: Color32,
    /// Corner radius
    pub corner_radius: f32,
    /// Shadow configuration
    pub shadow: egui::epaint::Shadow,
    /// Border stroke width
    pub stroke_width: f32,
}

impl OverlayStyle {
    /// Create overlay style for the given theme and variant
    pub fn new(theme: AppTheme, variant: OverlayStyleVariant) -> Self {
        match variant {
            OverlayStyleVariant::FrostedGlass => Self::frosted_glass(theme),
            OverlayStyleVariant::MinimalFlat => Self::minimal_flat(theme),
            OverlayStyleVariant::SubtleNeon => Self::subtle_neon(theme),
        }
    }

    /// Frosted glass style: semi-transparent background with soft edges
    pub fn frosted_glass(theme: AppTheme) -> Self {
        let (bg, border) = match theme {
            AppTheme::Light => (
                Color32::from_rgba_unmultiplied(255, 255, 255, 240), // ~94% opacity
                Color32::from_rgba_unmultiplied(220, 220, 220, 200),
            ),
            AppTheme::Dark => (
                Color32::from_rgba_unmultiplied(20, 20, 20, 240), // ~94% opacity
                Color32::from_rgba_unmultiplied(60, 60, 60, 180),
            ),
        };

        Self {
            bg,
            border,
            corner_radius: 12.0,
            stroke_width: 1.0,
            shadow: egui::epaint::Shadow {
                offset: [0, 4],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(60),
            },
        }
    }

    /// Minimal flat style: solid background, no shadows
    pub fn minimal_flat(theme: AppTheme) -> Self {
        let (bg, border) = match theme {
            AppTheme::Light => (palette::light_bg::SURFACE, palette::light_border::DEFAULT),
            AppTheme::Dark => (palette::bg::SURFACE, palette::border::SUBTLE),
        };

        Self {
            bg,
            border,
            corner_radius: 4.0,
            stroke_width: 1.0,
            shadow: egui::epaint::Shadow::NONE,
        }
    }

    /// Subtle neon style: solid background with glowing accent border
    pub fn subtle_neon(theme: AppTheme) -> Self {
        let bg = match theme {
            AppTheme::Light => palette::light_bg::SURFACE,
            AppTheme::Dark => palette::bg::BASE,
        };
        let glow_color = palette::accent::PRIMARY;

        Self {
            bg,
            border: glow_color,
            corner_radius: 4.0,
            stroke_width: 1.5,
            shadow: egui::epaint::Shadow {
                offset: [0, 0],
                blur: 16,
                spread: 2,
                color: glow_color.gamma_multiply(0.4),
            },
        }
    }

    /// Apply this style to an egui Frame
    pub fn apply_to_frame(&self, frame: egui::Frame) -> egui::Frame {
        frame
            .fill(self.bg)
            .stroke(egui::Stroke::new(self.stroke_width, self.border))
            .corner_radius(self.corner_radius)
            .shadow(self.shadow)
    }

    /// Create a new styled egui Frame
    pub fn frame(&self) -> egui::Frame {
        self.apply_to_frame(egui::Frame::new().inner_margin(0.0))
    }
}

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
                bg: palette::light_bg::SURFACE,
                border: palette::light_border::DEFAULT,
                separator: palette::light_border::SUBTLE,
                highlight: palette::highlight::MATCH,
                selected_bg: palette::light_bg::SELECTED,
                hover_bg: palette::light_bg::HOVER,
                preview_bg: palette::light_bg::ELEVATED,
                panel_bg: palette::light_bg::BASE,
            },
            AppTheme::Dark => Self {
                bg: palette::bg::SURFACE,
                border: palette::border::SUBTLE,
                separator: palette::border::SUBTLE,
                highlight: palette::highlight::MATCH,
                selected_bg: palette::bg::SELECTED,
                hover_bg: palette::bg::HOVER,
                preview_bg: palette::bg::ELEVATED,
                panel_bg: palette::bg::BASE,
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
    let font_id = typography::proportional(typography::XL);

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
/// Uses the modern, muted PlanetScale-style palette
pub fn chart_color(_theme: AppTheme) -> Color32 {
    palette::chart::PRIMARY
}

/// Render keyboard hints footer for finder modals
pub fn render_keyboard_hints(ui: &mut egui::Ui, hint_color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("↑↓")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("navigate")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("↵")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("select")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("ctrl+p")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("preview")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("esc")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("close")
                .color(hint_color)
                .size(typography::SM),
        );
    });
}
