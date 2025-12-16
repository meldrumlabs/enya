//! Integration tests for the Prometheus client using testcontainers.
//!
//! These tests spin up a real Prometheus instance in a Docker container
//! and test the enya-client's ability to query it.
//!
//! All tests share a single Prometheus container to avoid startup overhead.

use std::time::Duration;

use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};
use tokio::sync::OnceCell;

/// Port that Prometheus listens on inside the container.
const PROMETHEUS_PORT: u16 = 9090;

/// Global container instance shared across all tests.
/// This avoids starting a new container for each test.
static PROMETHEUS_CONTAINER: OnceCell<PrometheusFixture> = OnceCell::const_new();

/// Create a Prometheus container with default config.
fn prometheus_image() -> GenericImage {
    GenericImage::new("prom/prometheus", "latest")
        .with_exposed_port(PROMETHEUS_PORT.tcp())
        .with_wait_for(WaitFor::message_on_stderr("Server is ready to receive web requests"))
}

/// Wait for Prometheus to be ready by polling /api/v1/status/runtimeinfo.
async fn wait_for_prometheus(
    url: &str,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Ok(resp) = client
            .get(format!("{url}/api/v1/status/runtimeinfo"))
            .send()
            .await
        {
            if resp.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    Err("Prometheus did not become ready in time".into())
}

/// Test fixture for Prometheus integration tests.
struct PrometheusFixture {
    _container: ContainerAsync<GenericImage>,
    base_url: String,
}

// Safety: The fixture is only accessed via the OnceCell which ensures
// safe concurrent access.
unsafe impl Send for PrometheusFixture {}
unsafe impl Sync for PrometheusFixture {}

impl PrometheusFixture {
    /// Start a Prometheus container and return the fixture.
    async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        eprintln!("Starting Prometheus container...");
        let container = prometheus_image().start().await?;
        let host_port = container.get_host_port_ipv4(PROMETHEUS_PORT).await?;
        let base_url = format!("http://127.0.0.1:{host_port}");

        eprintln!("Prometheus container started, waiting for readiness at {base_url}...");

        // Wait for Prometheus to be ready
        wait_for_prometheus(&base_url, Duration::from_secs(60)).await?;

        eprintln!("Prometheus is ready!");

        Ok(Self {
            _container: container,
            base_url,
        })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Get the shared Prometheus fixture, starting the container if needed.
async fn get_prometheus() -> &'static PrometheusFixture {
    PROMETHEUS_CONTAINER
        .get_or_init(|| async {
            PrometheusFixture::new()
                .await
                .expect("Failed to start Prometheus container")
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use enya_client::prometheus::PrometheusClient;
    use enya_client::MetricsClient;

    /// Test that we can connect to a Prometheus instance.
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_prometheus_connectivity() {
        let fixture = get_prometheus().await;

        let client = PrometheusClient::new(fixture.base_url());
        assert_eq!(client.backend_type(), "prometheus");
    }

    /// Test fetching label names from Prometheus.
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_fetch_label_names() {
        let fixture = get_prometheus().await;

        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/labels", fixture.base_url());

        let resp = client.get(&url).send().await.expect("request failed");
        assert!(resp.status().is_success());

        let body: serde_json::Value = resp.json().await.expect("json parse failed");
        assert_eq!(body["status"], "success");
        assert!(body["data"].is_array());
    }

    /// Test fetching metric names from Prometheus.
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_fetch_metric_names() {
        let fixture = get_prometheus().await;

        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/label/__name__/values", fixture.base_url());

        let resp = client.get(&url).send().await.expect("request failed");
        assert!(resp.status().is_success());

        let body: serde_json::Value = resp.json().await.expect("json parse failed");
        assert_eq!(body["status"], "success");

        let metrics = body["data"].as_array().expect("data should be array");
        assert!(
            !metrics.is_empty(),
            "Prometheus should have built-in metrics"
        );

        let metric_names: Vec<&str> = metrics.iter().filter_map(|m| m.as_str()).collect();

        assert!(
            metric_names.iter().any(|m| m.starts_with("prometheus_")),
            "Expected prometheus_* metrics, got: {metric_names:?}"
        );
    }

    /// Test querying Prometheus for built-in metrics.
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_query_builtin_metric() {
        let fixture = get_prometheus().await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/query?query=prometheus_build_info",
            fixture.base_url()
        );

        let resp = client.get(&url).send().await.expect("request failed");
        assert!(resp.status().is_success());

        let body: serde_json::Value = resp.json().await.expect("json parse failed");
        assert_eq!(body["status"], "success");

        let result = &body["data"]["result"];
        assert!(result.is_array());
        let results = result.as_array().unwrap();
        assert!(
            !results.is_empty(),
            "prometheus_build_info should return data"
        );
    }

    /// Test range query against Prometheus.
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_range_query() {
        let fixture = get_prometheus().await;

        let client = reqwest::Client::new();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let start = now - 300;
        let end = now;

        let url = format!(
            "{}/api/v1/query_range?query=up&start={}&end={}&step=15",
            fixture.base_url(),
            start,
            end
        );

        let resp = client.get(&url).send().await.expect("request failed");
        assert!(resp.status().is_success());

        let body: serde_json::Value = resp.json().await.expect("json parse failed");
        assert_eq!(body["status"], "success");
        assert_eq!(body["data"]["resultType"], "matrix");
    }

    /// Test that invalid queries return appropriate errors.
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_invalid_query() {
        let fixture = get_prometheus().await;

        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/query?query=invalid{{{{", fixture.base_url());

        let resp = client.get(&url).send().await.expect("request failed");

        assert_eq!(resp.status().as_u16(), 400);

        let body: serde_json::Value = resp.json().await.expect("json parse failed");
        assert_eq!(body["status"], "error");
        assert!(body["error"].as_str().is_some());
    }

    /// Test translation from enya-lang to PromQL via the client.
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_enya_query_translation() {
        let fixture = get_prometheus().await;

        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/query?query=sum%20by%20(job)%20(up)",
            fixture.base_url()
        );

        let resp = client.get(&url).send().await.expect("request failed");
        assert!(resp.status().is_success());

        let body: serde_json::Value = resp.json().await.expect("json parse failed");
        assert_eq!(body["status"], "success");
    }
}
