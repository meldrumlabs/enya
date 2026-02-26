//! HTTP-based OTLP logs client.
//!
//! Queries the agent daemon's OTLP store over HTTP, following the same
//! promise-based async pattern as [`LokiClient`](crate::logs::LokiClient).

use poll_promise::Promise;

use crate::error::ClientError;
use crate::logs::{LogsClient, LogsQuery, LogsResponse, LogsResult, StreamsResult};
use crate::normalize_url;
use crate::promise::promise_channel;
use crate::url_encode;
use crate::{BackendInfo, HealthCheckResult};

/// HTTP client for querying the agent's OTLP log store.
///
/// Sends GET requests to `/api/otlp/logs/query`, `/api/otlp/labels`,
/// and `/api/otlp/health` endpoints on the agent daemon.
pub struct OtlpHttpLogsClient {
    base_url: String,
    http_client: reqwest::Client,
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: tokio::runtime::Handle,
}

impl OtlpHttpLogsClient {
    /// Create a new OTLP HTTP logs client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The agent daemon URL (e.g., "http://localhost:3030")
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

    /// Create a new client with an explicit runtime handle.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_runtime(base_url: impl Into<String>, handle: tokio::runtime::Handle) -> Self {
        Self {
            base_url: normalize_url(base_url),
            http_client: reqwest::Client::new(),
            runtime_handle: handle,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime_handle.spawn(future);
    }

    #[cfg(target_arch = "wasm32")]
    fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + 'static,
    {
        wasm_bindgen_futures::spawn_local(future);
    }

    fn build_query_url(&self, query: &LogsQuery) -> String {
        let mut url = format!(
            "{}/api/otlp/logs/query?start_ns={}&end_ns={}&limit={}",
            self.base_url, query.start_ns, query.end_ns, query.limit
        );

        if let Some(ref text) = query.contains {
            url.push_str("&contains=");
            url.push_str(&url_encode(text));
        }

        if !query.labels.is_empty() {
            if let Ok(labels_json) = serde_json::to_string(&query.labels) {
                url.push_str("&labels=");
                url.push_str(&url_encode(&labels_json));
            }
        }

        url
    }
}

impl LogsClient for OtlpHttpLogsClient {
    fn query_logs(&self, query: LogsQuery, ctx: &egui::Context) -> Promise<LogsResult> {
        let url = self.build_query_url(&query);

        log::debug!("OTLP HTTP query_logs: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => serde_json::from_slice::<LogsResponse>(&bytes)
                                .map_err(|e| ClientError::ParseError(e.to_string())),
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
        let url = format!("{}/api/otlp/labels", self.base_url);

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => serde_json::from_slice::<Vec<String>>(&bytes)
                                .map_err(|e| ClientError::ParseError(e.to_string())),
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
        "otlp"
    }

    fn health_check(&self, ctx: &egui::Context) -> Promise<HealthCheckResult> {
        let url = format!("{}/api/otlp/health", self.base_url);

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => serde_json::from_slice::<BackendInfo>(&bytes)
                                .map_err(|e| ClientError::ParseError(e.to_string())),
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
    use crate::url_encode;
    use rustc_hash::FxHashMap;

    fn with_runtime<F: FnOnce()>(f: F) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        f();
    }

    #[test]
    fn test_new_removes_trailing_slash() {
        with_runtime(|| {
            let client = OtlpHttpLogsClient::new("http://localhost:3030/");
            assert_eq!(client.base_url, "http://localhost:3030");
        });
    }

    #[test]
    fn test_new_adds_http_protocol() {
        with_runtime(|| {
            let client = OtlpHttpLogsClient::new("localhost:3030");
            assert_eq!(client.base_url, "http://localhost:3030");
        });
    }

    #[test]
    fn test_new_preserves_https() {
        with_runtime(|| {
            let client = OtlpHttpLogsClient::new("https://agent.example.com");
            assert_eq!(client.base_url, "https://agent.example.com");
        });
    }

    #[test]
    fn test_backend_type() {
        with_runtime(|| {
            let client = OtlpHttpLogsClient::new("http://localhost:3030");
            assert_eq!(client.backend_type(), "otlp");
        });
    }

    #[test]
    fn test_build_query_url_minimal() {
        with_runtime(|| {
            let client = OtlpHttpLogsClient::new("http://localhost:3030");
            let query = LogsQuery {
                query: None,
                labels: FxHashMap::default(),
                contains: None,
                start_ns: 1000,
                end_ns: 2000,
                limit: 100,
                direction: crate::logs::QueryDirection::Backward,
            };
            let url = client.build_query_url(&query);
            assert!(url.starts_with("http://localhost:3030/api/otlp/logs/query?"));
            assert!(url.contains("start_ns=1000"));
            assert!(url.contains("end_ns=2000"));
            assert!(url.contains("limit=100"));
            assert!(!url.contains("contains="));
            assert!(!url.contains("labels="));
        });
    }

    #[test]
    fn test_build_query_url_with_contains() {
        with_runtime(|| {
            let client = OtlpHttpLogsClient::new("http://localhost:3030");
            let query = LogsQuery {
                query: None,
                labels: FxHashMap::default(),
                contains: Some("error".to_string()),
                start_ns: 0,
                end_ns: 1000,
                limit: 50,
                direction: crate::logs::QueryDirection::Backward,
            };
            let url = client.build_query_url(&query);
            assert!(url.contains("contains=error"));
        });
    }

    #[test]
    fn test_build_query_url_with_labels() {
        with_runtime(|| {
            let client = OtlpHttpLogsClient::new("http://localhost:3030");
            let mut labels = FxHashMap::default();
            labels.insert("service".to_string(), "api".to_string());
            let query = LogsQuery {
                query: None,
                labels,
                contains: None,
                start_ns: 0,
                end_ns: 1000,
                limit: 50,
                direction: crate::logs::QueryDirection::Backward,
            };
            let url = client.build_query_url(&query);
            assert!(url.contains("labels="));
            // Labels are JSON-encoded, so should contain %7B (encoded '{')
            assert!(url.contains("%7B"));
        });
    }

    #[test]
    fn test_url_encode_simple() {
        assert_eq!(url_encode("simple"), "simple");
        assert_eq!(url_encode("hello world"), "hello%20world");
    }

    #[test]
    fn test_url_encode_special_chars() {
        assert_eq!(url_encode("{\"key\":\"val\"}"), "%7B%22key%22:%22val%22%7D");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(url_encode("[1,2]"), "%5B1,2%5D");
        assert_eq!(url_encode("a+b"), "a%2Bb");
    }
}
