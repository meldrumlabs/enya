//! Annotation API routes.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::responses::{AnnotationResponse, ThreadResponse, ThreadResponseBuilder};
use crate::auth::{AuthUser, middleware::require_team_member};
use crate::db;
use crate::error::ApiError;
use crate::realtime::RealtimeEvent;
use crate::state::AppState;
use enya_team_api::{Annotation, NewAnnotation, TeamEvent, Thread};

/// Query parameters for listing annotations.
#[derive(Debug, Deserialize)]
pub struct ListAnnotationsQuery {
    /// Query fingerprint to filter by.
    pub query_fp: String,
}

/// List annotations for a query.
pub async fn list_annotations(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
    Query(query): Query<ListAnnotationsQuery>,
) -> Result<Json<Vec<AnnotationResponse>>, ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    let annotations = db::queries::list_annotations(&state.db, team_id, &query.query_fp).await?;

    let mut result = Vec::with_capacity(annotations.len());
    for annotation in annotations {
        let thread = db::queries::get_thread(&state.db, annotation.thread_id).await?;
        let messages = db::queries::list_messages(&state.db, annotation.thread_id).await?;
        let participants =
            db::queries::get_thread_participants(&state.db, annotation.thread_id).await?;

        result.push(AnnotationResponse {
            id: annotation.id,
            pane_id: annotation.pane_id,
            query_fingerprint: annotation.query_fingerprint,
            timestamp_ns: annotation.timestamp_ns,
            created_by: annotation.created_by,
            created_at: annotation.created_at.timestamp() as u64,
            thread: ThreadResponseBuilder {
                thread,
                messages,
                participants,
            }
            .build(),
        });
    }

    Ok(Json(result))
}

/// Create a new annotation.
pub async fn create_annotation(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
    Json(request): Json<NewAnnotation>,
) -> Result<Json<AnnotationResponse>, ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    let (annotation, thread, message) = db::queries::create_annotation(
        &state.db,
        team_id,
        &request.pane_id,
        &request.query_fingerprint,
        request.timestamp_ns,
        user.id,
        &request.initial_message,
    )
    .await?;

    let response = AnnotationResponse {
        id: annotation.id,
        pane_id: annotation.pane_id.clone(),
        query_fingerprint: annotation.query_fingerprint.clone(),
        timestamp_ns: annotation.timestamp_ns,
        created_by: annotation.created_by,
        created_at: annotation.created_at.timestamp() as u64,
        thread: ThreadResponse {
            id: thread.id,
            messages: vec![message.clone().into()],
            participants: vec![user.id],
            resolved: false,
        },
    };

    // Broadcast event
    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::AnnotationCreated {
            annotation: Annotation {
                id: annotation.id,
                pane_id: annotation.pane_id,
                query_fingerprint: annotation.query_fingerprint,
                timestamp_ns: annotation.timestamp_ns,
                created_by: annotation.created_by,
                created_at: annotation.created_at.timestamp() as u64,
                thread: Thread {
                    id: thread.id,
                    messages: vec![message.into()],
                    participants: vec![user.id],
                    resolved: false,
                },
            },
        },
    ));

    Ok(Json(response))
}

/// Delete an annotation.
pub async fn delete_annotation(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, annotation_id)): Path<(Uuid, Uuid)>,
) -> Result<(), ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    let annotation = db::queries::get_annotation(&state.db, annotation_id).await?;
    if annotation.team_id != team_id {
        return Err(ApiError::not_found("Annotation not found"));
    }

    // Only creator can delete (in production, also check for admin role)
    if annotation.created_by != user.id {
        return Err(ApiError::forbidden(
            "Only the creator can delete this annotation",
        ));
    }

    db::queries::delete_annotation(&state.db, annotation_id).await?;

    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::AnnotationDeleted { id: annotation_id },
    ));

    Ok(())
}
