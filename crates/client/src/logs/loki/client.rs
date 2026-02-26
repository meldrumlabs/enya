//! Loki HTTP client implementation.

use poll_promise::Promise;

use crate::error::ClientError;
use crate::logs::{LogsClient, LogsQuery, LogsResult, QueryDirection, StreamsResult};
use crate::normalize_url;
use crate::promise::promise_channel;
use crate::url_encode;
use crate::{BackendInfo, HealthCheckResult};

use super::response::{parse_buildinfo_response, parse_labels_response, parse_logs_response};

/// Client for querying Loki via its HTTP API.
///
/// Executes LogQL queries against the `/loki/api/v1/query_range` endpoint.
/// Uses `reqwest` for HTTP requests on both native (with tokio) and WASM
/// (with wasm-bindgen-futures).
///
/// # Example
///
/// ```ignore
/// use enya_client::logs::{LokiClient, LogsClient, LogsQuery};
///
/// let client = LokiClient::new("http://localhost:3100");
/// let query = LogsQuery::new(start_ns, end_ns)
///     .with_label("app", "myservice");
/// let promise = client.query_logs(query, &ctx);
/// ```
pub struct LokiClient {
    base_url: String,
    http_client: reqwest::Client,
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: tokio::runtime::Handle,
}

impl LokiClient {
    /// Create a new Loki client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Loki server (e.g., "http://localhost:3100")
    ///
    /// If no protocol is specified, `http://` is assumed.
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

    /// Create a new Loki client with an explicit runtime handle.
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

    /// Build the query_range URL for a logs query.
    fn build_query_url(&self, query: &LogsQuery) -> String {
        // Convert nanoseconds to seconds for the API
        let start_secs = query.start_ns / 1_000_000_000;
        let end_secs = query.end_ns / 1_000_000_000;

        // Build LogQL query
        let logql = self.build_logql(query);
        let encoded_query = url_encode(&logql);

        let direction = match query.direction {
            QueryDirection::Forward => "forward",
            QueryDirection::Backward => "backward",
        };

        format!(
            "{}/loki/api/v1/query_range?query={}&start={}&end={}&limit={}&direction={}",
            self.base_url, encoded_query, start_secs, end_secs, query.limit, direction
        )
    }

    /// Build a LogQL query string from query parameters.
    fn build_logql(&self, query: &LogsQuery) -> String {
        // If a raw query is provided, use it directly
        if let Some(ref raw) = query.query {
            return raw.clone();
        }

        // Build stream selector from labels
        let mut selector_parts: Vec<String> = query
            .labels
            .iter()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect();

        // If no labels specified, match all streams
        if selector_parts.is_empty() {
            selector_parts.push("__name__=~\".+\"".to_string());
        }

        let selector = format!("{{{}}}", selector_parts.join(", "));

        // Add line filter if contains is specified
        if let Some(ref text) = query.contains {
            format!("{selector} |= \"{text}\"")
        } else {
            selector
        }
    }
}

impl LogsClient for LokiClient {
    fn query_logs(&self, query: LogsQuery, ctx: &egui::Context) -> Promise<LogsResult> {
        let url = self.build_query_url(&query);

        log::debug!("Loki query: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_logs_response(&bytes),
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

    fn fetch_streams(&self, ctx: &egui::Context) -> Promise<StreamsResult> {
        let url = format!("{}/loki/api/v1/labels", self.base_url);

        log::debug!("Loki fetch labels: {url}");

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

    fn backend_type(&self) -> &'static str {
        "loki"
    }

    fn health_check(&self, ctx: &egui::Context) -> Promise<HealthCheckResult> {
        let url = format!("{}/loki/api/v1/status/buildinfo", self.base_url);

        log::debug!("Loki health check: {url}");

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
                                backend_type: "loki".to_string(),
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
    use rustc_hash::FxHashMap;

    /// Helper to create a tokio runtime for tests
    fn with_runtime<F: FnOnce()>(f: F) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        f();
    }

    #[test]
    fn test_new_removes_trailing_slash() {
        with_runtime(|| {
            let client = LokiClient::new("http://localhost:3100/");
            assert_eq!(client.base_url, "http://localhost:3100");
        });
    }

    #[test]
    fn test_new_adds_http_protocol() {
        with_runtime(|| {
            let client = LokiClient::new("localhost:3100");
            assert_eq!(client.base_url, "http://localhost:3100");
        });
    }

    #[test]
    fn test_new_preserves_https() {
        with_runtime(|| {
            let client = LokiClient::new("https://loki.example.com");
            assert_eq!(client.base_url, "https://loki.example.com");
        });
    }

    #[test]
    fn test_build_logql_with_labels() {
        with_runtime(|| {
            let client = LokiClient::new("http://localhost:3100");
            let mut labels = FxHashMap::default();
            labels.insert("app".to_string(), "myservice".to_string());
            labels.insert("env".to_string(), "prod".to_string());

            let query = LogsQuery {
                query: None,
                labels,
                contains: None,
                start_ns: 1000000000,
                end_ns: 2000000000,
                limit: 100,
                direction: QueryDirection::Backward,
            };

            let logql = client.build_logql(&query);
            // Labels may be in any order due to HashMap
            assert!(logql.contains("app=\"myservice\""));
            assert!(logql.contains("env=\"prod\""));
            assert!(logql.starts_with('{'));
            assert!(logql.ends_with('}'));
        });
    }

    #[test]
    fn test_build_logql_with_contains() {
        with_runtime(|| {
            let client = LokiClient::new("http://localhost:3100");
            let mut labels = FxHashMap::default();
            labels.insert("app".to_string(), "myservice".to_string());

            let query = LogsQuery {
                query: None,
                labels,
                contains: Some("SELECT".to_string()),
                start_ns: 1000000000,
                end_ns: 2000000000,
                limit: 100,
                direction: QueryDirection::Backward,
            };

            let logql = client.build_logql(&query);
            assert!(logql.contains("|= \"SELECT\""));
        });
    }

    #[test]
    fn test_build_logql_raw_query() {
        with_runtime(|| {
            let client = LokiClient::new("http://localhost:3100");

            let query = LogsQuery {
                query: Some("{app=\"test\"} |~ \"error|warn\"".to_string()),
                labels: FxHashMap::default(),
                contains: None,
                start_ns: 1000000000,
                end_ns: 2000000000,
                limit: 100,
                direction: QueryDirection::Backward,
            };

            let logql = client.build_logql(&query);
            assert_eq!(logql, "{app=\"test\"} |~ \"error|warn\"");
        });
    }

    #[test]
    fn test_build_query_url() {
        with_runtime(|| {
            let client = LokiClient::new("http://localhost:3100");

            let query = LogsQuery::new(1000000000000, 2000000000000)
                .with_label("app", "myservice")
                .with_limit(500)
                .with_direction(QueryDirection::Forward);

            let url = client.build_query_url(&query);
            assert!(url.starts_with("http://localhost:3100/loki/api/v1/query_range?"));
            assert!(url.contains("start=1000"));
            assert!(url.contains("end=2000"));
            assert!(url.contains("limit=500"));
            assert!(url.contains("direction=forward"));
        });
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("simple"), "simple");
        assert_eq!(url_encode("{app=\"test\"}"), "%7Bapp%3D%22test%22%7D");
        assert_eq!(url_encode("a|b"), "a%7Cb");
    }

    #[test]
    fn test_backend_type() {
        with_runtime(|| {
            let client = LokiClient::new("http://localhost:3100");
            assert_eq!(client.backend_type(), "loki");
        });
    }
}
