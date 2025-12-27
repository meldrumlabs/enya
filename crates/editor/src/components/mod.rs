use std::any::Any;

use crate::ui::theme::AppTheme;

pub mod overlay;
pub mod pane;
pub mod util;
pub mod widget;

// Re-export from pane
pub use pane::{
    Bar, BarChartViz, DataPoint, GaugeChart, HeatmapCell, HeatmapLabels, HeatmapViz, QueryPane,
    QueryPaneAction, Series, SparklineViz, StatChart, Threshold, TimeSeriesChart, Visualization,
    VisualizationType,
};

// Re-export from overlay
pub use overlay::{
    BufferEditor, BufferEditorResult, CommandPalette, CommandResult, Diagnostic, DiagnosticLevel,
    DiagnosticSource, DiagnosticsFilter, DiagnosticsPane, DiagnosticsPaneAction, EditExcerpt,
    InfoOverlay, MetricItem, MetricsFinder, MultiEditOverlay, MultiEditResult,
    SourcePreviewOverlay, SourcePreviewResult, TutorialOverlay, ViewportFilter,
    ViewportFilterResult, WhichKey, WorkspaceFinder, WorkspaceItem,
};

// Re-export from widget
pub use widget::{
    Buffer, BufferAction, BufferMode, LandingPage, LandingPageAction, Notification,
    NotificationLevel, NotificationManager, Sparkline, StatusLine, StatusMode, TimeRange,
    TimeRangePreset, TimeRangeToolbar,
};

// Re-export from util
pub use util::{
    Backend, CompletionItem, CompletionKind, CompletionResult, ExecuteParams, Finder, FinderColors,
    FinderConfig, FinderItem, FinderKeyboardInput, FinderResult, Granularity, MultiBufferMode,
    MultiBufferState, OverlayColors, OverlayStyle, OverlayStyleVariant, QueryCompletion,
    QueryExecutor, QueryPollResult, QueryState, Selection, ValidationResult, draw_backdrop,
    draw_separator, draw_separator_colored, is_valid_query, next_id, next_id_usize,
    render_key_badge, render_key_badge_large, validate_query,
};

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
