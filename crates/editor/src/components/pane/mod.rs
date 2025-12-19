//! Pane components - tile content types that implement the Component trait.

pub mod flamegraph;
pub mod heatmap;
pub mod query_pane;
pub mod time_series_chart;
pub mod visualization;

pub use flamegraph::{FlameFrame, FlamegraphViz, ProfileType};
pub use heatmap::{HeatmapCell, HeatmapLabels, HeatmapViz};
pub use query_pane::{QueryPane, QueryPaneAction};
pub use time_series_chart::{DataPoint, Series, TimeSeriesChart};
pub use visualization::{
    Bar, BarChartViz, GaugeChart, SparklineViz, StatChart, Threshold, Visualization,
    VisualizationType, populate_demo_data,
};
