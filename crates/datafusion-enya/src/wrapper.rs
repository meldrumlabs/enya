//! Execution plan wrapper that records metrics via metrics-rs.

use datafusion::arrow::array::RecordBatch;
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::common::Result;
use datafusion::error::DataFusionError;
use datafusion::execution::{RecordBatchStream, SendableRecordBatchStream, TaskContext};
use datafusion::physical_plan::metrics::MetricsSet;
use datafusion::physical_plan::{DisplayAs, DisplayFormatType, ExecutionPlan, PlanProperties};
use futures::Stream;
use metrics::{counter, gauge, histogram};
use std::any::Any;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::metric_names;
use crate::parquet::ParquetScanMetadata;

/// A wrapper around an [`ExecutionPlan`] that records metrics after execution.
///
/// This wrapper delegates all execution to the inner plan and, upon stream
/// completion, harvests the plan's metrics and records them via the `metrics` crate.
#[derive(Debug)]
pub struct MetricsExecWrapper {
    /// The wrapped execution plan.
    inner: Arc<dyn ExecutionPlan>,
    /// Query ID for metric correlation.
    query_id: Option<String>,
    /// Cached plan properties from inner plan.
    properties: PlanProperties,
}

impl MetricsExecWrapper {
    /// Creates a new wrapper around the given execution plan.
    pub fn new(inner: Arc<dyn ExecutionPlan>) -> Self {
        let properties = inner.properties().clone();
        Self {
            inner,
            query_id: None,
            properties,
        }
    }

    /// Creates a new wrapper with a query ID for metric correlation.
    pub fn with_query_id(inner: Arc<dyn ExecutionPlan>, query_id: impl Into<String>) -> Self {
        let properties = inner.properties().clone();
        Self {
            inner,
            query_id: Some(query_id.into()),
            properties,
        }
    }
}

impl DisplayAs for MetricsExecWrapper {
    fn fmt_as(&self, t: DisplayFormatType, f: &mut fmt::Formatter) -> fmt::Result {
        match t {
            DisplayFormatType::Default
            | DisplayFormatType::Verbose
            | DisplayFormatType::TreeRender => {
                write!(f, "MetricsExecWrapper: ")?;
                self.inner.fmt_as(t, f)
            }
        }
    }
}

impl ExecutionPlan for MetricsExecWrapper {
    fn name(&self) -> &str {
        "MetricsExecWrapper"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn properties(&self) -> &PlanProperties {
        &self.properties
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![&self.inner]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        if children.len() != 1 {
            return Err(DataFusionError::Internal(
                "MetricsExecWrapper expects exactly one child".to_string(),
            ));
        }
        let new_inner = children.into_iter().next().expect("checked length above");
        Ok(Arc::new(MetricsExecWrapper {
            inner: new_inner,
            query_id: self.query_id.clone(),
            properties: self.properties.clone(),
        }))
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        let inner_stream = self.inner.execute(partition, context)?;
        let schema = inner_stream.schema();

        // Extract parquet metadata from the plan (only on partition 0 to avoid redundant work)
        let parquet_metadata = if partition == 0 {
            let metadata = ParquetScanMetadata::from_plan(&self.inner);
            if metadata.is_empty() {
                None
            } else {
                Some(metadata)
            }
        } else {
            None
        };

        let recording_stream = MetricsRecordingStream {
            inner: inner_stream,
            plan: Arc::clone(&self.inner),
            query_id: self.query_id.clone(),
            partition,
            recorded: false,
            schema,
            parquet_metadata,
        };

        Ok(Box::pin(recording_stream))
    }

    fn metrics(&self) -> Option<MetricsSet> {
        self.inner.metrics()
    }
}

/// A stream wrapper that records metrics when fully consumed or dropped.
struct MetricsRecordingStream {
    inner: SendableRecordBatchStream,
    plan: Arc<dyn ExecutionPlan>,
    query_id: Option<String>,
    partition: usize,
    recorded: bool,
    schema: SchemaRef,
    /// Pre-extracted parquet metadata (only recorded once per query).
    parquet_metadata: Option<Vec<ParquetScanMetadata>>,
}

impl MetricsRecordingStream {
    fn record_metrics(&mut self) {
        if self.recorded {
            return;
        }
        self.recorded = true;

        // Record parquet metadata if available (only on partition 0 to avoid duplicates)
        if self.partition == 0 {
            self.record_parquet_metadata();
        }

        let Some(metrics) = self.plan.metrics() else {
            return;
        };

        let operator = format!("{:?}", self.plan)
            .split([' ', '{', '('])
            .next()
            .unwrap_or("Unknown")
            .to_string();
        let partition_str = self.partition.to_string();

        let labels: Vec<(&str, String)> = if let Some(ref qid) = self.query_id {
            vec![
                ("operator", operator.clone()),
                ("query_id", qid.clone()),
                ("partition", partition_str),
            ]
        } else {
            vec![("operator", operator.clone()), ("partition", partition_str)]
        };

        for metric in metrics.iter() {
            let value = metric.value();
            match value.name() {
                "output_rows" => {
                    let count = value.as_usize();
                    counter!(metric_names::OUTPUT_ROWS, &labels).increment(count as u64);
                }
                "output_bytes" => {
                    let count = value.as_usize();
                    counter!(metric_names::OUTPUT_BYTES, &labels).increment(count as u64);
                }
                "elapsed_compute" => {
                    let nanos = value.as_usize();
                    histogram!(metric_names::ELAPSED_COMPUTE_NS, &labels).record(nanos as f64);
                }
                "bytes_scanned" => {
                    let count = value.as_usize();
                    counter!(metric_names::BYTES_SCANNED, &labels).increment(count as u64);
                }
                "row_groups_pruned_statistics" => {
                    let count = value.as_usize();
                    counter!(metric_names::ROW_GROUPS_PRUNED_STATISTICS, &labels)
                        .increment(count as u64);
                }
                "row_groups_pruned_bloom_filter" => {
                    let count = value.as_usize();
                    counter!(metric_names::ROW_GROUPS_PRUNED_BLOOM_FILTER, &labels)
                        .increment(count as u64);
                }
                "row_groups_matched_statistics" => {
                    let count = value.as_usize();
                    counter!(metric_names::ROW_GROUPS_MATCHED_STATISTICS, &labels)
                        .increment(count as u64);
                }
                "row_groups_matched_bloom_filter" => {
                    let count = value.as_usize();
                    counter!(metric_names::ROW_GROUPS_MATCHED_BLOOM_FILTER, &labels)
                        .increment(count as u64);
                }
                "pushdown_rows_pruned" => {
                    let count = value.as_usize();
                    counter!(metric_names::PUSHDOWN_ROWS_PRUNED, &labels).increment(count as u64);
                }
                "pushdown_rows_matched" => {
                    let count = value.as_usize();
                    counter!(metric_names::PUSHDOWN_ROWS_MATCHED, &labels).increment(count as u64);
                }
                "page_index_rows_pruned" => {
                    let count = value.as_usize();
                    counter!(metric_names::PAGE_INDEX_ROWS_PRUNED, &labels).increment(count as u64);
                }
                "page_index_rows_matched" => {
                    let count = value.as_usize();
                    counter!(metric_names::PAGE_INDEX_ROWS_MATCHED, &labels)
                        .increment(count as u64);
                }
                "metadata_load_time" => {
                    let nanos = value.as_usize();
                    histogram!(metric_names::METADATA_LOAD_TIME_NS, &labels).record(nanos as f64);
                }
                "spilled_bytes" => {
                    let count = value.as_usize();
                    counter!(metric_names::SPILLED_BYTES, &labels).increment(count as u64);
                }
                "spill_count" => {
                    let count = value.as_usize();
                    counter!(metric_names::SPILL_COUNT, &labels).increment(count as u64);
                }
                _ => {}
            }
        }
    }

    fn record_parquet_metadata(&self) {
        let Some(ref parquet_scans) = self.parquet_metadata else {
            return;
        };

        for metadata in parquet_scans {
            // Build base labels with query_id and table name
            let mut base_labels: Vec<(&str, String)> = Vec::new();

            if let Some(ref qid) = self.query_id {
                base_labels.push(("query_id", qid.clone()));
            }

            if let Some(ref table) = metadata.table_name {
                base_labels.push(("table", table.clone()));
            }

            // Record aggregate metrics with base labels
            gauge!(metric_names::PARQUET_FILES_SCANNED, &base_labels)
                .set(metadata.file_count as f64);

            gauge!(metric_names::PARQUET_TOTAL_FILE_SIZE_BYTES, &base_labels)
                .set(metadata.total_file_size_bytes as f64);

            gauge!(metric_names::PARQUET_SCHEMA_COLUMNS, &base_labels)
                .set(metadata.schema_column_count as f64);

            // Record per-file metrics with file path label
            for file in &metadata.files {
                let mut file_labels = base_labels.clone();
                // Extract just the filename from the path for the label
                let filename = file
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&file.path)
                    .to_string();
                file_labels.push(("file", filename));

                gauge!(metric_names::PARQUET_FILE_SIZE_BYTES, &file_labels)
                    .set(file.size_bytes as f64);
            }
        }
    }
}

impl Drop for MetricsRecordingStream {
    fn drop(&mut self) {
        self.record_metrics();
    }
}

impl Stream for MetricsRecordingStream {
    type Item = Result<RecordBatch>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = self.inner.as_mut().poll_next(cx);

        // Record metrics when stream is exhausted
        if matches!(result, Poll::Ready(None)) {
            self.record_metrics();
        }

        result
    }
}

impl RecordBatchStream for MetricsRecordingStream {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
}
