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

use crate::Result;
use crate::error::{Error, QueryId};
use crate::types::{ExecutionStats, ExplainRequest, QueryEvent, QueryRequest};

/// Commands sent to the executor.
#[derive(Debug)]
pub enum ExecutorCommand {
    /// Execute a SQL query.
    Execute(QueryRequest),
    /// Explain a query plan.
    Explain(ExplainRequest),
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
