//! HTTP server and API routes for the Enya agent.
//!
//! Combines the JSON API endpoints, Prometheus proxy, and embedded
//! WASM asset serving into a single Axum router.

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get};
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

    // 6. Build state
    let state = ServeState {
        upstream_url,
        api_key,
        workspace_param,
        http_client: reqwest::Client::new(),
        db: Arc::new(db),
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
        // Spawn watch engine as a background task
        let engine_db = state.db.clone();
        tokio::spawn(crate::engine::run(engine_db));

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

        axum::serve(listener, app).await.map_err(crate::Error::Io)?;

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

/// Build a JSON error response.
fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}
