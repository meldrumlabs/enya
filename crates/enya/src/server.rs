//! Axum-based server for hosting Enya endpoints

// v1/api/search/
// v1/api/metrics/
// v1/api/memory/
// v1/api/cpu/

use super::core::Core;
use crate::util::value_as_f64;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use std::net::SocketAddr;
use talna::MetricName;

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
    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/metrics/preview", get(metrics_preview_handler))
        .with_state(core)
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
