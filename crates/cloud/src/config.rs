//! Configuration management.

use anyhow::{Context, Result};

/// Application configuration.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    /// Server host.
    pub host: String,
    /// Server port.
    pub port: u16,
    /// Database URL (PostgreSQL).
    pub database_url: String,
    /// JWT secret for token signing.
    pub jwt_secret: String,
    /// JWT token expiry in seconds.
    pub jwt_expiry_secs: u64,
    /// GitHub OAuth client ID.
    pub github_client_id: Option<String>,
    /// GitHub OAuth client secret.
    pub github_client_secret: Option<String>,
    /// Slack OAuth client ID (for future use).
    pub slack_client_id: Option<String>,
    /// Slack OAuth client secret (for future use).
    pub slack_client_secret: Option<String>,
    /// Frontend URL (for OAuth redirects).
    pub frontend_url: String,
}

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .context("Invalid PORT")?,
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?,
            jwt_secret: std::env::var("JWT_SECRET").context("JWT_SECRET must be set")?,
            jwt_expiry_secs: std::env::var("JWT_EXPIRY_SECS")
                .unwrap_or_else(|_| "604800".to_string()) // 7 days
                .parse()
                .context("Invalid JWT_EXPIRY_SECS")?,
            github_client_id: std::env::var("GITHUB_CLIENT_ID").ok(),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET").ok(),
            slack_client_id: std::env::var("SLACK_CLIENT_ID").ok(),
            slack_client_secret: std::env::var("SLACK_CLIENT_SECRET").ok(),
            frontend_url: std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        })
    }

    /// Check if GitHub OAuth is configured.
    #[allow(dead_code)]
    pub fn github_configured(&self) -> bool {
        self.github_client_id.is_some() && self.github_client_secret.is_some()
    }

    /// Check if Slack OAuth is configured.
    #[allow(dead_code)]
    pub fn slack_configured(&self) -> bool {
        self.slack_client_id.is_some() && self.slack_client_secret.is_some()
    }
}
