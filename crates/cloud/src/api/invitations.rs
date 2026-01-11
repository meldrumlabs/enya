//! Team invitation API routes.

use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{Duration, Utc};
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::api::responses::{InvitationAcceptedResponse, InvitationResponse};
use crate::auth::AuthUser;
use crate::auth::middleware::require_team_admin;
use crate::db;
use crate::error::ApiError;
use crate::metrics;
use crate::realtime::RealtimeEvent;
use crate::state::AppState;
use enya_team_api::TeamEvent;

/// Create invitation request.
#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    /// Email to invite (optional for magic links).
    pub email: Option<String>,
    /// Role for the invited user (default: member).
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "member".to_string()
}

/// Generate a secure random token for invitations.
fn generate_invite_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    hex::encode(bytes)
}

/// Create a team invitation.
///
/// Admin-only. Creates an invitation that can be sent via email or as a magic link.
pub async fn create_invitation(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<Json<InvitationResponse>, ApiError> {
    // Require admin access
    require_team_admin(&state, team_id, user.id).await?;

    // Validate role
    if request.role != "admin" && request.role != "member" {
        return Err(ApiError::bad_request("Role must be 'admin' or 'member'"));
    }

    // Check if invitation already exists for this email
    if let Some(email) = &request.email {
        if db::queries::invitation_exists_for_email(&state.db, team_id, email).await? {
            return Err(ApiError::bad_request(
                "An active invitation already exists for this email",
            ));
        }

        // Check if user is already a member
        if let Some(existing_user) = db::queries::get_user_by_email(&state.db, email).await? {
            if db::queries::is_team_member(&state.db, team_id, existing_user.id).await? {
                return Err(ApiError::bad_request("User is already a team member"));
            }
        }
    }

    // Generate token and expiry (7 days)
    let token = generate_invite_token();
    let expires_at = Utc::now() + Duration::days(7);

    // Create invitation
    let invitation = db::queries::create_invitation(
        &state.db,
        team_id,
        request.email.as_deref(),
        &token,
        &request.role,
        user.id,
        expires_at,
    )
    .await?;

    let invitation_type = if request.email.is_some() {
        "email"
    } else {
        "magic_link"
    };
    metrics::record_invitation_sent(invitation_type);

    // Log the action
    db::queries::create_audit_log(
        &state.db,
        team_id,
        user.id,
        "invitation.created",
        "invitation",
        Some(invitation.id),
        Some(json!({
            "email": request.email,
            "role": request.role,
        })),
        None,
        None,
    )
    .await?;

    // Build invite URL
    let invite_url = format!("{}/invite/{}", state.config.frontend_url, invitation.token);

    Ok(Json(InvitationResponse {
        id: invitation.id,
        team_id: invitation.team_id,
        email: invitation.email,
        role: invitation.role,
        invited_by: invitation.invited_by,
        invite_url,
        expires_at: invitation.expires_at.timestamp() as u64,
        created_at: invitation.created_at.timestamp() as u64,
    }))
}

/// List pending invitations for a team.
///
/// Admin-only.
pub async fn list_invitations(
    State(state): State<AppState>,
    user: AuthUser,
    Path(team_id): Path<Uuid>,
) -> Result<Json<Vec<InvitationResponse>>, ApiError> {
    // Require admin access
    require_team_admin(&state, team_id, user.id).await?;

    let invitations = db::queries::list_team_invitations(&state.db, team_id).await?;

    let responses: Vec<InvitationResponse> = invitations
        .into_iter()
        .filter(|inv| !inv.is_expired())
        .map(|inv| {
            let invite_url = format!("{}/invite/{}", state.config.frontend_url, inv.token);
            InvitationResponse {
                id: inv.id,
                team_id: inv.team_id,
                email: inv.email,
                role: inv.role,
                invited_by: inv.invited_by,
                invite_url,
                expires_at: inv.expires_at.timestamp() as u64,
                created_at: inv.created_at.timestamp() as u64,
            }
        })
        .collect();

    Ok(Json(responses))
}

/// Delete/revoke an invitation.
///
/// Admin-only.
pub async fn delete_invitation(
    State(state): State<AppState>,
    user: AuthUser,
    Path((team_id, invitation_id)): Path<(Uuid, Uuid)>,
) -> Result<(), ApiError> {
    // Require admin access
    require_team_admin(&state, team_id, user.id).await?;

    let deleted = db::queries::delete_invitation(&state.db, invitation_id).await?;

    if !deleted {
        return Err(ApiError::not_found("Invitation not found"));
    }

    metrics::record_invitation_revoked();

    // Log the action
    db::queries::create_audit_log(
        &state.db,
        team_id,
        user.id,
        "invitation.revoked",
        "invitation",
        Some(invitation_id),
        None,
        None,
        None,
    )
    .await?;

    Ok(())
}

/// Accept invitation request.
#[derive(Debug, Deserialize)]
pub struct AcceptInvitationRequest {
    /// The invitation token.
    pub token: String,
}

/// Accept an invitation and join the team.
pub async fn accept_invitation(
    State(state): State<AppState>,
    user: AuthUser,
    Json(request): Json<AcceptInvitationRequest>,
) -> Result<Json<InvitationAcceptedResponse>, ApiError> {
    // Get invitation by token
    let invitation = db::queries::get_invitation_by_token(&state.db, &request.token)
        .await?
        .ok_or_else(|| ApiError::not_found("Invalid or expired invitation"))?;

    // Check if expired
    if invitation.is_expired() {
        return Err(ApiError::bad_request("Invitation has expired"));
    }

    // Check if already accepted
    if invitation.is_accepted() {
        return Err(ApiError::bad_request("Invitation has already been used"));
    }

    // Check if user is already a member
    if db::queries::is_team_member(&state.db, invitation.team_id, user.id).await? {
        return Err(ApiError::bad_request(
            "You are already a member of this team",
        ));
    }

    // If email-based invitation, verify email matches
    if let Some(invite_email) = &invitation.email {
        let db_user = db::queries::get_user(&state.db, user.id).await?;
        if db_user.email.to_lowercase() != invite_email.to_lowercase() {
            return Err(ApiError::forbidden(
                "This invitation was sent to a different email address",
            ));
        }
    }

    // Add user to team
    db::queries::add_team_member(&state.db, invitation.team_id, user.id, &invitation.role).await?;

    // Mark invitation as accepted
    db::queries::accept_invitation(&state.db, invitation.id, user.id).await?;

    metrics::record_invitation_accepted();
    metrics::record_user_joined_team();

    // Get team info for response
    let team = db::queries::get_team(&state.db, invitation.team_id).await?;

    // Log the action
    db::queries::create_audit_log(
        &state.db,
        invitation.team_id,
        user.id,
        "member.joined",
        "user",
        Some(user.id),
        Some(json!({
            "role": invitation.role,
            "invitation_id": invitation.id,
        })),
        None,
        None,
    )
    .await?;

    // Broadcast member joined event
    let db_user = db::queries::get_user(&state.db, user.id).await?;
    state.broadcast(RealtimeEvent::broadcast(
        invitation.team_id,
        TeamEvent::MemberJoined {
            user: db_user.into(),
        },
    ));

    Ok(Json(InvitationAcceptedResponse {
        team_id: team.id,
        team_name: team.name,
        role: invitation.role,
    }))
}

/// Get invitation info by token (public endpoint for showing invite details).
#[derive(Debug, serde::Serialize)]
pub struct InvitationInfoResponse {
    pub team_name: String,
    pub invited_by_name: String,
    pub role: String,
    pub expires_at: u64,
}

pub async fn get_invitation_info(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InvitationInfoResponse>, ApiError> {
    let invitation = db::queries::get_invitation_by_token(&state.db, &token)
        .await?
        .ok_or_else(|| ApiError::not_found("Invalid invitation"))?;

    if invitation.is_expired() {
        return Err(ApiError::bad_request("Invitation has expired"));
    }

    if invitation.is_accepted() {
        return Err(ApiError::bad_request("Invitation has already been used"));
    }

    let team = db::queries::get_team(&state.db, invitation.team_id).await?;
    let inviter = db::queries::get_user(&state.db, invitation.invited_by).await?;

    Ok(Json(InvitationInfoResponse {
        team_name: team.name,
        invited_by_name: inviter.display_name,
        role: invitation.role,
        expires_at: invitation.expires_at.timestamp() as u64,
    }))
}
