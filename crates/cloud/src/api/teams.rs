//! Team API routes.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::responses::{MemberResponse, TeamResponse};
use crate::auth::{AuthUser, middleware::require_team_member};
use crate::db;
use crate::error::ApiError;
use crate::realtime::RealtimeEvent;
use crate::state::AppState;
use enya_team_api::TeamEvent;

/// Create team request.
#[derive(Debug, Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
}

/// Share view request.
#[derive(Debug, Deserialize)]
pub struct ShareViewRequest {
    pub workspace_url: String,
}

/// List teams for the current user.
pub async fn list_teams(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<TeamResponse>>, ApiError> {
    let teams = db::queries::get_user_teams(&state.db, user.id).await?;

    let mut result = Vec::with_capacity(teams.len());
    for team in teams {
        let members = db::queries::get_team_members(&state.db, team.id).await?;
        result.push(TeamResponse {
            id: team.id,
            name: team.name,
            members: members.into_iter().map(Into::into).collect(),
        });
    }

    Ok(Json(result))
}

/// Create a new team.
pub async fn create_team(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<CreateTeamRequest>,
) -> Result<Json<TeamResponse>, ApiError> {
    let team = db::queries::create_team(&state.db, &request.name, user.id).await?;
    let members = db::queries::get_team_members(&state.db, team.id).await?;

    Ok(Json(TeamResponse {
        id: team.id,
        name: team.name,
        members: members.into_iter().map(Into::into).collect(),
    }))
}

/// Get a specific team.
pub async fn get_team(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
) -> Result<Json<TeamResponse>, ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    let team = db::queries::get_team(&state.db, team_id).await?;
    let members = db::queries::get_team_members(&state.db, team_id).await?;

    Ok(Json(TeamResponse {
        id: team.id,
        name: team.name,
        members: members.into_iter().map(Into::into).collect(),
    }))
}

/// List team members.
pub async fn list_members(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<MemberResponse>>, ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    let members = db::queries::get_team_members(&state.db, team_id).await?;

    Ok(Json(members.into_iter().map(Into::into).collect()))
}

/// Share current view with team (war room).
pub async fn share_view(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
    Json(request): Json<ShareViewRequest>,
) -> Result<(), ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    let db_user = db::queries::get_user(&state.db, user.id).await?;

    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::ViewShared {
            user: db_user.into(),
            workspace_url: request.workspace_url,
        },
    ));

    Ok(())
}
