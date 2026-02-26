//! HTTP-based OTLP tracing client.
//!
//! Queries the agent daemon's OTLP store over HTTP, following the same
//! promise-based async pattern as [`TempoClient`](crate::tracing::tempo::TempoClient).

use poll_promise::Promise;

use crate::error::ClientError;
use crate::normalize_url;
use crate::promise::promise_channel;
use crate::tracing::tempo::types::{Trace, TraceSearchParams, TraceSummary};
use crate::tracing::{SearchResult, TraceResult, TracingClient};

/// HTTP client for querying the agent's OTLP telemetry store.
///
/// Sends GET requests to `/api/otlp/traces/{id}` and `/api/otlp/traces/search`
/// endpoints on the agent daemon.
pub struct OtlpHttpTracingClient {
    base_url: String,
    http_client: reqwest::Client,
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: tokio::runtime::Handle,
}

impl OtlpHttpTracingClient {
    /// Create a new OTLP HTTP tracing client.
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

    fn build_search_url(&self, params: &TraceSearchParams) -> String {
        let mut url = format!("{}/api/otlp/traces/search?", self.base_url);
        let mut first = true;

        fn append(url: &mut String, first: &mut bool, key: &str, value: &str) {
            if !*first {
                url.push('&');
            }
            *first = false;
            url.push_str(key);
            url.push('=');
            url.push_str(value);
        }

        if let Some(ref service) = params.service_name {
            append(&mut url, &mut first, "service_name", service);
        }

        if let Some(ref op) = params.operation_name {
            append(&mut url, &mut first, "operation_name", op);
        }

        if let Some(min_dur) = params.min_duration_ms {
            append(
                &mut url,
                &mut first,
                "min_duration_ms",
                &min_dur.to_string(),
            );
        }

        if let Some(max_dur) = params.max_duration_ms {
            append(
                &mut url,
                &mut first,
                "max_duration_ms",
                &max_dur.to_string(),
            );
        }

        if let Some(limit) = params.limit {
            append(&mut url, &mut first, "limit", &limit.to_string());
        } else {
            append(&mut url, &mut first, "limit", "20");
        }

        if let Some(start) = params.start_time_secs {
            append(&mut url, &mut first, "start_time_secs", &start.to_string());
        }

        if let Some(end) = params.end_time_secs {
            append(&mut url, &mut first, "end_time_secs", &end.to_string());
        }

        url
    }
}

impl TracingClient for OtlpHttpTracingClient {
    fn get_trace(&self, trace_id: &str, ctx: &egui::Context) -> Promise<TraceResult> {
        let url = format!("{}/api/otlp/traces/{}", self.base_url, trace_id);

        log::debug!("OTLP HTTP get_trace: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => serde_json::from_slice::<Trace>(&bytes)
                                .map_err(|e| ClientError::ParseError(e.to_string())),
                            Err(e) => Err(ClientError::NetworkError(e.to_string())),
                        }
                    } else if status.as_u16() == 404 {
                        Err(ClientError::BackendError {
                            status: 404,
                            message: "Trace not found".to_string(),
                        })
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

    fn search_traces(
        &self,
        params: TraceSearchParams,
        ctx: &egui::Context,
    ) -> Promise<SearchResult> {
        let url = self.build_search_url(&params);

        log::debug!("OTLP HTTP search_traces: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => serde_json::from_slice::<Vec<TraceSummary>>(&bytes)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_runtime<F: FnOnce()>(f: F) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        f();
    }

    #[test]
    fn test_new_removes_trailing_slash() {
        with_runtime(|| {
            let client = OtlpHttpTracingClient::new("http://localhost:3030/");
            assert_eq!(client.base_url, "http://localhost:3030");
        });
    }

    #[test]
    fn test_new_adds_http_protocol() {
        with_runtime(|| {
            let client = OtlpHttpTracingClient::new("localhost:3030");
            assert_eq!(client.base_url, "http://localhost:3030");
        });
    }

    #[test]
    fn test_new_preserves_https() {
        with_runtime(|| {
            let client = OtlpHttpTracingClient::new("https://agent.example.com");
            assert_eq!(client.base_url, "https://agent.example.com");
        });
    }

    #[test]
    fn test_backend_type() {
        with_runtime(|| {
            let client = OtlpHttpTracingClient::new("http://localhost:3030");
            assert_eq!(client.backend_type(), "otlp");
        });
    }

    #[test]
    fn test_build_search_url_minimal() {
        with_runtime(|| {
            let client = OtlpHttpTracingClient::new("http://localhost:3030");
            let params = TraceSearchParams::default();
            let url = client.build_search_url(&params);
            assert!(url.starts_with("http://localhost:3030/api/otlp/traces/search?"));
            assert!(url.contains("limit=20")); // default limit
        });
    }

    #[test]
    fn test_build_search_url_with_service() {
        with_runtime(|| {
            let client = OtlpHttpTracingClient::new("http://localhost:3030");
            let params = TraceSearchParams {
                service_name: Some("my-api".to_string()),
                limit: Some(10),
                ..Default::default()
            };
            let url = client.build_search_url(&params);
            assert!(url.contains("service_name=my-api"));
            assert!(url.contains("limit=10"));
        });
    }

    #[test]
    fn test_build_search_url_all_params() {
        with_runtime(|| {
            let client = OtlpHttpTracingClient::new("http://localhost:3030");
            let params = TraceSearchParams {
                service_name: Some("svc".to_string()),
                operation_name: Some("GET /".to_string()),
                tags: Default::default(),
                min_duration_ms: Some(100),
                max_duration_ms: Some(5000),
                limit: Some(50),
                start_time_secs: Some(1000),
                end_time_secs: Some(2000),
            };
            let url = client.build_search_url(&params);
            assert!(url.contains("service_name=svc"));
            assert!(url.contains("operation_name=GET /"));
            assert!(url.contains("min_duration_ms=100"));
            assert!(url.contains("max_duration_ms=5000"));
            assert!(url.contains("limit=50"));
            assert!(url.contains("start_time_secs=1000"));
            assert!(url.contains("end_time_secs=2000"));
        });
    }
}
