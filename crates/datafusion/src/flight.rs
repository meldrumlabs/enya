//! Arrow Flight SQL client for connecting to remote databases.
//!
//! This module provides a high-level client for Flight SQL servers, including:
//! - DataFusion-based servers
//! - DuckDB
//! - InfluxDB IOx
//! - Dremio
//! - Any Flight SQL compatible database
//!
//! # Example
//!
//! ```ignore
//! use enya_datafusion::flight::FlightClient;
//!
//! let client = FlightClient::connect("http://localhost:50051").await?;
//! let mut stream = client.execute("SELECT * FROM events LIMIT 10").await?;
//! while let Some(batch) = stream.next().await {
//!     // Process RecordBatch
//! }
//! ```

use std::sync::Arc;
use std::time::Duration;

use arrow::array::{Array, RecordBatch};
use arrow::datatypes::SchemaRef;
use arrow_flight::FlightInfo;
use arrow_flight::decode::FlightRecordBatchStream;
use arrow_flight::sql::client::FlightSqlServiceClient;
use arrow_flight::sql::{CommandGetDbSchemas, CommandGetTables};
use futures::{StreamExt, TryStreamExt};
use tonic::transport::{Channel, Endpoint};

use crate::Result;
use crate::error::Error;
use crate::types::{ColumnInfo, TableInfo, TableSource};

/// Connection state for a Flight SQL client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected to any server.
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Connected and ready.
    Connected,
    /// Connection failed with error.
    Failed(String),
}

/// Configuration for Flight SQL connections.
#[derive(Debug, Clone)]
pub struct FlightConfig {
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Request timeout.
    pub request_timeout: Duration,
    /// Optional bearer token for authentication.
    pub token: Option<String>,
    /// Optional username for basic auth.
    pub username: Option<String>,
    /// Optional password for basic auth.
    pub password: Option<String>,
}

impl Default for FlightConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(60),
            token: None,
            username: None,
            password: None,
        }
    }
}

/// A Flight SQL client for executing queries on remote servers.
pub struct FlightClient {
    /// The underlying Flight SQL client.
    client: FlightSqlServiceClient<Channel>,
    /// The endpoint URL.
    endpoint: String,
}

impl FlightClient {
    /// Connect to a Flight SQL server.
    ///
    /// # Arguments
    /// * `endpoint` - The server URL (e.g., "http://localhost:50051" or "grpc://host:port")
    ///
    /// # Example
    /// ```ignore
    /// let client = FlightClient::connect("http://localhost:50051").await?;
    /// ```
    pub async fn connect(endpoint: &str) -> Result<Self> {
        Self::connect_with_config(endpoint, FlightConfig::default()).await
    }

    /// Connect to a Flight SQL server with custom configuration.
    pub async fn connect_with_config(endpoint: &str, config: FlightConfig) -> Result<Self> {
        // Normalize endpoint URL
        let endpoint_url = if endpoint.starts_with("grpc://") {
            endpoint.replace("grpc://", "http://")
        } else if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            format!("http://{endpoint}")
        } else {
            endpoint.to_string()
        };

        // Build tonic channel
        let channel = Endpoint::from_shared(endpoint_url.clone())
            .map_err(|e| Error::FlightConnection {
                endpoint: endpoint.to_string(),
                message: format!("Invalid endpoint: {e}"),
            })?
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .connect()
            .await
            .map_err(|e| Error::FlightConnection {
                endpoint: endpoint.to_string(),
                message: format!("Connection failed: {e}"),
            })?;

        let mut client = FlightSqlServiceClient::new(channel);

        // Authenticate if credentials provided
        if let Some(token) = &config.token {
            client.set_token(token.clone());
        }

        // TODO: Handle basic auth with handshake if username/password provided

        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
        })
    }

    /// Get the connected endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Execute a SQL query and return a stream of record batches.
    ///
    /// # Arguments
    /// * `sql` - The SQL query to execute
    ///
    /// # Returns
    /// A stream of `RecordBatch` results.
    pub async fn execute(&mut self, sql: &str) -> Result<QueryStream> {
        // Execute the query to get FlightInfo
        let flight_info = self
            .client
            .execute(sql.to_string(), None)
            .await
            .map_err(|e| Error::FlightQuery {
                sql: sql.to_string(),
                message: format!("Execute failed: {e}"),
            })?;

        // Get schema from FlightInfo
        let schema =
            Arc::new(
                flight_info
                    .clone()
                    .try_decode_schema()
                    .map_err(|e| Error::FlightQuery {
                        sql: sql.to_string(),
                        message: format!("Failed to decode schema: {e}"),
                    })?,
            );

        // Collect all endpoints and fetch data
        let endpoints = flight_info.endpoint;
        if endpoints.is_empty() {
            return Ok(QueryStream::empty(schema));
        }

        // For now, handle the first endpoint (most common case)
        // TODO: Handle multiple endpoints for distributed queries
        let ticket = endpoints[0]
            .ticket
            .clone()
            .ok_or_else(|| Error::FlightQuery {
                sql: sql.to_string(),
                message: "No ticket in flight endpoint".to_string(),
            })?;

        let stream = self
            .client
            .do_get(ticket)
            .await
            .map_err(|e| Error::FlightQuery {
                sql: sql.to_string(),
                message: format!("DoGet failed: {e}"),
            })?;

        Ok(QueryStream::new(schema, stream))
    }

    /// Get the schema for a query without executing it.
    pub async fn get_schema(&mut self, sql: &str) -> Result<SchemaRef> {
        let flight_info = self
            .client
            .execute(sql.to_string(), None)
            .await
            .map_err(|e| Error::FlightQuery {
                sql: sql.to_string(),
                message: format!("Execute failed: {e}"),
            })?;

        let schema = flight_info
            .try_decode_schema()
            .map_err(|e| Error::FlightQuery {
                sql: sql.to_string(),
                message: format!("Failed to decode schema: {e}"),
            })?;

        Ok(Arc::new(schema))
    }

    /// Get list of catalogs from the server.
    pub async fn get_catalogs(&mut self) -> Result<Vec<String>> {
        let flight_info = self
            .client
            .get_catalogs()
            .await
            .map_err(|e| Error::FlightMetadata {
                message: format!("GetCatalogs failed: {e}"),
            })?;

        let batches = self.fetch_flight_info(flight_info).await?;

        // Extract catalog names from batches
        let mut catalogs = Vec::new();
        for batch in batches {
            if let Some(col) = batch.column_by_name("catalog_name") {
                if let Some(arr) = col.as_any().downcast_ref::<arrow::array::StringArray>() {
                    for i in 0..arr.len() {
                        if !arr.is_null(i) {
                            catalogs.push(arr.value(i).to_string());
                        }
                    }
                }
            }
        }

        Ok(catalogs)
    }

    /// Get list of schemas (databases) from the server.
    pub async fn get_schemas(&mut self, catalog: Option<&str>) -> Result<Vec<String>> {
        let cmd = CommandGetDbSchemas {
            catalog: catalog.map(String::from),
            db_schema_filter_pattern: None,
        };

        let flight_info =
            self.client
                .get_db_schemas(cmd)
                .await
                .map_err(|e| Error::FlightMetadata {
                    message: format!("GetSchemas failed: {e}"),
                })?;

        let batches = self.fetch_flight_info(flight_info).await?;

        let mut schemas = Vec::new();
        for batch in batches {
            if let Some(col) = batch.column_by_name("db_schema_name") {
                if let Some(arr) = col.as_any().downcast_ref::<arrow::array::StringArray>() {
                    for i in 0..arr.len() {
                        if !arr.is_null(i) {
                            schemas.push(arr.value(i).to_string());
                        }
                    }
                }
            }
        }

        Ok(schemas)
    }

    /// Get list of tables from the server.
    pub async fn get_tables(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
    ) -> Result<Vec<TableInfo>> {
        let cmd = CommandGetTables {
            catalog: catalog.map(String::from),
            db_schema_filter_pattern: schema.map(String::from),
            table_name_filter_pattern: None,
            table_types: vec!["TABLE".to_string(), "VIEW".to_string()],
            include_schema: false,
        };

        let flight_info = self
            .client
            .get_tables(cmd)
            .await
            .map_err(|e| Error::FlightMetadata {
                message: format!("GetTables failed: {e}"),
            })?;

        let batches = self.fetch_flight_info(flight_info).await?;

        let mut tables = Vec::new();
        for batch in batches {
            let catalog_col = batch.column_by_name("catalog_name");
            let schema_col = batch.column_by_name("db_schema_name");
            let table_col = batch.column_by_name("table_name");
            let _type_col = batch.column_by_name("table_type");

            if let (Some(table_arr), Some(schema_arr), Some(catalog_arr)) =
                (table_col, schema_col, catalog_col)
            {
                let table_arr = table_arr
                    .as_any()
                    .downcast_ref::<arrow::array::StringArray>();
                let schema_arr = schema_arr
                    .as_any()
                    .downcast_ref::<arrow::array::StringArray>();
                let catalog_arr = catalog_arr
                    .as_any()
                    .downcast_ref::<arrow::array::StringArray>();

                if let (Some(tables_a), Some(schemas_a), Some(catalogs_a)) =
                    (table_arr, schema_arr, catalog_arr)
                {
                    for i in 0..tables_a.len() {
                        if !tables_a.is_null(i) {
                            let schema_str = if schemas_a.is_null(i) {
                                String::new()
                            } else {
                                schemas_a.value(i).to_string()
                            };
                            let catalog_str = if catalogs_a.is_null(i) {
                                String::new()
                            } else {
                                catalogs_a.value(i).to_string()
                            };
                            tables.push(TableInfo {
                                name: tables_a.value(i).to_string(),
                                schema: schema_str,
                                catalog: catalog_str,
                                columns: vec![], // Fetch separately if needed
                                row_count: None,
                                source: TableSource::Memory, // Remote tables
                            });
                        }
                    }
                }
            }
        }

        Ok(tables)
    }

    /// Get column information for a specific table.
    pub async fn get_columns(
        &mut self,
        _catalog: Option<&str>,
        _schema: Option<&str>,
        table: &str,
    ) -> Result<Vec<ColumnInfo>> {
        // Use information_schema query as fallback since GetColumns isn't always implemented
        // TODO: Use catalog/schema filters when available
        let sql = format!(
            "SELECT column_name, data_type, is_nullable FROM information_schema.columns WHERE table_name = '{}'",
            table.replace('\'', "''")
        );

        let mut stream = self.execute(&sql).await?;
        let batches = stream.collect().await?;

        let mut columns = Vec::new();
        for batch in batches {
            let name_col = batch.column_by_name("column_name");
            let type_col = batch.column_by_name("data_type");
            let null_col = batch.column_by_name("is_nullable");

            if let (Some(names), Some(types)) = (name_col, type_col) {
                let names = names.as_any().downcast_ref::<arrow::array::StringArray>();
                let types = types.as_any().downcast_ref::<arrow::array::StringArray>();
                let nulls =
                    null_col.and_then(|c| c.as_any().downcast_ref::<arrow::array::StringArray>());

                if let (Some(names_a), Some(types_a)) = (names, types) {
                    for i in 0..names_a.len() {
                        if !names_a.is_null(i) {
                            let nullable = nulls
                                .map(|n| !n.is_null(i) && n.value(i).to_uppercase() == "YES")
                                .unwrap_or(true);

                            columns.push(ColumnInfo {
                                name: names_a.value(i).to_string(),
                                data_type: types_a.value(i).to_string(),
                                nullable,
                            });
                        }
                    }
                }
            }
        }

        Ok(columns)
    }

    /// Fetch all batches from a FlightInfo.
    async fn fetch_flight_info(&mut self, info: FlightInfo) -> Result<Vec<RecordBatch>> {
        let endpoints = info.endpoint;
        if endpoints.is_empty() {
            return Ok(vec![]);
        }

        let ticket = endpoints[0]
            .ticket
            .clone()
            .ok_or_else(|| Error::FlightMetadata {
                message: "No ticket in flight endpoint".to_string(),
            })?;

        let stream = self
            .client
            .do_get(ticket)
            .await
            .map_err(|e| Error::FlightMetadata {
                message: format!("DoGet failed: {e}"),
            })?;

        let batches: Vec<RecordBatch> =
            stream
                .try_collect()
                .await
                .map_err(|e| Error::FlightMetadata {
                    message: format!("Stream error: {e}"),
                })?;

        Ok(batches)
    }
}

/// A stream of query results from a Flight SQL server.
pub struct QueryStream {
    schema: SchemaRef,
    inner: Option<FlightRecordBatchStream>,
}

impl QueryStream {
    fn new(schema: SchemaRef, stream: FlightRecordBatchStream) -> Self {
        Self {
            schema,
            inner: Some(stream),
        }
    }

    fn empty(schema: SchemaRef) -> Self {
        Self {
            schema,
            inner: None,
        }
    }

    /// Get the schema of the query results.
    pub fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    /// Get the next batch from the stream.
    pub async fn next(&mut self) -> Option<Result<RecordBatch>> {
        let stream = self.inner.as_mut()?;
        match stream.next().await {
            Some(Ok(batch)) => Some(Ok(batch)),
            Some(Err(e)) => Some(Err(Error::FlightStream {
                message: format!("Stream error: {e}"),
            })),
            None => None,
        }
    }

    /// Collect all batches from the stream.
    pub async fn collect(&mut self) -> Result<Vec<RecordBatch>> {
        let mut batches = Vec::new();
        while let Some(result) = self.next().await {
            batches.push(result?);
        }
        Ok(batches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = FlightConfig::default();
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert!(config.token.is_none());
    }

    #[test]
    fn test_connection_state() {
        let state = ConnectionState::Disconnected;
        assert_eq!(state, ConnectionState::Disconnected);

        let failed = ConnectionState::Failed("timeout".to_string());
        assert!(matches!(failed, ConnectionState::Failed(_)));
    }
}
