//! Stat visualization - big number display with optional sparkline

use egui::{Color32, RichText, Stroke};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;

use super::{VIZ_PADDING_BOTTOM, VIZ_PADDING_TOP};
use crate::components::util::id_generator::next_id_usize;

/// A threshold configuration for stat/gauge visualizations
#[derive(Debug, Clone)]
pub struct Threshold {
    /// Value at which this threshold applies
    pub value: f64,
    /// Color to use when value exceeds this threshold
    pub color: Color32,
    /// Optional label for the threshold
    pub label: Option<String>,
}

impl Threshold {
    pub fn new(value: f64, color: Color32) -> Self {
        Self {
            value,
            color,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A stat visualization showing a big number with optional sparkline
pub struct StatChart {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    pub(crate) metric_name: String,
    /// Current/latest value to display
    current_value: f64,
    /// Unit suffix (e.g., "jobs", "ms", "%")
    unit: String,
    /// Recent history for sparkline
    sparkline_data: Vec<f64>,
    /// Whether to show the sparkline
    show_sparkline: bool,
    /// Value change from previous period
    change_value: Option<f64>,
    /// Description of the change period (e.g., "vs last hour")
    change_period: String,
    /// Color thresholds for the value
    thresholds: Vec<Threshold>,
    /// Current theme
    pub(crate) theme: AppTheme,
    /// Title (shown in tab)
    title: String,
}

impl Default for StatChart {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl StatChart {
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: next_id_usize(),
            title: name.clone(),
            metric_name: name,
            current_value: 0.0,
            unit: String::new(),
            sparkline_data: Vec::new(),
            show_sparkline: true,
            change_value: None,
            change_period: "vs last period".to_string(),
            thresholds: Vec::new(),
            theme: AppTheme::default(),
        }
    }

    /// Set the current value to display
    pub fn set_value(&mut self, value: f64) {
        self.current_value = value;
    }

    /// Set the unit suffix
    pub fn set_unit(&mut self, unit: impl Into<String>) {
        self.unit = unit.into();
    }

    /// Set the sparkline data
    pub fn set_sparkline_data(&mut self, data: Vec<f64>) {
        self.sparkline_data = data;
    }

    /// Set the change value and period
    pub fn set_change(&mut self, value: f64, period: impl Into<String>) {
        self.change_value = Some(value);
        self.change_period = period.into();
    }

    /// Clear the change indicator
    pub fn clear_change(&mut self) {
        self.change_value = None;
    }

    /// Add a threshold
    pub fn add_threshold(&mut self, threshold: Threshold) {
        self.thresholds.push(threshold);
        // Keep thresholds sorted by value
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

    /// Toggle sparkline visibility
    pub fn set_show_sparkline(&mut self, show: bool) {
        self.show_sparkline = show;
    }

    /// Clear the stat (reset to defaults)
    pub fn clear(&mut self) {
        self.current_value = 0.0;
        self.sparkline_data.clear();
        self.change_value = None;
    }

    /// Get color based on thresholds
    fn color_for_value(&self, value: f64) -> Color32 {
        // Find the highest threshold that the value exceeds
        let mut color = palette::text_primary(self.theme);
        for threshold in &self.thresholds {
            if value >= threshold.value {
                color = threshold.color;
            }
        }
        color
    }

    /// Format the current value for display
    pub(crate) fn format_value(&self) -> String {
        let value = self.current_value;

        // Format large numbers with K/M/B suffixes
        if value.abs() >= 1_000_000_000.0 {
            format!("{:.1}B", value / 1_000_000_000.0)
        } else if value.abs() >= 1_000_000.0 {
            format!("{:.1}M", value / 1_000_000.0)
        } else if value.abs() >= 1_000.0 {
            format!("{:.1}K", value / 1_000.0)
        } else if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            format!("{value:.2}")
        }
    }

    /// Get the theme-appropriate primary color
    fn theme_color(&self) -> Color32 {
        palette::accent::PRIMARY
    }

    /// Render the sparkline at the bottom of the stat
    fn render_sparkline(&self, ui: &mut egui::Ui, width: f32) {
        let height = 48.0;
        let (response, painter) =
            ui.allocate_painter(egui::vec2(width, height), egui::Sense::hover());

        let rect = response.rect;

        if self.sparkline_data.len() < 2 {
            return;
        }

        let min_val = self
            .sparkline_data
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let max_val = self
            .sparkline_data
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let range = (max_val - min_val).max(0.001);

        // Build points for the line
        let n = self.sparkline_data.len();
        let points: Vec<egui::Pos2> = self
            .sparkline_data
            .iter()
            .enumerate()
            .map(|(i, &val)| {
                let x = rect.left() + (i as f32 / (n - 1) as f32) * rect.width();
                let normalized = ((val - min_val) / range) as f32;
                let y = rect.bottom() - normalized * rect.height() * 0.9; // 90% height for padding
                egui::pos2(x, y)
            })
            .collect();

        // Fill area under the line
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.right(), rect.bottom()));
        fill_points.push(egui::pos2(rect.left(), rect.bottom()));

        let fill_color = self.theme_color().gamma_multiply(0.12);
        painter.add(egui::Shape::convex_polygon(
            fill_points,
            fill_color,
            Stroke::NONE,
        ));

        // Draw the line
        let line_color = self.theme_color().gamma_multiply(0.6);
        painter.add(egui::Shape::line(points, Stroke::new(1.5, line_color)));
    }

    /// Render the stat chart
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);

        // Calculate content height to center vertically
        // Approximate: title(13) + spacing(12) + value(56) + unit(14) + change(20) + padding
        let content_height = 130.0;
        let available_height = ui.available_height();
        let vertical_offset = ((available_height - content_height) / 2.0).max(VIZ_PADDING_TOP);

        ui.vertical_centered(|ui| {
            ui.add_space(vertical_offset);

            // Title / metric name
            ui.label(
                RichText::new(&self.metric_name)
                    .color(text_col.gamma_multiply(0.6))
                    .size(13.0),
            );

            ui.add_space(12.0);

            // Big number
            let value_color = self.color_for_value(self.current_value);
            let formatted = self.format_value();

            ui.label(
                RichText::new(&formatted)
                    .color(value_color)
                    .size(56.0)
                    .strong(),
            );

            // Unit
            if !self.unit.is_empty() {
                ui.label(
                    RichText::new(&self.unit)
                        .color(text_col.gamma_multiply(0.5))
                        .size(14.0),
                );
            }

            ui.add_space(8.0);

            // Change indicator
            if let Some(change) = self.change_value {
                let (icon, color) = if change >= 0.0 {
                    ("\u{25B2}", palette::semantic::SUCCESS) // ▲
                } else {
                    ("\u{25BC}", palette::semantic::ERROR) // ▼
                };

                ui.label(
                    RichText::new(format!("{} {:+.1}% {}", icon, change, self.change_period))
                        .color(color)
                        .size(12.0),
                );
            }

            // Sparkline at bottom
            if self.show_sparkline && self.sparkline_data.len() >= 2 {
                ui.add_space(VIZ_PADDING_TOP);
                let available_width = ui.available_width().min(300.0);
                self.render_sparkline(ui, available_width);
            }

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stat_format_value() {
        let mut stat = StatChart::new("test");

        stat.set_value(1234.0);
        assert_eq!(stat.format_value(), "1.2K");

        stat.set_value(1_234_567.0);
        assert_eq!(stat.format_value(), "1.2M");

        stat.set_value(1_234_567_890.0);
        assert_eq!(stat.format_value(), "1.2B");

        stat.set_value(42.0);
        assert_eq!(stat.format_value(), "42");

        stat.set_value(42.5);
        assert_eq!(stat.format_value(), "42.50");
    }
}
