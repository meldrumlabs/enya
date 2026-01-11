//! Authentication API routes.

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::auth::{AuthUser, github, jwt};
use crate::db;
use crate::error::ApiError;
use crate::metrics;
use crate::state::AppState;

/// GitHub login response - returns the authorization URL.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub auth_url: String,
}

/// GitHub callback request.
#[derive(Debug, Deserialize)]
pub struct GitHubCallbackRequest {
    pub code: String,
}

/// Auth response with token and user info.
#[derive(Debug, Serialize)]
pub struct AuthResponseBody {
    pub access_token: String,
    pub token_type: String,
    pub user: UserResponse,
    pub teams: Vec<TeamResponse>,
}

/// User response.
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: uuid::Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// Team response.
#[derive(Debug, Serialize)]
pub struct TeamResponse {
    pub id: uuid::Uuid,
    pub name: String,
}

/// Get GitHub authorization URL.
pub async fn github_login(State(state): State<AppState>) -> Result<Json<LoginResponse>, ApiError> {
    let auth_url = github::authorization_url(&state.config)?;

    Ok(Json(LoginResponse { auth_url }))
}

/// Handle GitHub OAuth callback.
pub async fn github_callback(
    State(state): State<AppState>,
    Json(request): Json<GitHubCallbackRequest>,
) -> Result<Json<AuthResponseBody>, ApiError> {
    // Exchange code for access token
    let access_token = github::exchange_code(&state.config, &request.code).await?;

    // Fetch user info from GitHub
    let github_user = github::fetch_user(&access_token).await?;

    // Get primary email
    let email = match github_user.email {
        Some(email) => email,
        None => github::fetch_primary_email(&access_token).await?,
    };

    // Upsert user in database
    let user = db::queries::upsert_github_user(
        &state.db,
        &github_user.id.to_string(),
        &email,
        github_user.name.as_deref().unwrap_or(&github_user.login),
        github_user.avatar_url.as_deref(),
    )
    .await?;

    // Get user's teams
    let teams = db::queries::get_user_teams(&state.db, user.id).await?;

    // If user has no teams, create a personal team
    let teams = if teams.is_empty() {
        let team =
            db::queries::create_team(&state.db, &format!("{}'s Team", user.display_name), user.id)
                .await?;
        vec![team]
    } else {
        teams
    };

    // Create JWT token
    let token = jwt::create_token(
        user.id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_secs,
    )?;

    metrics::record_auth_success("github");

    Ok(Json(AuthResponseBody {
        access_token: token,
        token_type: "Bearer".to_string(),
        user: UserResponse {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
        },
        teams: teams
            .into_iter()
            .map(|t| TeamResponse {
                id: t.id,
                name: t.name,
            })
            .collect(),
    }))
}

/// Get current authenticated user.
pub async fn get_current_user(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<UserResponse>, ApiError> {
    let db_user = db::queries::get_user(&state.db, user.id).await?;

    Ok(Json(UserResponse {
        id: db_user.id,
        email: db_user.email,
        display_name: db_user.display_name,
        avatar_url: db_user.avatar_url,
    }))
}

// =============================================================================
// Development-only endpoints
// =============================================================================

/// Request body for dev login.
#[derive(Debug, Deserialize)]
pub struct DevLoginRequest {
    /// Display name for the test user.
    pub name: String,
    /// Optional email (defaults to name@dev.local).
    pub email: Option<String>,
}

/// Create a test user and return auth token (development only).
///
/// This endpoint is only available when DEV_AUTH=true is set.
/// It creates a new user (or reuses existing) and returns a valid JWT.
///
/// POST /auth/dev
/// { "name": "Alice" }
pub async fn dev_login(
    State(state): State<AppState>,
    Json(request): Json<DevLoginRequest>,
) -> Result<Json<AuthResponseBody>, ApiError> {
    // Only allow in development mode
    if std::env::var("DEV_AUTH").unwrap_or_default() != "true" {
        return Err(ApiError::forbidden(
            "Dev auth is disabled. Set DEV_AUTH=true to enable.",
        ));
    }

    let email = request.email.unwrap_or_else(|| {
        format!(
            "{}@dev.local",
            request.name.to_lowercase().replace(' ', ".")
        )
    });

    // Create a fake GitHub ID based on the name (deterministic for same name)
    let github_id = format!("dev-{}", request.name.to_lowercase().replace(' ', "-"));

    // Upsert user in database
    let user = db::queries::upsert_github_user(
        &state.db,
        &github_id,
        &email,
        &request.name,
        None, // No avatar for dev users
    )
    .await?;

    // Get user's teams
    let teams = db::queries::get_user_teams(&state.db, user.id).await?;

    // If user has no teams, create a personal team
    let teams = if teams.is_empty() {
        let team =
            db::queries::create_team(&state.db, &format!("{}'s Team", user.display_name), user.id)
                .await?;
        vec![team]
    } else {
        teams
    };

    // Create JWT token
    let token = jwt::create_token(
        user.id,
        &state.config.jwt_secret,
        state.config.jwt_expiry_secs,
    )?;

    tracing::info!(
        "Dev login: created token for user {} ({})",
        user.display_name,
        user.id
    );

    Ok(Json(AuthResponseBody {
        access_token: token,
        token_type: "Bearer".to_string(),
        user: UserResponse {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
        },
        teams: teams
            .into_iter()
            .map(|t| TeamResponse {
                id: t.id,
                name: t.name,
            })
            .collect(),
    }))
}
