//! Pane components - tile content types that implement the Component trait.

pub mod annotation;
pub mod git;
pub mod inline_content;
pub mod logs_pane;
pub mod pane_info;
pub mod plugin;
pub mod query_pane;
pub mod sql;
#[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
pub mod terminal_pane;
pub mod time_series_chart;
pub mod tracing;
pub mod tracing_pane;
pub mod visualization;

pub use git::{PrReviewPane, PrReviewPaneAction};
pub use inline_content::{
    InlineChart, InlineContent, InlineDiff, InlineDiffFile, InlineDiffLine, InlineDiffLineKind,
    InlineSearchResults, InlineSource, InlineTable, InlineTableColumn, SearchResultItem,
};
pub use logs_pane::{LogsBackend, LogsPane, LogsPaneAction};
pub use plugin::{PluginChartPane, PluginGaugePane, PluginStatPane, PluginTablePane};
#[cfg(all(not(target_arch = "wasm32"), feature = "sql"))]
pub use sql::{DiffView, PlanTreeView, PlanViewMode, PlanViewer, StatsView};
pub use sql::{SqlPane, SqlPaneAction};
#[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
pub use terminal_pane::{TerminalPane, TerminalPaneAction};
pub use tracing_pane::{TracingPane, TracingPaneAction};
// Re-export AiProvider from util for backwards compatibility
pub use super::util::AiProvider as AgentAiProvider;
pub use annotation::{
    Annotation, AnnotationAuthor, AnnotationId, AnnotationPriority, AnnotationTarget,
};
pub use pane_info::{CommitInfo, PaneInfo, PaneVisualization};
pub use query_pane::{QueryPane, QueryPaneAction};
pub use time_series_chart::{CommitMarker, DataPoint, Series, TimeSeriesChart};
pub use visualization::{
    Bar, BarChartViz, GaugeChart, HeatmapCell, HeatmapLabels, HeatmapViz, SparklineViz, StatChart,
    Threshold, Visualization, VisualizationType, populate_demo_data,
};
