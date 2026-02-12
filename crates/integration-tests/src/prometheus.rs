//! Integration tests for PrometheusClient using testcontainers.
//!
//! These tests spin up a real Prometheus instance and verify that the client
//! can correctly query metrics, fetch labels, and perform health checks.
//!
//! # Requirements
//!
//! These tests require Docker to be running. They are ignored by default
//! to avoid blocking CI pipelines without Docker support.
//!
//! # Running the tests
//!
//! ```bash
//! # Run with Docker available
//! cargo nextest run -p enya-integration-tests --run-ignored ignored-only
//!
//! # Or with standard cargo test
//! cargo test -p enya-integration-tests -- --ignored
//! ```

use enya_client::prometheus::PrometheusClient;
use enya_client::{MetricsClient, QueryRequest};
use std::time::Duration;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};

/// Prometheus container wrapper for tests.
struct PrometheusContainer {
    #[allow(dead_code)]
    container: ContainerAsync<GenericImage>,
    port: u16,
}

impl PrometheusContainer {
    /// Start a new Prometheus container with self-scraping enabled.
    async fn start() -> Self {
        // Use prom/prometheus image - the default config includes self-scraping
        // but we need to wait long enough for Prometheus to scrape itself
        let image = GenericImage::new("prom/prometheus", "latest")
            .with_exposed_port(9090.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "Server is ready to receive web requests.",
            ));

        let container = image.start().await.unwrap();
        let port = container.get_host_port_ipv4(9090).await.unwrap();

        // Additional wait for Prometheus API to be ready
        let client = reqwest::Client::new();
        let url = format!("http://localhost:{port}/api/v1/status/buildinfo");

        for _ in 0..30 {
            if client.get(&url).send().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Self { container, port }
    }

    /// Get the base URL for the Prometheus instance.
    fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}

/// Create a minimal egui context for testing.
fn test_context() -> egui::Context {
    egui::Context::default()
}

/// Helper to block on a promise until it resolves.
async fn await_promise<T: Clone + Send + 'static>(promise: enya_client::Promise<T>) -> T {
    loop {
        if let Some(result) = promise.ready() {
            return result.clone();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Wait for Prometheus to have scraped data (polls until `up` metric exists).
async fn wait_for_scrape_data(client: &PrometheusClient, ctx: &egui::Context) {
    for _ in 0..30 {
        let request = QueryRequest::new("up", "up").with_step(15);
        let promise = client.query(request, ctx);
        let result = await_promise(promise).await;
        if let Ok(response) = result {
            if !response.groups.is_empty() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_health_check() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();
    let promise = client.health_check(&ctx);
    let result = await_promise(promise).await;

    let info = result.expect("health check should succeed");
    assert_eq!(info.backend_type, "prometheus");
    assert!(!info.version.is_empty(), "version should not be empty");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_backend_type() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    assert_eq!(client.backend_type(), "prometheus");

    drop(prometheus);
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_fetch_label_names() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();
    let promise = client.fetch_label_names(&ctx);
    let result = await_promise(promise).await;

    // Fresh Prometheus has no labels, but the call should succeed
    result.expect("fetch label names should succeed");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_fetch_metric_names_empty() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();
    let promise = client.fetch_metric_names(&ctx);
    let result = await_promise(promise).await;

    // Fresh Prometheus may have no metrics or just internal ones
    result.expect("fetch metric names should succeed");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_query_nonexistent_metric() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();
    let request = QueryRequest::new("nonexistent_metric", "nonexistent_metric");
    let promise = client.query(request, &ctx);
    let result = await_promise(promise).await;

    // Query should succeed but return empty results
    let response = result.expect("query should succeed even for nonexistent metric");
    assert!(
        response.groups.is_empty(),
        "should have no groups for nonexistent metric"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_query_prometheus_internal_metrics() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();

    // Wait for Prometheus to scrape itself
    wait_for_scrape_data(&client, &ctx).await;

    // Query Prometheus's own internal metric
    let request = QueryRequest::new("up", "up").with_step(15);
    let promise = client.query(request, &ctx);
    let result = await_promise(promise).await;

    let response = result.expect("query should succeed");
    // Prometheus self-scrapes by default, so 'up' should have at least one group
    assert!(
        !response.groups.is_empty(),
        "Prometheus should have 'up' metric from self-scraping"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_query_with_time_range() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();

    // Wait for Prometheus to scrape itself
    wait_for_scrape_data(&client, &ctx).await;

    #[allow(clippy::disallowed_types)] // integration tests are native-only
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let hour_ago_ns = now_ns.saturating_sub(3600 * 1_000_000_000);

    let request = QueryRequest::new("up", "up")
        .with_step(15)
        .with_range(hour_ago_ns, now_ns);
    let promise = client.query(request, &ctx);
    let result = await_promise(promise).await;

    let response = result.expect("query with time range should succeed");
    // Should get data for the 'up' metric
    assert!(
        !response.groups.is_empty(),
        "should have groups for 'up' metric"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_fetch_label_values_for_job() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();

    // Wait for Prometheus to scrape itself
    wait_for_scrape_data(&client, &ctx).await;

    let promise = client.fetch_label_values("job", &ctx);
    let result = await_promise(promise).await;

    let values = result.expect("fetch label values should succeed");
    // Prometheus self-scrapes with job="prometheus"
    assert!(
        values.contains(&"prometheus".to_string()),
        "should have 'prometheus' job label value"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_fetch_metric_labels() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();

    // Wait for Prometheus to scrape itself
    wait_for_scrape_data(&client, &ctx).await;

    let promise = client.fetch_metric_labels("up", &ctx);
    let result = await_promise(promise).await;

    let metric_labels = result.expect("fetch metric labels should succeed");
    // 'up' metric should have at least 'job' and 'instance' labels
    assert!(
        metric_labels.labels.contains_key("job"),
        "up metric should have 'job' label"
    );
    assert!(
        metric_labels.labels.contains_key("instance"),
        "up metric should have 'instance' label"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_promql_aggregation_query() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();

    // Wait for Prometheus to scrape itself
    wait_for_scrape_data(&client, &ctx).await;

    let request = QueryRequest::new("up", "sum(up)").with_step(15);
    let promise = client.query(request, &ctx);
    let result = await_promise(promise).await;

    let response = result.expect("aggregation query should succeed");
    // Sum should have exactly one group (aggregated result)
    assert!(
        !response.groups.is_empty(),
        "sum query should return results"
    );
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn test_promql_rate_query() {
    let prometheus = PrometheusContainer::start().await;
    let runtime = tokio::runtime::Handle::current();
    let client = PrometheusClient::with_runtime(prometheus.url(), runtime);

    let ctx = test_context();

    // Query rate of prometheus_http_requests_total
    // This may return empty results if there haven't been enough scrapes
    let request = QueryRequest::new(
        "prometheus_http_requests_total",
        "rate(prometheus_http_requests_total[1m])",
    )
    .with_step(15);
    let promise = client.query(request, &ctx);
    let result = await_promise(promise).await;

    // Rate query should succeed (may have empty results if no requests)
    result.expect("rate query should succeed");
}
