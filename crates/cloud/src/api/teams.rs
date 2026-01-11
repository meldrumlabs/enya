//! Team API routes.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::api::responses::{MemberResponse, MemberWithRoleResponse, TeamResponse};
use crate::auth::AuthUser;
use crate::auth::middleware::{require_team_admin, require_team_member};
use crate::db;
use crate::error::ApiError;
use crate::metrics;
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

    metrics::record_team_created();

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

// =============================================================================
// Member management endpoints
// =============================================================================

/// Update member role request.
#[derive(Debug, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: String,
}

/// List team members with roles.
pub async fn list_members_with_roles(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<MemberWithRoleResponse>>, ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    let members = db::queries::get_team_members(&state.db, team_id).await?;

    let mut responses = Vec::with_capacity(members.len());
    for member in members {
        let team_member = db::queries::get_team_member(&state.db, team_id, member.id).await?;
        let role = team_member
            .map(|tm| tm.role)
            .unwrap_or_else(|| "member".to_string());

        responses.push(MemberWithRoleResponse {
            id: member.id,
            display_name: member.display_name,
            avatar_url: member.avatar_url,
            email: Some(member.email),
            role,
        });
    }

    Ok(Json(responses))
}

/// Update a team member's role.
///
/// Admin-only. Cannot demote the last admin.
pub async fn update_member_role(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, member_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateMemberRoleRequest>,
) -> Result<(), ApiError> {
    // Require admin access
    require_team_admin(&state, team_id, user.id).await?;

    // Validate role
    if request.role != "admin" && request.role != "member" {
        return Err(ApiError::bad_request("Role must be 'admin' or 'member'"));
    }

    // Check if member exists
    let member = db::queries::get_team_member(&state.db, team_id, member_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Member not found"))?;

    // Prevent demoting the last admin
    if member.role == "admin" && request.role == "member" {
        let admin_count = db::queries::count_team_admins(&state.db, team_id).await?;
        if admin_count <= 1 {
            return Err(ApiError::bad_request(
                "Cannot demote the last admin. Promote another member first.",
            ));
        }
    }

    // Update role
    db::queries::update_member_role(&state.db, team_id, member_id, &request.role).await?;

    metrics::record_role_changed();

    // Log the action
    db::queries::create_audit_log(
        &state.db,
        team_id,
        user.id,
        "member.role_changed",
        "user",
        Some(member_id),
        Some(json!({
            "old_role": member.role,
            "new_role": request.role,
        })),
        None,
        None,
    )
    .await?;

    Ok(())
}

/// Remove a member from the team.
///
/// Admin-only. Cannot remove the last admin.
pub async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<(), ApiError> {
    // Require admin access
    require_team_admin(&state, team_id, user.id).await?;

    // Cannot remove yourself (use leave_team instead)
    if member_id == user.id {
        return Err(ApiError::bad_request(
            "Cannot remove yourself. Use the leave endpoint instead.",
        ));
    }

    // Check if member exists and get their role
    let member = db::queries::get_team_member(&state.db, team_id, member_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Member not found"))?;

    // Prevent removing the last admin
    if member.role == "admin" {
        let admin_count = db::queries::count_team_admins(&state.db, team_id).await?;
        if admin_count <= 1 {
            return Err(ApiError::bad_request("Cannot remove the last admin"));
        }
    }

    // Remove member
    db::queries::remove_team_member(&state.db, team_id, member_id).await?;

    metrics::record_member_removed();

    // Log the action
    db::queries::create_audit_log(
        &state.db,
        team_id,
        user.id,
        "member.removed",
        "user",
        Some(member_id),
        Some(json!({
            "removed_role": member.role,
        })),
        None,
        None,
    )
    .await?;

    // Broadcast member left event
    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::MemberLeft { user_id: member_id },
    ));

    Ok(())
}

/// Leave a team voluntarily.
///
/// Cannot leave if you're the last admin.
pub async fn leave_team(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
) -> Result<(), ApiError> {
    require_team_member(&state, team_id, user.id).await?;

    // Check if user is an admin
    let member = db::queries::get_team_member(&state.db, team_id, user.id)
        .await?
        .ok_or_else(|| ApiError::not_found("Member not found"))?;

    // Prevent leaving if last admin
    if member.role == "admin" {
        let admin_count = db::queries::count_team_admins(&state.db, team_id).await?;
        if admin_count <= 1 {
            return Err(ApiError::bad_request(
                "Cannot leave as the last admin. Transfer ownership first.",
            ));
        }
    }

    // Remove self from team
    db::queries::remove_team_member(&state.db, team_id, user.id).await?;

    metrics::record_member_left();

    // Log the action
    db::queries::create_audit_log(
        &state.db,
        team_id,
        user.id,
        "member.left",
        "user",
        Some(user.id),
        None,
        None,
        None,
    )
    .await?;

    // Broadcast member left event
    state.broadcast(RealtimeEvent::broadcast(
        team_id,
        TeamEvent::MemberLeft { user_id: user.id },
    ));

    Ok(())
}
