use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{any, get};
use base64::Engine;
use enya_config::{Config, WorkspaceConfig, enya_dir, resolve_workspace_path};
use rust_embed::Embed;

use crate::db::Db;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

/// Maximum request body size for proxied requests (10 MB).
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

#[derive(Embed)]
#[folder = "../editor/dist/"]
struct Assets;

#[derive(Clone)]
struct ServeState {
    /// Upstream Prometheus URL (e.g. "http://localhost:9090")
    upstream_url: String,
    /// API key for Authorization header (optional)
    api_key: Option<String>,
    /// Base64-encoded workspace TOML (with rewritten endpoint)
    workspace_param: String,
    /// HTTP client for proxying
    http_client: reqwest::Client,
    /// SQLite database for persistent daemon state
    db: Arc<Db>,
}

pub fn run(workspace: &str, port: u16, bind: &str, open: bool) -> Result {
    // 1. Load daemon config and workspace
    let daemon_config = Config::load_or_default();
    let path = resolve_workspace_path(workspace);
    let config = WorkspaceConfig::load(&path)
        .map_err(|e| format!("failed to load workspace '{}': {e}", path.display()))?;

    // 2. Extract upstream endpoint (daemon config takes precedence over workspace)
    let upstream_url = if !daemon_config.datasources.prometheus.url.is_empty() {
        daemon_config.datasources.prometheus.url.clone()
    } else {
        config
            .effective_endpoint()
            .ok_or("no metrics endpoint configured (set datasources.prometheus.url in ~/.enya/config.toml or metrics.endpoint in workspace)")?
            .to_string()
    };

    // 3. Extract API key (daemon config takes precedence over workspace)
    let api_key = if !daemon_config.datasources.prometheus.api_key.is_empty() {
        Some(daemon_config.datasources.prometheus.api_key.clone())
    } else {
        let effective = config.effective_metrics();
        if effective.api_key.is_empty() {
            None
        } else {
            Some(effective.api_key.clone())
        }
    };

    // 4. Rewrite workspace endpoint to point to our proxy
    let mut serve_config = config.clone();
    serve_config.metrics.endpoint = format!("http://localhost:{port}/proxy");
    serve_config.workspace.endpoint.clear();
    // Strip API key from the workspace served to the browser
    serve_config.metrics.api_key.clear();

    // 5. Encode workspace as full TOML base64 for URL param
    // We use TOML encoding (not the compact format) because the compact format
    // strips endpoint, sections, and other fields needed for serve to work.
    let toml_str = serve_config
        .to_toml()
        .map_err(|e| format!("failed to encode workspace: {e}"))?;
    let workspace_param =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(toml_str.as_bytes());

    // 6. Open database
    let db_path = enya_dir().join("enya.db");
    let db = Db::open(&db_path).map_err(|e| format!("failed to open database: {e}"))?;

    // 7. Build state
    let state = ServeState {
        upstream_url,
        api_key,
        workspace_param,
        http_client: reqwest::Client::new(),
        db: Arc::new(db),
    };

    // 8. Print startup info
    let url = format!("http://localhost:{port}");
    eprintln!("Serving workspace '{}' at {url}", config.workspace.name);
    eprintln!("Database at {}", db_path.display());
    eprintln!("Proxying Prometheus at {}", state.upstream_url);

    // 9. Start tokio runtime and server
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to create tokio runtime: {e}"))?;

    rt.block_on(async move {
        let app = router(state);
        let addr: std::net::SocketAddr = format!("{bind}:{port}")
            .parse()
            .map_err(|e| format!("invalid bind address: {e}"))?;

        if open {
            let _ = open::that(&url);
        }

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("failed to bind to {addr}: {e}"))?;
        eprintln!("Listening on {addr}");

        axum::serve(listener, app)
            .await
            .map_err(|e| format!("server error: {e}"))?;

        Ok(())
    })
}

fn router(state: ServeState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/proxy/{*path}", any(proxy_handler))
        .fallback(static_handler)
        .with_state(state)
}

/// Redirects `/` to `/index.html?workspace=<base64>` so the WASM app picks up the workspace.
async fn index_handler(State(state): State<ServeState>) -> Redirect {
    Redirect::temporary(&format!("/index.html?workspace={}", state.workspace_param))
}

/// Proxies requests from `/proxy/api/v1/*` to the upstream Prometheus endpoint.
async fn proxy_handler(
    State(state): State<ServeState>,
    Path(path): Path<String>,
    req: Request,
) -> Response {
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let upstream = format!("{}/{}{}", state.upstream_url, path, query);

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
