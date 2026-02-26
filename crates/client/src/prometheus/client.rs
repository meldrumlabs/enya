//! Prometheus HTTP client implementation.

use crate::error::ClientError;
use crate::now_unix_secs;
use crate::normalize_url;
use crate::promise::promise_channel;
use crate::request::QueryRequest;
use crate::url_encode;
use crate::{
    BackendInfo, HealthCheckResult, LabelsResult, MetricLabelsResult, MetricsClient, QueryResult,
};
use poll_promise::Promise;

use super::response::{
    parse_buildinfo_response, parse_labels_response, parse_response, parse_series_response,
};

/// Client for querying Prometheus via its HTTP API.
///
/// Executes PromQL queries directly against the `/api/v1/query_range` endpoint.
/// Uses `reqwest` for HTTP requests on both native (with tokio) and WASM (with
/// wasm-bindgen-futures).
///
/// # Example
///
/// ```ignore
/// use enya_client::{QueryManager, QueryRequest};
/// use enya_client::prometheus::PrometheusClient;
///
/// let client = PrometheusClient::new("http://localhost:9090");
/// let mut manager = QueryManager::new();
///
/// let request = QueryRequest::new("cpu_usage", "sum(env:prod) by (host)");
/// manager.execute(&client, request, &ctx);
/// ```
pub struct PrometheusClient {
    base_url: String,
    http_client: reqwest::Client,
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: tokio::runtime::Handle,
}

impl PrometheusClient {
    /// Create a new Prometheus client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Prometheus server (e.g., "http://localhost:9090")
    ///
    /// If no protocol is specified, `http://` is assumed (Prometheus default).
    ///
    /// # Panics
    ///
    /// On native, panics if called outside a tokio runtime context.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: normalize_url(base_url),
            http_client: reqwest::Client::new(),
            #[cfg(not(target_arch = "wasm32"))]
            runtime_handle: tokio::runtime::Handle::current(),
        }
    }

    /// Create a new Prometheus client with an explicit runtime handle.
    ///
    /// Use this when creating the client outside a tokio runtime context.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_runtime(base_url: impl Into<String>, handle: tokio::runtime::Handle) -> Self {
        Self {
            base_url: normalize_url(base_url),
            http_client: reqwest::Client::new(),
            runtime_handle: handle,
        }
    }

    /// Spawn an async task on the runtime.
    #[cfg(not(target_arch = "wasm32"))]
    fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime_handle.spawn(future);
    }

    /// Spawn an async task using wasm-bindgen-futures.
    #[cfg(target_arch = "wasm32")]
    fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        wasm_bindgen_futures::spawn_local(future);
    }

    /// Build the query_range URL for a request.
    fn build_url(&self, promql: &str, request: &QueryRequest) -> String {
        let now_secs = now_unix_secs();

        // Default time range: 1 hour ago to now
        let end_secs = request
            .end
            .map(|ns| (ns / 1_000_000_000) as u64)
            .unwrap_or(now_secs);
        let start_secs = request
            .start
            .map(|ns| (ns / 1_000_000_000) as u64)
            .unwrap_or(end_secs.saturating_sub(3600));

        let step = request.step_secs;

        // URL-encode the query
        let encoded_query = url_encode(promql);

        format!(
            "{}/api/v1/query_range?query={}&start={}&end={}&step={}",
            self.base_url, encoded_query, start_secs, end_secs, step
        )
    }
}

impl MetricsClient for PrometheusClient {
    fn query(&self, request: QueryRequest, ctx: &egui::Context) -> Promise<QueryResult> {
        // Use query directly as PromQL (no translation)
        // If query is empty or "*", use the metric name as the query
        let promql = if request.query.is_empty() || request.query == "*" {
            request.metric.clone()
        } else {
            request.query.clone()
        };

        let url = self.build_url(&promql, &request);
        let metric = request.metric.clone();
        let query = request.query.clone();
        let granularity_ns = (request.step_secs as u128) * 1_000_000_000;

        log::debug!("Prometheus query: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_response(&bytes, &metric, &query, granularity_ns),
                            Err(e) => Err(ClientError::NetworkError(e.to_string())),
                        }
                    } else {
                        Err(ClientError::BackendError {
                            status: status.as_u16(),
                            message: status.canonical_reason().unwrap_or("Unknown").to_string(),
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e.to_string())),
            };
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_label_names(&self, ctx: &egui::Context) -> Promise<LabelsResult> {
        let url = format!("{}/api/v1/labels", self.base_url);

        log::debug!("Prometheus fetch labels: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_labels_response(&bytes),
                            Err(e) => Err(ClientError::NetworkError(e.to_string())),
                        }
                    } else {
                        Err(ClientError::BackendError {
                            status: status.as_u16(),
                            message: status.canonical_reason().unwrap_or("Unknown").to_string(),
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e.to_string())),
            };
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_label_values(&self, label: &str, ctx: &egui::Context) -> Promise<LabelsResult> {
        let url = format!("{}/api/v1/label/{}/values", self.base_url, label);

        log::debug!("Prometheus fetch label values for '{label}': {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_labels_response(&bytes),
                            Err(e) => Err(ClientError::NetworkError(e.to_string())),
                        }
                    } else {
                        Err(ClientError::BackendError {
                            status: status.as_u16(),
                            message: status.canonical_reason().unwrap_or("Unknown").to_string(),
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e.to_string())),
            };
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_metric_names(&self, ctx: &egui::Context) -> Promise<LabelsResult> {
        // In Prometheus, metric names are stored in the special __name__ label
        self.fetch_label_values("__name__", ctx)
    }

    fn fetch_metric_labels(
        &self,
        metric: &str,
        ctx: &egui::Context,
    ) -> Promise<MetricLabelsResult> {
        // Build the series query URL
        // match[]={__name__="metric_name"}
        let selector = format!(r#"{{__name__="{metric}"}}"#);
        let encoded_selector = url_encode(&selector);
        let url = format!(
            "{}/api/v1/series?match[]={}",
            self.base_url, encoded_selector
        );

        log::debug!("Prometheus fetch metric labels for '{metric}': {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_series_response(&bytes),
                            Err(e) => Err(ClientError::NetworkError(e.to_string())),
                        }
                    } else {
                        Err(ClientError::BackendError {
                            status: status.as_u16(),
                            message: status.canonical_reason().unwrap_or("Unknown").to_string(),
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e.to_string())),
            };
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn backend_type(&self) -> &'static str {
        "prometheus"
    }

    fn health_check(&self, ctx: &egui::Context) -> Promise<HealthCheckResult> {
        let url = format!("{}/api/v1/status/buildinfo", self.base_url);

        log::debug!("Prometheus health check: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_buildinfo_response(&bytes).map(|info| BackendInfo {
                                backend_type: "prometheus".to_string(),
                                version: info.version,
                            }),
                            Err(e) => Err(ClientError::NetworkError(e.to_string())),
                        }
                    } else {
                        Err(ClientError::BackendError {
                            status: status.as_u16(),
                            message: status.canonical_reason().unwrap_or("Unknown").to_string(),
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e.to_string())),
            };
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a tokio runtime for tests
    fn with_runtime<F: FnOnce()>(f: F) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        f();
    }

    #[test]
    fn test_new_removes_trailing_slash() {
        with_runtime(|| {
            let client = PrometheusClient::new("http://localhost:9090/");
            assert_eq!(client.base_url, "http://localhost:9090");
        });
    }

    #[test]
    fn test_new_adds_http_protocol() {
        with_runtime(|| {
            let client = PrometheusClient::new("localhost:9090");
            assert_eq!(client.base_url, "http://localhost:9090");
        });
    }

    #[test]
    fn test_new_preserves_https() {
        with_runtime(|| {
            let client = PrometheusClient::new("https://prometheus.example.com");
            assert_eq!(client.base_url, "https://prometheus.example.com");
        });
    }

    #[test]
    fn test_build_url() {
        with_runtime(|| {
            let client = PrometheusClient::new("http://localhost:9090");
            let request = QueryRequest::new("cpu", "*").with_step(60);

            let url = client.build_url("cpu", &request);
            assert!(url.starts_with("http://localhost:9090/api/v1/query_range?"));
            assert!(url.contains("query=cpu"));
            assert!(url.contains("step=60"));
        });
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("simple"), "simple");
        assert_eq!(
            url_encode(r#"cpu{env="prod"}"#),
            "cpu%7Benv%3D%22prod%22%7D"
        );
        assert_eq!(url_encode("rate(m[5m])"), "rate(m%5B5m%5D)");
    }

    #[test]
    fn test_backend_type() {
        with_runtime(|| {
            let client = PrometheusClient::new("http://localhost:9090");
            assert_eq!(client.backend_type(), "prometheus");
        });
    }
}
