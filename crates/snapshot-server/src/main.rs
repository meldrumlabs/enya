//! Local snapshot blob server for development testing.
//!
//! Stores snapshot blobs on the local filesystem. Provides upload, download,
//! and metadata inspection endpoints for testing the encode → upload → download
//! → decode flow before deploying to R2.
//!
//! Usage: `cargo run -p enya-snapshot-server`

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use enya_config::workspace::snapshot;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

const DEFAULT_PORT: u16 = 3001;
const STORAGE_DIR: &str = "snapshots";
const ID_LENGTH: usize = 12;

#[derive(Clone)]
struct AppState {
    storage_dir: PathBuf,
    port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "enya_snapshot_server=info,tower_http=info".into()),
        )
        .init();

    let storage_dir = PathBuf::from(STORAGE_DIR);
    if let Err(e) = tokio::fs::create_dir_all(&storage_dir).await {
        tracing::error!("Failed to create storage directory: {e}");
        std::process::exit(1);
    }

    let state = Arc::new(AppState {
        storage_dir,
        port: DEFAULT_PORT,
    });

    let app = Router::new()
        .route("/snapshot", post(upload_snapshot))
        .route("/snapshot/{id}", get(download_snapshot))
        .route("/snapshot/{id}/meta", get(snapshot_meta))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: std::net::SocketAddr = ([127, 0, 0, 1], DEFAULT_PORT).into();
    tracing::info!("Snapshot server listening on http://localhost:{DEFAULT_PORT}");
    tracing::info!("Storage directory: ./{STORAGE_DIR}/");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

/// Upload a snapshot blob. Validates the blob by decoding it, then stores on disk.
async fn upload_snapshot(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if body.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty body").into_response();
    }

    // Validate the blob decodes correctly
    if let Err(e) = snapshot::decode_snapshot(&body) {
        return (StatusCode::BAD_REQUEST, format!("invalid snapshot: {e}")).into_response();
    }

    let id = generate_id();
    let path = state.storage_dir.join(format!("{id}.bin"));

    if let Err(e) = tokio::fs::write(&path, &body).await {
        tracing::error!("Failed to write snapshot {id}: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
    }

    tracing::info!("Stored snapshot {id} ({} bytes)", body.len());

    Json(serde_json::json!({
        "id": id,
        "url": format!("http://localhost:{}/snapshot/{}", state.port, id),
        "bytes": body.len(),
    }))
    .into_response()
}

/// Download a snapshot blob by ID.
async fn download_snapshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !is_valid_id(&id) {
        return (StatusCode::BAD_REQUEST, "invalid id").into_response();
    }

    let path = state.storage_dir.join(format!("{id}.bin"));
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "snapshot not found").into_response(),
    }
}

/// Return decoded metadata about a snapshot (useful for debugging).
async fn snapshot_meta(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !is_valid_id(&id) {
        return (StatusCode::BAD_REQUEST, "invalid id").into_response();
    }

    let path = state.storage_dir.join(format!("{id}.bin"));
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::NOT_FOUND, "snapshot not found").into_response(),
    };

    let decoded = match snapshot::decode_snapshot(&bytes) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("decode failed: {e}"),
            )
                .into_response();
        }
    };

    let message_count = decoded
        .conversation
        .as_ref()
        .map_or(0, |c| c.messages.len());

    Json(serde_json::json!({
        "id": id,
        "workspace_name": decoded.workspace.workspace.name,
        "pane_count": decoded.workspace.panes.len(),
        "captured_at": decoded.captured_at,
        "has_conversation": decoded.conversation.is_some(),
        "message_count": message_count,
        "blob_bytes": bytes.len(),
    }))
    .into_response()
}

fn generate_id() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    (0..ID_LENGTH)
        .map(|_| CHARS[fastrand::usize(..CHARS.len())] as char)
        .collect()
}

fn is_valid_id(id: &str) -> bool {
    id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
    tracing::info!("Shutting down...");
}
