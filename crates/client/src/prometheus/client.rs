//! Prometheus HTTP client implementation.

use poll_promise::Promise;

use crate::error::ClientError;
use crate::now_unix_secs;
use crate::promise::promise_channel;
use crate::request::QueryRequest;
use crate::{LabelsResult, MetricLabelsResult, MetricsClient, QueryResult};

use super::response::{parse_labels_response, parse_response, parse_series_response};
use super::translate::translate;

/// Client for querying Prometheus via its HTTP API.
///
/// Translates enya-lang queries to PromQL and executes them against
/// the `/api/v1/query_range` endpoint.
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
}

impl PrometheusClient {
    /// Create a new Prometheus client.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the Prometheus server (e.g., "http://localhost:9090")
    ///
    /// If no protocol is specified, `http://` is assumed (Prometheus default).
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        let mut url = base_url.into();

        // Add http:// if no protocol specified (Prometheus runs on HTTP by default)
        if !url.starts_with("http://") && !url.starts_with("https://") {
            url = format!("http://{url}");
        }

        // Remove trailing slash if present
        if url.ends_with('/') {
            url.pop();
        }
        Self { base_url: url }
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
        // Translate enya-lang to PromQL
        let promql = match translate(&request.metric, &request.query) {
            Ok(p) => p.query,
            Err(e) => {
                // Return an immediately-resolved promise with the error
                return Promise::from_ready(Err(e));
            }
        };

        let url = self.build_url(&promql, &request);
        let metric = request.metric.clone();
        let query = request.query.clone();
        let granularity_ns = (request.step_secs as u128) * 1_000_000_000;

        let ctx = ctx.clone();

        log::debug!("Prometheus query: {url}");

        let (sender, promise) = promise_channel();

        ehttp::fetch(ehttp::Request::get(&url), move |response| {
            let result = match response {
                Ok(response) => {
                    if response.ok {
                        parse_response(&response.bytes, &metric, &query, granularity_ns)
                    } else {
                        Err(ClientError::BackendError {
                            status: response.status,
                            message: response.status_text,
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e)),
            };

            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_label_names(&self, ctx: &egui::Context) -> Promise<LabelsResult> {
        let url = format!("{}/api/v1/labels", self.base_url);
        let ctx = ctx.clone();

        log::debug!("Prometheus fetch labels: {url}");

        let (sender, promise) = promise_channel();

        ehttp::fetch(ehttp::Request::get(&url), move |response| {
            let result = match response {
                Ok(response) => {
                    if response.ok {
                        parse_labels_response(&response.bytes)
                    } else {
                        Err(ClientError::BackendError {
                            status: response.status,
                            message: response.status_text,
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e)),
            };

            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn fetch_label_values(&self, label: &str, ctx: &egui::Context) -> Promise<LabelsResult> {
        let url = format!("{}/api/v1/label/{}/values", self.base_url, label);
        let ctx = ctx.clone();

        log::debug!("Prometheus fetch label values for '{label}': {url}");

        let (sender, promise) = promise_channel();

        ehttp::fetch(ehttp::Request::get(&url), move |response| {
            let result = match response {
                Ok(response) => {
                    if response.ok {
                        parse_labels_response(&response.bytes)
                    } else {
                        Err(ClientError::BackendError {
                            status: response.status,
                            message: response.status_text,
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e)),
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

        let ctx = ctx.clone();

        log::debug!("Prometheus fetch metric labels for '{metric}': {url}");

        let (sender, promise) = promise_channel();

        ehttp::fetch(ehttp::Request::get(&url), move |response| {
            let result = match response {
                Ok(response) => {
                    if response.ok {
                        parse_series_response(&response.bytes)
                    } else {
                        Err(ClientError::BackendError {
                            status: response.status,
                            message: response.status_text,
                        })
                    }
                }
                Err(e) => Err(ClientError::NetworkError(e)),
            };

            sender.send(result);
            ctx.request_repaint();
        });

        promise
    }

    fn backend_type(&self) -> &'static str {
        "prometheus"
    }
}

/// Simple URL encoding for query parameters.
///
/// This handles the most common characters that need encoding in PromQL queries.
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '"' => result.push_str("%22"),
            '#' => result.push_str("%23"),
            '%' => result.push_str("%25"),
            '&' => result.push_str("%26"),
            '+' => result.push_str("%2B"),
            '=' => result.push_str("%3D"),
            '{' => result.push_str("%7B"),
            '}' => result.push_str("%7D"),
            '[' => result.push_str("%5B"),
            ']' => result.push_str("%5D"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_removes_trailing_slash() {
        let client = PrometheusClient::new("http://localhost:9090/");
        assert_eq!(client.base_url, "http://localhost:9090");
    }

    #[test]
    fn test_new_adds_http_protocol() {
        let client = PrometheusClient::new("localhost:9090");
        assert_eq!(client.base_url, "http://localhost:9090");
    }

    #[test]
    fn test_new_preserves_https() {
        let client = PrometheusClient::new("https://prometheus.example.com");
        assert_eq!(client.base_url, "https://prometheus.example.com");
    }

    #[test]
    fn test_build_url() {
        let client = PrometheusClient::new("http://localhost:9090");
        let request = QueryRequest::new("cpu", "*").with_step(60);

        let url = client.build_url("cpu", &request);
        assert!(url.starts_with("http://localhost:9090/api/v1/query_range?"));
        assert!(url.contains("query=cpu"));
        assert!(url.contains("step=60"));
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
        let client = PrometheusClient::new("http://localhost:9090");
        assert_eq!(client.backend_type(), "prometheus");
    }
}
