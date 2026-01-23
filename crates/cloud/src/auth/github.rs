//! GitHub OAuth integration.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::ApiError;

/// Shared HTTP client for GitHub API requests.
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("enya-cloud")
            .build()
            .expect("failed to create HTTP client")
    })
}

/// GitHub user info from API.
#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// GitHub email info.
#[derive(Debug, Deserialize)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

/// OAuth state for CSRF protection.
#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthState {
    pub csrf_token: String,
    pub redirect_uri: Option<String>,
}

/// GitHub OAuth token response.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GitHubTokenResponse {
    access_token: String,
    token_type: String,
    #[serde(default)]
    scope: String,
}

/// Generate the GitHub authorization URL.
pub fn authorization_url(config: &Config) -> Result<String, ApiError> {
    let client_id = config
        .github_client_id
        .as_ref()
        .ok_or_else(|| ApiError::internal("GitHub OAuth not configured"))?;

    let redirect_url = format!("{}/auth/github/callback", config.frontend_url);

    let url = format!(
        "https://github.com/login/oauth/authorize?client_id={client_id}&redirect_uri={}&scope=user:email%20read:user",
        urlencoding::encode(&redirect_url)
    );

    Ok(url)
}

/// Exchange an authorization code for an access token.
pub async fn exchange_code(config: &Config, code: &str) -> Result<String, ApiError> {
    let client_id = config
        .github_client_id
        .as_ref()
        .ok_or_else(|| ApiError::internal("GitHub OAuth not configured"))?;
    let client_secret = config
        .github_client_secret
        .as_ref()
        .ok_or_else(|| ApiError::internal("GitHub OAuth not configured"))?;

    let response = http_client()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code),
        ])
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to exchange code: {e}")))?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(ApiError::bad_request(format!(
            "Failed to exchange code: {text}"
        )));
    }

    let token_response: GitHubTokenResponse = response
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse token response: {e}")))?;

    Ok(token_response.access_token)
}

/// Fetch the authenticated user's info from GitHub.
pub async fn fetch_user(access_token: &str) -> Result<GitHubUser, ApiError> {
    let response = http_client()
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to fetch GitHub user: {e}")))?;

    if !response.status().is_success() {
        return Err(ApiError::bad_request("Failed to fetch GitHub user info"));
    }

    response
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse GitHub user: {e}")))
}

/// Fetch the authenticated user's primary email from GitHub.
pub async fn fetch_primary_email(access_token: &str) -> Result<String, ApiError> {
    let response = http_client()
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to fetch GitHub emails: {e}")))?;

    if !response.status().is_success() {
        return Err(ApiError::bad_request("Failed to fetch GitHub emails"));
    }

    let emails: Vec<GitHubEmail> = response
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to parse GitHub emails: {e}")))?;

    // Find primary verified email
    emails
        .into_iter()
        .find(|e| e.primary && e.verified)
        .map(|e| e.email)
        .ok_or_else(|| ApiError::bad_request("No verified primary email found"))
}
