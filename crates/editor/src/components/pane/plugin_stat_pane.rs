//! Plugin stat pane component for displaying a big number with optional sparkline.
//!
//! This pane wraps the StatChart component and accepts data from Lua plugins,
//! allowing plugins to create custom stat views for metrics fetched via HTTP or
//! generated locally.
//!
//! ## Features
//!
//! - **Big number display** with automatic formatting (K, M, B suffixes)
//! - **Optional sparkline** showing recent history
//! - **Change indicator** showing delta vs previous period
//! - **Threshold-based coloring** for value states
//! - **Error display** if data fetch fails

use std::any::Any;

use egui::Color32;
use enya_plugin::{StatPaneConfig, StatPaneData};

use crate::components::pane::visualization::{StatChart, Threshold};
use crate::components::util::id_generator::next_id_usize;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

/// A plugin stat pane that displays a big number with optional sparkline.
///
/// This pane wraps the StatChart component and converts plugin data
/// types to the chart's internal representation.
pub struct PluginStatPane {
    /// Unique identifier for this pane
    id: usize,
    /// Display name for the pane (from config title)
    name: String,
    /// Current theme
    theme: AppTheme,
    /// Configuration for this pane type
    config: StatPaneConfig,
    /// The wrapped stat chart
    chart: StatChart,
    /// Current data (kept for reference/updates)
    data: StatPaneData,
    /// Error message to display (if any)
    error: Option<String>,
}

impl PluginStatPane {
    /// Create a new plugin stat pane with the given configuration and initial data.
    pub fn new(config: StatPaneConfig, data: StatPaneData) -> Self {
        let name = config.title.clone();
        let mut chart = StatChart::new(&name);

        // Set unit if configured
        if let Some(ref unit) = config.unit {
            chart.set_unit(unit);
        }

        // Apply initial data
        Self::apply_data_to_chart(&mut chart, &data);

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
    pub fn set_data(&mut self, data: StatPaneData) {
        self.error = data.error.clone();
        self.data = data.clone();

        if data.error.is_none() {
            Self::apply_data_to_chart(&mut self.chart, &data);
        }
    }

    /// Apply plugin data to the StatChart.
    fn apply_data_to_chart(chart: &mut StatChart, data: &StatPaneData) {
        chart.set_value(data.value);
        chart.set_sparkline_data(data.sparkline.clone());

        // Apply change indicator if present
        if let Some(change_value) = data.change_value {
            let period = data
                .change_period
                .clone()
                .unwrap_or_else(|| "vs last period".to_string());
            chart.set_change(change_value, period);
        } else {
            chart.clear_change();
        }

        // Apply thresholds
        chart.clear_thresholds();
        for thresh in &data.thresholds {
            let color = Self::parse_color(&thresh.color);
            let mut threshold = Threshold::new(thresh.value, color);
            if let Some(ref label) = thresh.label {
                threshold = threshold.with_label(label.clone());
            }
            chart.add_threshold(threshold);
        }
    }

    /// Parse a color string to Color32.
    fn parse_color(color_str: &str) -> Color32 {
        // First try semantic colors
        match color_str.to_lowercase().as_str() {
            "green" | "success" | "ok" => return Color32::from_rgb(76, 175, 80),
            "yellow" | "warning" | "warn" => return Color32::from_rgb(255, 193, 7),
            "red" | "error" | "critical" => return Color32::from_rgb(244, 67, 54),
            "blue" | "info" => return Color32::from_rgb(33, 150, 243),
            _ => {}
        }

        // Try parsing as hex color
        let hex = color_str.trim_start_matches('#');
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return Color32::from_rgb(r, g, b);
            }
        }

        // Default to primary text color (will be overridden by theme)
        Color32::WHITE
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

        // Show the stat chart
        self.chart.show(ui);
    }
}

/// Implement Component trait so PluginStatPane can be used in the dashboard.
impl crate::components::Component for PluginStatPane {
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
        // Not needed for plugin stat pane
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed for plugin stat pane
    }

    fn label(&self) -> egui::RichText {
        egui::RichText::new(format!("{} {}", semantic_icons::action::CHART, self.name))
    }

    fn description(&self) -> &str {
        "Custom stat from plugin"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
