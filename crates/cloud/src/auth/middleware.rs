//! Authentication middleware.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use uuid::Uuid;

use crate::db;
use crate::error::ApiError;
use crate::state::AppState;

use super::jwt;

/// Authenticated user extracted from request.
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// User ID.
    pub id: Uuid,
    /// User email (available for API responses).
    #[allow(dead_code)]
    pub email: String,
    /// User display name (available for API responses).
    #[allow(dead_code)]
    pub display_name: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract Bearer token from Authorization header
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("Missing authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::unauthorized("Invalid authorization header format"))?;

        // Verify token
        let claims = jwt::verify_token(token, &state.config.jwt_secret)?;

        // Fetch user from database
        let user = db::queries::get_user(&state.db, claims.user_id()).await?;

        Ok(AuthUser {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
        })
    }
}

/// Optional authentication - doesn't fail if no token provided.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OptionalAuthUser(pub Option<AuthUser>);

impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Try to extract auth, but don't fail if not present
        match AuthUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalAuthUser(Some(user))),
            Err(_) => Ok(OptionalAuthUser(None)),
        }
    }
}

/// Require user to be a member of a specific team.
pub async fn require_team_member(
    state: &AppState,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    if !db::queries::is_team_member(&state.db, team_id, user_id).await? {
        return Err(ApiError::forbidden("Not a member of this team"));
    }
    Ok(())
}

/// Require user to be an admin of a specific team.
pub async fn require_team_admin(
    state: &AppState,
    team_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    if !db::queries::is_team_admin(&state.db, team_id, user_id).await? {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}
