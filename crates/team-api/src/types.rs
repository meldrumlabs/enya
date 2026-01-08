//! Core types for team collaboration.
//!
//! These types are shared between the editor and the cloud backend.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique user identifier.
pub type UserId = Uuid;

/// Unique team identifier.
pub type TeamId = Uuid;

/// Unique annotation identifier.
pub type AnnotationId = Uuid;

/// Unique thread identifier.
pub type ThreadId = Uuid;

/// Unique message identifier.
pub type MessageId = Uuid;

/// User information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    /// Unique identifier.
    pub id: UserId,
    /// Display name shown in UI.
    pub display_name: String,
    /// URL to avatar image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    /// Email address (may be hidden based on privacy settings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Team/workspace that users collaborate in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Unique identifier.
    pub id: TeamId,
    /// Team name.
    pub name: String,
    /// Team members.
    #[serde(default)]
    pub members: Vec<User>,
}

/// Annotation pinned to a chart at a specific timestamp.
///
/// Annotations link discussions to specific points in time on a chart,
/// allowing teams to collaborate around metrics data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    /// Unique identifier.
    pub id: AnnotationId,
    /// ID of the pane this annotation is attached to.
    pub pane_id: String,
    /// Hash of the query - used to match annotations across sessions.
    /// This allows annotations to persist even when panes are recreated.
    pub query_fingerprint: String,
    /// Timestamp on the chart where the annotation is pinned (nanoseconds since Unix epoch).
    /// Uses i64 for JSON compatibility (fits ~292 years from epoch).
    pub timestamp_ns: i64,
    /// User who created the annotation.
    pub created_by: UserId,
    /// When the annotation was created (Unix timestamp).
    pub created_at: u64,
    /// Discussion thread attached to this annotation.
    pub thread: Thread,
}

/// Request to create a new annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAnnotation {
    /// ID of the pane to attach to.
    pub pane_id: String,
    /// Hash of the query.
    pub query_fingerprint: String,
    /// Timestamp on the chart (nanoseconds since Unix epoch).
    pub timestamp_ns: i64,
    /// Initial message content.
    pub initial_message: String,
}

/// Discussion thread.
///
/// Threads can be standalone or attached to an annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    /// Unique identifier.
    pub id: ThreadId,
    /// Messages in the thread.
    #[serde(default)]
    pub messages: Vec<Message>,
    /// Users participating in the thread.
    #[serde(default)]
    pub participants: Vec<UserId>,
    /// Whether the thread has been resolved/closed.
    #[serde(default)]
    pub resolved: bool,
}

impl Thread {
    /// Create a new empty thread.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            messages: Vec::new(),
            participants: Vec::new(),
            resolved: false,
        }
    }
}

impl Default for Thread {
    fn default() -> Self {
        Self::new()
    }
}

/// Chat message in a thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier.
    pub id: MessageId,
    /// User who sent the message.
    pub author_id: UserId,
    /// Message content (supports markdown).
    pub content: String,
    /// Users mentioned in this message.
    #[serde(default)]
    pub mentions: Vec<UserId>,
    /// When the message was created (Unix timestamp).
    pub created_at: u64,
    /// When the message was last edited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<u64>,
}

/// Request to send a new message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMessage {
    /// Message content.
    pub content: String,
}

/// Real-time event from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TeamEvent {
    /// New annotation created.
    AnnotationCreated {
        /// The new annotation.
        annotation: Annotation,
    },
    /// Annotation updated.
    AnnotationUpdated {
        /// The updated annotation.
        annotation: Annotation,
    },
    /// Annotation deleted.
    AnnotationDeleted {
        /// ID of the deleted annotation.
        id: AnnotationId,
    },
    /// New message in a thread.
    MessageReceived {
        /// Thread the message belongs to.
        thread_id: ThreadId,
        /// The new message.
        message: Message,
        /// Author info (for display).
        author: User,
    },
    /// Someone is typing in a thread.
    UserTyping {
        /// Thread being typed in.
        thread_id: ThreadId,
        /// User who is typing.
        user_id: UserId,
    },
    /// User presence changed.
    PresenceChanged {
        /// User whose presence changed.
        user_id: UserId,
        /// Whether the user is online.
        online: bool,
    },
    /// War room: someone is sharing their view.
    ViewShared {
        /// User sharing their view.
        user: User,
        /// Encoded workspace URL.
        workspace_url: String,
    },
    /// You were @mentioned.
    Mentioned {
        /// The message containing the mention.
        message: Message,
        /// Author of the message.
        author: User,
        /// Thread containing the message.
        thread_id: ThreadId,
    },
    /// Team member joined.
    MemberJoined {
        /// The new member.
        user: User,
    },
    /// Team member left.
    MemberLeft {
        /// ID of the user who left.
        user_id: UserId,
    },
}

/// Connection state to the team server.
#[derive(Debug, Clone, Default)]
pub enum TeamConnectionStatus {
    /// Not connected to any server.
    #[default]
    Disconnected,
    /// Currently attempting to connect.
    Connecting,
    /// Successfully connected.
    Connected {
        /// Current user.
        user: User,
        /// Current team.
        team: Team,
    },
    /// Attempting to reconnect after connection loss.
    Reconnecting {
        /// Number of reconnection attempts.
        attempt: u32,
    },
    /// Connection failed.
    Failed {
        /// Error message.
        error: String,
    },
}

impl TeamConnectionStatus {
    /// Returns true if connected.
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// Returns true if connecting or reconnecting.
    pub fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting | Self::Reconnecting { .. })
    }

    /// Returns the current user if connected.
    pub fn current_user(&self) -> Option<&User> {
        match self {
            Self::Connected { user, .. } => Some(user),
            _ => None,
        }
    }

    /// Returns the current team if connected.
    pub fn current_team(&self) -> Option<&Team> {
        match self {
            Self::Connected { team, .. } => Some(team),
            _ => None,
        }
    }
}

/// Authentication response from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// Access token for API requests.
    pub access_token: String,
    /// Token type (usually "Bearer").
    pub token_type: String,
    /// When the token expires (Unix timestamp).
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// Current user info.
    pub user: User,
    /// User's teams.
    pub teams: Vec<Team>,
}

/// OAuth provider for authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    /// GitHub OAuth.
    GitHub,
    /// Slack OAuth.
    Slack,
}

impl std::fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => write!(f, "github"),
            Self::Slack => write!(f, "slack"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_event_serialization() {
        let event = TeamEvent::AnnotationCreated {
            annotation: Annotation {
                id: Uuid::new_v4(),
                pane_id: "pane-1".to_string(),
                query_fingerprint: "abc123".to_string(),
                timestamp_ns: 1_234_567_890_000_000_000,
                created_by: Uuid::new_v4(),
                created_at: 1234567890,
                thread: Thread::new(),
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"annotation_created\""));

        let deserialized: TeamEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, TeamEvent::AnnotationCreated { .. }));
    }

    #[test]
    fn test_connection_status_helpers() {
        let status = TeamConnectionStatus::Disconnected;
        assert!(!status.is_connected());
        assert!(!status.is_connecting());
        assert!(status.current_user().is_none());

        let status = TeamConnectionStatus::Connecting;
        assert!(!status.is_connected());
        assert!(status.is_connecting());

        let user = User {
            id: Uuid::new_v4(),
            display_name: "Test User".to_string(),
            avatar_url: None,
            email: None,
        };
        let team = Team {
            id: Uuid::new_v4(),
            name: "Test Team".to_string(),
            members: vec![],
        };
        let status = TeamConnectionStatus::Connected {
            user: user.clone(),
            team,
        };
        assert!(status.is_connected());
        assert!(!status.is_connecting());
        assert_eq!(status.current_user().unwrap().id, user.id);
    }
}
