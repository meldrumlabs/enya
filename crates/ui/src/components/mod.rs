use crate::theme::AppTheme;

pub mod command_palette;
pub mod custom_queries;
pub mod fuzzy_finder;
pub mod inspector;
pub mod metrics_tree;
pub mod status_line;
pub mod time_range;
pub mod time_series_chart;

pub use command_palette::{CommandPalette, CommandResult};
pub use custom_queries::{CustomQueriesPanel, CustomQuery};
pub use fuzzy_finder::{FuzzyFinder, FuzzyItem};
pub use inspector::{
    InspectorPanel, InspectorTarget, MetricStats, inspector_toggle_button,
    metrics_panel_toggle_button,
};
pub use metrics_tree::{MetricCategory, MetricInfo, MetricSelection, MetricsTree};
pub use status_line::{StatusLine, StatusMode};
pub use time_range::{TimeRange, TimeRangePreset, TimeRangeToolbar};
pub use time_series_chart::{DataPoint, Series, TimeSeriesChart};

/// Trait that defines an Enya Component
pub trait Component {
    /// The core function that is responsible for drawing the component
    fn show(&mut self, ui: &mut egui::Ui);
    /// Returns the identifier for the component
    fn id(&self) -> usize;
    /// Returns the name for the component (e.g., SQL)
    fn name(&self) -> String;
    /// Saves the current theme for the component
    fn set_theme(&mut self, theme: AppTheme);
    fn set_api_key(&mut self, key: &str);
    fn set_staging_api_key(&mut self, key: &str);
    /// Returns a RichText label for the given component
    fn label(&self) -> egui::RichText;
}
