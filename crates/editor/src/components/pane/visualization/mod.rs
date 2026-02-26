//! Visualization types for dashboard panes
//!
//! This module provides an enum-based abstraction over different visualization types,
//! allowing a single QueryPane to switch between time series charts, stat displays,
//! gauges, and other visualization styles (similar to Grafana).

mod bar;
mod demo;
mod gauge;
mod heatmap;
mod sparkline;
mod stat;
mod suggester;

pub use bar::{Bar, BarChartViz};
pub use demo::populate_demo_data;
pub use gauge::GaugeChart;
pub use heatmap::{HeatmapCell, HeatmapLabels, HeatmapViz};
pub use sparkline::SparklineViz;
pub use stat::{StatChart, Threshold};
pub use suggester::{ResultCharacteristics, suggest_visualization};

use enya_config::{SnapshotPaneData, SnapshotSeries};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

use super::annotation::{Annotation, AnnotationId};
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
    /// Heatmap for 2D data grids
    Heatmap,
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
            Self::Heatmap => Self::TimeSeries,
        }
    }

    /// Get all visualization types
    pub fn all() -> &'static [Self] {
        &[
            Self::TimeSeries,
            Self::Stat,
            Self::Gauge,
            Self::BarChart,
            Self::Sparkline,
            Self::Heatmap,
        ]
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
        }
    }

    /// Extract current visualization data as a snapshot.
    pub fn extract_snapshot_data(&self) -> SnapshotPaneData {
        match self {
            Self::TimeSeries(chart) => {
                let series = chart
                    .series()
                    .iter()
                    .map(|s| {
                        let mut tags: Vec<(String, String)> =
                            s.tags.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        tags.sort_by(|a, b| a.0.cmp(&b.0));
                        SnapshotSeries {
                            name: s.name.clone(),
                            tags,
                            points: s.points.iter().map(|p| (p.timestamp, p.value)).collect(),
                        }
                    })
                    .collect();
                SnapshotPaneData::TimeSeries { series }
            }
            Self::Stat(stat) => SnapshotPaneData::Stat {
                value: stat.value(),
                sparkline: stat.sparkline_data().to_vec(),
            },
            Self::Gauge(gauge) => SnapshotPaneData::Gauge {
                value: gauge.value(),
                min: gauge.min(),
                max: gauge.max(),
            },
            Self::BarChart(bar) => SnapshotPaneData::BarChart {
                bars: bar
                    .bars()
                    .iter()
                    .map(|b| (b.label.clone(), b.value))
                    .collect(),
            },
            Self::Sparkline(spark) => SnapshotPaneData::TimeSeries {
                series: vec![SnapshotSeries {
                    name: spark.metric_name.clone(),
                    tags: Vec::new(),
                    points: spark
                        .data()
                        .iter()
                        .enumerate()
                        .map(|(i, &v)| (i as f64, v))
                        .collect(),
                }],
            },
            Self::Heatmap(heatmap) => {
                let (cols, rows) = heatmap.grid_size();
                let mut values = vec![0.0f32; cols * rows];
                for cell in heatmap.cells() {
                    if cell.col < cols && cell.row < rows {
                        values[cell.row * cols + cell.col] = cell.value;
                    }
                }
                SnapshotPaneData::Heatmap {
                    cols: cols as u16,
                    rows: rows as u16,
                    values,
                }
            }
        }
    }

    /// Populate this visualization from snapshot data.
    pub fn load_snapshot_data(&mut self, data: &SnapshotPaneData) {
        match data {
            SnapshotPaneData::TimeSeries { series } => {
                let series_list: Vec<Series> = series
                    .iter()
                    .map(|s| {
                        let tags = s.tags.iter().cloned().collect();
                        let points = s
                            .points
                            .iter()
                            .map(|&(t, v)| super::time_series_chart::DataPoint {
                                timestamp: t,
                                value: v,
                            })
                            .collect();
                        Series::new(&s.name).with_tags_map(tags).with_points(points)
                    })
                    .collect();
                self.set_series(series_list);
            }
            SnapshotPaneData::Stat { value, sparkline } => {
                self.set_stat_value(*value);
                self.set_stat_sparkline(sparkline.clone());
            }
            SnapshotPaneData::Gauge { value, min, max } => {
                if let Self::Gauge(gauge) = self {
                    gauge.set_range(*min, *max);
                    gauge.set_value(*value);
                }
            }
            SnapshotPaneData::BarChart { bars } => {
                if let Self::BarChart(bar) = self {
                    bar.set_bars(
                        bars.iter()
                            .map(|(label, value)| Bar::new(label, *value))
                            .collect(),
                    );
                }
            }
            SnapshotPaneData::Heatmap { cols, rows, values } => {
                if let Self::Heatmap(heatmap) = self {
                    // Convert flat values back to 2D array
                    let cols = *cols as usize;
                    let rows = *rows as usize;
                    let mut data_2d = Vec::with_capacity(rows);
                    for r in 0..rows {
                        let start = r * cols;
                        let end = (start + cols).min(values.len());
                        data_2d.push(values[start..end].iter().map(|&v| v as f64).collect());
                    }
                    heatmap.set_data(data_2d);
                }
            }
        }
    }

    /// Render the visualization
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        match self {
            Self::TimeSeries(chart) => chart.show(ui),
            Self::Stat(stat) => stat.show(ui),
            Self::Gauge(gauge) => gauge.show(ui),
            Self::BarChart(bar) => bar.show(ui),
            Self::Sparkline(spark) => spark.show(ui),
            Self::Heatmap(heatmap) => heatmap.show(ui),
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

    /// Set all commit markers at once (only for time series)
    pub fn set_commits(&mut self, commits: Vec<CommitMarker>) {
        if let Self::TimeSeries(chart) = self {
            chart.set_commits(commits);
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

    /// Get the unit suffix for values (e.g., "ms", "req/s", "%")
    pub fn unit(&self) -> &str {
        match self {
            Self::TimeSeries(chart) => chart.unit(),
            Self::Stat(stat) => stat.unit(),
            Self::Gauge(gauge) => gauge.unit(),
            Self::BarChart(bar) => bar.unit(),
            Self::Sparkline(spark) => spark.unit(),
            Self::Heatmap(_) => "",
        }
    }

    /// Set the unit suffix for values (e.g., "ms", "req/s", "%")
    /// This applies to visualization types that support units.
    pub fn set_unit(&mut self, unit: impl Into<String>) {
        let unit = unit.into();
        match self {
            Self::TimeSeries(chart) => chart.set_unit(unit),
            Self::Stat(stat) => stat.set_unit(unit),
            Self::Gauge(gauge) => gauge.set_unit(unit),
            Self::BarChart(bar) => bar.set_unit(unit),
            Self::Sparkline(spark) => spark.set_unit(unit),
            Self::Heatmap(_) => {
                // Heatmaps don't use simple unit suffixes
            }
        }
    }

    // ==================== Annotation Methods ====================

    /// Add an annotation (only for time series charts).
    pub fn add_annotation(&mut self, annotation: Annotation) {
        if let Self::TimeSeries(chart) = self {
            chart.add_annotation(annotation);
        }
    }

    /// Update an existing annotation (only for time series charts).
    pub fn update_annotation(&mut self, annotation: Annotation) {
        if let Self::TimeSeries(chart) = self {
            if let Some(existing) = chart.find_annotation_mut(annotation.id) {
                *existing = annotation;
            }
        }
    }

    /// Remove an annotation (only for time series charts).
    pub fn remove_annotation(&mut self, id: AnnotationId) {
        if let Self::TimeSeries(chart) = self {
            chart.remove_annotation(id);
        }
    }

    /// Get all annotations (only for time series charts).
    pub fn annotations(&self) -> Vec<&Annotation> {
        if let Self::TimeSeries(chart) = self {
            chart.annotations().iter().collect()
        } else {
            Vec::new()
        }
    }

    /// Toggle annotations visibility (only for time series charts).
    pub fn toggle_annotations(&mut self) {
        if let Self::TimeSeries(chart) = self {
            chart.toggle_annotations();
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
        assert_eq!(viz.viz_type(), VisualizationType::TimeSeries);
    }
}
