//! Core types for the SQL pane module.

use enya_datafusion::arrow::array::RecordBatch;
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::{ExecutionStats, QueryId};

use crate::util::Instant;

use super::connections::ConnectionId;

/// Actions that can be triggered by the SQL pane.
#[derive(Debug, Clone)]
pub enum SqlPaneAction {
    /// No action.
    None,
}

/// Current mode of the SQL pane.
#[derive(Debug, Clone, Default)]
pub enum SqlMode {
    /// Normal query mode.
    #[default]
    Normal,
    /// Diff mode - comparing two environments.
    Diff {
        left: ConnectionId,
        right: ConnectionId,
    },
    /// Explain mode - showing query plan.
    #[allow(dead_code)] // Used for UI rendering, may be set in future workflows
    Explain,
    /// Profile mode - detailed execution stats.
    Profile,
}

/// Active overlay for viewing results in expanded mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ResultOverlay {
    /// No overlay shown - compact preview mode.
    #[default]
    None,
    /// Full table view with pagination and filtering.
    Table,
    /// Query execution plan tree.
    Plan,
    /// Diff comparison between two results.
    #[allow(dead_code)] // Will be used for diff view feature
    Diff { other_idx: usize },
}

/// Status of a query execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum QueryStatus {
    /// Query is running.
    Running,
    /// Query completed successfully.
    Completed,
    /// Query failed with an error.
    Failed,
    /// Query was cancelled.
    Cancelled,
}

/// A single executed query with its results.
pub(super) struct QueryCell {
    /// The SQL query that was executed.
    pub sql: String,
    /// Query ID for tracking.
    pub id: QueryId,
    /// Current status of the query.
    pub status: QueryStatus,
    /// When the query started executing.
    pub started_at: Instant,
    /// Schema of results (if available).
    pub schema: Option<SchemaRef>,
    /// Result batches.
    pub batches: Vec<RecordBatch>,
    /// Execution statistics.
    pub stats: Option<ExecutionStats>,
    /// Error message if query failed.
    pub error: Option<String>,
    /// Whether this is an info/system message (not a user query).
    pub is_info: bool,
}
