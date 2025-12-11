use std::any::Any;

use crate::theme::AppTheme;

pub mod buffer;
pub mod buffer_editor;
pub mod command_palette;
pub mod custom_queries;
pub mod diagnostics_pane;
pub mod diff_view;
pub mod finder_utils;
pub mod info_overlay;
pub mod landing_page;
pub mod metrics_finder;
pub mod metrics_tree;
pub mod multi_edit;
pub mod notifications;
pub mod query_completion;
pub mod query_finder;
pub mod query_pane;
pub mod query_state;
pub mod query_validation;
pub mod status_line;
pub mod tags;
pub mod time_range;
pub mod time_series_chart;
pub mod visualization;
pub mod which_key;

pub use buffer::{Buffer, BufferAction, BufferMode};
pub use buffer_editor::{BufferEditor, BufferEditorResult};
pub use command_palette::{CommandPalette, CommandResult};
pub use custom_queries::{CustomQueriesPanel, CustomQuery};
pub use diagnostics_pane::{
    Diagnostic, DiagnosticLevel, DiagnosticSource, DiagnosticsFilter, DiagnosticsPane,
    DiagnosticsPaneAction,
};
pub use diff_view::{DiffOffset, DiffView, DiffViewAction};
pub use info_overlay::InfoOverlay;
pub use landing_page::{LandingPage, LandingPageAction};
pub use metrics_finder::{MetricItem, MetricsFinder};
pub use metrics_tree::{MetricCategory, MetricInfo, MetricSelection, MetricsTree};
pub use multi_edit::{EditExcerpt, MultiEditOverlay, MultiEditResult};
pub use notifications::{Notification, NotificationLevel, NotificationManager};
pub use query_completion::{CompletionItem, CompletionKind, CompletionResult, QueryCompletion};
pub use query_finder::{QueryFinder, QueryItem};
pub use query_pane::{QueryPane, QueryPaneAction};
pub use query_state::{AggregationMode, Granularity, QueryState};
pub use query_validation::{QueryValidator, ValidationResult, is_valid_query, validate_query};
pub use status_line::{Sparkline, StatusLine, StatusMode};
pub use tags::{TagFilter, TagPath, TagTree};
pub use time_range::{TimeRange, TimeRangePreset, TimeRangeToolbar};
pub use time_series_chart::{DataPoint, Series, TimeSeriesChart};
pub use visualization::{
    Bar, BarChartViz, GaugeChart, SparklineViz, StatChart, Threshold, Visualization,
    VisualizationType,
};
pub use which_key::WhichKey;

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
