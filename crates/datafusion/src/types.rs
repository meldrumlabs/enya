//! Core types for DataFusion operations.

use std::time::Duration;

use rustc_hash::FxHashMap;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use datafusion::scalar::ScalarValue;

pub use crate::error::QueryId;

/// File format for data sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileFormat {
    Parquet,
    Csv,
    Json,
    NdJson,
    Arrow,
    Avro,
}

impl FileFormat {
    /// Detect format from file extension.
    pub fn from_path(path: &str) -> Option<Self> {
        let path = path.to_lowercase();
        if path.ends_with(".parquet") || path.ends_with(".pq") {
            Some(Self::Parquet)
        } else if path.ends_with(".csv") {
            Some(Self::Csv)
        } else if path.ends_with(".json") {
            Some(Self::Json)
        } else if path.ends_with(".ndjson") || path.ends_with(".jsonl") {
            Some(Self::NdJson)
        } else if path.ends_with(".arrow") || path.ends_with(".ipc") {
            Some(Self::Arrow)
        } else if path.ends_with(".avro") {
            Some(Self::Avro)
        } else {
            None
        }
    }

    /// Get the format name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Parquet => "parquet",
            Self::Csv => "csv",
            Self::Json => "json",
            Self::NdJson => "ndjson",
            Self::Arrow => "arrow",
            Self::Avro => "avro",
        }
    }
}

/// Request to execute a SQL query.
#[derive(Debug, Clone)]
pub struct QueryRequest {
    /// Unique ID for this query.
    pub id: QueryId,
    /// SQL query text.
    pub sql: String,
    /// Maximum rows to return (None = unlimited).
    pub limit: Option<usize>,
    /// Whether to collect execution metrics.
    pub collect_metrics: bool,
}

impl QueryRequest {
    /// Create a new query request.
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            id: QueryId::new(),
            sql: sql.into(),
            limit: None,
            collect_metrics: false,
        }
    }

    /// Set a row limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Enable metrics collection.
    pub fn with_metrics(mut self) -> Self {
        self.collect_metrics = true;
        self
    }

    /// Use a specific query ID.
    pub fn with_id(mut self, id: QueryId) -> Self {
        self.id = id;
        self
    }
}

/// Request to explain a query plan.
#[derive(Debug, Clone)]
pub struct ExplainRequest {
    /// Unique ID for this request.
    pub id: QueryId,
    /// SQL query text.
    pub sql: String,
    /// Whether to run the query and collect actual metrics (EXPLAIN ANALYZE).
    pub analyze: bool,
    /// Whether to include verbose details.
    pub verbose: bool,
}

impl ExplainRequest {
    /// Create a new explain request.
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            id: QueryId::new(),
            sql: sql.into(),
            analyze: false,
            verbose: false,
        }
    }

    /// Enable EXPLAIN ANALYZE mode.
    pub fn with_analyze(mut self) -> Self {
        self.analyze = true;
        self
    }

    /// Enable verbose output.
    pub fn with_verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}

/// Result of a completed query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Query ID.
    pub id: QueryId,
    /// Schema of the result.
    pub schema: SchemaRef,
    /// Result batches.
    pub batches: Vec<RecordBatch>,
    /// Execution statistics.
    pub stats: ExecutionStats,
}

impl QueryResult {
    /// Get the total number of rows.
    pub fn num_rows(&self) -> usize {
        self.batches.iter().map(|b| b.num_rows()).sum()
    }

    /// Get the number of columns.
    pub fn num_columns(&self) -> usize {
        self.schema.fields().len()
    }
}

/// Execution statistics for a query.
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// Total query execution time.
    pub total_time: Duration,
    /// Time spent planning.
    pub planning_time: Duration,
    /// Time spent executing.
    pub execution_time: Duration,
    /// Total rows returned.
    pub rows_returned: usize,
    /// Bytes scanned from sources.
    pub bytes_scanned: usize,
    /// Number of partitions scanned.
    pub partitions_scanned: usize,
}

/// A node in a query execution plan tree.
#[derive(Debug, Clone)]
pub struct PlanNode {
    /// Operator name (e.g., "ProjectionExec", "FilterExec").
    pub operator: String,
    /// Short description of what this node does.
    pub description: String,
    /// Detailed properties (predicate, projection, etc.).
    pub properties: FxHashMap<String, String>,
    /// Child nodes.
    pub children: Vec<PlanNode>,
    /// Execution metrics (if EXPLAIN ANALYZE was used).
    pub metrics: Option<OperatorMetrics>,
}

/// Execution metrics for a single operator.
#[derive(Debug, Clone, Default)]
pub struct OperatorMetrics {
    /// Rows output by this operator.
    pub output_rows: usize,
    /// Time spent in this operator.
    pub elapsed_time: Duration,
    /// Peak memory usage.
    pub memory_bytes: usize,
    /// Number of spills to disk.
    pub spill_count: usize,
    /// Bytes spilled to disk.
    pub spill_bytes: usize,
}

/// Information about a registered table.
#[derive(Debug, Clone)]
pub struct TableInfo {
    /// Table name.
    pub name: String,
    /// Schema name (default is "public").
    pub schema: String,
    /// Catalog name (default is "datafusion").
    pub catalog: String,
    /// Column information.
    pub columns: Vec<ColumnInfo>,
    /// Approximate row count (if known).
    pub row_count: Option<usize>,
    /// Table source type.
    pub source: TableSource,
}

/// Information about a table column.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// Data type as string.
    pub data_type: String,
    /// Whether nulls are allowed.
    pub nullable: bool,
}

/// Source of a table's data.
#[derive(Debug, Clone)]
pub enum TableSource {
    /// Local file path.
    LocalFile { path: String, format: FileFormat },
    /// Object store URL (s3://, gs://, etc.).
    ObjectStore { url: String, format: FileFormat },
    /// In-memory table.
    Memory,
    /// View (SQL query).
    View { sql: String },
}

/// Profile data for a column.
#[derive(Debug, Clone)]
pub struct ColumnProfile {
    /// Column name.
    pub name: String,
    /// Data type.
    pub data_type: String,
    /// Number of null values.
    pub null_count: usize,
    /// Percentage of nulls.
    pub null_percent: f64,
    /// Number of distinct values (approximate).
    pub distinct_count: Option<usize>,
    /// Minimum value.
    pub min: Option<ScalarValue>,
    /// Maximum value.
    pub max: Option<ScalarValue>,
    /// Mean (for numeric types).
    pub mean: Option<f64>,
    /// Standard deviation (for numeric types).
    pub std_dev: Option<f64>,
    /// Top N most frequent values.
    pub top_values: Vec<(ScalarValue, usize)>,
}

/// Profile data for a table.
#[derive(Debug, Clone)]
pub struct TableProfile {
    /// Table name.
    pub table_name: String,
    /// Total row count.
    pub row_count: usize,
    /// Column profiles.
    pub columns: Vec<ColumnProfile>,
    /// Time taken to profile.
    pub profile_time: Duration,
}

/// Events streamed during query execution.
#[derive(Debug, Clone)]
pub enum QueryEvent {
    /// Query started, schema is available.
    Started { id: QueryId, schema: SchemaRef },
    /// A batch of results is ready.
    Batch {
        id: QueryId,
        batch: RecordBatch,
        batch_num: usize,
    },
    /// Query execution progress update.
    Progress {
        id: QueryId,
        rows_so_far: usize,
        elapsed: Duration,
    },
    /// Query completed successfully.
    Completed { id: QueryId, stats: ExecutionStats },
    /// Query failed with an error.
    Failed { id: QueryId, error: String },
    /// Query was cancelled.
    Cancelled { id: QueryId },
}

impl QueryEvent {
    /// Get the query ID for this event.
    pub fn query_id(&self) -> QueryId {
        match self {
            Self::Started { id, .. }
            | Self::Batch { id, .. }
            | Self::Progress { id, .. }
            | Self::Completed { id, .. }
            | Self::Failed { id, .. }
            | Self::Cancelled { id, .. } => *id,
        }
    }
}
