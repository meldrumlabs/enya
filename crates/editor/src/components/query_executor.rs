//! Query execution management for the Enya editor.
//!
//! Handles executing queries against backends (Prometheus, Enya) and
//! converting responses to visualization-ready data structures.

use std::collections::HashMap;

use enya_client::{QueryManager, QueryRequest, QueryResponse, prometheus::PrometheusClient};

use super::time_series_chart::{DataPoint, Series};
use super::visualization::{Visualization, populate_demo_data};

/// Backend type for query execution.
#[derive(Debug, Clone, PartialEq)]
pub enum Backend {
    /// Demo mode - uses generated data
    Demo,
    /// Prometheus backend
    Prometheus(String),
}

impl Default for Backend {
    fn default() -> Self {
        Self::Demo
    }
}

/// Parameters for executing a query.
pub struct ExecuteParams<'a> {
    /// The metric name
    pub metric: &'a str,
    /// The enya-lang query string
    pub query: &'a str,
    /// Query step/granularity in seconds
    pub step_secs: u64,
    /// Start of time range (nanoseconds since epoch)
    pub start_ns: Option<u128>,
    /// End of time range (nanoseconds since epoch)
    pub end_ns: Option<u128>,
}

/// Manages query execution against a backend.
pub struct QueryExecutor {
    /// The current backend
    backend: Backend,
    /// Prometheus client (if connected)
    prometheus_client: Option<PrometheusClient>,
    /// Query manager for tracking in-flight queries
    query_manager: QueryManager,
}

impl Default for QueryExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryExecutor {
    /// Create a new query executor in demo mode.
    pub fn new() -> Self {
        Self {
            backend: Backend::Demo,
            prometheus_client: None,
            query_manager: QueryManager::new(),
        }
    }

    /// Connect to a Prometheus backend.
    pub fn connect_prometheus(&mut self, endpoint: &str) {
        let client = PrometheusClient::new(endpoint);
        self.prometheus_client = Some(client);
        self.backend = Backend::Prometheus(endpoint.to_string());
    }

    /// Disconnect and return to demo mode.
    pub fn disconnect(&mut self) {
        self.prometheus_client = None;
        self.backend = Backend::Demo;
        self.query_manager.cancel();
    }

    /// Check if connected to a backend.
    pub fn is_connected(&self) -> bool {
        !matches!(self.backend, Backend::Demo)
    }

    /// Get the current backend type.
    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    /// Check if a query is currently in flight.
    pub fn is_querying(&self) -> bool {
        self.query_manager.is_querying()
    }

    /// Execute a query.
    ///
    /// For demo mode, this immediately populates the visualization with demo data.
    /// For real backends, this fires off an async request.
    pub fn execute(
        &mut self,
        params: &ExecuteParams<'_>,
        visualization: &mut Visualization,
        ctx: &egui::Context,
    ) {
        match &self.backend {
            Backend::Demo => {
                // Demo mode - populate with generated data
                visualization.clear();
                visualization.set_metric_name(params.query);
                populate_demo_data(visualization, params.query);
            }
            Backend::Prometheus(_) => {
                if let Some(client) = &self.prometheus_client {
                    // Build request
                    let mut request =
                        QueryRequest::new(params.metric, params.query).with_step(params.step_secs);
                    if let (Some(start), Some(end)) = (params.start_ns, params.end_ns) {
                        request = request.with_range(start, end);
                    }

                    // Fire off query
                    self.query_manager.execute(client, request, ctx);
                }
            }
        }
    }

    /// Poll for query completion and update visualization if ready.
    ///
    /// Returns `true` if data was updated.
    pub fn poll(&mut self, visualization: &mut Visualization) -> bool {
        if let Some(result) = self.query_manager.poll() {
            match result {
                Ok(response) => {
                    visualization.clear();
                    visualization.set_metric_name(&response.metric);
                    populate_from_response(visualization, &response);
                    true
                }
                Err(e) => {
                    log::error!("Query failed: {e}");
                    // Could show error in visualization
                    false
                }
            }
        } else {
            false
        }
    }
}

/// Convert a QueryResponse to visualization data.
pub fn populate_from_response(visualization: &mut Visualization, response: &QueryResponse) {
    let series_list = response_to_series(response);
    visualization.set_series(series_list);
}

/// Convert a QueryResponse to a list of Series for time series charts.
pub fn response_to_series(response: &QueryResponse) -> Vec<Series> {
    response
        .groups
        .iter()
        .map(|group| {
            // Parse group identifier into tags
            let tags = parse_group_tags(&group.group);

            // Convert buckets to data points
            let points: Vec<DataPoint> = group
                .buckets
                .iter()
                .map(|bucket| {
                    // Convert nanoseconds to seconds for plotting
                    let timestamp = (bucket.start as f64) / 1_000_000_000.0;
                    DataPoint {
                        timestamp,
                        value: bucket.value,
                    }
                })
                .collect();

            Series::new(&response.metric)
                .with_points(points)
                .with_tags_map(tags)
        })
        .collect()
}

/// Parse a group identifier string into a tag map.
///
/// Group format: "key1:value1,key2:value2" or "{key1=\"value1\", key2=\"value2\"}"
fn parse_group_tags(group: &str) -> HashMap<String, String> {
    let mut tags = HashMap::new();

    if group.is_empty() {
        return tags;
    }

    // Handle Prometheus-style format: {key="value", ...}
    let group = group.trim_start_matches('{').trim_end_matches('}');

    for part in group.split(',') {
        let part = part.trim();
        // Try both "key=value" and "key:value" formats
        if let Some((key, value)) = part.split_once('=') {
            let key = key.trim().trim_matches('"');
            let value = value.trim().trim_matches('"');
            tags.insert(key.to_string(), value.to_string());
        } else if let Some((key, value)) = part.split_once(':') {
            tags.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_group_tags_empty() {
        let tags = parse_group_tags("");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_group_tags_enya_format() {
        let tags = parse_group_tags("env:prod,host:server1");
        assert_eq!(tags.get("env"), Some(&"prod".to_string()));
        assert_eq!(tags.get("host"), Some(&"server1".to_string()));
    }

    #[test]
    fn test_parse_group_tags_prometheus_format() {
        let tags = parse_group_tags(r#"{env="prod", host="server1"}"#);
        assert_eq!(tags.get("env"), Some(&"prod".to_string()));
        assert_eq!(tags.get("host"), Some(&"server1".to_string()));
    }

    #[test]
    fn test_query_executor_default_demo() {
        let executor = QueryExecutor::new();
        assert!(!executor.is_connected());
        assert_eq!(executor.backend(), &Backend::Demo);
    }

    #[test]
    fn test_query_executor_connect_prometheus() {
        let mut executor = QueryExecutor::new();
        executor.connect_prometheus("http://localhost:9090");
        assert!(executor.is_connected());
        assert!(matches!(executor.backend(), Backend::Prometheus(_)));
    }
}
