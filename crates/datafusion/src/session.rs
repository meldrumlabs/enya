//! DataFusion session management.
//!
//! The [`Session`] provides a high-level interface for SQL query execution,
//! table registration, and catalog management.

use datafusion::execution::context::SessionState;
use datafusion::prelude::*;
use datafusion_app::extensions::DftSessionStateBuilder;
use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::Result;
use crate::catalog::Catalog;
use crate::error::{Error, QueryId};
use crate::executor::{Executor, ExecutorHandle};
use crate::types::{
    BenchmarkRequest, ColumnInfo, DescribeRequest, FileFormat, QueryEvent, QueryRequest, TableInfo,
};

/// Configuration for a DataFusion session.
#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum memory per query (bytes).
    pub memory_limit: Option<usize>,
    /// Number of target partitions for parallelism.
    pub target_partitions: usize,
    /// Enable statement-level caching.
    pub enable_cache: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            memory_limit: None,
            target_partitions: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4),
            enable_cache: true,
        }
    }
}

/// A DataFusion session for executing SQL queries.
///
/// This is the main entry point for the `enya-datafusion` crate. It wraps a
/// DataFusion `SessionContext` and provides:
///
/// - Async query execution via channels
/// - Table registration from files and object stores
/// - Catalog browsing and search
/// - Query plan extraction
pub struct Session {
    /// The underlying DataFusion session.
    ctx: SessionContext,
    /// Catalog state.
    catalog: RwLock<Catalog>,
    /// Executor handle for submitting queries.
    executor_handle: Option<ExecutorHandle>,
}

impl Session {
    /// Create a new session with default configuration.
    pub fn new() -> Self {
        Self::with_config(Config::default())
    }

    /// Create a new session with custom configuration.
    pub fn with_config(config: Config) -> Self {
        let exec_config = to_execution_config(&config);

        let state = DftSessionStateBuilder::try_new(Some(exec_config))
            .and_then(|b| Ok(b.build()?))
            .expect("failed to build DataFusion session state");

        let ctx = SessionContext::new_with_state(state);

        Self {
            ctx,
            catalog: RwLock::new(Catalog::new()),
            executor_handle: None,
        }
    }

    /// Create a session with dft extensions (S3, Delta Lake, etc.) enabled.
    ///
    /// This is async because extension registration (e.g. connecting to S3)
    /// may perform IO.
    pub async fn with_extensions(config: Config) -> crate::Result<Self> {
        let exec_config = to_execution_config(&config);

        let state = DftSessionStateBuilder::try_new(Some(exec_config))
            .map_err(|e| Error::SessionBuilder(e.to_string()))?
            .with_extensions()
            .await
            .map_err(|e| Error::SessionBuilder(e.to_string()))?
            .build()
            .map_err(Error::Execution)?;

        let ctx = SessionContext::new_with_state(state);

        Ok(Self {
            ctx,
            catalog: RwLock::new(Catalog::new()),
            executor_handle: None,
        })
    }

    /// Create a session from a pre-built [`SessionState`].
    ///
    /// Use this for advanced configuration via [`DftSessionStateBuilder`]
    /// or DataFusion's [`SessionStateBuilder`](datafusion::execution::session_state::SessionStateBuilder).
    pub fn with_session_state(state: SessionState) -> Self {
        let ctx = SessionContext::new_with_state(state);
        Self {
            ctx,
            catalog: RwLock::new(Catalog::new()),
            executor_handle: None,
        }
    }

    /// Initialize the async executor.
    ///
    /// This must be called before executing queries. Returns a receiver
    /// for query events that should be polled each frame.
    ///
    /// The runtime handle is used to spawn the executor task. This allows
    /// the session to be created outside of an async context (e.g., in egui).
    pub fn init_executor(
        &mut self,
        runtime_handle: tokio::runtime::Handle,
    ) -> mpsc::Receiver<QueryEvent> {
        let (event_tx, event_rx) = mpsc::channel(256);
        let (executor, handle) = Executor::new(self.ctx.clone(), event_tx);

        // Spawn executor on the provided runtime
        runtime_handle.spawn(executor.run());

        self.executor_handle = Some(handle);

        event_rx
    }

    /// Get the executor handle.
    pub fn executor(&self) -> Option<&ExecutorHandle> {
        self.executor_handle.as_ref()
    }

    /// Execute a SQL query.
    ///
    /// Returns immediately. Poll the event receiver for results.
    pub fn execute(&self, request: QueryRequest) -> Result<()> {
        let handle = self.executor_handle.as_ref().ok_or(Error::NotInitialized)?;
        handle.execute_blocking(request)
    }

    /// Submit a benchmark request for execution.
    ///
    /// Returns immediately. Poll the event receiver for `BenchmarkProgress`
    /// and `BenchmarkCompleted` events.
    pub fn benchmark(&self, request: BenchmarkRequest) -> Result<()> {
        let handle = self.executor_handle.as_ref().ok_or(Error::NotInitialized)?;
        handle.benchmark_blocking(request)
    }

    /// Submit a describe request for a table.
    ///
    /// Returns immediately. Poll the event receiver for `DescribeCompleted` events.
    pub fn describe(&self, request: DescribeRequest) -> Result<()> {
        let handle = self.executor_handle.as_ref().ok_or(Error::NotInitialized)?;
        handle.describe_blocking(request)
    }

    /// Cancel a running query by ID.
    ///
    /// Signals the executor to stop the query at the next batch boundary.
    /// The query will emit a `QueryEvent::Cancelled` event.
    pub fn cancel(&self, id: QueryId) -> Result<()> {
        let handle = self.executor_handle.as_ref().ok_or(Error::NotInitialized)?;
        handle.cancel_blocking(id)
    }

    /// Execute a SQL query and collect all results.
    ///
    /// This is a blocking convenience method for simple queries.
    pub async fn execute_collect(&self, sql: &str) -> Result<Vec<arrow::array::RecordBatch>> {
        let df = self.ctx.sql(sql).await?;
        Ok(df.collect().await?)
    }

    /// Register a Parquet file as a table.
    pub async fn register_parquet(&self, name: &str, path: &str) -> Result<()> {
        self.ctx
            .register_parquet(name, path, ParquetReadOptions::default())
            .await
            .map_err(|e| Error::TableRegistration {
                name: name.to_string(),
                source: e,
            })?;

        // Update catalog
        self.refresh_catalog_for_table(name, path, FileFormat::Parquet)
            .await;

        Ok(())
    }

    /// Register a CSV file as a table.
    pub async fn register_csv(&self, name: &str, path: &str) -> Result<()> {
        self.ctx
            .register_csv(name, path, CsvReadOptions::default())
            .await
            .map_err(|e| Error::TableRegistration {
                name: name.to_string(),
                source: e,
            })?;

        self.refresh_catalog_for_table(name, path, FileFormat::Csv)
            .await;

        Ok(())
    }

    /// Register a JSON file as a table.
    pub async fn register_json(&self, name: &str, path: &str) -> Result<()> {
        self.ctx
            .register_json(name, path, NdJsonReadOptions::default())
            .await
            .map_err(|e| Error::TableRegistration {
                name: name.to_string(),
                source: e,
            })?;

        self.refresh_catalog_for_table(name, path, FileFormat::Json)
            .await;

        Ok(())
    }

    /// Register a file as a table, auto-detecting the format.
    pub async fn register_file(&self, name: &str, path: &str) -> Result<()> {
        let format = FileFormat::from_path(path)
            .ok_or_else(|| Error::UnsupportedFormat(path.to_string()))?;

        match format {
            FileFormat::Parquet => self.register_parquet(name, path).await,
            FileFormat::Csv => self.register_csv(name, path).await,
            FileFormat::Json | FileFormat::NdJson => self.register_json(name, path).await,
            _ => Err(Error::UnsupportedFormat(format.as_str().to_string())),
        }
    }

    /// Deregister a table.
    pub async fn deregister_table(&self, name: &str) -> Result<()> {
        self.ctx.deregister_table(name)?;
        self.catalog.write().remove_table(name);
        Ok(())
    }

    /// Get all registered tables.
    pub fn tables(&self) -> Vec<TableInfo> {
        self.catalog.read().tables().cloned().collect()
    }

    /// Get a table by name.
    pub fn get_table(&self, name: &str) -> Option<TableInfo> {
        self.catalog.read().get_table(name).cloned()
    }

    /// Search tables by pattern.
    pub fn search_tables(&self, pattern: &str) -> Vec<TableInfo> {
        self.catalog
            .read()
            .search_tables(pattern)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Refresh the entire catalog from the session context.
    pub fn refresh_catalog(&self) {
        self.catalog.write().refresh(&self.ctx);
    }

    /// Get the underlying DataFusion session context.
    ///
    /// Use this for advanced operations not exposed by the Session API.
    pub fn context(&self) -> &SessionContext {
        &self.ctx
    }

    async fn refresh_catalog_for_table(&self, name: &str, path: &str, format: FileFormat) {
        // Get schema from the registered table
        if let Ok(provider) = self.ctx.table_provider(name).await {
            let schema = provider.schema();
            let columns = schema
                .fields()
                .iter()
                .map(|f| ColumnInfo {
                    name: f.name().clone(),
                    data_type: f.data_type().to_string(),
                    nullable: f.is_nullable(),
                })
                .collect();

            self.catalog.write().record_file_table(
                name.to_string(),
                path.to_string(),
                format,
                columns,
            );
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert our [`Config`] to a [`datafusion_app::config::ExecutionConfig`].
#[allow(clippy::disallowed_types)]
fn to_execution_config(config: &Config) -> datafusion_app::config::ExecutionConfig {
    let mut df = std::collections::HashMap::new();
    df.insert(
        "datafusion.execution.target_partitions".to_string(),
        config.target_partitions.to_string(),
    );

    datafusion_app::config::ExecutionConfig {
        datafusion: Some(df),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let session = Session::new();
        assert!(session.tables().is_empty());
    }

    #[tokio::test]
    async fn test_session_simple_query() {
        let session = Session::new();
        let result = session
            .execute_collect("SELECT 1 + 1 AS sum")
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].num_rows(), 1);
    }
}
