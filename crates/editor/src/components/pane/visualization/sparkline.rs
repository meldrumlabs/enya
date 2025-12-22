//! Sparkline visualization - compact inline line chart

use egui::{Color32, RichText, Stroke};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;

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
        self.color.unwrap_or(palette::accent::PRIMARY)
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
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);
        let line_color = self.line_color();

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
                                .size(14.0)
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
                                            .size(18.0)
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
                            .size(14.0),
                    );
                });
                return;
            }

            // Render the sparkline
            let available = ui.available_size();
            let height = (available.y - 24.0).clamp(60.0, 200.0);
            let width = available.x - 16.0;

            let (response, painter) =
                ui.allocate_painter(egui::vec2(width, height), egui::Sense::hover());

            let rect = response.rect;
            let padding = 4.0;
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
                Stroke::new(2.0, line_color),
            ));

            // Draw endpoint dot
            if let Some(&last_point) = points.last() {
                painter.circle_filled(last_point, 4.0, line_color);
            }

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }
}
