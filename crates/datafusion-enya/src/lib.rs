//! DataFusion integration for Enya observability.
//!
//! This crate provides automatic metrics collection from DataFusion query executions,
//! recording them via the `metrics` crate. When used with Enya's `StoreRecorder`,
//! metrics are persisted to the Enya metrics store with full tag support.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │              User Query Execution                       │
//! │  ctx.sql("SELECT ...").await?.collect().await?          │
//! └─────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │           EnyaPhysicalOptimizerRule                     │
//! │  - Wraps each ExecutionPlan with MetricsExecWrapper     │
//! │  - Preserves original plan semantics                    │
//! └─────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │              MetricsExecWrapper                         │
//! │  - Delegates execution to inner plan                    │
//! │  - On stream completion: harvests metrics()             │
//! │  - Records via metrics-rs                               │
//! └─────────────────────────────────────────────────────────┘
//!                           │
//!                           ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │              metrics-rs Recorder                        │
//! │  - Enya's StoreRecorder captures metrics                │
//! │  - Tags: query_id, operator, partition                  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use datafusion::prelude::*;
//! use datafusion_enya::EnyaSessionContextExt;
//!
//! // Create a session context with Enya instrumentation
//! let ctx = SessionContext::new_with_enya();
//!
//! // Execute queries normally - metrics are recorded automatically
//! let df = ctx.sql("SELECT * FROM my_table").await?;
//! let results = df.collect().await?;
//!
//! // Metrics like "datafusion.output_rows", "datafusion.elapsed_compute_ns"
//! // are now recorded with operator and query_id tags
//! ```

mod error;
mod optimizer_rule;
mod parquet;
mod wrapper;

pub use error::Error;
pub use optimizer_rule::EnyaPhysicalOptimizerRule;
pub use parquet::{ParquetFileInfo, ParquetScanMetadata};
pub use wrapper::MetricsExecWrapper;

use datafusion::execution::SessionStateBuilder;
use datafusion::prelude::SessionContext;
use std::sync::Arc;

/// Extension trait for creating a [`SessionContext`] with Enya instrumentation.
pub trait EnyaSessionContextExt {
    /// Creates a new [`SessionContext`] with Enya metrics instrumentation enabled.
    ///
    /// This adds the [`EnyaPhysicalOptimizerRule`] which wraps execution plans
    /// to automatically record DataFusion metrics via the `metrics` crate.
    fn new_with_enya() -> SessionContext;

    /// Creates a new [`SessionContext`] with Enya metrics instrumentation and
    /// a specific query ID prefix for correlation.
    ///
    /// The query ID helps correlate metrics across multiple queries, useful for
    /// tracking metrics per commit, test run, or user session.
    fn new_with_enya_query_id(query_id: impl Into<String>) -> SessionContext;
}

impl EnyaSessionContextExt for SessionContext {
    fn new_with_enya() -> SessionContext {
        let rule = Arc::new(EnyaPhysicalOptimizerRule::new());
        let state = SessionStateBuilder::new()
            .with_default_features()
            .with_physical_optimizer_rule(rule)
            .build();
        SessionContext::new_with_state(state)
    }

    fn new_with_enya_query_id(query_id: impl Into<String>) -> SessionContext {
        let rule = Arc::new(EnyaPhysicalOptimizerRule::with_query_id(query_id));
        let state = SessionStateBuilder::new()
            .with_default_features()
            .with_physical_optimizer_rule(rule)
            .build();
        SessionContext::new_with_state(state)
    }
}

/// Metric names emitted by this crate.
///
/// All metrics are prefixed with `datafusion.` and tagged with:
/// - `operator`: The execution plan operator name (e.g., "ParquetExec", "FilterExec")
/// - `query_id`: Optional query identifier for correlation
/// - `partition`: The partition number (when applicable)
pub mod metric_names {
    /// Counter: Total rows output by operators.
    pub const OUTPUT_ROWS: &str = "datafusion.output_rows";

    /// Counter: Total bytes output by operators.
    pub const OUTPUT_BYTES: &str = "datafusion.output_bytes";

    /// Histogram: Compute time in nanoseconds.
    pub const ELAPSED_COMPUTE_NS: &str = "datafusion.elapsed_compute_ns";

    /// Counter: Bytes scanned from data sources.
    pub const BYTES_SCANNED: &str = "datafusion.bytes_scanned";

    /// Counter: Row groups pruned by statistics.
    pub const ROW_GROUPS_PRUNED_STATISTICS: &str = "datafusion.row_groups_pruned_statistics";

    /// Counter: Row groups pruned by bloom filter.
    pub const ROW_GROUPS_PRUNED_BLOOM_FILTER: &str = "datafusion.row_groups_pruned_bloom_filter";

    /// Counter: Bytes spilled to disk.
    pub const SPILLED_BYTES: &str = "datafusion.spilled_bytes";

    /// Counter: Number of spill operations.
    pub const SPILL_COUNT: &str = "datafusion.spill_count";

    // Parquet-specific metrics

    /// Gauge: Number of Parquet files scanned.
    /// Tagged with `query_id` and `table` when available.
    pub const PARQUET_FILES_SCANNED: &str = "datafusion.parquet.files_scanned";

    /// Gauge: Total size of Parquet files scanned in bytes.
    /// Tagged with `query_id` and `table` when available.
    pub const PARQUET_TOTAL_FILE_SIZE_BYTES: &str = "datafusion.parquet.total_file_size_bytes";

    /// Gauge: Number of columns in the scanned Parquet schema.
    /// Tagged with `query_id` and `table` when available.
    pub const PARQUET_SCHEMA_COLUMNS: &str = "datafusion.parquet.schema_columns";

    /// Gauge: Size of an individual Parquet file in bytes.
    /// Tagged with `query_id`, `table`, and `file` when available.
    pub const PARQUET_FILE_SIZE_BYTES: &str = "datafusion.parquet.file_size_bytes";

    /// Counter: Row groups matched by statistics (not pruned).
    pub const ROW_GROUPS_MATCHED_STATISTICS: &str = "datafusion.row_groups_matched_statistics";

    /// Counter: Row groups matched by bloom filter (not pruned).
    pub const ROW_GROUPS_MATCHED_BLOOM_FILTER: &str = "datafusion.row_groups_matched_bloom_filter";

    /// Counter: Rows pruned by pushdown predicates.
    pub const PUSHDOWN_ROWS_PRUNED: &str = "datafusion.pushdown_rows_pruned";

    /// Counter: Rows matched by pushdown predicates.
    pub const PUSHDOWN_ROWS_MATCHED: &str = "datafusion.pushdown_rows_matched";

    /// Counter: Rows pruned by page index.
    pub const PAGE_INDEX_ROWS_PRUNED: &str = "datafusion.page_index_rows_pruned";

    /// Counter: Rows matched by page index.
    pub const PAGE_INDEX_ROWS_MATCHED: &str = "datafusion.page_index_rows_matched";

    /// Histogram: Time spent loading parquet metadata in nanoseconds.
    pub const METADATA_LOAD_TIME_NS: &str = "datafusion.parquet.metadata_load_time_ns";
}

#[cfg(test)]
mod tests {
    use super::*;
    use datafusion::arrow::array::{Int32Array, RecordBatch};
    use datafusion::arrow::datatypes::{DataType, Field, Schema};
    use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
    use std::sync::{Arc, OnceLock};

    /// Global snapshotter for all metrics tests.
    /// We use a single global recorder because `metrics::set_global_recorder` can only be called once.
    static SNAPSHOTTER: OnceLock<Snapshotter> = OnceLock::new();

    fn get_snapshotter() -> &'static Snapshotter {
        SNAPSHOTTER.get_or_init(|| {
            let recorder = DebuggingRecorder::new();
            let snapshotter = recorder.snapshotter();
            // Install as global recorder
            metrics::set_global_recorder(recorder).expect("Failed to set global recorder");
            snapshotter
        })
    }

    fn create_test_batches() -> Vec<RecordBatch> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("value", DataType::Int32, false),
        ]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5])),
                Arc::new(Int32Array::from(vec![10, 20, 30, 40, 50])),
            ],
        )
        .expect("create batch");

        vec![batch]
    }

    #[tokio::test]
    async fn test_session_context_executes_query() {
        // Create a session context with Enya instrumentation
        let ctx = SessionContext::new_with_enya();

        // Register a memory table
        let batches = create_test_batches();
        ctx.register_batch("test_table", batches[0].clone())
            .expect("register batch");

        // Execute a query
        let df = ctx
            .sql("SELECT * FROM test_table WHERE value > 20")
            .await
            .expect("sql");

        let results = df.collect().await.expect("collect");

        // Verify query results
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].num_rows(), 3); // rows with value 30, 40, 50
    }

    /// Test that DataFusion metrics are recorded when queries are executed.
    /// This test uses a global DebuggingRecorder to capture metrics.
    #[tokio::test]
    async fn test_metrics_recorded_on_query_execution() {
        let snapshotter = get_snapshotter();

        // Create a session context with Enya instrumentation
        let ctx = SessionContext::new_with_enya();

        // Register a memory table
        let batches = create_test_batches();
        ctx.register_batch("metrics_test", batches[0].clone())
            .expect("register batch");

        // Execute a query with a filter to trigger metrics recording
        // (simple SELECT * without predicates uses DataSourceExec which doesn't track output_rows)
        let df = ctx
            .sql("SELECT id, value FROM metrics_test WHERE value > 20")
            .await
            .expect("sql");

        let _results = df.collect().await.expect("collect");

        // Take a snapshot of recorded metrics
        let snapshot = snapshotter.snapshot();
        let metrics: Vec<_> = snapshot.into_vec();

        // Verify that datafusion metrics were recorded
        let datafusion_metrics: Vec<_> = metrics
            .iter()
            .filter(|(composite_key, _, _, _)| {
                composite_key.key().name().starts_with("datafusion.")
            })
            .collect();

        // We expect at least output_rows to be recorded
        assert!(
            !datafusion_metrics.is_empty(),
            "Expected datafusion metrics to be recorded, but found none. All metrics: {:?}",
            metrics
                .iter()
                .map(|(k, _, _, _)| k.key().name())
                .collect::<Vec<_>>()
        );

        // Check for output_rows metric specifically
        let output_rows = datafusion_metrics.iter().find(|(composite_key, _, _, _)| {
            composite_key.key().name() == metric_names::OUTPUT_ROWS
        });

        assert!(
            output_rows.is_some(),
            "Expected datafusion.output_rows metric, found: {:?}",
            datafusion_metrics
                .iter()
                .map(|(k, _, _, _)| k.key().name())
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_query_id_tag_included() {
        let snapshotter = get_snapshotter();

        // Create context with query ID
        let query_id = "test-query-123";
        let ctx = SessionContext::new_with_enya_query_id(query_id);

        // Register and query
        let batches = create_test_batches();
        ctx.register_batch("query_id_test", batches[0].clone())
            .expect("register batch");

        let df = ctx
            .sql("SELECT * FROM query_id_test WHERE value > 20")
            .await
            .expect("sql");
        let _results = df.collect().await.expect("collect");

        // Check metrics have query_id label
        let snapshot = snapshotter.snapshot();
        let metrics: Vec<_> = snapshot.into_vec();

        let metrics_with_query_id: Vec<_> = metrics
            .iter()
            .filter(|(composite_key, _, _, _)| {
                composite_key
                    .key()
                    .labels()
                    .any(|l| l.key() == "query_id" && l.value() == query_id)
            })
            .collect();

        assert!(
            !metrics_with_query_id.is_empty(),
            "Expected metrics with query_id={}, found labels: {:?}",
            query_id,
            metrics
                .iter()
                .flat_map(|(k, _, _, _)| k.key().labels())
                .map(|l| format!("{}={}", l.key(), l.value()))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_operator_tag_included() {
        let snapshotter = get_snapshotter();

        let ctx = SessionContext::new_with_enya();

        let batches = create_test_batches();
        ctx.register_batch("operator_test", batches[0].clone())
            .expect("register batch");

        let df = ctx
            .sql("SELECT * FROM operator_test WHERE value > 20")
            .await
            .expect("sql");
        let _results = df.collect().await.expect("collect");

        let snapshot = snapshotter.snapshot();
        let metrics: Vec<_> = snapshot.into_vec();

        // Check that metrics have operator labels
        let metrics_with_operator: Vec<_> = metrics
            .iter()
            .filter(|(composite_key, _, _, _)| {
                composite_key.key().labels().any(|l| l.key() == "operator")
            })
            .collect();

        assert!(
            !metrics_with_operator.is_empty(),
            "Expected metrics with operator tag"
        );
    }

    #[tokio::test]
    async fn test_counter_metrics_increment() {
        let snapshotter = get_snapshotter();

        let ctx = SessionContext::new_with_enya();

        // Create a larger batch to ensure we get measurable output rows
        let schema = Arc::new(Schema::new(vec![Field::new("n", DataType::Int32, false)]));
        let batch = RecordBatch::try_new(
            schema,
            vec![Arc::new(Int32Array::from((0..100).collect::<Vec<_>>()))],
        )
        .expect("create batch");

        ctx.register_batch("counter_test", batch)
            .expect("register batch");

        let df = ctx
            .sql("SELECT * FROM counter_test WHERE n > 50")
            .await
            .expect("sql");
        let _results = df.collect().await.expect("collect");

        let snapshot = snapshotter.snapshot();
        let metrics: Vec<_> = snapshot.into_vec();

        // Find output_rows counter and verify it has a value
        let output_rows: Vec<_> = metrics
            .iter()
            .filter(|(composite_key, _, _, _)| {
                composite_key.key().name() == metric_names::OUTPUT_ROWS
            })
            .collect();

        // Verify we have output_rows metrics
        assert!(!output_rows.is_empty(), "Expected output_rows counter");

        // Check that at least one has a non-zero value
        let has_nonzero = output_rows.iter().any(|(_, _, _, value)| match value {
            DebugValue::Counter(c) => *c > 0,
            _ => false,
        });

        assert!(has_nonzero, "Expected non-zero output_rows counter value");
    }
}
