//! Gauge visualization - circular arc showing value on a range

use egui::{Color32, RichText, Stroke};

use crate::ui::theme::AppTheme;

use super::stat::Threshold;
use super::{VIZ_PADDING_BOTTOM, VIZ_PADDING_TOP};
use crate::components::util::id_generator::next_id_usize;

/// A gauge visualization showing a value on a circular arc
pub struct GaugeChart {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    pub(crate) metric_name: String,
    /// Current value (0.0 to 1.0 for percentage, or actual value with min/max)
    current_value: f64,
    /// Minimum value of the gauge range
    min_value: f64,
    /// Maximum value of the gauge range
    max_value: f64,
    /// Unit suffix (e.g., "%", "MB", "req/s")
    unit: String,
    /// Color thresholds for the gauge arc
    thresholds: Vec<Threshold>,
    /// Current theme
    pub(crate) theme: AppTheme,
    /// Title (shown in tab)
    title: String,
    /// Whether to show min/max labels
    show_min_max: bool,
}

impl Default for GaugeChart {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl GaugeChart {
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: next_id_usize(),
            title: name.clone(),
            metric_name: name,
            current_value: 0.0,
            min_value: 0.0,
            max_value: 100.0,
            unit: "%".to_string(),
            thresholds: Vec::new(),
            theme: AppTheme::default(),
            show_min_max: true,
        }
    }

    /// Set the current value
    pub fn set_value(&mut self, value: f64) {
        self.current_value = value;
    }

    /// Set the range (min and max values)
    pub fn set_range(&mut self, min: f64, max: f64) {
        self.min_value = min;
        self.max_value = max;
    }

    /// Set the unit suffix
    pub fn set_unit(&mut self, unit: impl Into<String>) {
        self.unit = unit.into();
    }

    /// Set the metric name
    pub fn set_metric_name(&mut self, name: impl Into<String>) {
        self.metric_name = name.into();
        self.title = self.metric_name.clone();
    }

    /// Set the title
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Add a threshold (value should be in the same scale as min/max)
    pub fn add_threshold(&mut self, threshold: Threshold) {
        self.thresholds.push(threshold);
        self.thresholds.sort_by(|a, b| {
            a.value
                .partial_cmp(&b.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Clear all thresholds
    pub fn clear_thresholds(&mut self) {
        self.thresholds.clear();
    }

    /// Clear the gauge (reset to defaults)
    pub fn clear(&mut self) {
        self.current_value = 0.0;
    }

    /// Get the current value.
    pub fn value(&self) -> f64 {
        self.current_value
    }

    /// Get the minimum value.
    pub fn min(&self) -> f64 {
        self.min_value
    }

    /// Get the maximum value.
    pub fn max(&self) -> f64 {
        self.max_value
    }

    /// Get the unit string.
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// Get the normalized value (0.0 to 1.0)
    pub(crate) fn normalized_value(&self) -> f64 {
        let range = self.max_value - self.min_value;
        if range <= 0.0 {
            return 0.0;
        }
        ((self.current_value - self.min_value) / range).clamp(0.0, 1.0)
    }

    /// Get color based on thresholds
    fn color_for_value(&self) -> Color32 {
        let mut color = self.theme.accent_primary();
        for threshold in &self.thresholds {
            if self.current_value >= threshold.value {
                color = threshold.color;
            }
        }
        color
    }

    /// Format the current value for display
    pub(crate) fn format_value(&self) -> String {
        let value = self.current_value;

        if value.abs() >= 1_000_000.0 {
            format!("{:.1}M", value / 1_000_000.0)
        } else if value.abs() >= 1_000.0 {
            format!("{:.1}K", value / 1_000.0)
        } else if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            format!("{value:.1}")
        }
    }

    /// Render the gauge arc
    fn render_arc(&self, ui: &mut egui::Ui, size: f32) {
        // Arc height: radius + stroke_width/2 + needle overhang + small padding
        // radius = size * 0.4, stroke = 12, needle = ~10, padding = ~5
        let arc_visual_height = size * 0.4 + 12.0 + 15.0;
        let (response, painter) =
            ui.allocate_painter(egui::vec2(size, arc_visual_height), egui::Sense::hover());

        let rect = response.rect;
        let center = egui::pos2(rect.center().x, rect.bottom());
        let radius = size * 0.4;

        // Arc parameters: semicircle from 180° to 0° (left to right)
        let start_angle = std::f32::consts::PI; // 180° (left)
        let end_angle = 0.0; // 0° (right)
        let arc_span = start_angle - end_angle;

        let stroke_width = 12.0;
        let num_segments = 60;

        // Draw background arc (dimmed)
        let bg_color = self.theme.text_primary().gamma_multiply(0.15);
        let bg_points: Vec<egui::Pos2> = (0..=num_segments)
            .map(|i| {
                let t = i as f32 / num_segments as f32;
                let angle = start_angle - t * arc_span;
                egui::pos2(
                    center.x + radius * angle.cos(),
                    center.y - radius * angle.sin(),
                )
            })
            .collect();
        painter.add(egui::Shape::line(
            bg_points,
            Stroke::new(stroke_width, bg_color),
        ));

        // Draw filled arc based on value
        let fill_ratio = self.normalized_value() as f32;
        if fill_ratio > 0.0 {
            let fill_segments = ((num_segments as f32 * fill_ratio) as usize).max(1);
            let fill_color = self.color_for_value();

            let fill_points: Vec<egui::Pos2> = (0..=fill_segments)
                .map(|i| {
                    let t = i as f32 / num_segments as f32;
                    let angle = start_angle - t * arc_span;
                    egui::pos2(
                        center.x + radius * angle.cos(),
                        center.y - radius * angle.sin(),
                    )
                })
                .collect();
            painter.add(egui::Shape::line(
                fill_points,
                Stroke::new(stroke_width, fill_color),
            ));
        }

        // Draw needle indicator at current position
        let needle_angle = start_angle - fill_ratio * arc_span;
        let needle_outer = egui::pos2(
            center.x + (radius + stroke_width * 0.5 + 4.0) * needle_angle.cos(),
            center.y - (radius + stroke_width * 0.5 + 4.0) * needle_angle.sin(),
        );
        let needle_color = self.color_for_value();
        painter.circle_filled(needle_outer, 5.0, needle_color);
    }

    /// Render the gauge chart
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = self.theme.text_primary();

        let available_width = ui.available_width();
        let available_height = ui.available_height();

        // Scale gauge to fit within available space.
        // Work backwards from available height: the arc height is gauge_size * 0.4 + 27,
        // plus we need room for title, value text, min/max labels, and padding.
        // Fixed overhead ≈ title(18) + spacing(12) + value(64) + minmax(40) + padding(32) ≈ 166
        // Arc height = gauge_size * 0.4 + 27
        // So max gauge_size from height: (available_height - 166) / 0.4
        let max_from_height = ((available_height - 120.0) / 0.4).max(60.0);
        let gauge_size = available_width.min(max_from_height).clamp(60.0, 500.0);

        // Scale text sizes proportionally
        let scale_factor = gauge_size / 280.0; // 280 was the old fixed size
        let title_size = (14.0 * scale_factor).clamp(10.0, 18.0);
        let value_size = (36.0 * scale_factor).clamp(16.0, 64.0);
        let label_size = (11.0 * scale_factor).clamp(8.0, 14.0);

        // Calculate content height based on scaled sizes
        let arc_height = gauge_size * 0.4 + 12.0 + 15.0;
        let content_height = title_size
            + 12.0
            + arc_height
            + value_size
            + 40.0
            + VIZ_PADDING_TOP
            + VIZ_PADDING_BOTTOM;
        let vertical_offset = ((available_height - content_height) / 2.0).max(0.0);

        ui.vertical_centered(|ui| {
            ui.add_space(vertical_offset);

            // Title (only show if explicitly set and different from default)
            if !self.title.is_empty() && self.title != "Untitled" {
                let title_label = egui::Label::new(
                    RichText::new(&self.title)
                        .color(text_col)
                        .size(title_size)
                        .strong(),
                )
                .truncate();
                ui.add(title_label);
                ui.add_space(12.0);
            }

            // Render the arc gauge
            self.render_arc(ui, gauge_size);

            // Value display in center area
            let value_color = self.color_for_value();
            let formatted = self.format_value();

            ui.label(
                RichText::new(format!("{}{}", formatted, self.unit))
                    .color(value_color)
                    .size(value_size)
                    .strong(),
            );

            // Min/Max labels - hide when pane is too small to avoid overflow
            if self.show_min_max && gauge_size >= 120.0 {
                ui.add_space(8.0);
                // The arc uses size * 0.4 as radius, so the arc spans 2 * radius = size * 0.8
                let arc_width = gauge_size * 0.8;
                // Scale the label spacing based on gauge size
                let label_spacing = arc_width - (label_size * 3.5); // Approximate space for both labels

                ui.horizontal(|ui| {
                    let container_width = ui.available_width();
                    // Center the labels within the same width as the arc
                    let side_padding = (container_width - arc_width) / 2.0;

                    ui.add_space(side_padding);
                    ui.label(
                        RichText::new(format!("{:.0}", self.min_value))
                            .color(text_col.gamma_multiply(0.4))
                            .size(label_size),
                    );

                    ui.add_space(label_spacing);

                    ui.label(
                        RichText::new(format!("{:.0}", self.max_value))
                            .color(text_col.gamma_multiply(0.4))
                            .size(label_size),
                    );
                });
            }

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauge_format_value() {
        let mut gauge = GaugeChart::new("test");

        gauge.set_value(75.0);
        assert_eq!(gauge.format_value(), "75");

        gauge.set_value(1234.0);
        assert_eq!(gauge.format_value(), "1.2K");

        gauge.set_value(1_234_567.0);
        assert_eq!(gauge.format_value(), "1.2M");

        gauge.set_value(42.5);
        assert_eq!(gauge.format_value(), "42.5");
    }

    #[test]
    fn test_gauge_normalized_value() {
        let mut gauge = GaugeChart::new("test");
        gauge.set_range(0.0, 100.0);

        gauge.set_value(50.0);
        assert!((gauge.normalized_value() - 0.5).abs() < 0.001);

        gauge.set_value(0.0);
        assert!((gauge.normalized_value() - 0.0).abs() < 0.001);

        gauge.set_value(100.0);
        assert!((gauge.normalized_value() - 1.0).abs() < 0.001);

        // Test clamping
        gauge.set_value(150.0);
        assert!((gauge.normalized_value() - 1.0).abs() < 0.001);
    }
}
