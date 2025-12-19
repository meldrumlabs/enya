//! Demo metrics client for offline/showcase mode.
//!
//! Provides a `DemoMetricsClient` that implements `MetricsClient` with realistic
//! mock data, enabling the editor to work without a real Prometheus connection.

use std::collections::HashMap;

use poll_promise::Promise;

use crate::error::ClientError;
use crate::now_unix_secs;
use crate::prometheus::response::MetricLabels;
use crate::request::QueryRequest;
use crate::{
    BackendInfo, HealthCheckResult, LabelsResult, MetricLabelsResult, MetricsClient, QueryResponse,
    QueryResult,
};
use enya_common::{MetricsBucket, MetricsGroup};

/// A demo metric definition with its labels.
#[derive(Debug, Clone)]
struct DemoMetric {
    /// Metric name (e.g., "http_requests_total")
    name: String,
    /// Category for grouping in UI (reserved for future use)
    #[allow(dead_code)]
    category: MetricCategory,
    /// Label names this metric has
    labels: Vec<String>,
    /// Possible values for each label
    label_values: HashMap<String, Vec<String>>,
    /// Type of metric (affects data generation pattern)
    metric_type: MetricType,
}

/// Category for organizing metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricCategory {
    System,
    Http,
    Runtime,
    Application,
    Database,
}

/// Type of metric (affects data generation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetricType {
    /// Monotonically increasing counter
    Counter,
    /// Fluctuating gauge value
    Gauge,
    /// Histogram quantile
    Histogram,
}

impl DemoMetric {
    fn new(name: &str, category: MetricCategory, metric_type: MetricType) -> Self {
        Self {
            name: name.to_string(),
            category,
            labels: Vec::new(),
            label_values: HashMap::new(),
            metric_type,
        }
    }

    fn with_label(mut self, name: &str, values: &[&str]) -> Self {
        self.labels.push(name.to_string());
        self.label_values.insert(
            name.to_string(),
            values.iter().map(|s| (*s).to_string()).collect(),
        );
        self
    }
}

/// Demo metrics client providing realistic mock data.
///
/// This client implements the `MetricsClient` trait with a predefined catalog
/// of realistic Prometheus metrics. It generates time-series data with
/// appropriate patterns for different metric types.
///
/// # Example
///
/// ```ignore
/// use enya_client::demo::DemoMetricsClient;
/// use enya_client::MetricsClient;
///
/// let client = DemoMetricsClient::new();
/// let names = client.fetch_metric_names(&ctx);
/// ```
pub struct DemoMetricsClient {
    /// Catalog of demo metrics
    metrics: Vec<DemoMetric>,
}

impl Default for DemoMetricsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoMetricsClient {
    /// Create a new demo client with the standard metrics catalog.
    #[must_use]
    pub fn new() -> Self {
        Self {
            metrics: build_metrics_catalog(),
        }
    }

    /// Get all metric names.
    fn metric_names(&self) -> Vec<String> {
        self.metrics.iter().map(|m| m.name.clone()).collect()
    }

    /// Get all unique label names across all metrics.
    fn all_label_names(&self) -> Vec<String> {
        let mut labels: Vec<String> = self
            .metrics
            .iter()
            .flat_map(|m| m.labels.iter().cloned())
            .collect();
        labels.sort();
        labels.dedup();
        labels
    }

    /// Get labels for a specific metric.
    fn get_metric_labels(&self, metric_name: &str) -> Option<MetricLabels> {
        self.metrics
            .iter()
            .find(|m| m.name == metric_name)
            .map(|m| MetricLabels {
                labels: m.label_values.clone(),
            })
    }

    /// Get a metric by name.
    fn get_metric(&self, name: &str) -> Option<&DemoMetric> {
        self.metrics.iter().find(|m| m.name == name)
    }

    /// Generate demo time-series data for a query.
    fn generate_data(&self, request: &QueryRequest) -> QueryResponse {
        let now_secs = now_unix_secs();
        let end_secs = request
            .end
            .map(|ns| (ns / 1_000_000_000) as u64)
            .unwrap_or(now_secs);
        let start_secs = request
            .start
            .map(|ns| (ns / 1_000_000_000) as u64)
            .unwrap_or(end_secs.saturating_sub(3600));

        let step = request.step_secs.max(1);
        let num_points = ((end_secs - start_secs) / step).min(1000) as usize;

        // Get metric info for pattern generation
        let metric = self.get_metric(&request.metric);
        let metric_type = metric.map(|m| m.metric_type).unwrap_or(MetricType::Gauge);

        // Generate label combinations for series
        let series_labels = self.generate_series_labels(&request.metric);

        let groups: Vec<MetricsGroup> = series_labels
            .iter()
            .enumerate()
            .map(|(idx, labels)| {
                let buckets = generate_buckets(
                    start_secs,
                    step,
                    num_points,
                    metric_type,
                    idx,
                    &request.query,
                );

                let group_str = labels
                    .iter()
                    .map(|(k, v)| format!("{k}=\"{v}\""))
                    .collect::<Vec<_>>()
                    .join(", ");

                MetricsGroup {
                    group: format!("{{{group_str}}}"),
                    buckets,
                }
            })
            .collect();

        let start_ns = (start_secs as u128) * 1_000_000_000;
        let end_ns = (end_secs as u128) * 1_000_000_000;
        let granularity_ns = (step as u128) * 1_000_000_000;

        QueryResponse {
            metric: request.metric.clone(),
            query: request.query.clone(),
            parsed_agg: None,
            parsed_filter: String::new(),
            parsed_grouping: None,
            parsed_time_range: None,
            start: Some(start_ns),
            end: Some(end_ns),
            granularity_ns,
            groups,
        }
    }

    /// Generate label combinations for series.
    fn generate_series_labels(&self, metric_name: &str) -> Vec<HashMap<String, String>> {
        let Some(metric) = self.get_metric(metric_name) else {
            // Unknown metric - return single series with generic labels
            return vec![HashMap::from([
                ("env".to_string(), "prod".to_string()),
                ("host".to_string(), "server-1".to_string()),
            ])];
        };

        // Generate a few combinations based on first 2 labels
        let mut combinations = Vec::new();

        if metric.labels.is_empty() {
            combinations.push(HashMap::new());
        } else if metric.labels.len() == 1 {
            let label = &metric.labels[0];
            if let Some(values) = metric.label_values.get(label) {
                for value in values.iter().take(4) {
                    combinations.push(HashMap::from([(label.clone(), value.clone())]));
                }
            }
        } else {
            // Take first 2 labels and create combinations
            let label1 = &metric.labels[0];
            let label2 = &metric.labels[1];
            let values1 = metric.label_values.get(label1).cloned().unwrap_or_default();
            let values2 = metric.label_values.get(label2).cloned().unwrap_or_default();

            for v1 in values1.iter().take(2) {
                for v2 in values2.iter().take(2) {
                    combinations.push(HashMap::from([
                        (label1.clone(), v1.clone()),
                        (label2.clone(), v2.clone()),
                    ]));
                }
            }
        }

        if combinations.is_empty() {
            combinations.push(HashMap::new());
        }

        combinations
    }
}

/// Generate time-series buckets with appropriate patterns.
fn generate_buckets(
    start_secs: u64,
    step: u64,
    num_points: usize,
    metric_type: MetricType,
    series_idx: usize,
    query: &str,
) -> Vec<MetricsBucket> {
    // Use query hash for variety
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(u64::from(b)));
    let series_offset = series_idx as f64 * 17.3;

    (0..num_points)
        .map(|i| {
            let t = start_secs + (i as u64) * step;
            let t_f = t as f64;

            let value = match metric_type {
                MetricType::Counter => {
                    // Monotonically increasing with rate variations
                    let base_rate = 100.0 + (hash % 200) as f64;
                    let variation = (t_f / 300.0 + series_offset).sin() * 20.0;
                    (i as f64) * (base_rate + variation) / 60.0
                }
                MetricType::Gauge => {
                    // Fluctuating value with occasional spikes
                    let base = 50.0 + (hash % 50) as f64 + series_offset.abs() % 30.0;
                    let slow_wave = (t_f / 600.0 + series_offset).sin() * 15.0;
                    let fast_wave = (t_f / 60.0 + series_offset * 2.0).sin() * 5.0;
                    let spike = if (t_f / 1800.0 + series_offset).sin() > 0.95 {
                        30.0
                    } else {
                        0.0
                    };
                    (base + slow_wave + fast_wave + spike).max(0.0)
                }
                MetricType::Histogram => {
                    // Latency-like distribution (usually low, occasional highs)
                    let base = 0.05 + (hash % 10) as f64 * 0.01;
                    let jitter = (t_f * 7.0 + series_offset).sin().abs() * 0.02;
                    let spike = if (t_f / 900.0 + series_offset).sin() > 0.9 {
                        0.5
                    } else {
                        0.0
                    };
                    base + jitter + spike
                }
            };

            let start_ns = (t as u128) * 1_000_000_000;
            let end_ns = start_ns + (step as u128) * 1_000_000_000;
            MetricsBucket {
                start: start_ns,
                end: end_ns,
                value,
                count: 1,
            }
        })
        .collect()
}

impl MetricsClient for DemoMetricsClient {
    fn query(&self, request: QueryRequest, _ctx: &egui::Context) -> Promise<QueryResult> {
        let response = self.generate_data(&request);
        Promise::from_ready(Ok(response))
    }

    fn fetch_label_names(&self, _ctx: &egui::Context) -> Promise<LabelsResult> {
        Promise::from_ready(Ok(self.all_label_names()))
    }

    fn fetch_label_values(&self, label: &str, _ctx: &egui::Context) -> Promise<LabelsResult> {
        // Collect all values for this label across all metrics
        let values: Vec<String> = self
            .metrics
            .iter()
            .filter_map(|m| m.label_values.get(label))
            .flatten()
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Promise::from_ready(Ok(values))
    }

    fn fetch_metric_names(&self, _ctx: &egui::Context) -> Promise<LabelsResult> {
        Promise::from_ready(Ok(self.metric_names()))
    }

    fn fetch_metric_labels(
        &self,
        metric: &str,
        _ctx: &egui::Context,
    ) -> Promise<MetricLabelsResult> {
        match self.get_metric_labels(metric) {
            Some(labels) => Promise::from_ready(Ok(labels)),
            None => Promise::from_ready(Err(ClientError::BackendError {
                status: 404,
                message: format!("metric '{metric}' not found in demo catalog"),
            })),
        }
    }

    fn backend_type(&self) -> &'static str {
        "demo"
    }

    fn health_check(&self, _ctx: &egui::Context) -> Promise<HealthCheckResult> {
        // Demo mode is always "healthy"
        Promise::from_ready(Ok(BackendInfo {
            backend_type: "demo".to_string(),
            version: "offline".to_string(),
        }))
    }
}

/// Build the standard demo metrics catalog.
fn build_metrics_catalog() -> Vec<DemoMetric> {
    vec![
        // System metrics
        DemoMetric::new(
            "node_cpu_seconds_total",
            MetricCategory::System,
            MetricType::Counter,
        )
        .with_label("cpu", &["0", "1", "2", "3"])
        .with_label("mode", &["user", "system", "idle", "iowait"]),
        DemoMetric::new(
            "node_memory_bytes",
            MetricCategory::System,
            MetricType::Gauge,
        )
        .with_label("type", &["used", "free", "cached", "buffers"]),
        DemoMetric::new(
            "node_disk_read_bytes_total",
            MetricCategory::System,
            MetricType::Counter,
        )
        .with_label("device", &["sda", "sdb", "nvme0n1"]),
        DemoMetric::new(
            "node_disk_write_bytes_total",
            MetricCategory::System,
            MetricType::Counter,
        )
        .with_label("device", &["sda", "sdb", "nvme0n1"]),
        DemoMetric::new(
            "node_network_receive_bytes_total",
            MetricCategory::System,
            MetricType::Counter,
        )
        .with_label("device", &["eth0", "eth1", "lo"]),
        DemoMetric::new(
            "node_network_transmit_bytes_total",
            MetricCategory::System,
            MetricType::Counter,
        )
        .with_label("device", &["eth0", "eth1", "lo"]),
        DemoMetric::new("node_load1", MetricCategory::System, MetricType::Gauge),
        DemoMetric::new("node_load5", MetricCategory::System, MetricType::Gauge),
        DemoMetric::new("node_load15", MetricCategory::System, MetricType::Gauge),
        // HTTP metrics
        DemoMetric::new(
            "http_requests_total",
            MetricCategory::Http,
            MetricType::Counter,
        )
        .with_label("method", &["GET", "POST", "PUT", "DELETE"])
        .with_label(
            "path",
            &["/api/users", "/api/orders", "/api/products", "/health"],
        )
        .with_label("status_code", &["200", "201", "400", "404", "500"]),
        DemoMetric::new(
            "http_request_duration_seconds",
            MetricCategory::Http,
            MetricType::Histogram,
        )
        .with_label("method", &["GET", "POST", "PUT", "DELETE"])
        .with_label("path", &["/api/users", "/api/orders", "/api/products"])
        .with_label("quantile", &["0.5", "0.9", "0.99"]),
        DemoMetric::new(
            "http_requests_in_flight",
            MetricCategory::Http,
            MetricType::Gauge,
        )
        .with_label("service", &["api", "web", "worker"]),
        DemoMetric::new(
            "http_response_size_bytes",
            MetricCategory::Http,
            MetricType::Histogram,
        )
        .with_label("method", &["GET", "POST"])
        .with_label("quantile", &["0.5", "0.9", "0.99"]),
        // Tokio runtime metrics
        DemoMetric::new(
            "tokio_runtime_workers_count",
            MetricCategory::Runtime,
            MetricType::Gauge,
        )
        .with_label("runtime", &["main", "blocking"]),
        DemoMetric::new(
            "tokio_runtime_blocking_threads",
            MetricCategory::Runtime,
            MetricType::Gauge,
        )
        .with_label("runtime", &["main"]),
        DemoMetric::new(
            "tokio_tasks_spawned_total",
            MetricCategory::Runtime,
            MetricType::Counter,
        )
        .with_label("runtime", &["main", "blocking"]),
        DemoMetric::new(
            "tokio_task_poll_duration_seconds",
            MetricCategory::Runtime,
            MetricType::Histogram,
        )
        .with_label("quantile", &["0.5", "0.9", "0.99"]),
        // Application metrics
        DemoMetric::new(
            "app_cache_hits_total",
            MetricCategory::Application,
            MetricType::Counter,
        )
        .with_label("cache", &["users", "sessions", "products"]),
        DemoMetric::new(
            "app_cache_misses_total",
            MetricCategory::Application,
            MetricType::Counter,
        )
        .with_label("cache", &["users", "sessions", "products"]),
        DemoMetric::new(
            "app_queue_depth",
            MetricCategory::Application,
            MetricType::Gauge,
        )
        .with_label("queue", &["orders", "notifications", "emails"]),
        DemoMetric::new(
            "app_active_users",
            MetricCategory::Application,
            MetricType::Gauge,
        )
        .with_label("env", &["prod", "staging"]),
        // Database metrics
        DemoMetric::new(
            "db_connections_active",
            MetricCategory::Database,
            MetricType::Gauge,
        )
        .with_label("pool", &["primary", "replica"])
        .with_label("database", &["users", "orders"]),
        DemoMetric::new(
            "db_connections_idle",
            MetricCategory::Database,
            MetricType::Gauge,
        )
        .with_label("pool", &["primary", "replica"]),
        DemoMetric::new(
            "db_query_duration_seconds",
            MetricCategory::Database,
            MetricType::Histogram,
        )
        .with_label("query_type", &["select", "insert", "update", "delete"])
        .with_label("quantile", &["0.5", "0.9", "0.99"]),
        DemoMetric::new(
            "db_transactions_total",
            MetricCategory::Database,
            MetricType::Counter,
        )
        .with_label("status", &["commit", "rollback"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_client_metric_names() {
        let client = DemoMetricsClient::new();
        let names = client.metric_names();
        assert!(!names.is_empty());
        assert!(names.contains(&"http_requests_total".to_string()));
        assert!(names.contains(&"node_cpu_seconds_total".to_string()));
    }

    #[test]
    fn test_demo_client_label_names() {
        let client = DemoMetricsClient::new();
        let labels = client.all_label_names();
        assert!(labels.contains(&"method".to_string()));
        assert!(labels.contains(&"status_code".to_string()));
        assert!(labels.contains(&"cpu".to_string()));
    }

    #[test]
    fn test_demo_client_metric_labels() {
        let client = DemoMetricsClient::new();
        let labels = client.get_metric_labels("http_requests_total");
        assert!(labels.is_some());
        let labels = labels.unwrap();
        assert!(labels.labels.contains_key("method"));
        assert!(labels.labels.contains_key("status_code"));
    }

    #[test]
    fn test_demo_client_backend_type() {
        let client = DemoMetricsClient::new();
        assert_eq!(client.backend_type(), "demo");
    }

    #[test]
    fn test_generate_buckets_counter() {
        let buckets = generate_buckets(1000, 60, 10, MetricType::Counter, 0, "test");
        assert_eq!(buckets.len(), 10);
        // Counter should be monotonically increasing
        for window in buckets.windows(2) {
            assert!(window[1].value >= window[0].value);
        }
    }

    #[test]
    fn test_generate_buckets_gauge() {
        let buckets = generate_buckets(1000, 60, 10, MetricType::Gauge, 0, "test");
        assert_eq!(buckets.len(), 10);
        // Gauge values should all be non-negative
        for bucket in &buckets {
            assert!(bucket.value >= 0.0);
        }
    }
}
