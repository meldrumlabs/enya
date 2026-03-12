//! Query execution management for the Enya editor.
//!
//! Handles executing queries against backends (Prometheus, Enya) and
//! converting responses to visualization-ready data structures.

use rustc_hash::FxHashMap;

#[cfg(not(target_arch = "wasm32"))]
use enya_client::otlp::OtlpMetricsClient;
use enya_client::{
    DemoMetricsClient, HealthCheckManager, LabelsManager, MetricLabels, MetricLabelsManager,
    QueryManager, QueryRequest, QueryResponse, prometheus::PrometheusClient,
};

use crate::AsyncRuntime;
use crate::components::pane::time_series_chart::{DataPoint, Series};
use crate::components::pane::visualization::{
    Bar, ResultCharacteristics, Visualization, VisualizationType, suggest_visualization,
};

/// Backend type for query execution.
#[derive(Debug, Clone, PartialEq)]
pub enum Backend {
    /// Demo mode - uses generated data
    Demo,
    /// Prometheus backend
    Prometheus(String),
    /// In-memory OTLP backend (reads from embedded TelemetryStore)
    Otlp,
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
        /// Suggested visualization type based on result characteristics
        suggested_viz: VisualizationType,
        /// The query response data (boxed to reduce enum size)
        response: Box<QueryResponse>,
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

    /// Returns true if the connection health check failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, ConnectionHealth::Failed { .. })
    }

    /// Returns true if a health check is currently in progress.
    pub fn is_checking(&self) -> bool {
        matches!(self, ConnectionHealth::Checking)
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
    /// OTLP in-memory metrics client — available as a supplementary data source
    /// alongside the primary backend, or as the primary backend itself.
    #[cfg(not(target_arch = "wasm32"))]
    otlp_client: Option<OtlpMetricsClient>,
    /// Reference to the telemetry store for checking which metrics are OTLP-sourced.
    #[cfg(not(target_arch = "wasm32"))]
    otlp_store: Option<std::sync::Arc<enya_client::otlp::TelemetryStore>>,
    /// Query manager for tracking multiple in-flight queries by pane ID
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
    metric_labels_cache: FxHashMap<String, MetricLabels>,
    /// Async runtime for spawning background tasks (native only).
    #[cfg(not(target_arch = "wasm32"))]
    async_runtime: AsyncRuntime,
}

impl QueryExecutor {
    /// Create a new query executor in demo mode with the given async runtime.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(async_runtime: AsyncRuntime) -> Self {
        Self {
            backend: Backend::Demo,
            demo_client: DemoMetricsClient::new(),
            prometheus_client: None,
            otlp_client: None,
            otlp_store: None,
            query_manager: QueryManager::new(),
            labels_manager: LabelsManager::new(),
            label_names_manager: LabelsManager::new(),
            metric_labels_manager: MetricLabelsManager::new(),
            health_check_manager: HealthCheckManager::new(),
            connection_health: ConnectionHealth::Offline,
            metric_names: Vec::new(),
            label_names: Vec::new(),
            metric_labels_cache: FxHashMap::default(),
            async_runtime,
        }
    }

    /// Create a new query executor in demo mode (WASM version).
    #[cfg(target_arch = "wasm32")]
    pub fn new(_async_runtime: AsyncRuntime) -> Self {
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
            metric_labels_cache: FxHashMap::default(),
        }
    }

    /// Connect to a Prometheus backend and initiate a health check.
    ///
    /// The connection is not considered "online" until the health check passes.
    /// Call `poll_health_check()` to check for the result.
    pub fn connect_prometheus(&mut self, endpoint: &str, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        let client = PrometheusClient::with_runtime(endpoint, self.async_runtime.handle().clone());
        #[cfg(target_arch = "wasm32")]
        let client = PrometheusClient::new(endpoint);

        self.prometheus_client = Some(client);
        self.backend = Backend::Prometheus(endpoint.to_string());
        self.connection_health = ConnectionHealth::Checking;

        // Initiate health check
        if let Some(client) = &self.prometheus_client {
            self.health_check_manager.check(client, ctx);
        }
    }

    /// Connect to the in-memory OTLP telemetry store and initiate a health check.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn connect_otlp(
        &mut self,
        store: std::sync::Arc<enya_client::otlp::TelemetryStore>,
        ctx: &egui::Context,
    ) {
        let client = OtlpMetricsClient::new(store);
        self.health_check_manager.check(&client, ctx);
        self.otlp_client = Some(client);
        self.backend = Backend::Otlp;
        self.connection_health = ConnectionHealth::Checking;
    }

    /// Set the OTLP client as a supplementary data source without changing
    /// the primary backend. OTLP metric names will be merged into autocomplete,
    /// and queries for OTLP-only metrics will be routed to this client.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_otlp_client(&mut self, store: std::sync::Arc<enya_client::otlp::TelemetryStore>) {
        self.otlp_store = Some(store.clone());
        if self.otlp_client.is_none() {
            self.otlp_client = Some(OtlpMetricsClient::new(store));
        }
    }

    /// Check whether a metric name exists in the OTLP telemetry store.
    #[cfg(not(target_arch = "wasm32"))]
    fn is_otlp_metric(&self, metric: &str) -> bool {
        self.otlp_store
            .as_ref()
            .map(|store| store.metric_names().iter().any(|n| n == metric))
            .unwrap_or(false)
    }

    /// Disconnect and return to demo mode.
    pub fn disconnect(&mut self) {
        self.prometheus_client = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.otlp_client = None;
        }
        self.backend = Backend::Demo;
        self.connection_health = ConnectionHealth::Offline;
        self.query_manager.cancel_all();
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

    /// Get the tokio runtime handle (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.async_runtime.handle().clone()
    }

    /// Check if the connection is validated and online.
    pub fn is_online(&self) -> bool {
        self.connection_health.is_online()
    }

    /// Check if the connection health check failed.
    pub fn is_connection_failed(&self) -> bool {
        self.connection_health.is_failed()
    }

    /// Check if a health check is currently in progress.
    pub fn is_checking_connection(&self) -> bool {
        self.connection_health.is_checking()
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
                    log::debug!(
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

    /// Check if any queries are currently in flight.
    pub fn is_querying(&self) -> bool {
        self.query_manager.is_querying()
    }

    /// Check if a specific pane has a query in flight.
    pub fn is_querying_pane(&self, pane_id: usize) -> bool {
        self.query_manager.is_querying_id(pane_id)
    }

    /// Get the number of queries currently in flight.
    pub fn pending_query_count(&self) -> usize {
        self.query_manager.pending_count()
    }

    /// Cancel a specific pane's query.
    ///
    /// Note: This doesn't actually cancel the HTTP request (ehttp doesn't support that),
    /// but it will ignore the result when it arrives.
    pub fn cancel_query(&mut self, pane_id: usize) {
        self.query_manager.cancel(pane_id);
    }

    /// Cancel all pending queries.
    pub fn cancel_all_queries(&mut self) {
        self.query_manager.cancel_all();
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
            #[cfg(not(target_arch = "wasm32"))]
            Backend::Otlp => {
                if let Some(client) = &self.otlp_client {
                    self.labels_manager.fetch_metric_names(client, ctx);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Otlp => {}
        }
    }

    /// Check if metric names are currently being fetched.
    pub fn is_fetching_metrics(&self) -> bool {
        self.labels_manager.is_fetching()
    }

    /// Poll for metric names fetch completion.
    ///
    /// Returns `true` if new metric names were received.
    /// When OTLP is available as a supplementary source, its metric names
    /// are merged into the list alongside the primary backend's names.
    pub fn poll_metric_names(&mut self) -> bool {
        if let Some(result) = self.labels_manager.poll() {
            match result {
                #[allow(unused_mut)]
                Ok(mut names) => {
                    log::debug!("Fetched {} metric names from primary backend", names.len());
                    // Merge OTLP metric names when it's a supplementary source
                    #[cfg(not(target_arch = "wasm32"))]
                    if !matches!(self.backend, Backend::Otlp) {
                        if let Some(store) = &self.otlp_store {
                            let otlp_names = store.metric_names();
                            if !otlp_names.is_empty() {
                                log::debug!(
                                    "Merging {} OTLP metric names into autocomplete",
                                    otlp_names.len()
                                );
                                let existing: rustc_hash::FxHashSet<String> =
                                    names.iter().cloned().collect();
                                for name in otlp_names {
                                    if !existing.contains(&name) {
                                        names.push(name);
                                    }
                                }
                                names.sort();
                            }
                        }
                    }
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

    /// Get the cached metric names (includes merged OTLP names when available).
    pub fn metric_names(&self) -> &[String] {
        &self.metric_names
    }

    /// Check whether a supplementary OTLP data source is available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn has_otlp_supplementary(&self) -> bool {
        self.otlp_store.is_some()
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
            #[cfg(not(target_arch = "wasm32"))]
            Backend::Otlp => {
                if let Some(client) = &self.otlp_client {
                    self.label_names_manager.fetch_label_names(client, ctx);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Otlp => {}
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
    /// Routes to OTLP when the metric exists in the supplementary store.
    pub fn fetch_metric_labels(&mut self, metric: &str, ctx: &egui::Context) {
        // Check cache first
        if self.metric_labels_cache.contains_key(metric) {
            return;
        }

        // Route to supplementary OTLP if the metric lives there
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(self.backend, Backend::Prometheus(_)) && self.is_otlp_metric(metric) {
            if let Some(client) = &self.otlp_client {
                self.metric_labels_manager.fetch(client, metric, ctx);
                return;
            }
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
            #[cfg(not(target_arch = "wasm32"))]
            Backend::Otlp => {
                if let Some(client) = &self.otlp_client {
                    self.metric_labels_manager.fetch(client, metric, ctx);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Otlp => {}
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

    /// Execute a query for a specific pane.
    ///
    /// For demo mode, uses the DemoMetricsClient to generate realistic data.
    /// For real backends, this fires off an async request.
    /// Poll with `poll_all()` to receive results.
    ///
    /// Multiple queries can be in flight simultaneously - each is tracked by pane_id.
    pub fn execute_for_pane(
        &mut self,
        pane_id: usize,
        params: &ExecuteParams<'_>,
        ctx: &egui::Context,
    ) {
        // Build request
        let mut request =
            QueryRequest::new(params.metric, params.query).with_step(params.step_secs);
        if let (Some(start), Some(end)) = (params.start_ns, params.end_ns) {
            request = request.with_range(start, end);
        }

        // When the primary backend is Prometheus but the metric exists in the
        // supplementary OTLP store, route the query to OTLP instead.
        #[cfg(not(target_arch = "wasm32"))]
        let use_otlp_supplementary = matches!(self.backend, Backend::Prometheus(_))
            && self.otlp_client.is_some()
            && self.is_otlp_metric(params.query);

        #[cfg(not(target_arch = "wasm32"))]
        if use_otlp_supplementary {
            if let Some(client) = &self.otlp_client {
                log::debug!(
                    "Executing OTLP (supplementary) query for pane {}: metric '{}': {}",
                    pane_id,
                    params.metric,
                    params.query
                );
                self.query_manager.execute(pane_id, client, request, ctx);
                return;
            }
        }

        match &self.backend {
            Backend::Demo => {
                // Demo mode - use demo client for realistic data generation
                log::debug!(
                    "Executing DEMO query for pane {}: metric '{}': {}",
                    pane_id,
                    params.metric,
                    params.query
                );
                self.query_manager
                    .execute(pane_id, &self.demo_client, request, ctx);
            }
            Backend::Prometheus(endpoint) => {
                if let Some(client) = &self.prometheus_client {
                    log::debug!(
                        "Executing Prometheus query for pane {}: metric '{}': {} (endpoint: {})",
                        pane_id,
                        params.metric,
                        params.query,
                        endpoint
                    );
                    self.query_manager.execute(pane_id, client, request, ctx);
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            Backend::Otlp => {
                if let Some(client) = &self.otlp_client {
                    log::debug!(
                        "Executing OTLP query for pane {}: metric '{}': {}",
                        pane_id,
                        params.metric,
                        params.query
                    );
                    self.query_manager.execute(pane_id, client, request, ctx);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Backend::Otlp => {}
        }
    }

    /// Poll for all completed query results.
    ///
    /// Returns a vector of `(pane_id, poll_result)` pairs for queries that completed.
    /// Each result includes a suggested visualization type based on the query characteristics.
    pub fn poll_all(&mut self) -> Vec<(usize, QueryPollResult)> {
        let backend_name = match &self.backend {
            Backend::Demo => "Demo",
            Backend::Prometheus(_) => "Prometheus",
            Backend::Otlp => "OTLP",
        };

        self.query_manager
            .poll_all()
            .into_iter()
            .map(|(pane_id, result)| {
                let poll_result = match result {
                    Ok(response) => {
                        let series_count = response.groups.len();
                        let point_count: usize =
                            response.groups.iter().map(|g| g.buckets.len()).sum();

                        // Compute visualization suggestion based on result characteristics
                        let chars = ResultCharacteristics::from_response(&response);
                        let suggested_viz = suggest_visualization(&chars);
                        log::debug!(
                            "{backend_name} query completed for pane {pane_id}: {series_count} groups, {point_count} total points (suggested: {suggested_viz:?})"
                        );

                        QueryPollResult::Complete {
                            series_count,
                            point_count,
                            suggested_viz,
                            response: Box::new(response),
                        }
                    }
                    Err(e) => {
                        log::error!("Query failed for pane {pane_id}: {e}");
                        QueryPollResult::Error(e.to_string())
                    }
                };
                (pane_id, poll_result)
            })
            .collect()
    }
}

/// Convert a QueryResponse to visualization data.
///
/// For time series: converts groups into Series with data points.
/// For gauge/stat/sparkline/bar: extracts scalar values from the response.
pub fn populate_from_response(visualization: &mut Visualization, response: &QueryResponse) {
    match visualization {
        Visualization::TimeSeries(_) => {
            let series_list = response_to_series(response);
            visualization.set_series(series_list);
        }
        Visualization::Gauge(gauge) => {
            if let Some(value) = extract_latest_value(response) {
                gauge.set_value(value);
                // Auto-set range: use 0 to max(value * 1.5, 100) so the gauge looks reasonable
                let max = if value > 0.0 {
                    (value * 1.5).max(100.0)
                } else {
                    100.0
                };
                gauge.set_range(0.0, max);
                gauge.set_unit("");
            }
        }
        Visualization::Stat(stat) => {
            if let Some(value) = extract_latest_value(response) {
                stat.set_value(value);
                stat.set_unit("");
                // Build sparkline from the most recent series data points
                if let Some(group) = response.groups.first() {
                    let sparkline: Vec<f64> = group.buckets.iter().map(|b| b.value).collect();
                    stat.set_sparkline_data(sparkline);
                }
            }
        }
        Visualization::BarChart(bar) => {
            let bars: Vec<Bar> = response
                .groups
                .iter()
                .map(|group| {
                    let label = if group.group.is_empty() {
                        &response.metric
                    } else {
                        &group.group
                    };
                    let value = group.buckets.last().map(|b| b.value).unwrap_or(0.0);
                    Bar::new(label, value)
                })
                .collect();
            bar.set_bars(bars);
        }
        Visualization::Sparkline(spark) => {
            if let Some(group) = response.groups.first() {
                let data: Vec<f64> = group.buckets.iter().map(|b| b.value).collect();
                spark.set_data(data);
            }
        }
        Visualization::Heatmap(_) => {
            // Heatmap needs histogram data which isn't available from standard queries
            let series_list = response_to_series(response);
            visualization.set_series(series_list);
        }
    }
}

/// Extract the latest non-zero value from a query response.
///
/// Looks at the last bucket of the first group.
fn extract_latest_value(response: &QueryResponse) -> Option<f64> {
    response.groups.first().and_then(|group| {
        group
            .buckets
            .iter()
            .rev()
            .find(|b| b.value != 0.0)
            .or(group.buckets.last())
            .map(|b| b.value)
    })
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
        // Create a runtime for the test
        let rt = tokio::runtime::Runtime::new().unwrap();
        let async_runtime = crate::AsyncRuntime::new(rt.handle().clone());

        let executor = QueryExecutor::new(async_runtime);
        assert!(!executor.is_connected());
        assert_eq!(executor.backend(), &Backend::Demo);
    }

    #[test]
    fn test_query_executor_connect_prometheus() {
        // Create a runtime for the test
        let rt = tokio::runtime::Runtime::new().unwrap();
        let async_runtime = crate::AsyncRuntime::new(rt.handle().clone());

        let mut executor = QueryExecutor::new(async_runtime.clone());
        // Manually set up connection state for test (no egui context available)
        executor.prometheus_client = Some(enya_client::prometheus::PrometheusClient::with_runtime(
            "http://localhost:9090",
            async_runtime.handle().clone(),
        ));
        executor.backend = Backend::Prometheus("http://localhost:9090".to_string());
        assert!(executor.is_connected());
        assert!(matches!(executor.backend(), Backend::Prometheus(_)));
    }
}
