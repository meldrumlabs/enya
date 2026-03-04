//! Core types for DataFusion operations.

use std::time::Duration;

use arrow::array::{
    Array, BinaryArray, BooleanArray, Date32Array, Date64Array, Float32Array, Float64Array,
    Int8Array, Int16Array, Int32Array, Int64Array, LargeBinaryArray, LargeStringArray, RecordBatch,
    StringArray, TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    TimestampSecondArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array,
};
use arrow::datatypes::SchemaRef;

use rustc_hash::FxHashMap;

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

/// Request to benchmark a SQL query over multiple iterations.
#[derive(Debug, Clone)]
pub struct BenchmarkRequest {
    /// Unique ID for this benchmark run.
    pub id: QueryId,
    /// SQL query text to benchmark.
    pub sql: String,
    /// Number of iterations to run.
    pub iterations: usize,
}

impl BenchmarkRequest {
    /// Create a new benchmark request with default iterations (10).
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            id: QueryId::new(),
            sql: sql.into(),
            iterations: 10,
        }
    }

    /// Set the number of iterations.
    pub fn with_iterations(mut self, n: usize) -> Self {
        self.iterations = n;
        self
    }

    /// Use a specific query ID.
    pub fn with_id(mut self, id: QueryId) -> Self {
        self.id = id;
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

/// Timing statistics for a single benchmark phase (e.g., logical planning).
#[derive(Debug, Clone)]
pub struct PhaseTiming {
    /// Minimum duration across all iterations.
    pub min: Duration,
    /// Maximum duration across all iterations.
    pub max: Duration,
    /// Mean duration across all iterations.
    pub mean: Duration,
    /// Median duration across all iterations.
    pub median: Duration,
    /// Percentage of total execution time (0.0–100.0).
    pub percent_of_total: f64,
}

impl PhaseTiming {
    /// Create from a `DurationsSummary` returned by the vendored dft crate.
    fn from_dft(summary: &datafusion_app::local_benchmarks::DurationsSummary) -> Self {
        Self {
            min: summary.min,
            max: summary.max,
            mean: summary.mean,
            median: summary.median,
            percent_of_total: summary.percent_of_total,
        }
    }
}

/// Request to compute column-level statistics for a table.
#[derive(Debug, Clone)]
pub struct DescribeRequest {
    /// Unique ID for this request.
    pub id: QueryId,
    /// Table name (possibly schema-qualified, e.g. "public.users").
    pub table_name: String,
}

impl DescribeRequest {
    /// Create a new describe request.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            id: QueryId::new(),
            table_name: table_name.into(),
        }
    }

    /// Use a specific query ID.
    pub fn with_id(mut self, id: QueryId) -> Self {
        self.id = id;
        self
    }
}

/// Statistics for a single column from a `/describe` operation.
#[derive(Debug, Clone)]
pub struct ColumnStats {
    /// Column name.
    pub name: String,
    /// Data type as string.
    pub data_type: String,
    /// Total non-null count.
    pub count: usize,
    /// Number of null values.
    pub null_count: usize,
    /// Number of distinct values.
    pub distinct_count: usize,
    /// Minimum value (cast to string), None for unsupported types.
    pub min: Option<String>,
    /// Maximum value (cast to string), None for unsupported types.
    pub max: Option<String>,
    /// Mean value (numeric columns only).
    pub mean: Option<f64>,
}

/// Result of a table describe operation.
#[derive(Debug, Clone)]
pub struct DescribeStats {
    /// Table name that was described.
    pub table_name: String,
    /// Total row count.
    pub total_rows: usize,
    /// Per-column statistics.
    pub columns: Vec<ColumnStats>,
    /// How long the describe took.
    pub elapsed: Duration,
}

/// Aggregate benchmark statistics across all iterations.
#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    /// Number of iterations run.
    pub iterations: usize,
    /// Rows returned per iteration.
    pub rows_per_iteration: usize,
    /// Logical planning phase timings.
    pub logical_planning: PhaseTiming,
    /// Physical planning phase timings.
    pub physical_planning: PhaseTiming,
    /// Execution phase timings.
    pub execution: PhaseTiming,
    /// Total (end-to-end) timings.
    pub total: PhaseTiming,
    /// Custom phase names for non-local backends (e.g. Flight SQL).
    /// When `Some`, overrides default labels in the UI.
    /// Order: [phase1, phase2, phase3, "Total"].
    pub phase_names: Option<[String; 4]>,
}

impl BenchmarkStats {
    /// Convert from dft's `LocalBenchmarkStats` (which now has public fields).
    pub fn from_dft(stats: &datafusion_app::local_benchmarks::LocalBenchmarkStats) -> Self {
        let logical = stats.summarize(&stats.logical_planning_durations);
        let physical = stats.summarize(&stats.physical_planning_durations);
        let execution = stats.summarize(&stats.execution_durations);
        let total = stats.summarize(&stats.total_durations);

        let rows_per_iteration = stats.rows.first().copied().unwrap_or(0);

        Self {
            iterations: stats.runs,
            rows_per_iteration,
            logical_planning: PhaseTiming::from_dft(&logical),
            physical_planning: PhaseTiming::from_dft(&physical),
            execution: PhaseTiming::from_dft(&execution),
            total: PhaseTiming::from_dft(&total),
            phase_names: None,
        }
    }
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

impl PlanNode {
    /// Calculate the total execution time for this plan subtree.
    ///
    /// Returns the maximum of this node's time and its children's time,
    /// since children typically run in parallel in a pull-based execution model.
    #[must_use]
    pub fn total_time(&self) -> Duration {
        let self_time = self
            .metrics
            .as_ref()
            .map_or(Duration::ZERO, |m| m.elapsed_time);
        let child_time: Duration = self.children.iter().map(Self::total_time).sum();
        self_time.max(child_time)
    }

    /// Find the bottleneck time (maximum elapsed time) in this plan subtree.
    ///
    /// Returns the duration of the slowest operator in the tree.
    #[must_use]
    pub fn bottleneck_time(&self) -> Duration {
        let self_time = self
            .metrics
            .as_ref()
            .map_or(Duration::ZERO, |m| m.elapsed_time);
        let max_child = self
            .children
            .iter()
            .map(Self::bottleneck_time)
            .max()
            .unwrap_or(Duration::ZERO);
        self_time.max(max_child)
    }

    /// Count the total number of operators in this plan subtree.
    #[must_use]
    pub fn operator_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(Self::operator_count)
            .sum::<usize>()
    }
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

impl OperatorMetrics {
    /// Format elapsed time as a human-readable string.
    ///
    /// Returns values like "123µs", "45.67ms", or "1.23s" depending on magnitude.
    #[must_use]
    pub fn format_elapsed_time(&self) -> String {
        format_duration(self.elapsed_time)
    }

    /// Format memory usage as a human-readable string.
    ///
    /// Returns values like "512 B", "1.5 KB", "128.0 MB", or "2.1 GB".
    #[must_use]
    pub fn format_memory(&self) -> String {
        format_bytes(self.memory_bytes)
    }

    /// Format output rows as a human-readable string.
    ///
    /// Returns values like "123", "1.5K", or "2.3M" for large counts.
    #[must_use]
    pub fn format_output_rows(&self) -> String {
        format_rows(self.output_rows)
    }
}

/// Format a duration as a human-readable string.
///
/// Automatically selects appropriate units (µs, ms, s) based on magnitude.
#[must_use]
pub fn format_duration(d: Duration) -> String {
    let micros = d.as_micros();
    if micros < 1000 {
        format!("{micros}µs")
    } else if micros < 1_000_000 {
        format!("{:.2}ms", micros as f64 / 1000.0)
    } else {
        format!("{:.2}s", micros as f64 / 1_000_000.0)
    }
}

/// Format bytes as a human-readable string.
///
/// Automatically selects appropriate units (B, KB, MB, GB) based on magnitude.
#[must_use]
pub fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Format a row count as a human-readable string.
///
/// Returns compact notation like "1.5K" or "2.3M" for large counts.
#[must_use]
pub fn format_rows(rows: usize) -> String {
    if rows < 1000 {
        rows.to_string()
    } else if rows < 1_000_000 {
        format!("{:.1}K", rows as f64 / 1000.0)
    } else {
        format!("{:.1}M", rows as f64 / 1_000_000.0)
    }
}

/// Format an arrow array value at a specific index as a string.
///
/// Handles common arrow array types (strings, integers, floats, booleans,
/// timestamps, dates, binary). Returns "NULL" for null values.
#[must_use]
pub fn format_array_value(array: &dyn Array, idx: usize) -> String {
    if array.is_null(idx) {
        return "NULL".to_string();
    }

    // String types
    if let Some(arr) = array.as_any().downcast_ref::<StringArray>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<LargeStringArray>() {
        return arr.value(idx).to_string();
    }

    // Integer types
    if let Some(arr) = array.as_any().downcast_ref::<Int8Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<Int16Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<Int32Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<Int64Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<UInt8Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<UInt16Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<UInt32Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<UInt64Array>() {
        return arr.value(idx).to_string();
    }

    // Float types
    if let Some(arr) = array.as_any().downcast_ref::<Float32Array>() {
        return format!("{:.6}", arr.value(idx));
    }
    if let Some(arr) = array.as_any().downcast_ref::<Float64Array>() {
        return format!("{:.6}", arr.value(idx));
    }

    // Boolean
    if let Some(arr) = array.as_any().downcast_ref::<BooleanArray>() {
        return arr.value(idx).to_string();
    }

    // Date types
    if let Some(arr) = array.as_any().downcast_ref::<Date32Array>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<Date64Array>() {
        return arr.value(idx).to_string();
    }

    // Timestamp types
    if let Some(arr) = array.as_any().downcast_ref::<TimestampSecondArray>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<TimestampMillisecondArray>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<TimestampMicrosecondArray>() {
        return arr.value(idx).to_string();
    }
    if let Some(arr) = array.as_any().downcast_ref::<TimestampNanosecondArray>() {
        return arr.value(idx).to_string();
    }

    // Binary types
    if let Some(arr) = array.as_any().downcast_ref::<BinaryArray>() {
        return format!("{:?}", arr.value(idx));
    }
    if let Some(arr) = array.as_any().downcast_ref::<LargeBinaryArray>() {
        return format!("{:?}", arr.value(idx));
    }

    // Fallback for other types
    format!("{:?}", array.slice(idx, 1))
}

/// Operator category for plan visualization.
///
/// Categorizes DataFusion operators by their function for visualization purposes.
/// Each category maps to a specific color index in visualization palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorCategory {
    /// I/O operations (TableScan, ParquetScan, CsvScan, etc.)
    Scan,
    /// Filtering operations (Filter, Limit)
    Filter,
    /// Join operations (HashJoin, MergeJoin, NestedLoopJoin, etc.)
    Join,
    /// Aggregation operations (Aggregate, GroupBy, Window)
    Aggregate,
    /// Sorting operations (Sort, SortPreservingMerge)
    Sort,
    /// Projection operations (Projection)
    Project,
    /// Hash operations (HashBuild, HashProbe)
    Hash,
    /// Distribution operations (Repartition, Coalesce, Exchange)
    Remote,
    /// Union/combination operations (Union, Interleave)
    Union,
    /// Cooperative scheduling (CooperativeExec, Yield)
    Cooperative,
    /// Other execution operators ending in "Exec"
    Exec,
    /// Non-execution operators
    Other,
}

impl OperatorCategory {
    /// Categorize an operator by its name.
    ///
    /// Uses pattern matching on operator names to determine the category.
    /// DataFusion operators typically end in "Exec" (e.g., "FilterExec", "HashJoinExec").
    #[must_use]
    pub fn from_operator(operator: &str) -> Self {
        if operator.contains("Scan") || operator.contains("Read") {
            Self::Scan
        } else if operator.contains("Filter") || operator.contains("Limit") {
            Self::Filter
        } else if operator.contains("Join") {
            Self::Join
        } else if operator.contains("Aggregate") || operator.contains("Group") {
            Self::Aggregate
        } else if operator.contains("Sort") || operator.contains("Order") {
            Self::Sort
        } else if operator.contains("Project") {
            Self::Project
        } else if operator.contains("Hash") {
            Self::Hash
        } else if operator.contains("Remote")
            || operator.contains("Exchange")
            || operator.contains("Coalesce")
            || operator.contains("Repartition")
        {
            Self::Remote
        } else if operator.contains("Union") || operator.contains("Interleave") {
            Self::Union
        } else if operator.contains("Cooperative") || operator.contains("Yield") {
            Self::Cooperative
        } else if operator.ends_with("Exec") {
            Self::Exec
        } else {
            Self::Other
        }
    }

    /// Get the color index for this category in visualization palettes.
    ///
    /// Returns a stable index (0-11) that can be used with color palettes.
    #[must_use]
    pub const fn color_index(self) -> usize {
        match self {
            Self::Scan => 0,
            Self::Filter => 1,
            Self::Join => 2,
            Self::Aggregate => 3,
            Self::Sort => 4,
            Self::Project => 5,
            Self::Hash => 6,
            Self::Remote => 7,
            Self::Union => 8,
            Self::Cooperative => 9,
            Self::Exec => 10,
            Self::Other => 11,
        }
    }

    /// Get the display name for this category.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Scan => "Scan/Read",
            Self::Filter => "Filter",
            Self::Join => "Join",
            Self::Aggregate => "Aggregate",
            Self::Sort => "Sort",
            Self::Project => "Project",
            Self::Hash => "Hash",
            Self::Remote => "Remote/Exchange",
            Self::Union => "Union",
            Self::Cooperative => "Cooperative",
            Self::Exec => "Exec",
            Self::Other => "Other",
        }
    }
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
    /// Benchmark iteration completed (progress update).
    BenchmarkProgress {
        id: QueryId,
        iteration: usize,
        total_iterations: usize,
        last_duration: Duration,
    },
    /// Benchmark completed with aggregate statistics.
    BenchmarkCompleted {
        id: QueryId,
        stats: Box<BenchmarkStats>,
    },
    /// Describe table completed with column statistics.
    DescribeCompleted {
        id: QueryId,
        stats: Box<DescribeStats>,
    },
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
            | Self::Cancelled { id, .. }
            | Self::BenchmarkProgress { id, .. }
            | Self::BenchmarkCompleted { id, .. }
            | Self::DescribeCompleted { id, .. } => *id,
        }
    }
}
