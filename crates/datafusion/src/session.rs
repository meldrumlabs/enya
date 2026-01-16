//! DataFusion session management.
//!
//! The [`Session`] provides a high-level interface for SQL query execution,
//! table registration, and catalog management.

use datafusion::prelude::*;
use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::Result;
use crate::catalog::Catalog;
use crate::error::Error;
use crate::executor::{Executor, ExecutorHandle};
use crate::types::{ColumnInfo, FileFormat, QueryEvent, QueryRequest, TableInfo};

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
            target_partitions: num_cpus::get(),
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
        let df_config = datafusion::prelude::SessionConfig::new()
            .with_target_partitions(config.target_partitions);

        let ctx = SessionContext::new_with_config(df_config);

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
