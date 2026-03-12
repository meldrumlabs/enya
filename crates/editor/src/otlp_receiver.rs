//! Embedded OTLP HTTP receiver for the native editor.
//!
//! Starts a lightweight HTTP server on `localhost:4318` (the standard OTLP HTTP
//! port) that accepts OpenTelemetry data directly from instrumented applications.
//! Data is written to a shared [`TelemetryStore`] that the editor reads from
//! via in-process clients.
//!
//! This lets developers point their OTel SDKs at Enya without needing separate
//! Prometheus/Loki/Tempo infrastructure.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use enya_client::otlp::TelemetryStore;

/// Default port for the embedded OTLP receiver.
pub const OTLP_PORT: u16 = 4318;

/// Maximum request body size (10 MB).
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Start the embedded OTLP receiver in the background.
///
/// Binds to `127.0.0.1:{port}` and serves the standard OTLP HTTP endpoints.
/// Returns immediately; the server runs on the provided tokio handle.
pub fn start(store: Arc<TelemetryStore>, handle: &tokio::runtime::Handle, port: u16) {
    let app = Router::new()
        .route("/v1/traces", post(traces_handler))
        .route("/v1/logs", post(logs_handler))
        .route("/v1/metrics", post(metrics_handler))
        .with_state(store);

    handle.spawn(async move {
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                log::info!("OTLP receiver listening on http://{addr}");
                if let Err(e) = axum::serve(listener, app).await {
                    log::error!("OTLP receiver error: {e}");
                }
            }
            Err(e) => {
                log::warn!("Could not start OTLP receiver on {addr}: {e}");
            }
        }
    });
}

/// Returns true if the Content-Type header indicates protobuf.
fn is_protobuf(req: &axum::extract::Request) -> bool {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| {
            ct.contains("application/x-protobuf") || ct.contains("application/protobuf")
        })
}

async fn traces_handler(
    State(store): State<Arc<TelemetryStore>>,
    req: axum::extract::Request,
) -> Response {
    let proto = is_protobuf(&req);
    let body = match axum::body::to_bytes(req.into_body(), MAX_BODY_SIZE).await {
        Ok(b) => b,
        Err(e) => return error_response(&format!("failed to read body: {e}")),
    };

    let result = if proto {
        enya_client::otlp::ingest::ingest_traces_proto(&store, &body)
    } else {
        enya_client::otlp::ingest::ingest_traces(&store, &body)
    };

    match result {
        Ok(count) => Json(serde_json::json!({ "accepted_spans": count })).into_response(),
        Err(e) => error_response(&e.to_string()),
    }
}

async fn logs_handler(
    State(store): State<Arc<TelemetryStore>>,
    req: axum::extract::Request,
) -> Response {
    let proto = is_protobuf(&req);
    let body = match axum::body::to_bytes(req.into_body(), MAX_BODY_SIZE).await {
        Ok(b) => b,
        Err(e) => return error_response(&format!("failed to read body: {e}")),
    };

    let result = if proto {
        enya_client::otlp::ingest::ingest_logs_proto(&store, &body)
    } else {
        enya_client::otlp::ingest::ingest_logs(&store, &body)
    };

    match result {
        Ok(count) => Json(serde_json::json!({ "accepted_log_entries": count })).into_response(),
        Err(e) => error_response(&e.to_string()),
    }
}

async fn metrics_handler(
    State(store): State<Arc<TelemetryStore>>,
    req: axum::extract::Request,
) -> Response {
    let proto = is_protobuf(&req);
    let body = match axum::body::to_bytes(req.into_body(), MAX_BODY_SIZE).await {
        Ok(b) => b,
        Err(e) => return error_response(&format!("failed to read body: {e}")),
    };

    let result = if proto {
        enya_client::otlp::ingest::ingest_metrics_proto(&store, &body)
    } else {
        enya_client::otlp::ingest::ingest_metrics(&store, &body)
    };

    match result {
        Ok(count) => Json(serde_json::json!({ "accepted_data_points": count })).into_response(),
        Err(e) => error_response(&e.to_string()),
    }
}

fn error_response(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}
