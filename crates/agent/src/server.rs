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
use enya_lang::{AggregationFunc, Grouping, Query as LangQuery, parse_query};
use enya_metrics_store::{Duration as MetricsDuration, MetricName, Timestamp};
#[cfg(feature = "pprof")]
use pprof::protos::Message;
use serde::Deserialize;
use std::net::SocketAddr;
#[cfg(feature = "pprof")]
use std::time::Duration;
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
        .route("/api/metrics/query", get(query_handler));

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

fn default_granularity() -> String {
    "1m".to_string()
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
        "s" => MetricsDuration::seconds(n),
        "m" => MetricsDuration::minutes(n),
        "h" => MetricsDuration::hours(n),
        "d" => MetricsDuration::days(n),
        "w" => MetricsDuration::weeks(n),
        "M" => MetricsDuration::months(n),
        "y" => MetricsDuration::years(n),
        "ms" => MetricsDuration::millis(n),
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

// ============================================================================
// /api/metrics/query endpoint - enya-lang query string support
// ============================================================================

/// Query parameters for the /api/metrics/query endpoint
#[derive(Debug, Deserialize)]
pub struct LangQueryParams {
    /// The metric name (e.g. "cpu.total")
    metric: String,

    /// The enya-lang query string (e.g. "sum(env:prod) by (host)")
    query: String,

    /// Start time - either nanosecond timestamp or relative duration (e.g. "1h", "30m", "7d")
    start: Option<String>,

    /// End time - either nanosecond timestamp or relative duration
    end: Option<String>,

    /// Bucket granularity - either nanoseconds or human-readable (e.g. "1m", "1h", "1d")
    #[serde(default = "default_granularity")]
    granularity: String,
}

/// Response for query endpoint
#[derive(serde::Serialize)]
struct QueryResponse {
    metric: String,
    query: String,
    parsed_agg: Option<String>,
    parsed_filter: String,
    parsed_grouping: Option<String>,
    parsed_time_range: Option<String>,
    start: Option<Timestamp>,
    end: Option<Timestamp>,
    granularity_ns: u128,
    groups: Vec<MetricsGroup>,
}

/// Handler for /api/metrics/query endpoint
/// Accepts enya-lang query strings like "sum(env:prod) by (host)"
pub async fn query_handler(
    State(core): State<Core>,
    Query(params): Query<LangQueryParams>,
) -> impl IntoResponse {
    // Validate metric name
    let metric = match MetricName::try_from(params.metric.as_str()) {
        Ok(m) => m,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid metric name: {}", params.metric),
            )
                .into_response();
        }
    };

    // Parse the enya-lang query
    let parsed = match parse_query(&params.query) {
        Ok(q) => q,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid query syntax: {e}"),
            )
                .into_response();
        }
    };

    // Parse granularity
    let granularity = match parse_duration(&params.granularity) {
        Some(g) if g > 0 => g,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                format!("invalid granularity: {}", params.granularity),
            )
                .into_response();
        }
    };

    let now = enya_metrics_store::timestamp();
    let start_ts = params.start.as_ref().and_then(|s| parse_time_spec(s, now));
    let end_ts = params.end.as_ref().and_then(|s| parse_time_spec(s, now));

    let db = core.metrics().database();

    // Execute based on query type
    let result = execute_lang_query(&parsed, db, metric, granularity, start_ts, end_ts).await;

    match result {
        Ok((groups, filter_str, agg_str, grouping_str, time_range_str)) => {
            let response = QueryResponse {
                metric: params.metric,
                query: params.query,
                parsed_agg: agg_str,
                parsed_filter: filter_str,
                parsed_grouping: grouping_str,
                parsed_time_range: time_range_str,
                start: start_ts,
                end: end_ts,
                granularity_ns: granularity,
                groups,
            };
            Json(response).into_response()
        }
        Err(err) => err.into_response(),
    }
}

/// Execute a parsed enya-lang query against the database.
/// Returns (groups, filter_string, agg_string, grouping_string, time_range_string)
async fn execute_lang_query<'a>(
    query: &LangQuery<'a>,
    db: &enya_metrics_store::Database,
    metric: MetricName<'a>,
    granularity: u128,
    start_ts: Option<Timestamp>,
    end_ts: Option<Timestamp>,
) -> Result<
    (
        Vec<MetricsGroup>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ),
    (StatusCode, String),
> {
    match query {
        LangQuery::Filter(filter) => {
            // Simple filter query - use sum as default aggregation
            let filter_str = filter.to_string();
            let groups = execute_aggregation(
                db,
                metric,
                AggregationFunc::Sum,
                &filter_str,
                "",
                granularity,
                start_ts,
                end_ts,
            )
            .await?;
            Ok((groups, filter_str, None, None, None))
        }
        LangQuery::Aggregation(agg) => {
            let filter_str = agg.filter.to_string();
            let agg_str = Some(agg.func.to_string());
            let grouping_str = agg.grouping.as_ref().map(ToString::to_string);
            let time_range_str = agg.time_range.as_ref().map(ToString::to_string);

            // Extract group_by labels from grouping clause
            let group_by = match &agg.grouping {
                Some(Grouping::By(labels)) => labels.join(","),
                Some(Grouping::Without(_)) => {
                    // "without" grouping not yet supported in execution layer
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "without grouping not yet supported".to_string(),
                    ));
                }
                None => String::new(),
            };

            let groups = execute_aggregation(
                db,
                metric,
                agg.func,
                &filter_str,
                &group_by,
                granularity,
                start_ts,
                end_ts,
            )
            .await?;
            Ok((groups, filter_str, agg_str, grouping_str, time_range_str))
        }
    }
}

/// Execute a specific aggregation function against the database.
#[allow(clippy::too_many_arguments)]
async fn execute_aggregation<'a>(
    db: &enya_metrics_store::Database,
    metric: MetricName<'a>,
    func: AggregationFunc,
    filter: &str,
    group_by: &str,
    granularity: u128,
    start_ts: Option<Timestamp>,
    end_ts: Option<Timestamp>,
) -> Result<Vec<MetricsGroup>, (StatusCode, String)> {
    // Helper macro to build an aggregation builder with common settings
    macro_rules! build_agg {
        ($builder:expr) => {{
            let mut builder = $builder.filter(filter).granularity(granularity);
            if let Some(ts) = start_ts {
                builder = builder.start(ts);
            }
            if let Some(ts) = end_ts {
                builder = builder.end(ts);
            }
            builder.build()
        }};
    }

    // Helper to map collected results to MetricsGroup
    macro_rules! map_groups {
        ($collected:expr) => {{
            $collected
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
                .collect::<Vec<_>>()
        }};
    }

    // Helper to run an aggregation and map results
    macro_rules! run_agg {
        ($builder:expr) => {{
            async {
                let agg = build_agg!($builder).await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to build aggregation: {e}"),
                    )
                })?;
                let collected = agg.collect().await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to collect results: {e}"),
                    )
                })?;
                Ok::<_, (StatusCode, String)>(map_groups!(collected))
            }
        }};
    }

    match func {
        AggregationFunc::Sum => run_agg!(db.sum(metric, group_by)).await,
        AggregationFunc::Avg => run_agg!(db.avg(metric, group_by)).await,
        AggregationFunc::Min => run_agg!(db.min(metric, group_by)).await,
        AggregationFunc::Max => run_agg!(db.max(metric, group_by)).await,
        AggregationFunc::Count => run_agg!(db.count(metric, group_by)).await,
        AggregationFunc::AvgOverTime => run_agg!(db.avg_over_time(metric, group_by)).await,
        AggregationFunc::SumOverTime => run_agg!(db.sum_over_time(metric, group_by)).await,
        AggregationFunc::MinOverTime => run_agg!(db.min_over_time(metric, group_by)).await,
        AggregationFunc::MaxOverTime => run_agg!(db.max_over_time(metric, group_by)).await,
        AggregationFunc::CountOverTime => run_agg!(db.count_over_time(metric, group_by)).await,
        AggregationFunc::Rate | AggregationFunc::Irate | AggregationFunc::Increase => Err((
            StatusCode::BAD_REQUEST,
            format!("{func} is not yet supported"),
        )),
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
    use enya_metrics_store::{Database, MetricName, MetricsStore, object_store};
    use object_store::memory::InMemory;
    use std::sync::Arc;
    use std::time::Duration;

    async fn create_test_core() -> Core {
        let object_store = Arc::new(InMemory::new());
        let db = Database::builder()
            .with_flush_interval(Duration::from_millis(10))
            .open(object_store, "/")
            .await
            .expect("database");
        let metrics_store = MetricsStore::new(db, None, None);
        let build_info = enya_build_info::build_info!();
        Core::new(build_info, metrics_store)
    }

    fn metric(name: &str) -> MetricName<'_> {
        MetricName::try_from(name).expect("valid metric name")
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s"), Some(MetricsDuration::seconds(30.0)));
        assert_eq!(parse_duration("5m"), Some(MetricsDuration::minutes(5.0)));
        assert_eq!(parse_duration("2h"), Some(MetricsDuration::hours(2.0)));
        assert_eq!(parse_duration("7d"), Some(MetricsDuration::days(7.0)));
        assert_eq!(parse_duration("1w"), Some(MetricsDuration::weeks(1.0)));
        assert_eq!(parse_duration("1M"), Some(MetricsDuration::months(1.0)));
        assert_eq!(parse_duration("1y"), Some(MetricsDuration::years(1.0)));
        assert_eq!(
            parse_duration("100ms"),
            Some(MetricsDuration::millis(100.0))
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
        let one_sec = MetricsDuration::seconds(1.0);
        assert_eq!(parse_time_spec("1s", now), Some(now - one_sec));

        // Empty
        assert_eq!(parse_time_spec("", now), None);
    }

    // =========================================================================
    // /api/metrics/query endpoint tests
    // =========================================================================

    #[tokio::test]
    async fn test_query_endpoint_simple_filter() {
        let core = create_test_core().await;

        let m = metric("cpu.usage");
        let db = core.metrics().database();
        let tags = [("host", "server1"), ("env", "prod")];
        db.write_at(m, 1000, 10.0, &tags).await.unwrap();
        db.write_at(m, 2000, 20.0, &tags).await.unwrap();

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        // Simple filter query (no aggregation function)
        let response = server
            .get("/api/metrics/query")
            .add_query_param("metric", "cpu.usage")
            .add_query_param("query", "env:prod")
            .await;

        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        assert_eq!(body["metric"], "cpu.usage");
        assert_eq!(body["query"], "env:prod");
        assert_eq!(body["parsed_filter"], "env:prod");
        assert!(body["parsed_agg"].is_null()); // No aggregation for simple filter
    }

    #[tokio::test]
    async fn test_query_endpoint_sum_aggregation() {
        let core = create_test_core().await;

        let m = metric("requests.count");
        let db = core.metrics().database();
        let tags1 = [("service", "api"), ("env", "prod")];
        let tags2 = [("service", "web"), ("env", "prod")];
        db.write_at(m, 1000, 10.0, &tags1).await.unwrap();
        db.write_at(m, 2000, 20.0, &tags1).await.unwrap();
        db.write_at(m, 3000, 30.0, &tags2).await.unwrap();

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        let response = server
            .get("/api/metrics/query")
            .add_query_param("metric", "requests.count")
            .add_query_param("query", "sum(env:prod)")
            .await;

        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        assert_eq!(body["parsed_agg"], "sum");
        assert_eq!(body["parsed_filter"], "env:prod");
    }

    #[tokio::test]
    async fn test_query_endpoint_aggregation_with_grouping() {
        let core = create_test_core().await;

        let m = metric("latency.ms");
        let db = core.metrics().database();
        let tags1 = [("host", "server1"), ("region", "us-east")];
        let tags2 = [("host", "server2"), ("region", "us-west")];
        db.write_at(m, 1000, 10.0, &tags1).await.unwrap();
        db.write_at(m, 2000, 20.0, &tags2).await.unwrap();

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        let response = server
            .get("/api/metrics/query")
            .add_query_param("metric", "latency.ms")
            .add_query_param("query", "avg(*) by (region)")
            .await;

        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        assert_eq!(body["parsed_agg"], "avg");
        assert_eq!(body["parsed_filter"], "*");
        assert_eq!(body["parsed_grouping"], "by (region)");
    }

    #[tokio::test]
    async fn test_query_endpoint_over_time_function() {
        let core = create_test_core().await;

        let m = metric("cpu.usage");
        let db = core.metrics().database();
        let tags = [("host", "server1")];
        db.write_at(m, 1000, 10.0, &tags).await.unwrap();
        db.write_at(m, 2000, 20.0, &tags).await.unwrap();
        db.write_at(m, 3000, 30.0, &tags).await.unwrap();

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        let response = server
            .get("/api/metrics/query")
            .add_query_param("metric", "cpu.usage")
            .add_query_param("query", "avg_over_time(*)[5m]")
            .await;

        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        assert_eq!(body["parsed_agg"], "avg_over_time");
        assert_eq!(body["parsed_time_range"], "5m");
    }

    #[tokio::test]
    async fn test_query_endpoint_invalid_query() {
        let core = create_test_core().await;

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        // Invalid query syntax
        let response = server
            .get("/api/metrics/query")
            .add_query_param("metric", "cpu.usage")
            .add_query_param("query", "invalid()")
            .await;

        response.assert_status_bad_request();
    }

    #[tokio::test]
    async fn test_query_endpoint_invalid_metric() {
        let core = create_test_core().await;

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        // Invalid metric name (contains invalid characters)
        let response = server
            .get("/api/metrics/query")
            .add_query_param("metric", "invalid metric!")
            .add_query_param("query", "sum(*)")
            .await;

        response.assert_status_bad_request();
    }

    #[tokio::test]
    async fn test_query_endpoint_unsupported_rate() {
        let core = create_test_core().await;

        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");

        // Rate is not yet supported
        let response = server
            .get("/api/metrics/query")
            .add_query_param("metric", "cpu.usage")
            .add_query_param("query", "rate(*)[5m]")
            .await;

        response.assert_status_bad_request();
        let text = response.text();
        assert!(text.contains("not yet supported"));
    }
}
