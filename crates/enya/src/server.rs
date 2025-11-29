//! Axum-based server for hosting Enya endpoints

// v1/api/search/
// v1/api/metrics/
// v1/api/memory/
// v1/api/cpu/

use super::core::Core;
use crate::util::value_as_f64;
#[cfg(feature = "pprof")]
use axum::http::header;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
#[cfg(feature = "pprof")]
use pprof::protos::Message;
use serde::Deserialize;
use std::net::SocketAddr;
#[cfg(feature = "pprof")]
use std::time::Duration;
use talna::{MetricName, Timestamp};
#[cfg(feature = "pprof")]
use tokio::time::sleep;

/// Setup and serve the application on the specified port.
///
/// This function builds the router using the provided core and starts the HTTP server
/// on the specified port. It returns a future that resolves to the server.
///
/// # Arguments
///
/// * `core` - The core application state.
/// * `port` - The port number to listen on.
pub(crate) async fn setup_and_serve(core: Core, addr: SocketAddr) -> Result<(), std::io::Error> {
    // Build the router
    let app = build_router(core);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

/// Set up the Axum router using the core Enya state
pub fn build_router(core: Core) -> Router {
    let router = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/metrics", get(metrics_handler))
        .route("/api/metrics/preview", get(metrics_preview_handler));

    #[cfg(feature = "pprof")]
    let router = router.route("/api/pprof/profile", get(cpu_profile_handler));

    router.with_state(core)
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Health {
    msg: String,
    version: String,
    git_hash: String,
    git_branch: String,
    built_at: String,
    build_summary: String,
    metrics_git_version: Option<String>,
    metrics_git_timestamp: Option<String>,
}

impl Health {
    fn from_core(core: &Core) -> Self {
        let build_info = core.build_info();
        let (metrics_git_version, metrics_git_timestamp) = core.metrics().git_info();

        Self {
            msg: "Enya is up".to_owned(),
            version: build_info.version.to_string(),
            git_hash: build_info.git_hash_or_tag(),
            git_branch: build_info.git_branch.to_owned(),
            built_at: build_info.datetime.to_owned(),
            build_summary: build_info.to_string(),
            metrics_git_version: metrics_git_version.clone(),
            metrics_git_timestamp: metrics_git_timestamp.clone(),
        }
    }
}

pub async fn health_handler(State(core): State<Core>) -> impl IntoResponse {
    Json(Health::from_core(&core))
}

#[derive(Debug, Deserialize)]
pub struct MetricsPreviewQuery {
    metric: String,
    group_by: String,
    filter: Option<String>,
}

#[derive(serde::Serialize)]
struct MetricsPreviewBucket {
    start: talna::Timestamp,
    end: talna::Timestamp,
    value: f64,
    len: usize,
}

#[derive(serde::Serialize)]
struct MetricsPreviewGroup {
    group: String,
    buckets: Vec<MetricsPreviewBucket>,
}

#[derive(serde::Serialize)]
struct MetricsPreviewResponse {
    metric: String,
    group_by: String,
    filter: String,
    groups: Vec<MetricsPreviewGroup>,
}

pub async fn metrics_preview_handler(
    State(core): State<Core>,
    Query(query): Query<MetricsPreviewQuery>,
) -> impl IntoResponse {
    let metric = match MetricName::try_from(query.metric.as_str()) {
        Ok(metric) => metric,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid metric name {}", query.metric),
            )
                .into_response();
        }
    };

    let filter = query.filter.clone().unwrap_or_else(|| "*".to_string());

    match core
        .metrics()
        .database()
        .sum(metric, &query.group_by)
        .filter(&filter)
        .build()
        .and_then(|agg| agg.collect())
    {
        Ok(groups) => {
            let preview_groups = groups
                .into_iter()
                .map(|(group, buckets)| MetricsPreviewGroup {
                    group,
                    buckets: buckets
                        .into_iter()
                        .map(|bucket| MetricsPreviewBucket {
                            start: bucket.start,
                            end: bucket.end,
                            value: value_as_f64(bucket.value),
                            len: bucket.len,
                        })
                        .collect(),
                })
                .collect();

            Json(MetricsPreviewResponse {
                metric: query.metric,
                group_by: query.group_by,
                filter,
                groups: preview_groups,
            })
            .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to query metrics: {err}"),
        )
            .into_response(),
    }
}

// ============================================================================
// /api/metrics endpoint
// ============================================================================

/// Aggregation type for metrics queries
#[derive(Debug, Clone, Copy, Default, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregationType {
    #[default]
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

impl std::fmt::Display for AggregationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AggregationType::Sum => write!(f, "sum"),
            AggregationType::Avg => write!(f, "avg"),
            AggregationType::Min => write!(f, "min"),
            AggregationType::Max => write!(f, "max"),
            AggregationType::Count => write!(f, "count"),
        }
    }
}

fn default_granularity() -> String {
    "1m".to_string()
}

/// Query parameters for the /api/metrics endpoint
#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    /// The metric name (e.g. "cpu.total")
    metric: String,

    /// Tag to group results by (e.g. "host", "service")
    group_by: String,

    /// Aggregation function: "sum", "avg", "min", "max", "count"
    #[serde(default)]
    agg: AggregationType,

    /// Filter expression (e.g. "env:prod AND service:db")
    /// Supports AND, OR, NOT (!), wildcards (*), and nesting with parentheses
    filter: Option<String>,

    /// Start time - either nanosecond timestamp or relative duration (e.g. "1h", "30m", "7d")
    start: Option<String>,

    /// End time - either nanosecond timestamp or relative duration
    end: Option<String>,

    /// Bucket granularity - either nanoseconds or human-readable (e.g. "1m", "1h", "1d")
    #[serde(default = "default_granularity")]
    granularity: String,
}

#[derive(serde::Serialize)]
struct MetricsBucket {
    start: Timestamp,
    end: Timestamp,
    value: f64,
    count: usize,
}

#[derive(serde::Serialize)]
struct MetricsGroup {
    group: String,
    buckets: Vec<MetricsBucket>,
}

#[derive(serde::Serialize)]
struct MetricsResponse {
    metric: String,
    group_by: String,
    agg: String,
    filter: String,
    start: Option<Timestamp>,
    end: Option<Timestamp>,
    granularity_ns: u128,
    groups: Vec<MetricsGroup>,
}

/// Parse a duration string into nanoseconds.
/// Supports formats like "30s", "5m", "2h", "7d", "1w", "1M", "1y"
/// or raw nanosecond values.
fn parse_duration(s: &str) -> Option<u128> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try parsing as raw nanoseconds first
    if let Ok(ns) = s.parse::<u128>() {
        return Some(ns);
    }

    // Parse human-readable duration
    let (num_str, unit) = if let Some(stripped) = s.strip_suffix("ms") {
        (stripped, "ms")
    } else {
        let idx = s.len().saturating_sub(1);
        (&s[..idx], &s[idx..])
    };

    let n: f64 = num_str.parse().ok()?;

    let ns = match unit {
        "s" => talna::Duration::seconds(n),
        "m" => talna::Duration::minutes(n),
        "h" => talna::Duration::hours(n),
        "d" => talna::Duration::days(n),
        "w" => talna::Duration::weeks(n),
        "M" => talna::Duration::months(n),
        "y" => talna::Duration::years(n),
        "ms" => talna::Duration::millis(n),
        _ => return None,
    };

    Some(ns)
}

/// Parse a time specification which can be either an absolute nanosecond timestamp
/// or a relative duration from now.
fn parse_time_spec(s: &str, now: Timestamp) -> Option<Timestamp> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // If it's a pure number, treat as absolute timestamp
    if let Ok(ts) = s.parse::<u128>() {
        return Some(ts);
    }

    // Otherwise parse as relative duration and subtract from now
    let duration = parse_duration(s)?;
    Some(now.saturating_sub(duration))
}

/// Handler for /api/metrics endpoint
pub async fn metrics_handler(
    State(core): State<Core>,
    Query(query): Query<MetricsQuery>,
) -> impl IntoResponse {
    let metric = match MetricName::try_from(query.metric.as_str()) {
        Ok(m) => m,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid metric name: {}", query.metric),
            )
                .into_response();
        }
    };

    let filter = query.filter.clone().unwrap_or_else(|| "*".to_string());

    let granularity = match parse_duration(&query.granularity) {
        Some(g) if g > 0 => g,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid granularity: {}", query.granularity),
            )
                .into_response();
        }
    };

    let now = talna::timestamp();

    let start_ts = query.start.as_ref().and_then(|s| parse_time_spec(s, now));
    let end_ts = query.end.as_ref().and_then(|s| parse_time_spec(s, now));

    let db = core.metrics().database();

    // Helper macro to build and execute an aggregation
    macro_rules! run_agg {
        ($builder:expr) => {{
            let mut builder = $builder.filter(&filter).granularity(granularity);
            if let Some(ts) = start_ts {
                builder = builder.start(ts);
            }
            if let Some(ts) = end_ts {
                builder = builder.end(ts);
            }
            builder
                .build()
                .and_then(|agg| agg.collect())
                .map(|collected| {
                    collected
                        .into_iter()
                        .map(|(group, buckets)| MetricsGroup {
                            group,
                            buckets: buckets
                                .into_iter()
                                .map(|b| MetricsBucket {
                                    start: b.start,
                                    end: b.end,
                                    value: value_as_f64(b.value),
                                    count: b.len,
                                })
                                .collect(),
                        })
                        .collect()
                })
        }};
    }

    // Build and execute the query based on aggregation type
    let result: Result<Vec<MetricsGroup>, talna::Error> = match query.agg {
        AggregationType::Sum => run_agg!(db.sum(metric, &query.group_by)),
        AggregationType::Avg => run_agg!(db.avg(metric, &query.group_by)),
        AggregationType::Min => run_agg!(db.min(metric, &query.group_by)),
        AggregationType::Max => run_agg!(db.max(metric, &query.group_by)),
        AggregationType::Count => run_agg!(db.count(metric, &query.group_by)),
    };

    match result {
        Ok(groups) => {
            let response = MetricsResponse {
                metric: query.metric,
                group_by: query.group_by,
                agg: query.agg.to_string(),
                filter,
                start: start_ts,
                end: end_ts,
                granularity_ns: granularity,
                groups,
            };
            Json(response).into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to query metrics: {err}"),
        )
            .into_response(),
    }
}

#[cfg(feature = "pprof")]
const DEFAULT_PROFILE_SECONDS: u64 = 10;
#[cfg(feature = "pprof")]
const MIN_PROFILE_SECONDS: u64 = 1;
#[cfg(feature = "pprof")]
const MAX_PROFILE_SECONDS: u64 = 60;
#[cfg(feature = "pprof")]
const DEFAULT_PROFILE_FREQUENCY: i32 = 99;
#[cfg(feature = "pprof")]
const MIN_PROFILE_FREQUENCY: i32 = 1;
#[cfg(feature = "pprof")]
const MAX_PROFILE_FREQUENCY: i32 = 1000;

#[cfg(feature = "pprof")]
#[derive(Debug, Deserialize)]
struct CpuProfileQuery {
    seconds: Option<u64>,
    frequency: Option<i32>,
}

#[cfg(feature = "pprof")]
impl CpuProfileQuery {
    fn duration(&self) -> Duration {
        let secs = self
            .seconds
            .unwrap_or(DEFAULT_PROFILE_SECONDS)
            .clamp(MIN_PROFILE_SECONDS, MAX_PROFILE_SECONDS);
        Duration::from_secs(secs)
    }

    fn frequency(&self) -> i32 {
        self.frequency
            .unwrap_or(DEFAULT_PROFILE_FREQUENCY)
            .clamp(MIN_PROFILE_FREQUENCY, MAX_PROFILE_FREQUENCY)
    }
}

/// Sample a CPU profile and return it in the pprof protobuf format.
#[cfg(feature = "pprof")]
async fn cpu_profile_handler(Query(query): Query<CpuProfileQuery>) -> impl IntoResponse {
    let duration = query.duration();
    let frequency = query.frequency();

    let guard = match pprof::ProfilerGuardBuilder::default()
        .frequency(frequency)
        .build()
    {
        Ok(guard) => guard,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to start CPU profiler: {err}"),
            )
                .into_response();
        }
    };

    sleep(duration).await;

    let report = match guard.report().build() {
        Ok(report) => report,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to build CPU profile report: {err}"),
            )
                .into_response();
        }
    };

    let profile = match report.pprof() {
        Ok(profile) => profile,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to generate protobuf profile: {err}"),
            )
                .into_response();
        }
    };

    let mut body = Vec::new();
    if let Err(err) = profile.encode(&mut body) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to serialize CPU profile: {err}"),
        )
            .into_response();
    }

    (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"cpu-profile.pb\"",
            ),
        ],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use enya_metrics_store::MetricsStore;
    use talna::{Database, MetricName, tagset};

    fn create_test_core(dir: &std::path::Path) -> Core {
        let db = Database::builder().open(dir).expect("database");
        let metrics_store = MetricsStore::new(db, None, None);
        let build_info = build_info::build_info!();
        Core::new(build_info, metrics_store)
    }

    fn metric(name: &str) -> MetricName<'_> {
        MetricName::try_from(name).expect("valid metric name")
    }

    #[tokio::test]
    async fn test_metrics_endpoint_basic_query() {
        let dir = tempfile::tempdir().expect("tempdir");
        let core = create_test_core(dir.path());

        // Write some test data
        let m = metric("cpu.usage");
        let db = core.metrics().database();
        db.write_at(m, 1000, 10.0, tagset!("host" => "server1", "env" => "prod"))
            .unwrap();
        db.write_at(m, 2000, 20.0, tagset!("host" => "server1", "env" => "prod"))
            .unwrap();
        db.write_at(m, 3000, 30.0, tagset!("host" => "server2", "env" => "prod"))
            .unwrap();

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "cpu.usage")
            .add_query_param("group_by", "host")
            .await;

        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        assert_eq!(body["metric"], "cpu.usage");
        assert_eq!(body["group_by"], "host");
        assert_eq!(body["agg"], "sum");
        assert_eq!(body["filter"], "*");

        let groups = body["groups"].as_array().expect("groups array");
        assert_eq!(groups.len(), 2);
    }

    #[tokio::test]
    async fn test_metrics_endpoint_with_filter() {
        let dir = tempfile::tempdir().expect("tempdir");
        let core = create_test_core(dir.path());

        let m = metric("requests.count");
        let db = core.metrics().database();
        db.write_at(m, 1000, 5.0, tagset!("service" => "api", "env" => "prod"))
            .unwrap();
        db.write_at(
            m,
            2000,
            10.0,
            tagset!("service" => "api", "env" => "staging"),
        )
        .unwrap();
        db.write_at(m, 3000, 15.0, tagset!("service" => "web", "env" => "prod"))
            .unwrap();

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "requests.count")
            .add_query_param("group_by", "service")
            .add_query_param("filter", "env:prod")
            .await;

        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        assert_eq!(body["filter"], "env:prod");

        let groups = body["groups"].as_array().expect("groups array");
        // Should only have api and web with env:prod, not the staging one
        assert_eq!(groups.len(), 2);

        // Verify the values are correct (only prod entries)
        for group in groups {
            let group_name = group["group"].as_str().unwrap();
            let buckets = group["buckets"].as_array().unwrap();
            assert!(!buckets.is_empty());

            match group_name {
                "api" => {
                    // Only one entry with value 5.0
                    assert_eq!(buckets[0]["value"].as_f64().unwrap(), 5.0);
                }
                "web" => {
                    // Only one entry with value 15.0
                    assert_eq!(buckets[0]["value"].as_f64().unwrap(), 15.0);
                }
                _ => panic!("unexpected group: {group_name}"),
            }
        }
    }

    #[tokio::test]
    async fn test_metrics_endpoint_aggregation_types() {
        let dir = tempfile::tempdir().expect("tempdir");
        let core = create_test_core(dir.path());

        let m = metric("latency.ms");
        let db = core.metrics().database();
        // Write multiple values for the same group to test aggregations
        db.write_at(m, 1000, 10.0, tagset!("endpoint" => "/api/users"))
            .unwrap();
        db.write_at(m, 2000, 20.0, tagset!("endpoint" => "/api/users"))
            .unwrap();
        db.write_at(m, 3000, 30.0, tagset!("endpoint" => "/api/users"))
            .unwrap();
        db.write_at(m, 4000, 40.0, tagset!("endpoint" => "/api/users"))
            .unwrap();

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        // Test SUM aggregation (default)
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "latency.ms")
            .add_query_param("group_by", "endpoint")
            .add_query_param("agg", "sum")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "sum");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 100.0); // 10 + 20 + 30 + 40

        // Test AVG aggregation
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "latency.ms")
            .add_query_param("group_by", "endpoint")
            .add_query_param("agg", "avg")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "avg");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 25.0); // (10 + 20 + 30 + 40) / 4

        // Test MIN aggregation
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "latency.ms")
            .add_query_param("group_by", "endpoint")
            .add_query_param("agg", "min")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "min");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 10.0);

        // Test MAX aggregation
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "latency.ms")
            .add_query_param("group_by", "endpoint")
            .add_query_param("agg", "max")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "max");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 40.0);

        // Test COUNT aggregation
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "latency.ms")
            .add_query_param("group_by", "endpoint")
            .add_query_param("agg", "count")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "count");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 4.0); // 4 data points
    }

    #[tokio::test]
    async fn test_metrics_endpoint_invalid_metric_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let core = create_test_core(dir.path());

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        // MetricName only allows lowercase a-z, underscore, and period
        // Uppercase letters and special characters are invalid
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "CPU-Usage!") // invalid characters
            .add_query_param("group_by", "host")
            .await;

        response.assert_status_bad_request();
    }

    #[tokio::test]
    async fn test_metrics_endpoint_invalid_granularity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let core = create_test_core(dir.path());

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "cpu.usage")
            .add_query_param("group_by", "host")
            .add_query_param("granularity", "invalid")
            .await;

        response.assert_status_bad_request();
    }

    #[tokio::test]
    async fn test_metrics_endpoint_empty_result() {
        let dir = tempfile::tempdir().expect("tempdir");
        let core = create_test_core(dir.path());

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        // Query for a metric that doesn't exist
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "nonexistent.metric")
            .add_query_param("group_by", "host")
            .await;

        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups array");
        assert!(groups.is_empty());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s"), Some(talna::Duration::seconds(30.0)));
        assert_eq!(parse_duration("5m"), Some(talna::Duration::minutes(5.0)));
        assert_eq!(parse_duration("2h"), Some(talna::Duration::hours(2.0)));
        assert_eq!(parse_duration("7d"), Some(talna::Duration::days(7.0)));
        assert_eq!(parse_duration("1w"), Some(talna::Duration::weeks(1.0)));
        assert_eq!(parse_duration("1M"), Some(talna::Duration::months(1.0)));
        assert_eq!(parse_duration("1y"), Some(talna::Duration::years(1.0)));
        assert_eq!(
            parse_duration("100ms"),
            Some(talna::Duration::millis(100.0))
        );

        // Raw nanoseconds
        assert_eq!(parse_duration("1000000000"), Some(1000000000));

        // Invalid
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("invalid"), None);
        assert_eq!(parse_duration("10x"), None);
    }

    #[test]
    fn test_parse_time_spec() {
        let now = 1_000_000_000_000u128; // 1 second in nanoseconds

        // Absolute timestamp
        assert_eq!(parse_time_spec("500000000000", now), Some(500000000000));

        // Relative duration (1 second ago)
        let one_sec = talna::Duration::seconds(1.0);
        assert_eq!(parse_time_spec("1s", now), Some(now - one_sec));

        // Empty
        assert_eq!(parse_time_spec("", now), None);
    }
}
