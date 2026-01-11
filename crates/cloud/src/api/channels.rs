//! Channel and chat thread API routes.

use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db;
use crate::error::ApiError;
use crate::metrics;
use crate::realtime::RealtimeEvent;
use crate::state::AppState;
use enya_team_api::{Channel, ChatThread, Message, NewChannel, NewMessage, NewThread, TeamEvent};

/// List channels for a team.
pub async fn list_channels(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<Channel>>, ApiError> {
    let channels = db::queries::list_channels(&state.db, team_id).await?;
    Ok(Json(channels))
}

/// Create a new channel.
pub async fn create_channel(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
    Json(request): Json<NewChannel>,
) -> Result<Json<Channel>, ApiError> {
    // Verify user is team member
    crate::auth::middleware::require_team_member(&state, team_id, user.id).await?;

    let channel = db::queries::create_channel(&state.db, team_id, &request).await?;

    metrics::record_channel_created();

    // Broadcast channel creation
    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::ChannelCreated {
            channel: channel.clone(),
        },
    ));

    Ok(Json(channel))
}

/// Get a channel by ID.
pub async fn get_channel(
    State(state): State<AppState>,
    _user: AuthUser,
    Path((team_id, channel_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Channel>, ApiError> {
    let channel = db::queries::get_channel(&state.db, channel_id).await?;

    // Verify channel belongs to team
    if channel.team_id != team_id {
        return Err(ApiError::not_found("Channel not found"));
    }

    Ok(Json(channel))
}

/// List threads in a channel.
pub async fn list_threads(
    State(state): State<AppState>,
    _user: AuthUser,
    Path((_team_id, channel_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<ChatThread>>, ApiError> {
    let threads = db::queries::list_channel_threads(&state.db, channel_id).await?;
    Ok(Json(threads))
}

/// Create a new thread in a channel.
pub async fn create_thread(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, channel_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<NewThread>,
) -> Result<Json<ChatThread>, ApiError> {
    // Verify user is team member
    crate::auth::middleware::require_team_member(&state, team_id, user.id).await?;

    let (thread, initial_message) =
        db::queries::create_channel_thread(&state.db, channel_id, user.id, &request).await?;

    metrics::record_thread_created();

    // Get author info for broadcast
    let author = db::queries::get_user(&state.db, user.id).await?;

    // Broadcast thread creation
    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::ThreadCreated {
            thread: thread.clone(),
            initial_message: initial_message.clone().into(),
            author: author.into(),
        },
    ));

    Ok(Json(thread))
}

/// List messages in a channel thread.
pub async fn list_thread_messages(
    State(state): State<AppState>,
    _user: AuthUser,
    Path((_team_id, _channel_id, thread_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<Vec<Message>>, ApiError> {
    let messages = db::queries::list_messages(&state.db, thread_id).await?;
    Ok(Json(messages.into_iter().map(Into::into).collect()))
}

/// Send a message to a channel thread.
pub async fn send_thread_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, _channel_id, thread_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(request): Json<NewMessage>,
) -> Result<Json<Message>, ApiError> {
    // Verify user is team member
    crate::auth::middleware::require_team_member(&state, team_id, user.id).await?;

    let message =
        db::queries::create_message(&state.db, thread_id, user.id, &request.content).await?;
    let author = db::queries::get_user(&state.db, user.id).await?;

    metrics::record_message_sent();

    // Broadcast message
    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::MessageReceived {
            thread_id,
            message: message.clone().into(),
            author: author.into(),
        },
    ));

    Ok(Json(message.into()))
}

/// Resolve/unresolve a thread.
pub async fn resolve_thread(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, channel_id, thread_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<ChatThread>, ApiError> {
    // Verify user is team member
    crate::auth::middleware::require_team_member(&state, team_id, user.id).await?;

    db::queries::update_thread_resolved(&state.db, thread_id, true).await?;

    metrics::record_thread_resolved();

    // Broadcast resolution
    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::ThreadResolved {
            thread_id,
            channel_id,
        },
    ));

    let thread = db::queries::get_channel_thread(&state.db, thread_id).await?;
    Ok(Json(thread))
}
