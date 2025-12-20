//! Query execution management for the Enya editor.
//!
//! Handles executing queries against backends (Prometheus, Enya) and
//! converting responses to visualization-ready data structures.

use std::collections::HashMap;

use rustc_hash::FxHashMap;

use enya_client::{
    DemoMetricsClient, HealthCheckManager, LabelsManager, MetricLabels, MetricLabelsManager,
    QueryManager, QueryRequest, QueryResponse, prometheus::PrometheusClient,
};

use crate::components::pane::time_series_chart::{DataPoint, Series};
use crate::components::pane::visualization::Visualization;

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

/// Result of polling for query completion.
#[derive(Debug)]
pub enum QueryPollResult {
    /// Query is still in flight
    Pending,
    /// Query completed successfully with data
    Complete {
        /// Number of data series returned
        series_count: usize,
        /// Total number of data points
        point_count: usize,
    },
    /// Query failed with an error
    Error(String),
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

/// Connection health status.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ConnectionHealth {
    /// Not connected (demo mode or disconnected)
    #[default]
    Offline,
    /// Health check in progress
    Checking,
    /// Successfully connected and validated
    Online {
        /// Backend version string
        version: String,
    },
    /// Connection failed
    Failed {
        /// Error message
        error: String,
    },
}

impl ConnectionHealth {
    /// Returns true if the connection is validated and online.
    pub fn is_online(&self) -> bool {
        matches!(self, ConnectionHealth::Online { .. })
    }
}

/// Manages query execution against a backend.
pub struct QueryExecutor {
    /// The current backend
    backend: Backend,
    /// Demo client for offline mode
    demo_client: DemoMetricsClient,
    /// Prometheus client (if connected)
    prometheus_client: Option<PrometheusClient>,
    /// Query manager for tracking in-flight queries
    query_manager: QueryManager,
    /// Labels manager for fetching metric names
    labels_manager: LabelsManager,
    /// Labels manager for fetching label names (tag keys)
    label_names_manager: LabelsManager,
    /// Labels manager for fetching per-metric labels
    metric_labels_manager: MetricLabelsManager,
    /// Health check manager for validating backend connectivity
    health_check_manager: HealthCheckManager,
    /// Current connection health status
    connection_health: ConnectionHealth,
    /// Cached list of available metric names
    metric_names: Vec<String>,
    /// Cached list of available label names (tag keys)
    label_names: Vec<String>,
    /// Cached per-metric labels (metric name -> labels)
    metric_labels_cache: HashMap<String, MetricLabels>,
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
            demo_client: DemoMetricsClient::new(),
            prometheus_client: None,
            query_manager: QueryManager::new(),
            labels_manager: LabelsManager::new(),
            label_names_manager: LabelsManager::new(),
            metric_labels_manager: MetricLabelsManager::new(),
            health_check_manager: HealthCheckManager::new(),
            connection_health: ConnectionHealth::Offline,
            metric_names: Vec::new(),
            label_names: Vec::new(),
            metric_labels_cache: HashMap::new(),
        }
    }

    /// Connect to a Prometheus backend and initiate a health check.
    ///
    /// The connection is not considered "online" until the health check passes.
    /// Call `poll_health_check()` to check for the result.
    pub fn connect_prometheus(&mut self, endpoint: &str, ctx: &egui::Context) {
        let client = PrometheusClient::new(endpoint);
        self.prometheus_client = Some(client);
        self.backend = Backend::Prometheus(endpoint.to_string());
        self.connection_health = ConnectionHealth::Checking;

        // Initiate health check
        if let Some(client) = &self.prometheus_client {
            self.health_check_manager.check(client, ctx);
        }
    }

    /// Disconnect and return to demo mode.
    pub fn disconnect(&mut self) {
        self.prometheus_client = None;
        self.backend = Backend::Demo;
        self.connection_health = ConnectionHealth::Offline;
        self.query_manager.cancel();
        self.labels_manager.cancel();
        self.label_names_manager.cancel();
        self.metric_labels_manager.cancel();
        self.health_check_manager.cancel();
        self.metric_names.clear();
        self.label_names.clear();
        self.metric_labels_cache.clear();
    }

    /// Check if connected to a backend (configured, but not necessarily validated).
    pub fn is_connected(&self) -> bool {
        !matches!(self.backend, Backend::Demo)
    }

    /// Check if the connection is validated and online.
    pub fn is_online(&self) -> bool {
        self.connection_health.is_online()
    }

    /// Get the current connection health status.
    pub fn connection_health(&self) -> &ConnectionHealth {
        &self.connection_health
    }

    /// Poll for health check completion.
    ///
    /// Returns `Some(true)` if health check passed, `Some(false)` if it failed,
    /// `None` if still in progress or no check pending.
    pub fn poll_health_check(&mut self) -> Option<bool> {
        if let Some(result) = self.health_check_manager.poll() {
            match result {
                Ok(info) => {
                    log::info!(
                        "Health check passed: {} v{}",
                        info.backend_type,
                        info.version
                    );
                    self.connection_health = ConnectionHealth::Online {
                        version: info.version,
                    };
                    Some(true)
                }
                Err(e) => {
                    log::error!("Health check failed: {e}");
                    self.connection_health = ConnectionHealth::Failed {
                        error: e.to_string(),
                    };
                    Some(false)
                }
            }
        } else {
            None
        }
    }

    /// Get the current backend type.
    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    /// Check if a query is currently in flight.
    pub fn is_querying(&self) -> bool {
        self.query_manager.is_querying()
    }

    /// Fetch metric names from the backend.
    ///
    /// For Prometheus, this fetches the `__name__` label values.
    /// For demo mode, uses the demo client's metric catalog.
    pub fn fetch_metric_names(&mut self, ctx: &egui::Context) {
        match &self.backend {
            Backend::Demo => {
                self.labels_manager
                    .fetch_metric_names(&self.demo_client, ctx);
            }
            Backend::Prometheus(_) => {
                if let Some(client) = &self.prometheus_client {
                    self.labels_manager.fetch_metric_names(client, ctx);
                }
            }
        }
    }

    /// Check if metric names are currently being fetched.
    pub fn is_fetching_metrics(&self) -> bool {
        self.labels_manager.is_fetching()
    }

    /// Poll for metric names fetch completion.
    ///
    /// Returns `true` if new metric names were received.
    pub fn poll_metric_names(&mut self) -> bool {
        if let Some(result) = self.labels_manager.poll() {
            match result {
                Ok(names) => {
                    log::debug!("Fetched {} metric names from Prometheus", names.len());
                    self.metric_names = names;
                    true
                }
                Err(e) => {
                    log::error!("Failed to fetch metric names: {e}");
                    false
                }
            }
        } else {
            false
        }
    }

    /// Get the cached metric names.
    pub fn metric_names(&self) -> &[String] {
        &self.metric_names
    }

    /// Fetch label names (tag keys) from the backend.
    ///
    /// For Prometheus, this fetches from `/api/v1/labels`.
    /// For demo mode, uses the demo client's label catalog.
    pub fn fetch_label_names(&mut self, ctx: &egui::Context) {
        match &self.backend {
            Backend::Demo => {
                self.label_names_manager
                    .fetch_label_names(&self.demo_client, ctx);
            }
            Backend::Prometheus(_) => {
                if let Some(client) = &self.prometheus_client {
                    self.label_names_manager.fetch_label_names(client, ctx);
                }
            }
        }
    }

    /// Check if label names are currently being fetched.
    pub fn is_fetching_labels(&self) -> bool {
        self.label_names_manager.is_fetching()
    }

    /// Poll for label names fetch completion.
    ///
    /// Returns `true` if new label names were received.
    pub fn poll_label_names(&mut self) -> bool {
        if let Some(result) = self.label_names_manager.poll() {
            match result {
                Ok(names) => {
                    log::debug!("Fetched {} label names from Prometheus", names.len());
                    // Filter out internal Prometheus labels (starting with __)
                    self.label_names = names
                        .into_iter()
                        .filter(|name| !name.starts_with("__"))
                        .collect();
                    true
                }
                Err(e) => {
                    log::error!("Failed to fetch label names: {e}");
                    false
                }
            }
        } else {
            false
        }
    }

    /// Get the cached label names (tag keys).
    pub fn label_names(&self) -> &[String] {
        &self.label_names
    }

    /// Fetch labels for a specific metric.
    ///
    /// If the labels are already cached, this does nothing.
    /// If a fetch is already in flight, this does nothing.
    pub fn fetch_metric_labels(&mut self, metric: &str, ctx: &egui::Context) {
        // Check cache first
        if self.metric_labels_cache.contains_key(metric) {
            return;
        }

        match &self.backend {
            Backend::Demo => {
                self.metric_labels_manager
                    .fetch(&self.demo_client, metric, ctx);
            }
            Backend::Prometheus(_) => {
                if let Some(client) = &self.prometheus_client {
                    self.metric_labels_manager.fetch(client, metric, ctx);
                }
            }
        }
    }

    /// Check if metric labels are currently being fetched.
    pub fn is_fetching_metric_labels(&self) -> bool {
        self.metric_labels_manager.is_fetching()
    }

    /// Get the metric name currently being fetched (if any).
    pub fn fetching_metric(&self) -> Option<&str> {
        self.metric_labels_manager.fetching_metric()
    }

    /// Poll for metric labels fetch completion.
    ///
    /// Returns `Some(metric_name)` if labels were just received, `None` otherwise.
    pub fn poll_metric_labels(&mut self) -> Option<String> {
        if let Some((metric, result)) = self.metric_labels_manager.poll() {
            match result {
                Ok(labels) => {
                    log::debug!(
                        "Fetched labels for metric '{}': {} label names",
                        metric,
                        labels.labels.len()
                    );
                    self.metric_labels_cache.insert(metric.clone(), labels);
                    Some(metric)
                }
                Err(e) => {
                    log::error!("Failed to fetch labels for metric '{metric}': {e}");
                    None
                }
            }
        } else {
            None
        }
    }

    /// Get cached labels for a specific metric.
    pub fn get_metric_labels(&self, metric: &str) -> Option<&MetricLabels> {
        self.metric_labels_cache.get(metric)
    }

    /// Check if labels for a specific metric are cached.
    pub fn has_metric_labels(&self, metric: &str) -> bool {
        self.metric_labels_cache.contains_key(metric)
    }

    /// Execute a query.
    ///
    /// For demo mode, uses the DemoMetricsClient to generate realistic data.
    /// For real backends, this fires off an async request.
    /// Poll with `poll()` to receive results.
    pub fn execute(
        &mut self,
        params: &ExecuteParams<'_>,
        _visualization: &mut Visualization,
        ctx: &egui::Context,
    ) {
        // Build request
        let mut request =
            QueryRequest::new(params.metric, params.query).with_step(params.step_secs);
        if let (Some(start), Some(end)) = (params.start_ns, params.end_ns) {
            request = request.with_range(start, end);
        }

        match &self.backend {
            Backend::Demo => {
                // Demo mode - use demo client for realistic data generation
                log::debug!(
                    "Executing DEMO query for metric '{}': {}",
                    params.metric,
                    params.query
                );
                self.query_manager.execute(&self.demo_client, request, ctx);
            }
            Backend::Prometheus(endpoint) => {
                if let Some(client) = &self.prometheus_client {
                    log::info!(
                        "Executing Prometheus query for metric '{}': {} (endpoint: {})",
                        params.metric,
                        params.query,
                        endpoint
                    );
                    self.query_manager.execute(client, request, ctx);
                }
            }
        }
    }

    /// Poll for query completion and update visualization if ready.
    ///
    /// Returns the poll result indicating pending, complete, or error.
    pub fn poll(&mut self, visualization: &mut Visualization) -> QueryPollResult {
        if let Some(result) = self.query_manager.poll() {
            match result {
                Ok(response) => {
                    let backend_name = match &self.backend {
                        Backend::Demo => "Demo",
                        Backend::Prometheus(_) => "Prometheus",
                    };
                    let series_count = response.groups.len();
                    let point_count: usize = response.groups.iter().map(|g| g.buckets.len()).sum();
                    log::info!(
                        "{backend_name} query completed: {series_count} groups, {point_count} total points"
                    );
                    visualization.clear();
                    visualization.set_metric_name(&response.metric);
                    populate_from_response(visualization, &response);
                    QueryPollResult::Complete {
                        series_count,
                        point_count,
                    }
                }
                Err(e) => {
                    log::error!("Query failed: {e}");
                    QueryPollResult::Error(e.to_string())
                }
            }
        } else {
            QueryPollResult::Pending
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
fn parse_group_tags(group: &str) -> FxHashMap<String, String> {
    let mut tags = FxHashMap::default();

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
        // Manually set up connection state for test (no egui context available)
        executor.prometheus_client = Some(enya_client::prometheus::PrometheusClient::new(
            "http://localhost:9090",
        ));
        executor.backend = Backend::Prometheus("http://localhost:9090".to_string());
        assert!(executor.is_connected());
        assert!(matches!(executor.backend(), Backend::Prometheus(_)));
    }
}
