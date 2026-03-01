//! Async query executor with channel-based communication.
//!
//! This module provides non-blocking query execution suitable for immediate-mode
//! GUI applications like egui. Queries are submitted via channels and results
//! stream back as events.

use std::sync::Arc;
#[allow(clippy::disallowed_types)] // datafusion is native-only
use std::time::Instant;

use datafusion::prelude::*;
use futures::StreamExt;
use tokio::sync::mpsc;

use datafusion_app::local::ExecutionContext;
use datafusion_app::local_benchmarks::BenchmarkProgressReporter;

use crate::Result;
use crate::error::{Error, QueryId};
use crate::types::{
    BenchmarkRequest, BenchmarkStats, ColumnStats, DescribeRequest, DescribeStats, ExecutionStats,
    ExplainRequest, QueryEvent, QueryRequest,
};

/// Commands sent to the executor.
#[derive(Debug)]
pub enum ExecutorCommand {
    /// Execute a SQL query.
    Execute(QueryRequest),
    /// Explain a query plan.
    Explain(ExplainRequest),
    /// Benchmark a query over multiple iterations.
    Benchmark(BenchmarkRequest),
    /// Describe a table's column statistics.
    Describe(DescribeRequest),
    /// Cancel a running query.
    Cancel(QueryId),
    /// Shutdown the executor.
    Shutdown,
}

/// Handle for submitting queries to the executor.
#[derive(Clone)]
pub struct ExecutorHandle {
    command_tx: mpsc::Sender<ExecutorCommand>,
}

impl ExecutorHandle {
    /// Submit a query for execution.
    pub async fn execute(&self, request: QueryRequest) -> Result<()> {
        self.command_tx
            .send(ExecutorCommand::Execute(request))
            .await
            .map_err(|_| Error::Channel("executor channel closed".to_string()))
    }

    /// Submit a query for execution (non-async version).
    pub fn execute_blocking(&self, request: QueryRequest) -> Result<()> {
        self.command_tx
            .blocking_send(ExecutorCommand::Execute(request))
            .map_err(|_| Error::Channel("executor channel closed".to_string()))
    }

    /// Cancel a running query.
    pub async fn cancel(&self, id: QueryId) -> Result<()> {
        self.command_tx
            .send(ExecutorCommand::Cancel(id))
            .await
            .map_err(|_| Error::Channel("executor channel closed".to_string()))
    }

    /// Submit a benchmark for execution (non-async version).
    pub fn benchmark_blocking(&self, request: BenchmarkRequest) -> Result<()> {
        self.command_tx
            .blocking_send(ExecutorCommand::Benchmark(request))
            .map_err(|_| Error::Channel("executor channel closed".to_string()))
    }

    /// Submit a describe request for execution (non-async version).
    pub fn describe_blocking(&self, request: DescribeRequest) -> Result<()> {
        self.command_tx
            .blocking_send(ExecutorCommand::Describe(request))
            .map_err(|_| Error::Channel("executor channel closed".to_string()))
    }

    /// Shutdown the executor.
    pub async fn shutdown(&self) -> Result<()> {
        self.command_tx
            .send(ExecutorCommand::Shutdown)
            .await
            .map_err(|_| Error::Channel("executor channel closed".to_string()))
    }
}

/// Async query executor that runs on a background task.
pub struct Executor {
    /// DataFusion session context.
    ctx: SessionContext,
    /// Channel for receiving commands.
    command_rx: mpsc::Receiver<ExecutorCommand>,
    /// Channel for sending events.
    event_tx: mpsc::Sender<QueryEvent>,
    /// Currently running queries (for cancellation).
    running: rustc_hash::FxHashSet<QueryId>,
}

impl Executor {
    /// Create a new executor with the given session context.
    ///
    /// Returns the executor and a handle for submitting queries.
    pub fn new(ctx: SessionContext, event_tx: mpsc::Sender<QueryEvent>) -> (Self, ExecutorHandle) {
        let (command_tx, command_rx) = mpsc::channel(64);

        let executor = Self {
            ctx,
            command_rx,
            event_tx,
            running: rustc_hash::FxHashSet::default(),
        };

        let handle = ExecutorHandle { command_tx };

        (executor, handle)
    }

    /// Run the executor loop.
    ///
    /// This should be spawned on a tokio runtime.
    pub async fn run(mut self) {
        log::info!("DataFusion executor started");

        while let Some(command) = self.command_rx.recv().await {
            match command {
                ExecutorCommand::Execute(request) => {
                    self.handle_execute(request).await;
                }
                ExecutorCommand::Explain(request) => {
                    self.handle_explain(request).await;
                }
                ExecutorCommand::Benchmark(request) => {
                    self.handle_benchmark(request).await;
                }
                ExecutorCommand::Describe(request) => {
                    self.handle_describe(request).await;
                }
                ExecutorCommand::Cancel(id) => {
                    self.handle_cancel(id);
                }
                ExecutorCommand::Shutdown => {
                    log::info!("DataFusion executor shutting down");
                    break;
                }
            }
        }
    }

    async fn handle_execute(&mut self, request: QueryRequest) {
        let id = request.id;
        self.running.insert(id);

        let start = Instant::now();

        // Parse and optimize the query
        let df = match self.ctx.sql(&request.sql).await {
            Ok(df) => df,
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: e.to_string(),
                })
                .await;
                self.running.remove(&id);
                return;
            }
        };

        // Apply limit if specified
        let df = match request.limit {
            Some(limit) => match df.limit(0, Some(limit)) {
                Ok(limited) => limited,
                Err(e) => {
                    self.send_event(QueryEvent::Failed {
                        id,
                        error: e.to_string(),
                    })
                    .await;
                    self.running.remove(&id);
                    return;
                }
            },
            None => df,
        };

        // Get schema and send started event
        let schema = Arc::new(df.schema().as_arrow().clone());
        self.send_event(QueryEvent::Started { id, schema }).await;

        // Execute and stream results
        let mut stream = match df.execute_stream().await {
            Ok(stream) => stream,
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: e.to_string(),
                })
                .await;
                self.running.remove(&id);
                return;
            }
        };

        let mut batch_num = 0;
        let mut total_rows = 0;

        while let Some(batch_result) = stream.next().await {
            // Check for cancellation
            if !self.running.contains(&id) {
                self.send_event(QueryEvent::Cancelled { id }).await;
                return;
            }

            match batch_result {
                Ok(batch) => {
                    total_rows += batch.num_rows();
                    self.send_event(QueryEvent::Batch {
                        id,
                        batch,
                        batch_num,
                    })
                    .await;
                    batch_num += 1;
                }
                Err(e) => {
                    self.send_event(QueryEvent::Failed {
                        id,
                        error: e.to_string(),
                    })
                    .await;
                    self.running.remove(&id);
                    return;
                }
            }
        }

        let elapsed = start.elapsed();
        self.send_event(QueryEvent::Completed {
            id,
            stats: ExecutionStats {
                total_time: elapsed,
                planning_time: std::time::Duration::ZERO, // TODO: track separately
                execution_time: elapsed,
                rows_returned: total_rows,
                bytes_scanned: 0, // TODO: extract from metrics
                partitions_scanned: 0,
            },
        })
        .await;

        self.running.remove(&id);
    }

    async fn handle_explain(&mut self, request: ExplainRequest) {
        let id = request.id;

        // Build EXPLAIN query
        let explain_sql = if request.analyze {
            format!("EXPLAIN ANALYZE {}", request.sql)
        } else if request.verbose {
            format!("EXPLAIN VERBOSE {}", request.sql)
        } else {
            format!("EXPLAIN {}", request.sql)
        };

        // Execute explain
        let df = match self.ctx.sql(&explain_sql).await {
            Ok(df) => df,
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: e.to_string(),
                })
                .await;
                return;
            }
        };

        let batches = match df.collect().await {
            Ok(batches) => batches,
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: e.to_string(),
                })
                .await;
                return;
            }
        };

        // Send results
        let schema = if let Some(batch) = batches.first() {
            batch.schema()
        } else {
            return;
        };

        self.send_event(QueryEvent::Started { id, schema }).await;

        for (batch_num, batch) in batches.into_iter().enumerate() {
            self.send_event(QueryEvent::Batch {
                id,
                batch,
                batch_num,
            })
            .await;
        }

        self.send_event(QueryEvent::Completed {
            id,
            stats: ExecutionStats::default(),
        })
        .await;
    }

    async fn handle_benchmark(&mut self, request: BenchmarkRequest) {
        let id = request.id;
        self.running.insert(id);

        // Create a dft ExecutionContext from the current session state.
        // SessionContext::state() clones the state with Arc-shared catalogs,
        // so registered tables remain visible.
        let exec_config = datafusion_app::config::ExecutionConfig::default();
        let exec_ctx = match ExecutionContext::try_new(
            &exec_config,
            self.ctx.state(),
            "enya",
            env!("CARGO_PKG_VERSION"),
        ) {
            Ok(ctx) => ctx,
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: format!("Failed to create benchmark context: {e}"),
                })
                .await;
                self.running.remove(&id);
                return;
            }
        };

        // Progress reporter that forwards to our event channel
        let reporter = Arc::new(ChannelProgressReporter {
            id,
            event_tx: self.event_tx.clone(),
        });

        match exec_ctx
            .benchmark_query(
                &request.sql,
                Some(request.iterations),
                false,
                Some(reporter),
            )
            .await
        {
            Ok(dft_stats) => {
                let stats = Box::new(BenchmarkStats::from_dft(&dft_stats));
                self.send_event(QueryEvent::BenchmarkCompleted { id, stats })
                    .await;
            }
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: format!("Benchmark failed: {e}"),
                })
                .await;
            }
        }

        self.running.remove(&id);
    }

    async fn handle_describe(&mut self, request: DescribeRequest) {
        let id = request.id;
        let start = Instant::now();

        // Get table schema
        let provider = match self.ctx.table_provider(&request.table_name).await {
            Ok(p) => p,
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: format!("Table '{}' not found: {e}", request.table_name),
                })
                .await;
                return;
            }
        };

        let schema = provider.schema();
        let fields: Vec<_> = schema.fields().iter().collect();

        if fields.is_empty() {
            self.send_event(QueryEvent::Failed {
                id,
                error: format!("Table '{}' has no columns", request.table_name),
            })
            .await;
            return;
        }

        // Build dynamic SQL for column statistics
        let mut select_parts = vec!["COUNT(*) AS \"__total_rows\"".to_string()];
        for field in &fields {
            let name = field.name();
            let quoted = format!("\"{name}\"");
            select_parts.push(format!("COUNT({quoted}) AS \"__count_{name}\""));
            select_parts.push(format!("COUNT(DISTINCT {quoted}) AS \"__distinct_{name}\""));
            if supports_min_max(field.data_type()) {
                select_parts.push(format!(
                    "CAST(MIN({quoted}) AS VARCHAR) AS \"__min_{name}\""
                ));
                select_parts.push(format!(
                    "CAST(MAX({quoted}) AS VARCHAR) AS \"__max_{name}\""
                ));
            }
            if is_numeric(field.data_type()) {
                select_parts.push(format!(
                    "AVG(CAST({quoted} AS DOUBLE)) AS \"__mean_{name}\""
                ));
            }
        }

        let sql = format!(
            "SELECT {} FROM \"{}\"",
            select_parts.join(", "),
            request.table_name
        );

        // Execute the stats query
        let batches = match self.ctx.sql(&sql).await {
            Ok(df) => match df.collect().await {
                Ok(b) => b,
                Err(e) => {
                    self.send_event(QueryEvent::Failed {
                        id,
                        error: format!("Failed to compute statistics: {e}"),
                    })
                    .await;
                    return;
                }
            },
            Err(e) => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: format!("Failed to build statistics query: {e}"),
                })
                .await;
                return;
            }
        };

        // Parse results from the single row
        let batch = match batches.first() {
            Some(b) if b.num_rows() > 0 => b,
            _ => {
                self.send_event(QueryEvent::Failed {
                    id,
                    error: "Statistics query returned no results".to_string(),
                })
                .await;
                return;
            }
        };

        let result_schema = batch.schema();

        // Helper to get a u64 value from a named column
        let get_count = |col_name: &str| -> usize {
            result_schema
                .column_with_name(col_name)
                .and_then(|(idx, _)| {
                    let arr = batch.column(idx);
                    if arr.is_null(0) {
                        return None;
                    }
                    arr.as_any()
                        .downcast_ref::<arrow::array::Int64Array>()
                        .map(|a| a.value(0) as usize)
                })
                .unwrap_or(0)
        };

        let get_string = |col_name: &str| -> Option<String> {
            result_schema
                .column_with_name(col_name)
                .and_then(|(idx, _)| {
                    let arr = batch.column(idx);
                    if arr.is_null(0) {
                        return None;
                    }
                    arr.as_any()
                        .downcast_ref::<arrow::array::StringArray>()
                        .map(|a| a.value(0).to_string())
                })
        };

        let get_f64 = |col_name: &str| -> Option<f64> {
            result_schema
                .column_with_name(col_name)
                .and_then(|(idx, _)| {
                    let arr = batch.column(idx);
                    if arr.is_null(0) {
                        return None;
                    }
                    arr.as_any()
                        .downcast_ref::<arrow::array::Float64Array>()
                        .map(|a| a.value(0))
                })
        };

        let total_rows = get_count("__total_rows");

        let mut columns = Vec::with_capacity(fields.len());
        for field in &fields {
            let name = field.name();
            let count = get_count(&format!("__count_{name}"));
            let distinct_count = get_count(&format!("__distinct_{name}"));

            let min = if supports_min_max(field.data_type()) {
                get_string(&format!("__min_{name}"))
            } else {
                None
            };
            let max = if supports_min_max(field.data_type()) {
                get_string(&format!("__max_{name}"))
            } else {
                None
            };
            let mean = if is_numeric(field.data_type()) {
                get_f64(&format!("__mean_{name}"))
            } else {
                None
            };

            columns.push(ColumnStats {
                name: name.clone(),
                data_type: field.data_type().to_string(),
                count,
                null_count: total_rows.saturating_sub(count),
                distinct_count,
                min,
                max,
                mean,
            });
        }

        let stats = Box::new(DescribeStats {
            table_name: request.table_name,
            total_rows,
            columns,
            elapsed: start.elapsed(),
        });

        self.send_event(QueryEvent::DescribeCompleted { id, stats })
            .await;
    }

    fn handle_cancel(&mut self, id: QueryId) {
        if self.running.remove(&id) {
            log::info!("Cancelled query {id}");
        }
    }

    async fn send_event(&self, event: QueryEvent) {
        if self.event_tx.send(event).await.is_err() {
            log::warn!("Failed to send query event - receiver dropped");
        }
    }
}

/// Progress reporter that sends benchmark progress events through the executor's
/// event channel. Implements dft's [`BenchmarkProgressReporter`] trait.
struct ChannelProgressReporter {
    id: QueryId,
    event_tx: mpsc::Sender<QueryEvent>,
}

impl BenchmarkProgressReporter for ChannelProgressReporter {
    fn on_iteration_complete(
        &self,
        completed: usize,
        total: usize,
        last_duration: std::time::Duration,
    ) {
        // Use try_send (non-blocking) since this trait method is sync.
        let _ = self.event_tx.try_send(QueryEvent::BenchmarkProgress {
            id: self.id,
            iteration: completed,
            total_iterations: total,
            last_duration,
        });
    }

    fn finish(&self) {
        // Stats are sent separately via BenchmarkCompleted event.
    }
}

/// Check if a DataType supports MIN/MAX aggregation.
fn supports_min_max(dt: &arrow::datatypes::DataType) -> bool {
    use arrow::datatypes::DataType::*;
    matches!(
        dt,
        Int8 | Int16
            | Int32
            | Int64
            | UInt8
            | UInt16
            | UInt32
            | UInt64
            | Float16
            | Float32
            | Float64
            | Decimal128(_, _)
            | Decimal256(_, _)
            | Utf8
            | LargeUtf8
            | Date32
            | Date64
            | Timestamp(_, _)
            | Boolean
    )
}

/// Check if a DataType is numeric (supports AVG).
fn is_numeric(dt: &arrow::datatypes::DataType) -> bool {
    use arrow::datatypes::DataType::*;
    matches!(
        dt,
        Int8 | Int16
            | Int32
            | Int64
            | UInt8
            | UInt16
            | UInt32
            | UInt64
            | Float16
            | Float32
            | Float64
            | Decimal128(_, _)
            | Decimal256(_, _)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_simple_query() {
        let ctx = SessionContext::new();
        let (event_tx, mut event_rx) = mpsc::channel(64);

        let (executor, handle) = Executor::new(ctx, event_tx);

        // Spawn executor
        tokio::spawn(executor.run());

        // Execute a simple query
        let request = QueryRequest::new("SELECT 1 + 1 AS result");
        let _id = request.id;
        handle.execute(request).await.unwrap();

        // Collect events
        let mut events = vec![];
        while let Ok(event) =
            tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv()).await
        {
            if let Some(e) = event {
                let is_complete = matches!(e, QueryEvent::Completed { .. });
                events.push(e);
                if is_complete {
                    break;
                }
            }
        }

        // Verify we got expected events
        assert!(
            events
                .iter()
                .any(|e| matches!(e, QueryEvent::Started { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, QueryEvent::Completed { .. }))
        );

        handle.shutdown().await.unwrap();
    }
}
