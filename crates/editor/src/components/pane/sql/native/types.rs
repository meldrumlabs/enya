//! Core types for the SQL pane module.

use enya_datafusion::arrow::array::RecordBatch;
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::{ColumnInfo, ExecutionStats, PlanNode, QueryId};

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

pub use super::super::SqlPaneAction;

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

/// Shared metadata for all cell types.
pub(super) struct CellMeta {
    /// Unique identifier for this cell.
    pub id: QueryId,
    /// The SQL query or display text.
    pub sql: String,
}

/// Data specific to a query result cell.
pub(super) struct QueryData {
    /// Current execution status.
    pub status: QueryStatus,
    /// Result schema (if available).
    pub schema: Option<SchemaRef>,
    /// Result data batches.
    pub batches: Vec<RecordBatch>,
    /// Execution statistics.
    pub stats: Option<ExecutionStats>,
    /// Error message if query failed.
    pub error: Option<String>,
}

/// Data specific to an info/system message cell.
pub(super) struct InfoData {
    /// Error message (if this is an error info cell).
    pub error: Option<String>,
}

/// Data specific to a diff comparison cell.
pub(super) struct DiffData {
    /// Current execution status.
    pub status: QueryStatus,
    /// Error message if diff failed.
    pub error: Option<String>,
    /// Diff comparison result.
    pub diff_result: Option<DiffQueryResult>,
}

/// Data specific to an explain plan cell.
pub(super) struct ExplainData {
    /// Current execution status.
    pub status: QueryStatus,
    /// Error message if explain failed.
    pub error: Option<String>,
}

/// The kind of cell, carrying variant-specific data.
#[allow(clippy::large_enum_variant)]
pub(super) enum CellKind {
    /// Standard query with tabular results.
    Query(QueryData),
    /// Info or error system message.
    Info(InfoData),
    /// Diff comparison between two connections.
    Diff(DiffData),
    /// Explain/analyze execution plan.
    Explain(ExplainData),
}

/// A single cell in the SQL notebook history.
pub(super) struct Cell {
    /// Shared metadata.
    pub meta: CellMeta,
    /// Cell-specific data.
    pub kind: CellKind,
}

impl Cell {
    /// Create a new query cell in Running state.
    pub fn query(sql: impl Into<String>, id: QueryId) -> Self {
        Self {
            meta: CellMeta {
                id,
                sql: sql.into(),
            },
            kind: CellKind::Query(QueryData {
                status: QueryStatus::Running,
                schema: None,
                batches: Vec::new(),
                stats: None,
                error: None,
            }),
        }
    }

    /// Create a completed query cell (e.g. from snapshot or demo).
    pub fn query_completed(
        sql: impl Into<String>,
        id: QueryId,
        schema: Option<SchemaRef>,
        batches: Vec<RecordBatch>,
        stats: Option<ExecutionStats>,
        error: Option<String>,
    ) -> Self {
        let status = if error.is_some() {
            QueryStatus::Failed
        } else {
            QueryStatus::Completed
        };
        Self {
            meta: CellMeta {
                id,
                sql: sql.into(),
            },
            kind: CellKind::Query(QueryData {
                status,
                schema,
                batches,
                stats,
                error,
            }),
        }
    }

    /// Create an info message cell.
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            meta: CellMeta {
                id: QueryId::new(),
                sql: message.into(),
            },
            kind: CellKind::Info(InfoData { error: None }),
        }
    }

    /// Create an error message cell.
    pub fn error(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            meta: CellMeta {
                id: QueryId::new(),
                sql: String::new(),
            },
            kind: CellKind::Info(InfoData { error: Some(msg) }),
        }
    }

    /// Create a diff cell in Running state.
    pub fn diff(sql: impl Into<String>, id: QueryId) -> Self {
        Self {
            meta: CellMeta {
                id,
                sql: sql.into(),
            },
            kind: CellKind::Diff(DiffData {
                status: QueryStatus::Running,
                error: None,
                diff_result: None,
            }),
        }
    }

    /// Create a completed diff cell with result.
    pub fn diff_completed(
        sql: impl Into<String>,
        id: QueryId,
        diff_result: DiffQueryResult,
    ) -> Self {
        Self {
            meta: CellMeta {
                id,
                sql: sql.into(),
            },
            kind: CellKind::Diff(DiffData {
                status: QueryStatus::Completed,
                error: None,
                diff_result: Some(diff_result),
            }),
        }
    }

    /// Create an explain cell in Completed state.
    pub fn explain(sql: impl Into<String>, id: QueryId) -> Self {
        Self {
            meta: CellMeta {
                id,
                sql: sql.into(),
            },
            kind: CellKind::Explain(ExplainData {
                status: QueryStatus::Completed,
                error: None,
            }),
        }
    }

    // --- Convenience accessors ---

    /// Cell identifier.
    pub fn id(&self) -> QueryId {
        self.meta.id
    }

    /// The SQL text or display message.
    pub fn sql(&self) -> &str {
        &self.meta.sql
    }

    /// Current execution status.
    pub fn status(&self) -> QueryStatus {
        match &self.kind {
            CellKind::Query(q) => q.status.clone(),
            CellKind::Info(i) => {
                if i.error.is_some() {
                    QueryStatus::Failed
                } else {
                    QueryStatus::Completed
                }
            }
            CellKind::Diff(d) => d.status.clone(),
            CellKind::Explain(e) => e.status.clone(),
        }
    }

    /// Error message, if any.
    pub fn get_error(&self) -> Option<&str> {
        match &self.kind {
            CellKind::Query(q) => q.error.as_deref(),
            CellKind::Info(i) => i.error.as_deref(),
            CellKind::Diff(d) => d.error.as_deref(),
            CellKind::Explain(e) => e.error.as_deref(),
        }
    }

    /// Result schema (query cells only).
    pub fn schema(&self) -> Option<&SchemaRef> {
        match &self.kind {
            CellKind::Query(q) => q.schema.as_ref(),
            _ => None,
        }
    }

    /// Result data batches (query cells only, empty for others).
    pub fn batches(&self) -> &[RecordBatch] {
        match &self.kind {
            CellKind::Query(q) => &q.batches,
            _ => &[],
        }
    }

    /// Execution statistics (query cells only).
    pub fn stats(&self) -> Option<&ExecutionStats> {
        match &self.kind {
            CellKind::Query(q) => q.stats.as_ref(),
            _ => None,
        }
    }

    /// Diff result (diff cells only).
    pub fn diff_result(&self) -> Option<&DiffQueryResult> {
        match &self.kind {
            CellKind::Diff(d) => d.diff_result.as_ref(),
            _ => None,
        }
    }

    /// Whether this is an info/system message cell.
    pub fn is_info(&self) -> bool {
        matches!(self.kind, CellKind::Info(_))
    }

    /// Whether this cell is navigable (non-info cells).
    pub fn is_navigable(&self) -> bool {
        !self.is_info()
    }

    /// Get mutable access to query data (returns None for non-query cells).
    pub fn as_query_mut(&mut self) -> Option<&mut QueryData> {
        match &mut self.kind {
            CellKind::Query(q) => Some(q),
            _ => None,
        }
    }

    /// Get mutable access to diff data (returns None for non-diff cells).
    pub fn as_diff_mut(&mut self) -> Option<&mut DiffData> {
        match &mut self.kind {
            CellKind::Diff(d) => Some(d),
            _ => None,
        }
    }

    /// Set status across any cell kind that has one.
    pub fn set_status(&mut self, status: QueryStatus) {
        match &mut self.kind {
            CellKind::Query(q) => q.status = status,
            CellKind::Diff(d) => d.status = status,
            CellKind::Explain(e) => e.status = status,
            CellKind::Info(_) => {}
        }
    }

    /// Set error across any cell kind.
    pub fn set_error(&mut self, error: String) {
        match &mut self.kind {
            CellKind::Query(q) => q.error = Some(error),
            CellKind::Diff(d) => d.error = Some(error),
            CellKind::Explain(e) => e.error = Some(error),
            CellKind::Info(i) => i.error = Some(error),
        }
    }
}

/// Per-cell UI state.
#[derive(Debug, Clone, Default)]
pub(super) struct CellViewState {
    /// Current page in table view (0-indexed).
    pub table_page: usize,
    /// Column to sort by (None = original order).
    pub sort_column: Option<usize>,
    /// Sort direction (true = ascending).
    pub sort_ascending: bool,
}

/// Transient info/error message displayed between the result cell and input bar.
pub(super) struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}
