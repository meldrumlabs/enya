//! Thread and message API routes.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::responses::{FullThreadResponse, MessageResponse, ThreadResponseBuilder};
use crate::auth::AuthUser;
use crate::db;
use crate::error::ApiError;
use crate::metrics;
use crate::realtime::RealtimeEvent;
use crate::state::AppState;
use enya_team_api::TeamEvent;

/// Update thread request.
#[derive(Debug, Deserialize)]
pub struct UpdateThreadRequest {
    pub resolved: Option<bool>,
}

/// Send message request.
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
}

/// Helper to fetch thread data and build response.
async fn fetch_thread_response(
    state: &AppState,
    thread_id: Uuid,
) -> Result<FullThreadResponse, ApiError> {
    let thread = db::queries::get_thread(&state.db, thread_id).await?;
    let messages = db::queries::list_messages(&state.db, thread_id).await?;
    let participants = db::queries::get_thread_participants(&state.db, thread_id).await?;

    Ok(ThreadResponseBuilder {
        thread,
        messages,
        participants,
    }
    .build_full())
}

/// Get a thread with messages.
pub async fn get_thread(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(thread_id): Path<Uuid>,
) -> Result<Json<FullThreadResponse>, ApiError> {
    // TODO: Verify user has access to thread (via team membership)
    Ok(Json(fetch_thread_response(&state, thread_id).await?))
}

/// Update a thread (e.g., mark as resolved).
pub async fn update_thread(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(thread_id): Path<Uuid>,
    Json(request): Json<UpdateThreadRequest>,
) -> Result<Json<FullThreadResponse>, ApiError> {
    // TODO: Verify user has access to thread

    if let Some(resolved) = request.resolved {
        db::queries::update_thread_resolved(&state.db, thread_id, resolved).await?;
    }

    Ok(Json(fetch_thread_response(&state, thread_id).await?))
}

/// List messages in a thread.
pub async fn list_messages(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(thread_id): Path<Uuid>,
) -> Result<Json<Vec<MessageResponse>>, ApiError> {
    // TODO: Verify user has access to thread

    let messages = db::queries::list_messages(&state.db, thread_id).await?;
    Ok(Json(messages.into_iter().map(Into::into).collect()))
}

/// Send a message to a thread.
pub async fn send_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(thread_id): Path<Uuid>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    // TODO: Verify user has access to thread

    let message =
        db::queries::create_message(&state.db, thread_id, user.id, &request.content).await?;

    metrics::record_message_sent();

    // Get team_id for broadcasting (need to look up via annotation)
    let thread = db::queries::get_thread(&state.db, thread_id).await?;
    if let Some(annotation_id) = thread.annotation_id {
        let annotation = db::queries::get_annotation(&state.db, annotation_id).await?;
        let author = db::queries::get_user(&state.db, user.id).await?;

        // Broadcast new message
        state.broadcast(RealtimeEvent::broadcast(
            annotation.team_id,
            TeamEvent::MessageReceived {
                thread_id,
                message: message.clone().into(),
                author: author.clone().into(),
            },
        ));

        // Send @mention notifications
        for mentioned_id in &message.mentions {
            if *mentioned_id != user.id {
                state.broadcast(RealtimeEvent::targeted(
                    annotation.team_id,
                    vec![*mentioned_id],
                    TeamEvent::Mentioned {
                        message: message.clone().into(),
                        author: author.clone().into(),
                        thread_id,
                    },
                ));
            }
        }
    }

    Ok(Json(message.into()))
}
