//! API routes.

pub mod annotations;
pub mod auth;
pub mod channels;
pub mod responses;
pub mod teams;
pub mod threads;

use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::realtime;
use crate::state::AppState;

/// Build the API router.
pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(health))
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
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
}
