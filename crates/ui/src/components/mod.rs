use std::any::Any;

use crate::theme::AppTheme;

pub mod buffer;
pub mod buffer_editor;
pub mod command_palette;
pub mod custom_queries;
pub mod fuzzy_finder;
pub mod inspector;
pub mod landing_page;
pub mod metrics_tree;
pub mod notifications;
pub mod query_pane;
pub mod query_state;
pub mod status_line;
pub mod tags;
pub mod time_range;
pub mod time_series_chart;

pub use buffer::{Buffer, BufferAction, BufferMode};
pub use buffer_editor::{BufferEditor, BufferEditorResult};
pub use command_palette::{CommandPalette, CommandResult};
pub use custom_queries::{CustomQueriesPanel, CustomQuery};
pub use fuzzy_finder::{FuzzyFinder, FuzzyItem};
pub use inspector::{
    InspectorPanel, InspectorTarget, MetricStats, inspector_toggle_button,
    metrics_panel_toggle_button,
};
pub use landing_page::{LandingPage, LandingPageAction};
pub use metrics_tree::{MetricCategory, MetricInfo, MetricSelection, MetricsTree};
pub use notifications::{Notification, NotificationLevel, NotificationManager};
pub use query_pane::{QueryPane, QueryPaneAction};
pub use query_state::{AggregationMode, Granularity, QueryState};
pub use status_line::{StatusLine, StatusMode};
pub use tags::{TagFilter, TagPath, TagTree};
pub use time_range::{TimeRange, TimeRangePreset, TimeRangeToolbar};
pub use time_series_chart::{DataPoint, Series, TimeSeriesChart};

/// Trait that defines an Enya Component
pub trait Component: Any {
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

    /// Get a reference to self as Any (for downcasting)
    fn as_any(&self) -> &dyn Any;
    /// Get a mutable reference to self as Any (for downcasting)
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
