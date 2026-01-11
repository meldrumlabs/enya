//! API routes.

pub mod annotations;
pub mod audit;
pub mod auth;
pub mod channels;
pub mod invitations;
pub mod responses;
pub mod teams;
pub mod threads;

use axum::{
    Router, middleware,
    routing::{delete, get, patch, post},
};
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::metrics;
use crate::realtime;
use crate::state::AppState;

/// Build the API router.
pub fn router(state: AppState, metrics_handle: PrometheusHandle) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(health))
        // Metrics endpoint (Prometheus format)
        .route("/metrics", get(metrics::metrics_handler))
        .with_state(metrics_handle)
        .merge(api_routes(state, cors))
}

/// Internal API routes.
fn api_routes(state: AppState, cors: CorsLayer) -> Router {
    Router::new()
        // Auth routes
        .route("/auth/github", get(auth::github_login))
        .route("/auth/github/callback", post(auth::github_callback))
        .route("/auth/me", get(auth::get_current_user))
        .route("/auth/dev", post(auth::dev_login)) // Dev-only: requires DEV_AUTH=true
        // Team routes
        .route("/teams", get(teams::list_teams))
        .route("/teams", post(teams::create_team))
        .route("/teams/{team_id}", get(teams::get_team))
        .route("/teams/{team_id}/members", get(teams::list_members))
        .route(
            "/teams/{team_id}/members/roles",
            get(teams::list_members_with_roles),
        )
        .route(
            "/teams/{team_id}/members/{member_id}",
            delete(teams::remove_member),
        )
        .route(
            "/teams/{team_id}/members/{member_id}/role",
            patch(teams::update_member_role),
        )
        .route("/teams/{team_id}/leave", post(teams::leave_team))
        // Invitation routes
        .route(
            "/teams/{team_id}/invitations",
            get(invitations::list_invitations).post(invitations::create_invitation),
        )
        .route(
            "/teams/{team_id}/invitations/{invitation_id}",
            delete(invitations::delete_invitation),
        )
        .route("/invitations/accept", post(invitations::accept_invitation))
        .route(
            "/invitations/{token}",
            get(invitations::get_invitation_info),
        )
        // Audit log routes
        .route("/teams/{team_id}/audit-logs", get(audit::list_audit_logs))
        // Annotation routes
        .route(
            "/teams/{team_id}/annotations",
            get(annotations::list_annotations),
        )
        .route(
            "/teams/{team_id}/annotations",
            post(annotations::create_annotation),
        )
        .route(
            "/teams/{team_id}/annotations/{annotation_id}",
            delete(annotations::delete_annotation),
        )
        // Thread/message routes (annotation threads)
        .route("/threads/{thread_id}", get(threads::get_thread))
        .route("/threads/{thread_id}", patch(threads::update_thread))
        .route("/threads/{thread_id}/messages", get(threads::list_messages))
        .route("/threads/{thread_id}/messages", post(threads::send_message))
        // Channel routes
        .route(
            "/teams/{team_id}/channels",
            get(channels::list_channels).post(channels::create_channel),
        )
        .route(
            "/teams/{team_id}/channels/{channel_id}",
            get(channels::get_channel),
        )
        .route(
            "/teams/{team_id}/channels/{channel_id}/threads",
            get(channels::list_threads).post(channels::create_thread),
        )
        .route(
            "/teams/{team_id}/channels/{channel_id}/threads/{thread_id}/messages",
            get(channels::list_thread_messages).post(channels::send_thread_message),
        )
        .route(
            "/teams/{team_id}/channels/{channel_id}/threads/{thread_id}/resolve",
            post(channels::resolve_thread),
        )
        // War room
        .route("/teams/{team_id}/war-room/share", post(teams::share_view))
        // WebSocket
        .route("/realtime", get(realtime::ws_handler))
        // Middleware
        .layer(middleware::from_fn(metrics::track_request_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}
