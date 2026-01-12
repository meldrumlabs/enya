//! Prometheus metrics for the cloud backend.
//!
//! Exposes operational and business metrics:
//! - HTTP request latency and counts
//! - Database query metrics
//! - WebSocket connection counts
//! - Business metrics (teams, users, messages, etc.)
//! - Error tracking
//!
//! See METRICS.md for full documentation.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::time::Instant;

/// Initialize the Prometheus metrics recorder and return the handle for rendering.
pub fn init_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

/// Middleware to track HTTP request metrics.
pub async fn track_request_metrics(request: Request<Body>, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Normalize path to avoid high cardinality (replace UUIDs with placeholder)
    let normalized_path = normalize_path(&path);

    let response = next.run(request).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    // Record metrics
    histogram!("http_request_duration_seconds", "method" => method.to_string(), "path" => normalized_path.clone(), "status" => status.clone())
        .record(duration);
    counter!("http_requests_total", "method" => method.to_string(), "path" => normalized_path, "status" => status)
        .increment(1);

    response
}

/// Normalize path to reduce cardinality by replacing UUIDs with placeholders.
fn normalize_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let normalized: Vec<String> = parts
        .iter()
        .map(|part| {
            // Check if this looks like a UUID (32 hex chars with optional dashes)
            if is_uuid_like(part) {
                "{id}".to_string()
            } else {
                (*part).to_string()
            }
        })
        .collect();
    normalized.join("/")
}

/// Check if a string looks like a UUID.
fn is_uuid_like(s: &str) -> bool {
    let stripped: String = s.chars().filter(|c| *c != '-').collect();
    stripped.len() == 32 && stripped.chars().all(|c| c.is_ascii_hexdigit())
}

/// Handler for the /metrics endpoint.
pub async fn metrics_handler(State(handle): State<PrometheusHandle>) -> impl IntoResponse {
    match handle.render() {
        output if !output.is_empty() => (StatusCode::OK, output),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to render metrics".to_string(),
        ),
    }
}

// ============================================================================
// Database Metrics
// ============================================================================

/// Record a database query execution.
pub fn record_db_query(operation: &str, duration_secs: f64) {
    histogram!("enya_db_query_duration_seconds", "operation" => operation.to_string())
        .record(duration_secs);
    counter!("enya_db_queries_total", "operation" => operation.to_string()).increment(1);
}

/// Record a database query error.
pub fn record_db_error(operation: &str) {
    counter!("enya_db_errors_total", "operation" => operation.to_string()).increment(1);
}

/// Update the database connection pool metrics.
pub fn set_db_pool_size(active: u32, idle: u32) {
    gauge!("enya_db_pool_connections_active").set(active as f64);
    gauge!("enya_db_pool_connections_idle").set(idle as f64);
}

// ============================================================================
// Authentication Metrics
// ============================================================================

/// Record a successful authentication.
pub fn record_auth_success(provider: &str) {
    counter!("enya_auth_success_total", "provider" => provider.to_string()).increment(1);
}

/// Record a failed authentication.
pub fn record_auth_failure(provider: &str, reason: &str) {
    counter!("enya_auth_failure_total", "provider" => provider.to_string(), "reason" => reason.to_string()).increment(1);
}

// ============================================================================
// Team Metrics
// ============================================================================

/// Record a team creation.
pub fn record_team_created() {
    counter!("enya_teams_created_total").increment(1);
}

/// Record a user joining a team.
pub fn record_user_joined_team() {
    counter!("enya_team_joins_total").increment(1);
}

/// Record a member being removed from a team.
pub fn record_member_removed() {
    counter!("enya_members_removed_total").increment(1);
}

/// Record a member leaving a team.
pub fn record_member_left() {
    counter!("enya_members_left_total").increment(1);
}

/// Record a role change.
pub fn record_role_changed() {
    counter!("enya_role_changes_total").increment(1);
}

// ============================================================================
// Invitation Metrics
// ============================================================================

/// Record an invitation sent.
pub fn record_invitation_sent(invitation_type: &str) {
    counter!("enya_invitations_sent_total", "type" => invitation_type.to_string()).increment(1);
}

/// Record an invitation accepted.
pub fn record_invitation_accepted() {
    counter!("enya_invitations_accepted_total").increment(1);
}

/// Record an invitation revoked.
pub fn record_invitation_revoked() {
    counter!("enya_invitations_revoked_total").increment(1);
}

// ============================================================================
// Collaboration Metrics
// ============================================================================

/// Record a message sent.
pub fn record_message_sent() {
    counter!("enya_messages_sent_total").increment(1);
}

/// Record an annotation created.
pub fn record_annotation_created() {
    counter!("enya_annotations_created_total").increment(1);
}

/// Record an annotation deleted.
pub fn record_annotation_deleted() {
    counter!("enya_annotations_deleted_total").increment(1);
}

/// Record a thread created.
pub fn record_thread_created() {
    counter!("enya_threads_created_total").increment(1);
}

/// Record a thread resolved.
pub fn record_thread_resolved() {
    counter!("enya_threads_resolved_total").increment(1);
}

/// Record a channel created.
pub fn record_channel_created() {
    counter!("enya_channels_created_total").increment(1);
}

// ============================================================================
// WebSocket Metrics
// ============================================================================

/// Update the active WebSocket connections gauge.
pub fn set_websocket_connections(count: usize) {
    gauge!("enya_websocket_connections").set(count as f64);
}

/// Record a WebSocket connection.
pub fn record_websocket_connected() {
    counter!("enya_websocket_connections_total").increment(1);
}

/// Record a WebSocket disconnection.
pub fn record_websocket_disconnected() {
    counter!("enya_websocket_disconnections_total").increment(1);
}

/// Record a real-time event broadcast.
pub fn record_realtime_event(event_type: &str) {
    counter!("enya_realtime_events_total", "type" => event_type.to_string()).increment(1);
}

// ============================================================================
// Per-Team Usage Metrics (for billing)
// ============================================================================

/// Record an API call for a specific team.
pub fn record_team_api_call(team_id: &str) {
    counter!("enya_team_api_calls_total", "team_id" => team_id.to_string()).increment(1);
}

// ============================================================================
// Error Metrics
// ============================================================================

/// Record an error by type.
pub fn record_error(error_type: &str) {
    counter!("enya_errors_total", "type" => error_type.to_string()).increment(1);
}

/// Record an error by type and endpoint.
pub fn record_api_error(error_type: &str, endpoint: &str) {
    counter!("enya_api_errors_total", "type" => error_type.to_string(), "endpoint" => endpoint.to_string()).increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_uuid_like() {
        assert!(is_uuid_like("550e8400-e29b-41d4-a716-446655440000"));
        assert!(is_uuid_like("550e8400e29b41d4a716446655440000"));
        assert!(!is_uuid_like("teams"));
        assert!(!is_uuid_like("123"));
        assert!(!is_uuid_like(""));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(
            normalize_path("/teams/550e8400-e29b-41d4-a716-446655440000/members"),
            "/teams/{id}/members"
        );
        assert_eq!(normalize_path("/health"), "/health");
        assert_eq!(
            normalize_path("/teams/550e8400e29b41d4a716446655440000"),
            "/teams/{id}"
        );
    }
}
