//! Visualization types for dashboard panes
//!
//! This module provides an enum-based abstraction over different visualization types,
//! allowing a single QueryPane to switch between time series charts, stat displays,
//! gauges, and other visualization styles (similar to Grafana).

mod bar;
mod demo;
mod gauge;
mod sparkline;
mod stat;
mod suggester;

pub use bar::{Bar, BarChartViz};
pub use demo::populate_demo_data;
pub use gauge::GaugeChart;
pub use sparkline::SparklineViz;
pub use stat::{StatChart, Threshold};
pub use suggester::{ResultCharacteristics, suggest_visualization};

use crate::theme::AppTheme;
use crate::ui::semantic_icons;

use super::flamegraph::FlamegraphViz;
use super::heatmap::HeatmapViz;
use super::time_series_chart::{CommitMarker, Series, TimeSeriesChart};

/// Standard padding for visualization types (for consistent spacing)
pub const VIZ_PADDING_TOP: f32 = 16.0;
pub const VIZ_PADDING_BOTTOM: f32 = 16.0;

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
}
