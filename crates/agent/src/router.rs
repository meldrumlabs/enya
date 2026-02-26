//! HTTP server and API routes for the Enya agent.
//!
//! Combines the JSON API endpoints, Prometheus proxy, and embedded
//! WASM asset serving into a single Axum router.

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use base64::Engine;
use enya_config::{Config, WorkspaceConfig, enya_dir, resolve_workspace_path};
use rust_embed::Embed;
use serde::Deserialize;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::db::{Db, NewWatch};

type Result = std::result::Result<(), crate::Error>;

/// Maximum request body size for proxied requests (10 MB).
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Embed)]
#[folder = "../editor/dist/"]
struct Assets;

/// Shared state for all Axum handlers.
#[derive(Clone)]
pub(crate) struct ServeState {
    /// Upstream Prometheus URL (e.g. "http://localhost:9090"), None if not configured
    upstream_url: Option<String>,
    /// API key for Authorization header (optional)
    api_key: Option<String>,
    /// Base64-encoded workspace TOML for WASM UI redirect (None if no workspace)
    workspace_param: Option<String>,
    /// HTTP client for proxying
    http_client: reqwest::Client,
    /// SQLite database for persistent agent state
    db: Arc<Db>,
    /// In-memory OTLP telemetry store (None if OTLP receiver is disabled).
    telemetry_store: Option<Arc<enya_client::otlp::TelemetryStore>>,
}

// -- Server startup --

pub fn run(workspace: Option<&str>, port: u16, bind: &str, open: bool) -> Result {
    // 1. Load config
    let config = Config::load_or_default();

    // 2. Extract upstream endpoint (optional — proxy won't work without it)
    let upstream_url = if !config.datasources.prometheus.url.is_empty() {
        Some(config.datasources.prometheus.url.clone())
    } else {
        // Fall back to workspace endpoint if a workspace was provided
        workspace.and_then(|ws| {
            let path = resolve_workspace_path(ws);
            WorkspaceConfig::load(&path)
                .ok()
                .and_then(|c| c.effective_endpoint().map(|s| s.to_string()))
        })
    };

    // 3. Extract API key
    let api_key = if !config.datasources.prometheus.api_key.is_empty() {
        Some(config.datasources.prometheus.api_key.clone())
    } else {
        None
    };

    // 4. Optionally encode workspace for WASM UI redirect
    let workspace_param = if let Some(ws) = workspace {
        let path = resolve_workspace_path(ws);
        let config = WorkspaceConfig::load(&path).map_err(|e| {
            crate::Error::Config(format!(
                "failed to load workspace '{}': {e}",
                path.display()
            ))
        })?;

        let mut serve_config = config;
        serve_config.metrics.endpoint = format!("http://localhost:{port}/proxy");
        serve_config.workspace.endpoint.clear();
        serve_config.metrics.api_key.clear();

        let toml_str = serve_config
            .to_toml()
            .map_err(|e| crate::Error::Config(format!("failed to encode workspace: {e}")))?;
        Some(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(toml_str.as_bytes()))
    } else {
        None
    };

    // 5. Open database
    let db_path = enya_dir().join("enya.db");
    let db = Db::open(&db_path)
        .map_err(|e| crate::Error::Config(format!("failed to open database: {e}")))?;

    // 5b. Create OTLP telemetry store if enabled
    let telemetry_store = if config.otlp.enabled {
        info!(
            max_traces = config.otlp.max_traces,
            max_log_entries = config.otlp.max_log_entries,
            "OTLP receiver enabled"
        );
        Some(enya_client::otlp::TelemetryStore::new(
            enya_client::otlp::StoreConfig {
                max_traces: config.otlp.max_traces,
                max_log_entries: config.otlp.max_log_entries,
            },
        ))
    } else {
        None
    };

    // 6. Build state
    let state = ServeState {
        upstream_url,
        api_key,
        workspace_param,
        http_client: reqwest::Client::new(),
        db: Arc::new(db),
        telemetry_store,
    };

    // 7. Log startup info
    let url = format!("http://localhost:{port}");
    info!(url = %url, "enya agent starting");
    info!(path = %db_path.display(), "database opened");
    if let Some(ref upstream) = state.upstream_url {
        info!(upstream = %upstream, "proxying prometheus");
    }
    if workspace.is_some() {
        info!(url = %url, "workspace UI available");
    }

    // 9. Start tokio runtime and server
    let rt = tokio::runtime::Runtime::new().map_err(crate::Error::Io)?;

    rt.block_on(async move {
        // Create shutdown broadcast channel
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);

        // Spawn watch engine as a background task
        let engine_db = state.db.clone();
        tokio::spawn(crate::engine::run(engine_db, shutdown_tx.subscribe()));

        let app = router(state);
        let addr: std::net::SocketAddr = format!("{bind}:{port}")
            .parse()
            .map_err(|e| crate::Error::Config(format!("invalid bind address: {e}")))?;

        if open {
            let _ = open::that(&url);
        }

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(crate::Error::Io)?;
        info!(addr = %addr, "listening");

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = tokio::signal::ctrl_c().await;
                info!("shutdown signal received");
                let _ = shutdown_tx.send(());
            })
            .await
            .map_err(crate::Error::Io)?;

        Ok(())
    })
}

fn router(state: ServeState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/proxy/{*path}", any(proxy_handler))
        // API v1
        .route("/api/v1/status", get(status_handler))
        .route("/api/v1/watches", get(list_watches).post(create_watch))
        .route("/api/v1/watches/{id}", get(get_watch).delete(delete_watch))
        .route("/api/v1/watches/{id}/events", get(watch_events))
        .route("/api/v1/workspaces", get(list_workspaces_handler))
        // OTLP receiver endpoints (ingest)
        .route("/v1/traces", post(otlp_traces_handler))
        .route("/v1/logs", post(otlp_logs_handler))
        // OTLP query endpoints (read from store)
        .route("/api/otlp/traces/search", get(otlp_search_traces_handler))
        .route("/api/otlp/traces/{trace_id}", get(otlp_get_trace_handler))
        .route("/api/otlp/logs/query", get(otlp_query_logs_handler))
        .route("/api/otlp/labels", get(otlp_labels_handler))
        .route("/api/otlp/health", get(otlp_health_handler))
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// -- Serve handlers --

/// Redirects `/` to `/index.html`, optionally with a workspace param.
async fn index_handler(State(state): State<ServeState>) -> Redirect {
    match &state.workspace_param {
        Some(param) => Redirect::temporary(&format!("/index.html?workspace={param}")),
        None => Redirect::temporary("/index.html"),
    }
}

/// Proxies requests from `/proxy/*` to the upstream Prometheus endpoint.
async fn proxy_handler(
    State(state): State<ServeState>,
    Path(path): Path<String>,
    req: Request,
) -> Response {
    let Some(ref upstream_url) = state.upstream_url else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "no upstream endpoint configured (set datasources.prometheus.url in ~/.enya/config.toml)",
        );
    };

    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let upstream = format!("{upstream_url}/{path}{query}");

    let method = req.method().clone();
    let body = match axum::body::to_bytes(req.into_body(), MAX_BODY_SIZE).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read request body: {e}"),
            )
                .into_response();
        }
    };

    let mut builder = state.http_client.request(method, &upstream);
    if let Some(ref key) = state.api_key {
        builder = builder.header("Authorization", format!("Bearer {key}"));
    }
    if !body.is_empty() {
        builder = builder.header("Content-Type", "application/x-www-form-urlencoded");
        builder = builder.body(body);
    }

    match builder.send().await {
        Ok(response) => {
            let status = StatusCode::from_u16(response.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json")
                .to_string();
            let resp_body = response.bytes().await.unwrap_or_default();
            (status, [(header::CONTENT_TYPE, content_type)], resp_body).into_response()
        }
        Err(e) => {
            warn!(error = %e, upstream = %upstream, "proxy request failed");
            let body = serde_json::json!({"error": format!("proxy error: {e}")});
            (
                StatusCode::BAD_GATEWAY,
                [(header::CONTENT_TYPE, "application/json".to_string())],
                serde_json::to_vec(&body).unwrap_or_default(),
            )
                .into_response()
        }
    }
}

/// Serves embedded WASM assets (index.html, JS, WASM blob, fonts, etc.)
async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    // Preserve query params when serving index.html
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            let body = content.data.to_vec();

            // Set appropriate cache headers for immutable assets
            let cache = if path.ends_with(".wasm") || path.ends_with(".js") {
                "public, max-age=86400"
            } else {
                "public, max-age=3600"
            };

            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, cache.to_string()),
                ],
                body,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// -- API request / query types --

#[derive(Deserialize)]
struct CreateWatchRequest {
    name: String,
    expression: String,
    threshold_op: String,
    threshold_value: f64,
    #[serde(default = "default_interval")]
    interval_secs: u32,
    sustain_secs: Option<u32>,
    endpoint: Option<String>,
}

fn default_interval() -> u32 {
    30
}

#[derive(Deserialize)]
struct EventsQuery {
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    50
}

// -- API handlers --

/// GET /api/v1/status — agent version and datasource info.
async fn status_handler(State(state): State<ServeState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "upstream": state.upstream_url.as_deref().unwrap_or(""),
    }))
}

/// GET /api/v1/watches — list all enabled watches.
async fn list_watches(State(state): State<ServeState>) -> impl IntoResponse {
    match state.db.list_watches() {
        Ok(watches) => Json(serde_json::json!(watches)).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// POST /api/v1/watches — create a new watch.
async fn create_watch(
    State(state): State<ServeState>,
    Json(body): Json<CreateWatchRequest>,
) -> impl IntoResponse {
    // Validate threshold_op
    if body.threshold_op != "above" && body.threshold_op != "below" {
        return error_response(
            StatusCode::BAD_REQUEST,
            "threshold_op must be 'above' or 'below'",
        );
    }

    let endpoint = match body.endpoint.as_deref().or(state.upstream_url.as_deref()) {
        Some(ep) => ep,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "endpoint is required (no default upstream configured)",
            );
        }
    };

    let new_watch = NewWatch {
        name: &body.name,
        expression: &body.expression,
        threshold_op: &body.threshold_op,
        threshold_value: body.threshold_value,
        interval_secs: body.interval_secs,
        sustain_secs: body.sustain_secs,
        endpoint,
    };

    match state.db.insert_watch(&new_watch) {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// GET /api/v1/watches/:id — get a single watch.
async fn get_watch(State(state): State<ServeState>, Path(id): Path<i64>) -> impl IntoResponse {
    match state.db.get_watch(id) {
        Ok(Some(watch)) => Json(serde_json::json!(watch)).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "watch not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// DELETE /api/v1/watches/:id — disable a watch (soft delete).
async fn delete_watch(State(state): State<ServeState>, Path(id): Path<i64>) -> impl IntoResponse {
    match state.db.disable_watch(id) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "watch not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// GET /api/v1/watches/:id/events — recent events for a watch.
async fn watch_events(
    State(state): State<ServeState>,
    Path(id): Path<i64>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    // Verify the watch exists first
    match state.db.get_watch(id) {
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "watch not found"),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        Ok(Some(_)) => {}
    }

    match state.db.recent_events(id, query.limit) {
        Ok(events) => Json(serde_json::json!(events)).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// GET /api/v1/workspaces — list available workspace files.
async fn list_workspaces_handler() -> impl IntoResponse {
    let workspaces = enya_config::list_workspaces();
    let items: Vec<_> = workspaces
        .into_iter()
        .map(|(name, description)| {
            serde_json::json!({
                "name": name,
                "description": description,
            })
        })
        .collect();
    Json(serde_json::json!(items))
}

// -- OTLP receiver handlers --

/// POST /v1/traces — accept OTLP trace data.
async fn otlp_traces_handler(State(state): State<ServeState>, body: axum::body::Bytes) -> Response {
    let Some(ref store) = state.telemetry_store else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "OTLP receiver not enabled (set otlp.enabled = true in ~/.enya/config.toml)",
        );
    };

    match enya_client::otlp::ingest::ingest_traces(store, &body) {
        Ok(count) => {
            tracing::debug!(spans = count, "ingested OTLP traces");
            Json(serde_json::json!({ "accepted_spans": count })).into_response()
        }
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

/// POST /v1/logs — accept OTLP log data.
async fn otlp_logs_handler(State(state): State<ServeState>, body: axum::body::Bytes) -> Response {
    let Some(ref store) = state.telemetry_store else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "OTLP receiver not enabled (set otlp.enabled = true in ~/.enya/config.toml)",
        );
    };

    match enya_client::otlp::ingest::ingest_logs(store, &body) {
        Ok(count) => {
            tracing::debug!(entries = count, "ingested OTLP logs");
            Json(serde_json::json!({ "accepted_log_entries": count })).into_response()
        }
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// -- OTLP query endpoints (read from TelemetryStore) --

/// Query parameters for trace search.
#[derive(Deserialize)]
struct OtlpTraceSearchQuery {
    #[serde(default)]
    service_name: Option<String>,
    #[serde(default)]
    operation_name: Option<String>,
    #[serde(default)]
    min_duration_ms: Option<u64>,
    #[serde(default)]
    max_duration_ms: Option<u64>,
    #[serde(default = "default_trace_limit")]
    limit: usize,
    #[serde(default)]
    start_time_secs: Option<u64>,
    #[serde(default)]
    end_time_secs: Option<u64>,
}

fn default_trace_limit() -> usize {
    20
}

/// Query parameters for log queries.
#[derive(Deserialize)]
struct OtlpLogsQueryParams {
    #[serde(default)]
    start_ns: Option<i64>,
    #[serde(default)]
    end_ns: Option<i64>,
    #[serde(default)]
    contains: Option<String>,
    #[serde(default = "default_logs_limit")]
    limit: usize,
    /// Labels as a JSON-encoded object (e.g., `{"service":"api"}`)
    #[serde(default)]
    labels: Option<String>,
}

fn default_logs_limit() -> usize {
    1000
}

/// GET /api/otlp/traces/search — search traces in the OTLP store.
async fn otlp_search_traces_handler(
    State(state): State<ServeState>,
    Query(params): Query<OtlpTraceSearchQuery>,
) -> Response {
    let Some(ref store) = state.telemetry_store else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "OTLP store not enabled");
    };

    // Convert ms → us for the store's search API
    let min_duration_us = params.min_duration_ms.map(|ms| ms * 1000);
    let max_duration_us = params.max_duration_ms.map(|ms| ms * 1000);
    // Convert seconds → us for time range
    let start_time_us = params.start_time_secs.map(|s| s * 1_000_000);
    let end_time_us = params.end_time_secs.map(|s| s * 1_000_000);

    let summaries = store.search_traces(
        params.service_name.as_deref(),
        params.operation_name.as_deref(),
        min_duration_us,
        max_duration_us,
        start_time_us,
        end_time_us,
        params.limit,
    );

    Json(summaries).into_response()
}

/// GET /api/otlp/traces/{trace_id} — get a trace by ID.
async fn otlp_get_trace_handler(
    State(state): State<ServeState>,
    Path(trace_id): Path<String>,
) -> Response {
    let Some(ref store) = state.telemetry_store else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "OTLP store not enabled");
    };

    match store.get_trace(&trace_id) {
        Some(trace) => Json(trace).into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Trace {trace_id} not found"),
        ),
    }
}

/// GET /api/otlp/logs/query — query logs from the OTLP store.
async fn otlp_query_logs_handler(
    State(state): State<ServeState>,
    Query(params): Query<OtlpLogsQueryParams>,
) -> Response {
    let Some(ref store) = state.telemetry_store else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "OTLP store not enabled");
    };

    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64;
    let start_ns = params.start_ns.unwrap_or(now_ns - 3_600_000_000_000); // default: 1h ago
    let end_ns = params.end_ns.unwrap_or(now_ns);

    // Parse labels from JSON string
    let labels: rustc_hash::FxHashMap<String, String> = params
        .labels
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let entries = store.query_logs(
        start_ns,
        end_ns,
        &labels,
        params.contains.as_deref(),
        params.limit,
    );
    let streams_count = {
        let mut services = std::collections::HashSet::new();
        for entry in &entries {
            if let Some(svc) = entry.labels.get("service") {
                services.insert(svc.clone());
            }
        }
        services.len().max(1)
    };

    Json(enya_client::logs::LogsResponse {
        entries,
        streams_count,
    })
    .into_response()
}

/// GET /api/otlp/labels — list known log labels.
async fn otlp_labels_handler(State(state): State<ServeState>) -> Response {
    let Some(ref store) = state.telemetry_store else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "OTLP store not enabled");
    };

    Json(store.known_log_labels()).into_response()
}

/// GET /api/otlp/health — health check for the OTLP store.
async fn otlp_health_handler(State(state): State<ServeState>) -> Response {
    let Some(ref store) = state.telemetry_store else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "OTLP store not enabled");
    };

    Json(enya_client::BackendInfo {
        backend_type: "otlp".to_string(),
        version: format!(
            "in-memory ({} traces, {} logs)",
            store.trace_count(),
            store.log_count()
        ),
    })
    .into_response()
}

/// Build a JSON error response.
fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}
