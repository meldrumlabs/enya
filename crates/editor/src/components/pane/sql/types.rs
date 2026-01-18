//! Core types for the SQL pane module.

use enya_datafusion::arrow::array::RecordBatch;
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::{ColumnInfo, ExecutionStats, PlanNode, QueryId};

use crate::util::Instant;

use super::connections::ConnectionId;

/// Type of diff being displayed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum DiffType {
    /// Query result comparison.
    #[default]
    Data,
    /// Execution plan comparison.
    Plan,
    /// Table schema comparison.
    Schema,
    /// EXPLAIN ANALYZE profile comparison (with metric highlighting).
    Profile,
}

/// Status of a column in a schema diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ColumnDiffStatus {
    /// Column exists in both with same definition.
    Matching,
    /// Column only in left schema.
    LeftOnly,
    /// Column only in right schema.
    RightOnly,
    /// Column exists in both but definition differs.
    Changed,
}

/// A column in the schema diff.
#[derive(Debug, Clone)]
pub(super) struct SchemaDiffColumn {
    /// Column name.
    pub name: String,
    /// Type in left schema (if present).
    pub left_type: Option<String>,
    /// Nullable in left schema (if present).
    pub left_nullable: Option<bool>,
    /// Type in right schema (if present).
    pub right_type: Option<String>,
    /// Nullable in right schema (if present).
    pub right_nullable: Option<bool>,
    /// Status of this column in the diff.
    pub status: ColumnDiffStatus,
}

/// Schema diff result.
#[derive(Debug, Clone)]
pub(super) struct SchemaDiffResult {
    /// Table name being compared.
    pub table_name: String,
    /// Columns in the diff.
    pub columns: Vec<SchemaDiffColumn>,
    /// Count of matching columns.
    pub matching: usize,
    /// Count of left-only columns.
    pub left_only: usize,
    /// Count of right-only columns.
    pub right_only: usize,
    /// Count of changed columns.
    pub changed: usize,
}

impl SchemaDiffResult {
    /// Create a new schema diff result from two column lists.
    pub fn from_columns(
        table_name: &str,
        left_columns: &[ColumnInfo],
        right_columns: &[ColumnInfo],
    ) -> Self {
        super::diff::compute_schema_diff(table_name, left_columns, right_columns)
    }
}

/// A row in the profile diff split view.
#[derive(Debug, Clone)]
pub(super) struct ProfileRow {
    /// Operator name.
    pub operator: String,
    /// Operator description.
    pub description: String,
    /// Depth in the plan tree.
    pub depth: usize,
    /// Elapsed time in milliseconds.
    pub time_ms: u64,
    /// Time from the other side (for delta calculation).
    pub other_time_ms: Option<u64>,
    /// Output row count.
    pub rows: usize,
}

/// Statistics about differences between two result sets.
#[derive(Debug, Clone, Default)]
pub(super) struct DiffStats {
    /// Number of rows only in the left result set.
    pub left_only: usize,
    /// Number of rows only in the right result set.
    pub right_only: usize,
    /// Number of rows with differing values.
    pub different: usize,
    /// Number of rows that match exactly.
    pub matching: usize,
}

/// Result from a diff query comparing two connections.
pub(super) struct DiffQueryResult {
    /// Name of the left connection.
    pub left_name: String,
    /// Name of the right connection.
    pub right_name: String,
    /// Schema from the left query (if successful).
    pub left_schema: Option<SchemaRef>,
    /// Batches from the left query.
    pub left_batches: Vec<RecordBatch>,
    /// Error message from the left query (if failed).
    pub left_error: Option<String>,
    /// Schema from the right query (if successful).
    pub right_schema: Option<SchemaRef>,
    /// Batches from the right query.
    pub right_batches: Vec<RecordBatch>,
    /// Error message from the right query (if failed).
    pub right_error: Option<String>,
    /// Whether the schemas match between the two results.
    pub schemas_match: bool,
    /// Diff statistics (if both queries succeeded).
    pub diff_stats: Option<DiffStats>,
    /// Left execution plan (for plan diff mode).
    pub left_plan: Option<PlanNode>,
    /// Right execution plan (for plan diff mode).
    pub right_plan: Option<PlanNode>,
    /// Type of diff (data, plan, schema, profile).
    pub diff_type: DiffType,
    /// Schema diff result (for schema diff mode).
    pub schema_diff: Option<SchemaDiffResult>,
}

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
    /// Diff result when comparing two connections.
    pub diff_result: Option<DiffQueryResult>,
}
