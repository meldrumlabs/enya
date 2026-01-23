//! Plugin chart pane component for displaying time series data from plugins.
//!
//! This pane wraps the TimeSeriesChart component and accepts data from Lua plugins,
//! allowing plugins to create custom chart views for data fetched via HTTP or
//! generated locally.
//!
//! ## Features
//!
//! - **Time series charting** with multiple series support
//! - **Auto-scaling** Y-axis based on data range
//! - **Interactive zoom** and pan controls
//! - **Legend** showing series names and colors
//! - **Error display** if data fetch fails

use std::any::Any;

use enya_plugin::{ChartSeries, CustomChartConfig, CustomChartData};

use crate::components::pane::time_series_chart::{DataPoint, Series, TimeSeriesChart};
use crate::components::util::id_generator::next_id_usize;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

/// A plugin chart pane that displays time series data from plugins.
///
/// This pane wraps the TimeSeriesChart component and converts plugin data
/// types to the chart's internal representation.
pub struct PluginChartPane {
    /// Unique identifier for this pane
    id: usize,
    /// Display name for the pane (from config title)
    name: String,
    /// Current theme
    theme: AppTheme,
    /// Configuration for this pane type
    config: CustomChartConfig,
    /// The wrapped time series chart
    chart: TimeSeriesChart,
    /// Current data (kept for reference/updates)
    data: CustomChartData,
    /// Error message to display (if any)
    error: Option<String>,
}

impl PluginChartPane {
    /// Create a new plugin chart pane with the given configuration and initial data.
    pub fn new(config: CustomChartConfig, data: CustomChartData) -> Self {
        let name = config.title.clone();
        let mut chart = TimeSeriesChart::new(&name);

        // Set unit if configured
        if let Some(ref unit) = config.y_unit {
            chart.set_unit(unit);
        }

        // Convert plugin data to chart format and add to chart
        for series in Self::convert_series(&data.series) {
            chart.add_series(series);
        }

        Self {
            id: next_id_usize(),
            name,
            theme: AppTheme::default(),
            config,
            chart,
            error: data.error.clone(),
            data,
        }
    }

    /// Get the pane type name (for matching updates by type).
    pub fn pane_type(&self) -> &str {
        &self.config.name
    }

    /// Set new data for this pane.
    pub fn set_data(&mut self, data: CustomChartData) {
        self.error = data.error.clone();
        self.data = data.clone();

        if data.error.is_none() {
            // Clear existing series and add new ones
            self.chart.clear();
            for series in Self::convert_series(&data.series) {
                self.chart.add_series(series);
            }
        }
    }

    /// Convert plugin ChartSeries to TimeSeriesChart Series.
    fn convert_series(plugin_series: &[ChartSeries]) -> Vec<Series> {
        plugin_series
            .iter()
            .map(|ps| {
                let mut series = Series::new(&ps.name);

                // Convert tags
                for (key, value) in &ps.tags {
                    series = series.with_tag(key, value);
                }

                // Convert points
                let points: Vec<DataPoint> = ps
                    .points
                    .iter()
                    .map(|p| DataPoint {
                        timestamp: p.timestamp,
                        value: p.value,
                    })
                    .collect();
                series = series.with_points(points);

                series
            })
            .collect()
    }

    /// Internal show implementation that handles error and empty states.
    fn show_internal(&mut self, ui: &mut egui::Ui) {
        // If there's an error, show it
        if let Some(ref error) = self.error {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new(semantic_icons::status::ERROR)
                            .size(32.0)
                            .color(self.theme.semantic_error()),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Error")
                            .size(16.0)
                            .color(self.theme.text_primary()),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(error)
                            .size(12.0)
                            .color(self.theme.text_secondary()),
                    );
                });
            });
            return;
        }

        // If no data, show empty state
        if self.data.series.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new(semantic_icons::file::DATA)
                            .size(32.0)
                            .color(self.theme.text_tertiary()),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("No data")
                            .size(14.0)
                            .color(self.theme.text_secondary()),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Waiting for plugin to provide chart data...")
                            .size(12.0)
                            .color(self.theme.text_tertiary()),
                    );
                });
            });
            return;
        }

        // Show the chart
        self.chart.show(ui);
    }
}

/// Implement Component trait so PluginChartPane can be used in the dashboard.
impl crate::components::Component for PluginChartPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        self.show_internal(ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.chart.set_theme(theme);
    }

    fn set_api_key(&mut self, _key: &str) {
        // Not needed for plugin chart pane
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed for plugin chart pane
    }

    fn label(&self) -> egui::RichText {
        egui::RichText::new(format!("{} {}", semantic_icons::action::CHART, self.name))
    }

    fn description(&self) -> &str {
        "Custom chart from plugin"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
