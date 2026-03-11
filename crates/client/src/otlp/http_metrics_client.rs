//! HTTP-based OTLP metrics client.
//!
//! Queries the agent daemon's OTLP metrics store over HTTP, following the same
//! promise-based async pattern as [`PrometheusClient`](crate::prometheus::PrometheusClient).

use poll_promise::Promise;

use crate::error::ClientError;
use crate::normalize_url;
use crate::now_unix_secs;
use crate::promise::promise_channel;
use crate::url_encode;
use crate::{
    BackendInfo, HealthCheckResult, LabelsResult, MetricLabelsResult, MetricsClient, QueryRequest,
    QueryResult,
};

/// HTTP client for querying the agent's OTLP metrics store.
///
/// Sends GET requests to `/api/otlp/metrics/*` endpoints on the agent daemon.
pub struct OtlpHttpMetricsClient {
    base_url: String,
    http_client: reqwest::Client,
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: tokio::runtime::Handle,
}

impl OtlpHttpMetricsClient {
    /// Create a new OTLP HTTP metrics client.
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
}

impl MetricsClient for OtlpHttpMetricsClient {
    fn query(&self, request: QueryRequest, ctx: &egui::Context) -> Promise<QueryResult> {
        let now_ns = now_unix_secs() as u128 * 1_000_000_000;
        let start_ns = request.start.unwrap_or(now_ns - 3_600_000_000_000);
        let end_ns = request.end.unwrap_or(now_ns);
        let step_ns = request.step_secs as u128 * 1_000_000_000;

        let url = format!(
            "{}/api/otlp/metrics/query?metric={}&start_ns={}&end_ns={}&step_ns={}",
            self.base_url,
            url_encode(&request.metric),
            start_ns,
            end_ns,
            step_ns,
        );

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = fetch_json(&client, &url).await;
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_label_names(&self, ctx: &egui::Context) -> Promise<LabelsResult> {
        let url = format!("{}/api/otlp/metrics/labels", self.base_url);

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = fetch_json(&client, &url).await;
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_label_values(&self, label: &str, ctx: &egui::Context) -> Promise<LabelsResult> {
        let url = format!(
            "{}/api/otlp/metrics/label_values?label={}",
            self.base_url,
            url_encode(label)
        );

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = fetch_json(&client, &url).await;
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_metric_names(&self, ctx: &egui::Context) -> Promise<LabelsResult> {
        let url = format!("{}/api/otlp/metrics/names", self.base_url);

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = fetch_json(&client, &url).await;
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_metric_labels(
        &self,
        metric: &str,
        ctx: &egui::Context,
    ) -> Promise<MetricLabelsResult> {
        let url = format!(
            "{}/api/otlp/metrics/metric_labels?metric={}",
            self.base_url,
            url_encode(metric)
        );

        let (sender, promise) = promise_channel();
        let ctx = ctx.clone();
        let client = self.http_client.clone();

        self.spawn(async move {
            let result = fetch_json(&client, &url).await;
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
            let result: Result<BackendInfo, ClientError> = fetch_json(&client, &url).await;
            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }
}

/// Generic JSON fetch helper.
async fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T, ClientError> {
    match client.get(url).send().await {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                match response.bytes().await {
                    Ok(bytes) => serde_json::from_slice::<T>(&bytes)
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
    fn test_new_normalizes_url() {
        with_runtime(|| {
            let client = OtlpHttpMetricsClient::new("localhost:3030/");
            assert_eq!(client.base_url, "http://localhost:3030");
        });
    }

    #[test]
    fn test_backend_type() {
        with_runtime(|| {
            let client = OtlpHttpMetricsClient::new("http://localhost:3030");
            assert_eq!(client.backend_type(), "otlp");
        });
    }
}
