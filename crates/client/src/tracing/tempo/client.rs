//! Tempo HTTP client implementation.

use crate::error::ClientError;
use crate::normalize_url;
use crate::promise::promise_channel;
use crate::url_encode;
use poll_promise::Promise;

use super::response::{parse_search_response, parse_trace_response};
use super::types::{Trace, TraceSearchParams, TraceSummary};
use crate::tracing::{SearchResult, TraceResult, TracingClient};

/// Client for querying Grafana Tempo via its HTTP API.
///
/// Executes trace queries against the `/api/traces/{traceID}` and `/api/search` endpoints.
/// Uses `reqwest` for HTTP requests on both native (with tokio) and WASM (with
/// wasm-bindgen-futures).
///
/// # Example
///
/// ```ignore
/// use enya_client::tracing::{TracingClient, tempo::TempoClient};
///
/// let client = TempoClient::new("http://localhost:3200");
/// let promise = client.get_trace("abc123def456", &ctx);
///
/// // In update loop, poll for result
/// if let Some(result) = promise.ready() {
///     match result {
///         Ok(trace) => { /* render waterfall */ }
///         Err(e) => { /* show error */ }
///     }
/// }
/// ```
pub struct TempoClient {
    base_url: String,
    http_client: reqwest::Client,
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: tokio::runtime::Handle,
}

impl TempoClient {
    /// Create a new Tempo client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Tempo server (e.g., "http://localhost:3200")
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

    /// Create a new Tempo client with an explicit runtime handle.
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

    /// Build the search URL from parameters.
    fn build_search_url(&self, params: &TraceSearchParams) -> String {
        let mut url = format!("{}/api/search?", self.base_url);
        let mut first = true;

        fn append(url: &mut String, first: &mut bool, key: &str, value: &str) {
            if !*first {
                url.push('&');
            }
            *first = false;
            url.push_str(key);
            url.push('=');
            url.push_str(&url_encode(value));
        }

        if let Some(ref service) = params.service_name {
            append(&mut url, &mut first, "service.name", service);
        }

        if let Some(ref op) = params.operation_name {
            append(&mut url, &mut first, "name", op);
        }

        for (key, value) in &params.tags {
            append(&mut url, &mut first, key, value);
        }

        if let Some(min_dur) = params.min_duration_ms {
            append(&mut url, &mut first, "minDuration", &format!("{min_dur}ms"));
        }

        if let Some(max_dur) = params.max_duration_ms {
            append(&mut url, &mut first, "maxDuration", &format!("{max_dur}ms"));
        }

        if let Some(limit) = params.limit {
            append(&mut url, &mut first, "limit", &limit.to_string());
        } else {
            // Default limit
            append(&mut url, &mut first, "limit", "20");
        }

        if let Some(start) = params.start_time_secs {
            append(&mut url, &mut first, "start", &start.to_string());
        }

        if let Some(end) = params.end_time_secs {
            append(&mut url, &mut first, "end", &end.to_string());
        }

        url
    }
}

impl TracingClient for TempoClient {
    fn get_trace(&self, trace_id: &str, ctx: &egui::Context) -> Promise<TraceResult> {
        let url = format!("{}/api/traces/{}", self.base_url, trace_id);

        log::debug!("Tempo get_trace: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_trace_response(&bytes),
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

        log::debug!("Tempo search_traces: {url}");

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.bytes().await {
                            Ok(bytes) => parse_search_response(&bytes),
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
        "tempo"
    }
}

/// Generate demo trace data for testing without a backend.
pub fn demo_trace() -> Trace {
    use super::types::{Span, SpanStatus};
    use rustc_hash::FxHashMap;

    let trace_id = "demo-trace-abc123".to_string();

    let spans = vec![
        Span {
            span_id: "span-001".to_string(),
            trace_id: trace_id.clone(),
            parent_span_id: None,
            operation_name: "HTTP GET /api/users".to_string(),
            service_name: "api-gateway".to_string(),
            start_time_us: 0,
            duration_us: 250_000, // 250ms
            status: SpanStatus::Ok,
            tags: {
                let mut tags = FxHashMap::default();
                tags.insert("http.method".to_string(), "GET".to_string());
                tags.insert("http.url".to_string(), "/api/users".to_string());
                tags.insert("http.status_code".to_string(), "200".to_string());
                tags
            },
            logs: vec![],
            depth: 0,
        },
        Span {
            span_id: "span-002".to_string(),
            trace_id: trace_id.clone(),
            parent_span_id: Some("span-001".to_string()),
            operation_name: "authenticate".to_string(),
            service_name: "auth-service".to_string(),
            start_time_us: 5_000,
            duration_us: 45_000, // 45ms
            status: SpanStatus::Ok,
            tags: {
                let mut tags = FxHashMap::default();
                tags.insert("auth.method".to_string(), "jwt".to_string());
                tags
            },
            logs: vec![],
            depth: 0,
        },
        Span {
            span_id: "span-003".to_string(),
            trace_id: trace_id.clone(),
            parent_span_id: Some("span-001".to_string()),
            operation_name: "SELECT users".to_string(),
            service_name: "user-service".to_string(),
            start_time_us: 55_000,
            duration_us: 120_000, // 120ms
            status: SpanStatus::Ok,
            tags: {
                let mut tags = FxHashMap::default();
                tags.insert("db.type".to_string(), "postgresql".to_string());
                tags.insert(
                    "db.statement".to_string(),
                    "SELECT * FROM users".to_string(),
                );
                tags
            },
            logs: vec![],
            depth: 0,
        },
        Span {
            span_id: "span-004".to_string(),
            trace_id: trace_id.clone(),
            parent_span_id: Some("span-003".to_string()),
            operation_name: "pg_query".to_string(),
            service_name: "user-service".to_string(),
            start_time_us: 60_000,
            duration_us: 95_000, // 95ms
            status: SpanStatus::Ok,
            tags: {
                let mut tags = FxHashMap::default();
                tags.insert("db.rows_affected".to_string(), "42".to_string());
                tags
            },
            logs: vec![],
            depth: 0,
        },
        Span {
            span_id: "span-005".to_string(),
            trace_id: trace_id.clone(),
            parent_span_id: Some("span-001".to_string()),
            operation_name: "cache_lookup".to_string(),
            service_name: "cache-service".to_string(),
            start_time_us: 180_000,
            duration_us: 15_000, // 15ms
            status: SpanStatus::Error,
            tags: {
                let mut tags = FxHashMap::default();
                tags.insert("cache.key".to_string(), "users:list".to_string());
                tags.insert("error".to_string(), "true".to_string());
                tags.insert("error.message".to_string(), "Cache miss".to_string());
                tags
            },
            logs: vec![],
            depth: 0,
        },
        Span {
            span_id: "span-006".to_string(),
            trace_id: trace_id.clone(),
            parent_span_id: Some("span-001".to_string()),
            operation_name: "serialize_response".to_string(),
            service_name: "api-gateway".to_string(),
            start_time_us: 200_000,
            duration_us: 35_000, // 35ms
            status: SpanStatus::Ok,
            tags: {
                let mut tags = FxHashMap::default();
                tags.insert("response.size".to_string(), "4096".to_string());
                tags
            },
            logs: vec![],
            depth: 0,
        },
    ];

    Trace::from_spans(trace_id, spans)
}

/// Generate demo search results for testing without a backend.
pub fn demo_search_results() -> Vec<TraceSummary> {
    vec![
        TraceSummary {
            trace_id: "trace-abc123".to_string(),
            root_service_name: "api-gateway".to_string(),
            root_operation_name: "HTTP GET /api/users".to_string(),
            start_time_us: 1_704_067_200_000_000, // 2024-01-01 00:00:00 UTC
            duration_us: 250_000,
            span_count: 6,
            error_count: 1,
        },
        TraceSummary {
            trace_id: "trace-def456".to_string(),
            root_service_name: "api-gateway".to_string(),
            root_operation_name: "HTTP POST /api/orders".to_string(),
            start_time_us: 1_704_067_260_000_000,
            duration_us: 450_000,
            span_count: 12,
            error_count: 0,
        },
        TraceSummary {
            trace_id: "trace-ghi789".to_string(),
            root_service_name: "worker".to_string(),
            root_operation_name: "process_queue".to_string(),
            start_time_us: 1_704_067_320_000_000,
            duration_us: 1_200_000,
            span_count: 8,
            error_count: 2,
        },
    ]
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
            let client = TempoClient::new("http://localhost:3200/");
            assert_eq!(client.base_url, "http://localhost:3200");
        });
    }

    #[test]
    fn test_new_adds_http_protocol() {
        with_runtime(|| {
            let client = TempoClient::new("localhost:3200");
            assert_eq!(client.base_url, "http://localhost:3200");
        });
    }

    #[test]
    fn test_backend_type() {
        with_runtime(|| {
            let client = TempoClient::new("http://localhost:3200");
            assert_eq!(client.backend_type(), "tempo");
        });
    }

    #[test]
    fn test_demo_trace() {
        let trace = demo_trace();
        assert_eq!(trace.trace_id, "demo-trace-abc123");
        assert_eq!(trace.spans.len(), 6);
        assert!(trace.services.contains(&"api-gateway".to_string()));
        assert!(trace.services.contains(&"auth-service".to_string()));
    }

    #[test]
    fn test_build_search_url() {
        with_runtime(|| {
            let client = TempoClient::new("http://localhost:3200");
            let params = TraceSearchParams {
                service_name: Some("my-service".to_string()),
                limit: Some(10),
                ..Default::default()
            };
            let url = client.build_search_url(&params);
            assert!(url.contains("service.name=my-service"));
            assert!(url.contains("limit=10"));
        });
    }
}
