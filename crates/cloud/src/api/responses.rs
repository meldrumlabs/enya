//! Shared API response types with From trait implementations.
//!
//! This module centralizes response type definitions and provides
//! automatic conversions from database models.

use serde::Serialize;
use uuid::Uuid;

use crate::db::models::{DbMessage, DbThread, DbUser};

// =============================================================================
// User/Member responses
// =============================================================================

/// Team member response.
#[derive(Debug, Clone, Serialize)]
pub struct MemberResponse {
    pub id: Uuid,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
}

impl From<DbUser> for MemberResponse {
    fn from(user: DbUser) -> Self {
        Self {
            id: user.id,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
            email: Some(user.email),
        }
    }
}

// =============================================================================
// Message responses
// =============================================================================

/// Message response.
#[derive(Debug, Clone, Serialize)]
pub struct MessageResponse {
    pub id: Uuid,
    pub author_id: Uuid,
    pub content: String,
    pub mentions: Vec<Uuid>,
    pub created_at: u64,
    pub edited_at: Option<u64>,
}

impl From<DbMessage> for MessageResponse {
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

// =============================================================================
// Thread responses
// =============================================================================

/// Thread response (without annotation_id, for use in annotations).
#[derive(Debug, Clone, Serialize)]
pub struct ThreadResponse {
    pub id: Uuid,
    pub messages: Vec<MessageResponse>,
    pub participants: Vec<Uuid>,
    pub resolved: bool,
}

/// Full thread response (with annotation_id).
#[derive(Debug, Clone, Serialize)]
pub struct FullThreadResponse {
    pub id: Uuid,
    pub annotation_id: Option<Uuid>,
    pub messages: Vec<MessageResponse>,
    pub participants: Vec<Uuid>,
    pub resolved: bool,
}

/// Builder for thread responses - helps consolidate the common pattern
/// of fetching thread + messages + participants.
pub struct ThreadResponseBuilder {
    pub thread: DbThread,
    pub messages: Vec<DbMessage>,
    pub participants: Vec<Uuid>,
}

impl ThreadResponseBuilder {
    /// Build a ThreadResponse (without annotation_id).
    pub fn build(self) -> ThreadResponse {
        ThreadResponse {
            id: self.thread.id,
            messages: self.messages.into_iter().map(Into::into).collect(),
            participants: self.participants,
            resolved: self.thread.resolved,
        }
    }

    /// Build a FullThreadResponse (with annotation_id).
    pub fn build_full(self) -> FullThreadResponse {
        FullThreadResponse {
            id: self.thread.id,
            annotation_id: self.thread.annotation_id,
            messages: self.messages.into_iter().map(Into::into).collect(),
            participants: self.participants,
            resolved: self.thread.resolved,
        }
    }
}

// =============================================================================
// Team responses
// =============================================================================

/// Team response.
#[derive(Debug, Clone, Serialize)]
pub struct TeamResponse {
    pub id: Uuid,
    pub name: String,
    pub members: Vec<MemberResponse>,
}

// =============================================================================
// Annotation responses
// =============================================================================

/// Annotation response.
#[derive(Debug, Clone, Serialize)]
pub struct AnnotationResponse {
    pub id: Uuid,
    pub pane_id: String,
    pub query_fingerprint: String,
    pub timestamp_ns: i64,
    pub created_by: Uuid,
    pub created_at: u64,
    pub thread: ThreadResponse,
}

// =============================================================================
// Invitation responses
// =============================================================================

/// Team invitation response.
#[derive(Debug, Clone, Serialize)]
pub struct InvitationResponse {
    pub id: Uuid,
    pub team_id: Uuid,
    pub email: Option<String>,
    pub role: String,
    pub invited_by: Uuid,
    pub invite_url: String,
    pub expires_at: u64,
    pub created_at: u64,
}

/// Invitation accepted response.
#[derive(Debug, Clone, Serialize)]
pub struct InvitationAcceptedResponse {
    pub team_id: Uuid,
    pub team_name: String,
    pub role: String,
}

// =============================================================================
// Member responses with role
// =============================================================================

/// Team member response with role.
#[derive(Debug, Clone, Serialize)]
pub struct MemberWithRoleResponse {
    pub id: Uuid,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub role: String,
}

// =============================================================================
// Audit log responses
// =============================================================================

/// Audit log entry response.
#[derive(Debug, Clone, Serialize)]
pub struct AuditLogResponse {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub created_at: u64,
}

/// Paginated audit logs response.
#[derive(Debug, Clone, Serialize)]
pub struct AuditLogsResponse {
    pub logs: Vec<AuditLogResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
