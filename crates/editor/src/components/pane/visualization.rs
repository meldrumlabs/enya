//! Visualization types for dashboard panes
//!
//! This module provides an enum-based abstraction over different visualization types,
//! allowing a single QueryPane to switch between time series charts, stat displays,
//! gauges, and other visualization styles (similar to Grafana).

use egui::{Color32, RichText, Stroke};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

use super::flamegraph::{FlamegraphViz, populate_flamegraph_demo};
use super::heatmap::{HeatmapViz, populate_heatmap_demo};
use crate::components::pane::time_series_chart::{
    CommitMarker, DataPoint, Series, TimeSeriesChart,
};
use crate::components::util::id_generator::next_id_usize;

/// Standard padding for visualization types (for consistent spacing)
const VIZ_PADDING_TOP: f32 = 16.0;
const VIZ_PADDING_BOTTOM: f32 = 16.0;

/// Types of visualizations supported in panes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizationType {
    /// Line chart showing time series data (default)
    #[default]
    TimeSeries,
    /// Big number display with optional sparkline ("Total Jobs" style)
    Stat,
    /// Circular gauge for percentages/utilization
    Gauge,
    /// Horizontal bar chart for comparing values
    BarChart,
    /// Compact inline line chart showing trends
    Sparkline,
    /// GPU-accelerated heatmap for 2D data grids
    Heatmap,
    /// GPU-accelerated flamegraph for CPU/memory profiling
    Flamegraph,
}

impl VisualizationType {
    /// Get the display name for this visualization type
    pub fn label(&self) -> &'static str {
        match self {
            Self::TimeSeries => "Time Series",
            Self::Stat => "Stat",
            Self::Gauge => "Gauge",
            Self::BarChart => "Bar Chart",
            Self::Sparkline => "Sparkline",
            Self::Heatmap => "Heatmap",
            Self::Flamegraph => "Flamegraph",
        }
    }

    /// Get the icon for this visualization type
    pub fn icon(&self) -> &'static str {
        match self {
            Self::TimeSeries => semantic_icons::action::CHART,
            Self::Stat => egui_nerdfonts::regular::COUNTER,
            Self::Gauge => egui_nerdfonts::regular::GAUGE,
            Self::BarChart => egui_nerdfonts::regular::CHART_BAR,
            Self::Sparkline => egui_nerdfonts::regular::CHART_LINE,
            Self::Heatmap => egui_nerdfonts::regular::CHART_HISTOGRAM,
            Self::Flamegraph => egui_nerdfonts::regular::FIRE,
        }
    }

    /// Cycle to the next visualization type
    pub fn next(&self) -> Self {
        match self {
            Self::TimeSeries => Self::Stat,
            Self::Stat => Self::Gauge,
            Self::Gauge => Self::BarChart,
            Self::BarChart => Self::Sparkline,
            Self::Sparkline => Self::Heatmap,
            Self::Heatmap => Self::Flamegraph,
            Self::Flamegraph => Self::TimeSeries,
        }
    }

    /// Get the string representation for serialization
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TimeSeries => "time_series",
            Self::Stat => "stat",
            Self::Gauge => "gauge",
            Self::BarChart => "bar_chart",
            Self::Sparkline => "sparkline",
            Self::Heatmap => "heatmap",
            Self::Flamegraph => "flamegraph",
        }
    }

    /// Parse from string representation
    pub fn parse(s: &str) -> Self {
        match s {
            "stat" => Self::Stat,
            "gauge" => Self::Gauge,
            "bar_chart" => Self::BarChart,
            "sparkline" => Self::Sparkline,
            "heatmap" => Self::Heatmap,
            "flamegraph" => Self::Flamegraph,
            _ => Self::TimeSeries, // Default to time series
        }
    }
}

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
    metric_name: String,
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
    theme: AppTheme,
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
    fn format_value(&self) -> String {
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

/// A gauge visualization showing a value on a circular arc
pub struct GaugeChart {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    metric_name: String,
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
    theme: AppTheme,
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

    /// Get the normalized value (0.0 to 1.0)
    fn normalized_value(&self) -> f64 {
        let range = self.max_value - self.min_value;
        if range <= 0.0 {
            return 0.0;
        }
        ((self.current_value - self.min_value) / range).clamp(0.0, 1.0)
    }

    /// Get color based on thresholds
    fn color_for_value(&self) -> Color32 {
        let mut color = palette::accent::PRIMARY;
        for threshold in &self.thresholds {
            if self.current_value >= threshold.value {
                color = threshold.color;
            }
        }
        color
    }

    /// Format the current value for display
    fn format_value(&self) -> String {
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
        let (response, painter) =
            ui.allocate_painter(egui::vec2(size, size * 0.6), egui::Sense::hover());

        let rect = response.rect;
        let center = egui::pos2(rect.center().x, rect.bottom() - 10.0);
        let radius = (size * 0.4).min(rect.height() - 20.0);

        // Arc parameters: semicircle from 180° to 0° (left to right)
        let start_angle = std::f32::consts::PI; // 180° (left)
        let end_angle = 0.0; // 0° (right)
        let arc_span = start_angle - end_angle;

        let stroke_width = 12.0;
        let num_segments = 60;

        // Draw background arc (dimmed)
        let bg_color = text_color(self.theme).gamma_multiply(0.15);
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
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);

        // Calculate content height to center vertically
        // Approximate: title(13) + spacing(8) + arc(~120) + value(36) + minmax(20) + padding
        let content_height = 220.0;
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

            ui.add_space(8.0);

            // Render the arc gauge
            let available_width = ui.available_width().min(280.0);
            self.render_arc(ui, available_width);

            // Value display in center area
            let value_color = self.color_for_value();
            let formatted = self.format_value();

            ui.label(
                RichText::new(format!("{}{}", formatted, self.unit))
                    .color(value_color)
                    .size(36.0)
                    .strong(),
            );

            // Min/Max labels
            if self.show_min_max {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(ui.available_width() * 0.15);
                    ui.label(
                        RichText::new(format!("{:.0}", self.min_value))
                            .color(text_col.gamma_multiply(0.4))
                            .size(11.0),
                    );
                    ui.add_space(ui.available_width() * 0.5);
                    ui.label(
                        RichText::new(format!("{:.0}", self.max_value))
                            .color(text_col.gamma_multiply(0.4))
                            .size(11.0),
                    );
                });
            }

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }
}

/// A single bar in a bar chart
#[derive(Debug, Clone)]
pub struct Bar {
    /// Label for this bar (e.g., "server1", "us-east")
    pub label: String,
    /// Value of this bar
    pub value: f64,
    /// Optional custom color (uses theme color if None)
    pub color: Option<Color32>,
}

impl Bar {
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            color: None,
        }
    }

    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

/// A horizontal bar chart visualization for comparing values across categories
pub struct BarChartViz {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    metric_name: String,
    /// Bars to display
    bars: Vec<Bar>,
    /// Current theme
    theme: AppTheme,
    /// Title (shown in tab)
    title: String,
    /// Whether to show values on bars
    show_values: bool,
    /// Whether bars are sorted by value (descending)
    sorted: bool,
}

impl Default for BarChartViz {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl BarChartViz {
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: next_id_usize(),
            title: name.clone(),
            metric_name: name,
            bars: Vec::new(),
            theme: AppTheme::default(),
            show_values: true,
            sorted: true,
        }
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

    /// Add a bar to the chart
    pub fn add_bar(&mut self, bar: Bar) {
        self.bars.push(bar);
    }

    /// Set all bars at once
    pub fn set_bars(&mut self, bars: Vec<Bar>) {
        self.bars = bars;
    }

    /// Clear all bars
    pub fn clear(&mut self) {
        self.bars.clear();
    }

    /// Set whether to show values on bars
    pub fn set_show_values(&mut self, show: bool) {
        self.show_values = show;
    }

    /// Set whether bars are sorted by value
    pub fn set_sorted(&mut self, sorted: bool) {
        self.sorted = sorted;
    }

    /// Get bars sorted by value (descending) if sorted is true
    fn get_display_bars(&self) -> Vec<&Bar> {
        let mut bars: Vec<&Bar> = self.bars.iter().collect();
        if self.sorted {
            bars.sort_by(|a, b| {
                b.value
                    .partial_cmp(&a.value)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        bars
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

    /// Render the bar chart
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);
        let accent_color = palette::accent::PRIMARY;

        ui.vertical(|ui| {
            ui.add_space(VIZ_PADDING_TOP);

            // Title / metric name
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&self.metric_name)
                        .color(text_col.gamma_multiply(0.6))
                        .size(13.0),
                );
            });

            ui.add_space(8.0);

            if self.bars.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No data")
                            .color(text_col.gamma_multiply(0.4))
                            .size(14.0),
                    );
                });
                return;
            }

            let bars = self.get_display_bars();
            let max_value = bars
                .iter()
                .map(|b| b.value)
                .fold(0.0_f64, |a, b| a.max(b))
                .max(0.001);

            // Calculate label width (for alignment)
            let label_width = 100.0_f32;
            let value_width = 60.0_f32;
            let bar_height = 24.0_f32;
            let bar_spacing = 4.0_f32;

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for bar in bars {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);

                            // Label (left-aligned, fixed width)
                            let label_text = if bar.label.len() > 12 {
                                format!("{}...", &bar.label[..12])
                            } else {
                                bar.label.clone()
                            };
                            ui.add_sized(
                                [label_width, bar_height],
                                egui::Label::new(
                                    RichText::new(label_text)
                                        .color(text_col.gamma_multiply(0.8))
                                        .size(12.0),
                                ),
                            );

                            // Bar
                            let available_width = ui.available_width() - value_width - 16.0;
                            let bar_width = (bar.value / max_value) as f32 * available_width;
                            let bar_color = bar.color.unwrap_or(accent_color);

                            let (rect, _response) = ui.allocate_exact_size(
                                egui::vec2(available_width, bar_height),
                                egui::Sense::hover(),
                            );

                            // Draw background
                            ui.painter()
                                .rect_filled(rect, 4.0, text_col.gamma_multiply(0.05));

                            // Draw filled bar
                            if bar_width > 0.0 {
                                let bar_rect = egui::Rect::from_min_size(
                                    rect.min,
                                    egui::vec2(bar_width.max(4.0), bar_height),
                                );
                                ui.painter().rect_filled(bar_rect, 4.0, bar_color);
                            }

                            // Value (right-aligned)
                            if self.show_values {
                                ui.add_sized(
                                    [value_width, bar_height],
                                    egui::Label::new(
                                        RichText::new(Self::format_value(bar.value))
                                            .color(text_col.gamma_multiply(0.7))
                                            .size(12.0),
                                    ),
                                );
                            }
                        });

                        ui.add_space(bar_spacing);
                    }
                });

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }
}

/// A standalone sparkline visualization showing trends in minimal space
pub struct SparklineViz {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    metric_name: String,
    /// Data points to display
    data: Vec<f64>,
    /// Current theme
    theme: AppTheme,
    /// Title (shown in tab)
    title: String,
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
            show_value: true,
            fill: true,
            color: None,
        }
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

            // Header with metric name and optional value
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&self.metric_name)
                        .color(text_col.gamma_multiply(0.6))
                        .size(13.0),
                );

                if self.show_value {
                    if let Some(value) = self.current_value() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(Self::format_value(value))
                                    .color(line_color)
                                    .size(18.0)
                                    .strong(),
                            );
                        });
                    }
                }
            });

            ui.add_space(8.0);

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

/// Enum wrapping all visualization types for use in QueryPane
pub enum Visualization {
    TimeSeries(TimeSeriesChart),
    Stat(StatChart),
    Gauge(GaugeChart),
    BarChart(BarChartViz),
    Sparkline(SparklineViz),
    Heatmap(HeatmapViz),
    Flamegraph(FlamegraphViz),
}

impl Visualization {
    /// Create a new visualization of the specified type
    pub fn new(viz_type: VisualizationType, metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        match viz_type {
            VisualizationType::TimeSeries => Self::TimeSeries(TimeSeriesChart::new(&name)),
            VisualizationType::Stat => Self::Stat(StatChart::new(&name)),
            VisualizationType::Gauge => Self::Gauge(GaugeChart::new(&name)),
            VisualizationType::BarChart => Self::BarChart(BarChartViz::new(&name)),
            VisualizationType::Sparkline => Self::Sparkline(SparklineViz::new(&name)),
            VisualizationType::Heatmap => Self::Heatmap(HeatmapViz::new(&name)),
            VisualizationType::Flamegraph => Self::Flamegraph(FlamegraphViz::new(&name)),
        }
    }

    /// Get the current visualization type
    pub fn viz_type(&self) -> VisualizationType {
        match self {
            Self::TimeSeries(_) => VisualizationType::TimeSeries,
            Self::Stat(_) => VisualizationType::Stat,
            Self::Gauge(_) => VisualizationType::Gauge,
            Self::BarChart(_) => VisualizationType::BarChart,
            Self::Sparkline(_) => VisualizationType::Sparkline,
            Self::Heatmap(_) => VisualizationType::Heatmap,
            Self::Flamegraph(_) => VisualizationType::Flamegraph,
        }
    }

    /// Cycle to the next visualization type, preserving metric name
    pub fn cycle(&mut self) {
        let next_type = self.viz_type().next();
        let metric_name = self.metric_name().to_string();
        let theme = self.theme();

        *self = Self::new(next_type, &metric_name);
        self.set_theme(theme);
    }

    /// Get the metric name
    pub fn metric_name(&self) -> &str {
        match self {
            Self::TimeSeries(chart) => &chart.metric_name,
            Self::Stat(stat) => &stat.metric_name,
            Self::Gauge(gauge) => &gauge.metric_name,
            Self::BarChart(bar) => &bar.metric_name,
            Self::Sparkline(spark) => &spark.metric_name,
            Self::Heatmap(heatmap) => &heatmap.metric_name,
            Self::Flamegraph(fg) => &fg.title,
        }
    }

    /// Get the theme
    fn theme(&self) -> AppTheme {
        match self {
            Self::TimeSeries(chart) => chart.theme,
            Self::Stat(stat) => stat.theme,
            Self::Gauge(gauge) => gauge.theme,
            Self::BarChart(bar) => bar.theme,
            Self::Sparkline(spark) => spark.theme,
            Self::Heatmap(heatmap) => heatmap.theme,
            Self::Flamegraph(fg) => fg.theme,
        }
    }

    /// Set the metric name
    pub fn set_metric_name(&mut self, name: impl Into<String>) {
        match self {
            Self::TimeSeries(chart) => chart.set_metric_name(name),
            Self::Stat(stat) => stat.set_metric_name(name),
            Self::Gauge(gauge) => gauge.set_metric_name(name),
            Self::BarChart(bar) => bar.set_metric_name(name),
            Self::Sparkline(spark) => spark.set_metric_name(name),
            Self::Heatmap(heatmap) => heatmap.set_metric_name(name),
            Self::Flamegraph(fg) => fg.set_title(name),
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        match self {
            Self::TimeSeries(chart) => chart.set_theme(theme),
            Self::Stat(stat) => stat.set_theme(theme),
            Self::Gauge(gauge) => gauge.set_theme(theme),
            Self::BarChart(bar) => bar.set_theme(theme),
            Self::Sparkline(spark) => spark.set_theme(theme),
            Self::Heatmap(heatmap) => heatmap.set_theme(theme),
            Self::Flamegraph(fg) => fg.set_theme(theme),
        }
    }

    /// Clear the visualization data
    pub fn clear(&mut self) {
        match self {
            Self::TimeSeries(chart) => chart.clear(),
            Self::Stat(stat) => stat.clear(),
            Self::Gauge(gauge) => gauge.clear(),
            Self::BarChart(bar) => bar.clear(),
            Self::Sparkline(spark) => spark.clear(),
            Self::Heatmap(heatmap) => heatmap.clear(),
            Self::Flamegraph(fg) => fg.clear(),
        }
    }

    /// Render the visualization
    pub fn show(&mut self, ui: &mut egui::Ui) {
        match self {
            Self::TimeSeries(chart) => chart.show(ui),
            Self::Stat(stat) => stat.show(ui),
            Self::Gauge(gauge) => gauge.show(ui),
            Self::BarChart(bar) => bar.show(ui),
            Self::Sparkline(spark) => spark.show(ui),
            Self::Heatmap(heatmap) => heatmap.show(ui),
            Self::Flamegraph(fg) => fg.show(ui),
        }
    }

    /// Add a series to the time series chart (no-op for other types)
    pub fn add_series(&mut self, series: Series) {
        if let Self::TimeSeries(chart) = self {
            chart.add_series(series);
        }
    }

    /// Set multiple series at once (clears existing data first)
    pub fn set_series(&mut self, series_list: Vec<Series>) {
        self.clear();
        for series in series_list {
            self.add_series(series);
        }
    }

    /// Add a commit marker (only for time series)
    pub fn add_commit(&mut self, commit: CommitMarker) {
        if let Self::TimeSeries(chart) = self {
            chart.add_commit(commit);
        }
    }

    /// Toggle commit marker visibility (only for time series)
    pub fn toggle_commits(&mut self) {
        if let Self::TimeSeries(chart) = self {
            chart.toggle_commits();
        }
    }

    /// Set stat value (only for stat visualization)
    pub fn set_stat_value(&mut self, value: f64) {
        if let Self::Stat(stat) = self {
            stat.set_value(value);
        }
    }

    /// Set stat unit (only for stat visualization)
    pub fn set_stat_unit(&mut self, unit: impl Into<String>) {
        if let Self::Stat(stat) = self {
            stat.set_unit(unit);
        }
    }

    /// Set stat sparkline data (only for stat visualization)
    pub fn set_stat_sparkline(&mut self, data: Vec<f64>) {
        if let Self::Stat(stat) = self {
            stat.set_sparkline_data(data);
        }
    }

    /// Set stat change indicator (only for stat visualization)
    pub fn set_stat_change(&mut self, value: f64, period: impl Into<String>) {
        if let Self::Stat(stat) = self {
            stat.set_change(value, period);
        }
    }

    /// Add a threshold (for stat and gauge visualizations)
    pub fn add_threshold(&mut self, threshold: Threshold) {
        match self {
            Self::Stat(stat) => stat.add_threshold(threshold),
            Self::Gauge(gauge) => gauge.add_threshold(threshold),
            _ => {}
        }
    }

    /// Get access to the underlying TimeSeriesChart (if applicable)
    pub fn as_time_series(&self) -> Option<&TimeSeriesChart> {
        match self {
            Self::TimeSeries(chart) => Some(chart),
            _ => None,
        }
    }

    /// Get mutable access to the underlying TimeSeriesChart (if applicable)
    pub fn as_time_series_mut(&mut self) -> Option<&mut TimeSeriesChart> {
        match self {
            Self::TimeSeries(chart) => Some(chart),
            _ => None,
        }
    }

    /// Get access to the underlying StatChart (if applicable)
    pub fn as_stat(&self) -> Option<&StatChart> {
        match self {
            Self::Stat(stat) => Some(stat),
            _ => None,
        }
    }

    /// Get mutable access to the underlying StatChart (if applicable)
    pub fn as_stat_mut(&mut self) -> Option<&mut StatChart> {
        match self {
            Self::Stat(stat) => Some(stat),
            _ => None,
        }
    }

    /// Get access to the underlying GaugeChart (if applicable)
    pub fn as_gauge(&self) -> Option<&GaugeChart> {
        match self {
            Self::Gauge(gauge) => Some(gauge),
            _ => None,
        }
    }

    /// Get mutable access to the underlying GaugeChart (if applicable)
    pub fn as_gauge_mut(&mut self) -> Option<&mut GaugeChart> {
        match self {
            Self::Gauge(gauge) => Some(gauge),
            _ => None,
        }
    }

    /// Get access to the underlying BarChartViz (if applicable)
    pub fn as_bar_chart(&self) -> Option<&BarChartViz> {
        match self {
            Self::BarChart(bar) => Some(bar),
            _ => None,
        }
    }

    /// Get mutable access to the underlying BarChartViz (if applicable)
    pub fn as_bar_chart_mut(&mut self) -> Option<&mut BarChartViz> {
        match self {
            Self::BarChart(bar) => Some(bar),
            _ => None,
        }
    }

    /// Get access to the underlying SparklineViz (if applicable)
    pub fn as_sparkline(&self) -> Option<&SparklineViz> {
        match self {
            Self::Sparkline(spark) => Some(spark),
            _ => None,
        }
    }

    /// Get mutable access to the underlying SparklineViz (if applicable)
    pub fn as_sparkline_mut(&mut self) -> Option<&mut SparklineViz> {
        match self {
            Self::Sparkline(spark) => Some(spark),
            _ => None,
        }
    }

    /// Get access to the underlying HeatmapViz (if applicable)
    pub fn as_heatmap(&self) -> Option<&HeatmapViz> {
        match self {
            Self::Heatmap(heatmap) => Some(heatmap),
            _ => None,
        }
    }

    /// Get mutable access to the underlying HeatmapViz (if applicable)
    pub fn as_heatmap_mut(&mut self) -> Option<&mut HeatmapViz> {
        match self {
            Self::Heatmap(heatmap) => Some(heatmap),
            _ => None,
        }
    }

    /// Get access to the underlying FlamegraphViz (if applicable)
    pub fn as_flamegraph(&self) -> Option<&FlamegraphViz> {
        match self {
            Self::Flamegraph(fg) => Some(fg),
            _ => None,
        }
    }

    /// Get mutable access to the underlying FlamegraphViz (if applicable)
    pub fn as_flamegraph_mut(&mut self) -> Option<&mut FlamegraphViz> {
        match self {
            Self::Flamegraph(fg) => Some(fg),
            _ => None,
        }
    }
}

/// Populate demo data for a visualization based on its type
pub fn populate_demo_data(viz: &mut Visualization, query: &str) {
    match viz {
        Visualization::TimeSeries(chart) => {
            populate_time_series_demo(chart, query);
        }
        Visualization::Stat(stat) => {
            populate_stat_demo(stat, query);
        }
        Visualization::Gauge(gauge) => {
            populate_gauge_demo(gauge, query);
        }
        Visualization::BarChart(bar) => {
            populate_bar_chart_demo(bar, query);
        }
        Visualization::Sparkline(spark) => {
            populate_sparkline_demo(spark, query);
        }
        Visualization::Heatmap(heatmap) => {
            populate_heatmap_demo(heatmap, query);
        }
        Visualization::Flamegraph(fg) => {
            populate_flamegraph_demo(fg, query);
        }
    }
}

/// Populate demo data for time series
fn populate_time_series_demo(chart: &mut TimeSeriesChart, query: &str) {
    // Generate some demo data based on query hash for variety
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));
    let now = 1_700_000_000.0;
    let duration = 86400.0;
    let num_points = 240;

    // Series 1
    let base1 = 50.0 + (hash % 50) as f64;
    let freq1 = 200.0 + (hash % 100) as f64;
    let points1: Vec<DataPoint> = (0..num_points)
        .map(|i| {
            let t = now + (i as f64 / num_points as f64) * duration;
            let base = base1 + 20.0 * (t / freq1).sin();
            let noise = (t * 17.0).sin() * 5.0;
            DataPoint {
                timestamp: t,
                value: base + noise,
            }
        })
        .collect();

    chart.add_series(
        Series::new(query)
            .with_tag("host", "server1")
            .with_points(points1)
            .with_color(Color32::from_rgb(59, 130, 246)),
    );

    // Series 2
    let base2 = 70.0 + (hash % 30) as f64;
    let freq2 = 150.0 + (hash % 80) as f64;
    let points2: Vec<DataPoint> = (0..num_points)
        .map(|i| {
            let t = now + (i as f64 / num_points as f64) * duration;
            let base = base2 + 15.0 * (t / freq2).cos();
            let noise = (t * 23.0).sin() * 3.0;
            DataPoint {
                timestamp: t,
                value: base + noise,
            }
        })
        .collect();

    chart.add_series(
        Series::new(query)
            .with_tag("host", "server2")
            .with_points(points2)
            .with_color(Color32::from_rgb(16, 185, 129)),
    );

    // Add demo commit markers
    chart.add_commit(CommitMarker::new(
        "a1b2c3d",
        now + duration * 0.1,
        "Fix connection pooling",
    ));
    chart.add_commit(CommitMarker::new(
        "e4f5g6h",
        now + duration * 0.35,
        "Add retry logic",
    ));
    chart.add_commit(CommitMarker::new(
        "i7j8k9l",
        now + duration * 0.5,
        "Update dependencies",
    ));
    chart.add_commit(CommitMarker::new(
        "m0n1o2p",
        now + duration * 0.7,
        "Refactor auth module",
    ));
    chart.add_commit(CommitMarker::new(
        "q3r4s5t",
        now + duration * 0.9,
        "Performance improvements",
    ));
}

/// Populate demo data for stat visualization
fn populate_stat_demo(stat: &mut StatChart, query: &str) {
    // Generate a demo value based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Generate a reasonable value
    let base_value = 1000.0 + (hash % 50000) as f64;
    stat.set_value(base_value);

    // Set unit based on common metric patterns
    let unit = if query.contains("latency") || query.contains("duration") {
        "ms"
    } else if query.contains("rate") || query.contains("percent") {
        "%"
    } else if query.contains("bytes") || query.contains("size") {
        "bytes"
    } else {
        "" // No unit
    };
    stat.set_unit(unit);

    // Generate sparkline data (last 24 data points)
    let sparkline: Vec<f64> = (0..24)
        .map(|i| ((i as f64 * 0.3 + hash as f64 * 0.01).sin() * 0.2 + 1.0) * base_value)
        .collect();
    stat.set_sparkline_data(sparkline);

    // Set change indicator
    let change = ((hash % 200) as f64 - 100.0) / 10.0; // -10% to +10%
    stat.set_change(change, "vs last hour");

    // Add some thresholds for visual interest
    stat.add_threshold(Threshold::new(base_value * 0.8, palette::semantic::WARNING));
    stat.add_threshold(Threshold::new(base_value * 1.2, palette::semantic::ERROR));
}

/// Populate demo data for gauge visualization
fn populate_gauge_demo(gauge: &mut GaugeChart, query: &str) {
    // Generate a demo value based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Determine if this looks like a percentage metric
    let is_percentage = query.contains("percent")
        || query.contains("utilization")
        || query.contains("usage")
        || query.contains("cpu")
        || query.contains("memory");

    if is_percentage {
        // Percentage gauge (0-100%)
        gauge.set_range(0.0, 100.0);
        gauge.set_unit("%");
        let value = (hash % 85) as f64 + 15.0; // 15-100%
        gauge.set_value(value);

        // Traffic light thresholds for utilization
        gauge.add_threshold(Threshold::new(70.0, palette::semantic::WARNING));
        gauge.add_threshold(Threshold::new(90.0, palette::semantic::ERROR));
    } else {
        // Generic gauge with custom range
        let max_val = 1000.0 + (hash % 9000) as f64;
        gauge.set_range(0.0, max_val);

        // Set unit based on metric patterns
        let unit = if query.contains("latency") || query.contains("duration") {
            "ms"
        } else if query.contains("bytes") || query.contains("size") {
            "MB"
        } else if query.contains("rate") || query.contains("rps") {
            "req/s"
        } else {
            ""
        };
        gauge.set_unit(unit);

        // Value somewhere in the range
        let value = (hash % (max_val as u64)) as f64;
        gauge.set_value(value);

        // Thresholds at 70% and 90% of max
        gauge.add_threshold(Threshold::new(max_val * 0.7, palette::semantic::WARNING));
        gauge.add_threshold(Threshold::new(max_val * 0.9, palette::semantic::ERROR));
    }
}

/// Populate demo data for bar chart visualization
fn populate_bar_chart_demo(bar: &mut BarChartViz, query: &str) {
    // Generate demo data based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Generate category names based on query content
    let categories: Vec<&str> = if query.contains("region") || query.contains("location") {
        vec![
            "us-east-1",
            "us-west-2",
            "eu-west-1",
            "ap-south-1",
            "ap-northeast-1",
        ]
    } else if query.contains("service") || query.contains("app") {
        vec![
            "api-gateway",
            "auth-service",
            "db-primary",
            "cache",
            "worker",
        ]
    } else if query.contains("host") || query.contains("server") {
        vec![
            "server-01",
            "server-02",
            "server-03",
            "server-04",
            "server-05",
        ]
    } else if query.contains("status") || query.contains("code") {
        vec![
            "200 OK",
            "201 Created",
            "400 Bad Request",
            "404 Not Found",
            "500 Error",
        ]
    } else {
        vec![
            "Category A",
            "Category B",
            "Category C",
            "Category D",
            "Category E",
        ]
    };

    // Generate values with some variation
    let bars: Vec<Bar> = categories
        .iter()
        .enumerate()
        .map(|(i, &label)| {
            let base = 100.0 + (hash % 900) as f64;
            let variation = ((hash.wrapping_add(i as u64 * 17)) % 100) as f64 / 100.0;
            let value = base * (0.3 + variation * 0.7);
            Bar::new(label, value)
        })
        .collect();

    bar.set_bars(bars);
}

/// Populate demo data for sparkline visualization
fn populate_sparkline_demo(spark: &mut SparklineViz, query: &str) {
    // Generate demo data based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Generate 50 data points with some variation
    let base_value = 100.0 + (hash % 500) as f64;
    let data: Vec<f64> = (0..50)
        .map(|i| {
            let trend = (i as f64 * 0.02).sin() * 0.15; // Slight trend
            let noise = ((hash.wrapping_add(i as u64) % 100) as f64 - 50.0) / 200.0; // Random noise
            let seasonal = ((i as f64 * 0.2 + hash as f64 * 0.01).sin()) * 0.1; // Seasonal pattern
            base_value * (1.0 + trend + noise + seasonal)
        })
        .collect();

    spark.set_data(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visualization_type_cycle() {
        let vt = VisualizationType::TimeSeries;
        assert_eq!(vt.next(), VisualizationType::Stat);
        assert_eq!(vt.next().next(), VisualizationType::Gauge);
        assert_eq!(vt.next().next().next(), VisualizationType::BarChart);
        assert_eq!(vt.next().next().next().next(), VisualizationType::Sparkline);
        assert_eq!(
            vt.next().next().next().next().next(),
            VisualizationType::Heatmap
        );
        assert_eq!(
            vt.next().next().next().next().next().next(),
            VisualizationType::Flamegraph
        );
        assert_eq!(
            vt.next().next().next().next().next().next().next(),
            VisualizationType::TimeSeries
        );
    }

    #[test]
    fn test_visualization_type_serialization() {
        assert_eq!(VisualizationType::TimeSeries.as_str(), "time_series");
        assert_eq!(VisualizationType::Stat.as_str(), "stat");
        assert_eq!(VisualizationType::Gauge.as_str(), "gauge");
        assert_eq!(VisualizationType::BarChart.as_str(), "bar_chart");
        assert_eq!(VisualizationType::Sparkline.as_str(), "sparkline");
        assert_eq!(VisualizationType::Heatmap.as_str(), "heatmap");
        assert_eq!(VisualizationType::Flamegraph.as_str(), "flamegraph");

        assert_eq!(
            VisualizationType::parse("time_series"),
            VisualizationType::TimeSeries
        );
        assert_eq!(VisualizationType::parse("stat"), VisualizationType::Stat);
        assert_eq!(VisualizationType::parse("gauge"), VisualizationType::Gauge);
        assert_eq!(
            VisualizationType::parse("bar_chart"),
            VisualizationType::BarChart
        );
        assert_eq!(
            VisualizationType::parse("sparkline"),
            VisualizationType::Sparkline
        );
        assert_eq!(
            VisualizationType::parse("heatmap"),
            VisualizationType::Heatmap
        );
        assert_eq!(
            VisualizationType::parse("flamegraph"),
            VisualizationType::Flamegraph
        );
        assert_eq!(
            VisualizationType::parse("unknown"),
            VisualizationType::TimeSeries
        );
    }

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

    #[test]
    fn test_visualization_cycle() {
        let mut viz = Visualization::new(VisualizationType::TimeSeries, "test_metric");
        assert_eq!(viz.viz_type(), VisualizationType::TimeSeries);

        viz.cycle();
        assert_eq!(viz.viz_type(), VisualizationType::Stat);
        assert_eq!(viz.metric_name(), "test_metric");

        viz.cycle();
        assert_eq!(viz.viz_type(), VisualizationType::Gauge);
        assert_eq!(viz.metric_name(), "test_metric");

        viz.cycle();
        assert_eq!(viz.viz_type(), VisualizationType::BarChart);
        assert_eq!(viz.metric_name(), "test_metric");

        viz.cycle();
        assert_eq!(viz.viz_type(), VisualizationType::Sparkline);
        assert_eq!(viz.metric_name(), "test_metric");

        viz.cycle();
        assert_eq!(viz.viz_type(), VisualizationType::Heatmap);
        assert_eq!(viz.metric_name(), "test_metric");

        viz.cycle();
        assert_eq!(viz.viz_type(), VisualizationType::Flamegraph);
        assert_eq!(viz.metric_name(), "test_metric");

        viz.cycle();
        assert_eq!(viz.viz_type(), VisualizationType::TimeSeries);
    }

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

    #[test]
    fn test_bar_chart_format_value() {
        assert_eq!(BarChartViz::format_value(75.0), "75");
        assert_eq!(BarChartViz::format_value(1234.0), "1.2K");
        assert_eq!(BarChartViz::format_value(1_234_567.0), "1.2M");
        assert_eq!(BarChartViz::format_value(42.5), "42.5");
    }

    #[test]
    fn test_bar_chart_sorting() {
        let mut bar = BarChartViz::new("test");
        bar.add_bar(Bar::new("A", 10.0));
        bar.add_bar(Bar::new("B", 30.0));
        bar.add_bar(Bar::new("C", 20.0));

        // With sorting enabled (default)
        let sorted = bar.get_display_bars();
        assert_eq!(sorted[0].label, "B"); // highest
        assert_eq!(sorted[1].label, "C");
        assert_eq!(sorted[2].label, "A"); // lowest

        // With sorting disabled
        bar.set_sorted(false);
        let unsorted = bar.get_display_bars();
        assert_eq!(unsorted[0].label, "A"); // insertion order
        assert_eq!(unsorted[1].label, "B");
        assert_eq!(unsorted[2].label, "C");
    }
}
