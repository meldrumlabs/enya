//! Sparkline visualization - compact inline line chart

use egui::{Color32, RichText, Stroke};

use crate::ui::colors::text_color;
use crate::ui::theme::AppTheme;

use super::{VIZ_PADDING_BOTTOM, VIZ_PADDING_TOP};
use crate::components::util::id_generator::next_id_usize;

/// A standalone sparkline visualization showing trends in minimal space
pub struct SparklineViz {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    pub(crate) metric_name: String,
    /// Data points to display
    data: Vec<f64>,
    /// Current theme
    pub(crate) theme: AppTheme,
    /// Title (shown in tab)
    title: String,
    /// Unit suffix for values (e.g., "ms", "req/s", "%")
    unit: String,
    /// Whether to show the current value
    show_value: bool,
    /// Whether to fill under the line
    fill: bool,
    /// Line color (uses accent if None)
    color: Option<Color32>,
}

impl Default for SparklineViz {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl SparklineViz {
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: next_id_usize(),
            title: name.clone(),
            metric_name: name,
            data: Vec::new(),
            theme: AppTheme::default(),
            unit: String::new(),
            show_value: true,
            fill: true,
            color: None,
        }
    }

    /// Set the unit suffix for values (e.g., "ms", "req/s", "%")
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

    /// Set the data points
    pub fn set_data(&mut self, data: Vec<f64>) {
        self.data = data;
    }

    /// Add a data point
    pub fn add_point(&mut self, value: f64) {
        self.data.push(value);
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get the data points.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Set whether to show the current value
    pub fn set_show_value(&mut self, show: bool) {
        self.show_value = show;
    }

    /// Set whether to fill under the line
    pub fn set_fill(&mut self, fill: bool) {
        self.fill = fill;
    }

    /// Set a custom line color
    pub fn set_color(&mut self, color: Color32) {
        self.color = Some(color);
    }

    /// Get the line color (uses accent if not set)
    fn line_color(&self) -> Color32 {
        self.color.unwrap_or(self.theme.accent_primary())
    }

    /// Get the current (latest) value
    fn current_value(&self) -> Option<f64> {
        self.data.last().copied()
    }

    /// Format a value for display
    fn format_value(value: f64) -> String {
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

    /// Render the sparkline chart
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);
        let line_color = self.line_color();

        let available_width = ui.available_width();
        let available_height = ui.available_height();

        // Scale based on available space
        let base_size = available_width.min(available_height * 2.0);
        let scale_factor = (base_size / 300.0).clamp(0.8, 1.8);

        // Scale dimensions proportionally
        let title_size = (14.0 * scale_factor).clamp(12.0, 20.0);
        let value_size = (18.0 * scale_factor).clamp(14.0, 28.0);
        let line_width = (2.0 * scale_factor).clamp(1.5, 3.5);
        let dot_radius = (4.0 * scale_factor).clamp(3.0, 6.0);

        ui.vertical(|ui| {
            ui.add_space(VIZ_PADDING_TOP);

            // Header with title and optional value
            let has_title = !self.title.is_empty() && self.title != "Untitled";
            let has_value = self.show_value && self.current_value().is_some();

            if has_title || has_value {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);

                    // Title (only show if explicitly set and different from default)
                    if has_title {
                        ui.label(
                            RichText::new(&self.title)
                                .color(text_col)
                                .size(title_size)
                                .strong(),
                        );
                    }

                    if let Some(value) = self.current_value() {
                        if self.show_value {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(Self::format_value(value))
                                            .color(line_color)
                                            .size(value_size)
                                            .strong(),
                                    );
                                },
                            );
                        }
                    }
                });
                ui.add_space(8.0);
            }

            if self.data.len() < 2 {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No data")
                            .color(text_col.gamma_multiply(0.4))
                            .size(title_size),
                    );
                });
                return;
            }

            // Render the sparkline - scale height with available space
            let available = ui.available_size();
            let height = (available.y - 24.0).clamp(60.0, 400.0);
            let width = available.x - 16.0;

            let (response, painter) =
                ui.allocate_painter(egui::vec2(width, height), egui::Sense::hover());

            let rect = response.rect;
            let padding = 4.0 * scale_factor;
            let inner_rect = rect.shrink(padding);

            // Calculate value range
            let min_val = self.data.iter().copied().fold(f64::INFINITY, f64::min);
            let max_val = self.data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let range = (max_val - min_val).max(0.001);

            // Build points for the line
            let n = self.data.len();
            let points: Vec<egui::Pos2> = self
                .data
                .iter()
                .enumerate()
                .map(|(i, &val)| {
                    let x = inner_rect.left() + (i as f32 / (n - 1) as f32) * inner_rect.width();
                    let normalized = ((val - min_val) / range) as f32;
                    let y = inner_rect.bottom() - normalized * inner_rect.height();
                    egui::pos2(x, y)
                })
                .collect();

            // Fill area under the line
            if self.fill {
                let mut fill_points = points.clone();
                fill_points.push(egui::pos2(inner_rect.right(), inner_rect.bottom()));
                fill_points.push(egui::pos2(inner_rect.left(), inner_rect.bottom()));

                let fill_color = line_color.gamma_multiply(0.15);
                painter.add(egui::Shape::convex_polygon(
                    fill_points,
                    fill_color,
                    Stroke::NONE,
                ));
            }

            // Draw the line
            painter.add(egui::Shape::line(
                points.clone(),
                Stroke::new(line_width, line_color),
            ));

            // Draw endpoint dot
            if let Some(&last_point) = points.last() {
                painter.circle_filled(last_point, dot_radius, line_color);
            }

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }
}
