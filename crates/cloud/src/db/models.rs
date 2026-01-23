//! Database models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// User record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbUser {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub github_id: Option<String>,
    pub slack_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DbUser> for enya_team_api::User {
    fn from(user: DbUser) -> Self {
        Self {
            id: user.id,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
            email: Some(user.email),
        }
    }
}

/// Team record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbTeam {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DbTeam {
    /// Convert to API type with members.
    #[allow(dead_code)]
    pub fn into_team(self, members: Vec<enya_team_api::User>) -> enya_team_api::Team {
        enya_team_api::Team {
            id: self.id,
            name: self.name,
            members,
        }
    }
}

/// Team membership record (for future use with role-based access).
#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct DbTeamMember {
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// Annotation record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbAnnotation {
    pub id: Uuid,
    pub team_id: Uuid,
    pub pane_id: String,
    pub query_fingerprint: String,
    pub timestamp_ns: i64, // Stored as bigint in Postgres
    pub created_by: Uuid,
    pub thread_id: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Thread record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbThread {
    pub id: Uuid,
    pub annotation_id: Option<Uuid>,
    pub resolved: bool,
    pub created_at: DateTime<Utc>,
}

/// Message record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub mentions: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}

impl From<DbMessage> for enya_team_api::Message {
    fn from(msg: DbMessage) -> Self {
        Self {
            id: msg.id,
            author_id: msg.author_id,
            content: msg.content,
            mentions: msg.mentions,
            created_at: msg.created_at.timestamp() as u64,
            edited_at: msg.edited_at.map(|t| t.timestamp() as u64),
        }
    }
}

/// Channel record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbChannel {
    pub id: Uuid,
    pub team_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub kind: String,
    pub created_at: DateTime<Utc>,
}

impl From<DbChannel> for enya_team_api::Channel {
    fn from(ch: DbChannel) -> Self {
        Self {
            id: ch.id,
            team_id: ch.team_id,
            name: ch.name,
            description: ch.description,
            kind: match ch.kind.as_str() {
                "general" => enya_team_api::ChannelKind::General,
                "incidents" => enya_team_api::ChannelKind::Incidents,
                "deployments" => enya_team_api::ChannelKind::Deployments,
                "alerts" => enya_team_api::ChannelKind::Alerts,
                _ => enya_team_api::ChannelKind::Custom,
            },
            created_at: ch.created_at.timestamp() as u64,
        }
    }
}

/// Channel thread record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbChannelThread {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub title: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub resolved: bool,
    pub message_count: i32,
    pub last_message_at: Option<DateTime<Utc>>,
}

impl From<DbChannelThread> for enya_team_api::ChatThread {
    fn from(t: DbChannelThread) -> Self {
        Self {
            id: t.id,
            channel_id: t.channel_id,
            title: t.title,
            created_by: t.created_by,
            created_at: t.created_at.timestamp() as u64,
            resolved: t.resolved,
            message_count: t.message_count as u32,
            last_message_at: t.last_message_at.map(|t| t.timestamp() as u64),
        }
    }
}

/// Team invitation record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbTeamInvitation {
    pub id: Uuid,
    pub team_id: Uuid,
    pub email: Option<String>,
    pub token: String,
    pub role: String,
    pub invited_by: Uuid,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl DbTeamInvitation {
    /// Check if the invitation has expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at < Utc::now()
    }

    /// Check if the invitation has been accepted.
    pub fn is_accepted(&self) -> bool {
        self.accepted_at.is_some()
    }
}

/// Audit log record from database.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct DbAuditLog {
    pub id: Uuid,
    pub team_id: Uuid,
    pub actor_id: Uuid,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}
