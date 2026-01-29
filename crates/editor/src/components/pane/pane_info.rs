//! Pane information types for serialization and context.
//!
//! These types describe pane visualization data and are used for:
//! - Agent context generation (summarizing pane data for AI)
//! - Pane sharing and embedding
//! - Chat @mention autocomplete
//!
//! Note: This is in the `pane` module (not `chat`) so it's available even when
//! the `teams` feature is disabled, since agent_context and workspace use these types.

use super::time_series_chart::Series;
use super::visualization::VisualizationType;

/// Snapshot of visualization data for embedding or context generation.
#[derive(Debug, Clone)]
pub enum PaneVisualization {
    /// Time series chart with series data.
    TimeSeries { series: Vec<Series> },
    /// Stat card with current value and optional sparkline.
    Stat {
        value: f64,
        unit: String,
        sparkline: Vec<f64>,
    },
    /// Gauge with value and range.
    Gauge {
        value: f64,
        min: f64,
        max: f64,
        unit: String,
    },
    /// Bar chart with labeled bars.
    BarChart { bars: Vec<(String, f64)> },
    /// Sparkline (compact trend line).
    Sparkline { data: Vec<f64> },
    /// Heatmap placeholder (complex to embed, show as reference).
    Heatmap,
}

/// Information about an available pane for @mention autocomplete.
#[derive(Debug, Clone)]
pub struct PaneInfo {
    /// Display name for the pane.
    pub name: String,
    /// The visualization type.
    pub viz_type: VisualizationType,
    /// Snapshot of the visualization data.
    pub visualization: PaneVisualization,
}

/// Information about a commit for # reference autocomplete.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    /// Short commit hash (7 chars).
    pub short_hash: String,
    /// Full commit hash.
    pub full_hash: String,
    /// Commit message (first line).
    pub message: String,
    /// Unix timestamp.
    pub timestamp: i64,
    /// Full diff content for viewing.
    pub diff: String,
}
