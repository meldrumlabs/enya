//! Pane components - tile content types that implement the Component trait.

pub mod agent_pane;
pub mod annotation;
pub mod logs_pane;
pub mod query_pane;
#[cfg(not(target_arch = "wasm32"))]
pub mod terminal_pane;
pub mod time_series_chart;
pub mod visualization;

pub use agent_pane::{AgentPane, AgentPaneAction, InlineChart, InlineContent, InlineSource};
pub use logs_pane::{LogsBackend, LogsPane, LogsPaneAction};
#[cfg(not(target_arch = "wasm32"))]
pub use terminal_pane::{TerminalPane, TerminalPaneAction};
// Re-export AiProvider from util for backwards compatibility
pub use super::util::AiProvider as AgentAiProvider;
pub use annotation::{
    Annotation, AnnotationAuthor, AnnotationId, AnnotationPriority, AnnotationTarget,
};
pub use query_pane::{QueryPane, QueryPaneAction};
pub use time_series_chart::{CommitMarker, DataPoint, Series, TimeSeriesChart};
pub use visualization::{
    Bar, BarChartViz, GaugeChart, HeatmapCell, HeatmapLabels, HeatmapViz, SparklineViz, StatChart,
    Threshold, Visualization, VisualizationType, populate_demo_data,
};
