//! Chat message data model.
//!
//! Messages are the core content unit in channels and threads.
//! They support @mentions for users, AI agents, and charts.

use enya_team_api::UserId;
use uuid::Uuid;

use super::ThreadId;
use super::chat_view::{InlineChart, InlineVisualization};

/// Unique identifier for a message (matches API type).
pub type MessageId = Uuid;

/// The kind of mention in a message.
#[derive(Debug, Clone, PartialEq)]
pub enum MentionKind {
    /// Mention a team member (@alice).
    User(UserId),
    /// Mention an AI agent (@agent or @claude).
    Agent {
        /// Agent model identifier.
        model: String,
    },
    /// Mention a chart by name (@chart:cpu-usage).
    Chart {
        /// Chart/pane name or ID.
        chart_name: String,
    },
    /// Mention everyone (@here, @channel).
    Everyone,
}

/// A mention extracted from a message.
#[derive(Debug, Clone)]
pub struct Mention {
    /// The kind of mention.
    pub kind: MentionKind,
    /// Start position in message text.
    pub start: usize,
    /// End position in message text.
    pub end: usize,
    /// The raw mention text (e.g., "@alice").
    pub text: String,
}

impl Mention {
    /// Create a user mention.
    pub fn user(user_id: UserId, text: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            kind: MentionKind::User(user_id),
            start,
            end,
            text: text.into(),
        }
    }

    /// Create an agent mention.
    pub fn agent(
        model: impl Into<String>,
        text: impl Into<String>,
        start: usize,
        end: usize,
    ) -> Self {
        Self {
            kind: MentionKind::Agent {
                model: model.into(),
            },
            start,
            end,
            text: text.into(),
        }
    }

    /// Create a chart mention.
    pub fn chart(
        chart_name: impl Into<String>,
        text: impl Into<String>,
        start: usize,
        end: usize,
    ) -> Self {
        Self {
            kind: MentionKind::Chart {
                chart_name: chart_name.into(),
            },
            start,
            end,
            text: text.into(),
        }
    }
}

/// Author of a chat message.
#[derive(Debug, Clone)]
pub enum ChatMessageAuthor {
    /// A team member.
    User {
        /// User ID from team API.
        user_id: UserId,
        /// Display name.
        display_name: String,
    },
    /// An AI agent.
    Agent {
        /// Agent/model identifier.
        model: String,
        /// Optional context about what the agent was asked.
        context: Option<String>,
    },
    /// System message (joins, leaves, etc.).
    System,
}

impl ChatMessageAuthor {
    /// Create a user author.
    pub fn user(user_id: UserId, display_name: impl Into<String>) -> Self {
        Self::User {
            user_id,
            display_name: display_name.into(),
        }
    }

    /// Create an agent author.
    pub fn agent(model: impl Into<String>) -> Self {
        Self::Agent {
            model: model.into(),
            context: None,
        }
    }

    /// Get the display name for this author.
    pub fn display_name(&self) -> &str {
        match self {
            Self::User { display_name, .. } => display_name,
            Self::Agent { model, .. } => model,
            Self::System => "System",
        }
    }

    /// Check if this is the current user.
    pub fn is_user(&self, id: UserId) -> bool {
        matches!(self, Self::User { user_id, .. } if *user_id == id)
    }

    /// Check if this is an agent.
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }

    /// Check if this is a system message.
    pub fn is_system(&self) -> bool {
        matches!(self, Self::System)
    }
}

/// A chat message in a channel or thread.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Unique identifier.
    pub id: MessageId,
    /// Who sent this message.
    pub author: ChatMessageAuthor,
    /// Message content (plain text with @mentions).
    pub content: String,
    /// Extracted mentions from content.
    pub mentions: Vec<Mention>,
    /// Thread this message belongs to (None = channel root).
    pub thread_id: Option<ThreadId>,
    /// When this message was sent (Unix seconds).
    pub timestamp: f64,
    /// Whether this message has been edited.
    pub is_edited: bool,
    /// Emoji reactions (emoji -> count).
    pub reactions: Vec<(String, usize)>,
    /// Inline time series charts (data snapshot at share time).
    pub inline_charts: Vec<InlineChart>,
    /// Inline visualizations (stats, tables, bar charts, etc.).
    pub visualizations: Vec<InlineVisualization>,
}

impl ChatMessage {
    /// Create a new message from a user.
    pub fn from_user(
        user_id: UserId,
        display_name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let content = content.into();
        Self {
            id: Uuid::new_v4(),
            author: ChatMessageAuthor::user(user_id, display_name),
            content,
            mentions: Vec::new(),
            thread_id: None,
            timestamp: now_unix_secs(),
            is_edited: false,
            reactions: Vec::new(),
            inline_charts: Vec::new(),
            visualizations: Vec::new(),
        }
    }

    /// Create a new message from an agent.
    pub fn from_agent(model: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            id: Uuid::new_v4(),
            author: ChatMessageAuthor::agent(model),
            content,
            mentions: Vec::new(),
            thread_id: None,
            timestamp: now_unix_secs(),
            is_edited: false,
            reactions: Vec::new(),
            inline_charts: Vec::new(),
            visualizations: Vec::new(),
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            author: ChatMessageAuthor::System,
            content: content.into(),
            mentions: Vec::new(),
            thread_id: None,
            timestamp: now_unix_secs(),
            is_edited: false,
            reactions: Vec::new(),
            inline_charts: Vec::new(),
            visualizations: Vec::new(),
        }
    }

    /// Create a message from API data.
    /// Note: We need the author name from outside since the API only has author_id.
    pub fn from_api(
        api_message: &enya_team_api::Message,
        author_name: &str,
        thread_id: Option<ThreadId>,
    ) -> Self {
        Self {
            id: api_message.id,
            author: ChatMessageAuthor::user(api_message.author_id, author_name),
            content: api_message.content.clone(),
            mentions: Vec::new(), // TODO: Parse mentions from content
            thread_id,
            timestamp: api_message.created_at as f64,
            is_edited: api_message.edited_at.is_some(),
            reactions: Vec::new(),
            inline_charts: Vec::new(), // TODO: Deserialize from API if present
            visualizations: Vec::new(),
        }
    }

    /// Set the thread ID.
    pub fn in_thread(mut self, thread_id: ThreadId) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Add mentions to the message.
    pub fn with_mentions(mut self, mentions: Vec<Mention>) -> Self {
        self.mentions = mentions;
        self
    }

    /// Add an inline chart to the message (data snapshot).
    pub fn with_inline_chart(mut self, chart: InlineChart) -> Self {
        self.inline_charts.push(chart);
        self
    }

    /// Add an inline visualization to the message (stat, table, bar chart, etc.).
    pub fn with_visualization(mut self, viz: InlineVisualization) -> Self {
        self.visualizations.push(viz);
        self
    }

    /// Check if this message has any inline charts.
    pub fn has_charts(&self) -> bool {
        !self.inline_charts.is_empty()
    }

    /// Check if this message has any visualizations.
    pub fn has_visualizations(&self) -> bool {
        !self.visualizations.is_empty()
    }

    /// Get a preview of the message content.
    pub fn preview(&self, max_len: usize) -> &str {
        if self.content.len() <= max_len {
            &self.content
        } else if let Some(space_idx) = self.content[..max_len].rfind(' ') {
            &self.content[..space_idx]
        } else {
            &self.content[..max_len]
        }
    }

    /// Check if this message mentions a specific user.
    pub fn mentions_user(&self, user_id: UserId) -> bool {
        self.mentions
            .iter()
            .any(|m| matches!(&m.kind, MentionKind::User(id) if *id == user_id))
    }

    /// Check if this message mentions an agent.
    pub fn mentions_agent(&self) -> bool {
        self.mentions
            .iter()
            .any(|m| matches!(&m.kind, MentionKind::Agent { .. }))
    }

    /// Format timestamp for display (relative time).
    pub fn relative_time(&self) -> String {
        let now = now_unix_secs();
        let diff = now - self.timestamp;

        if diff < 60.0 {
            "just now".to_string()
        } else if diff < 3600.0 {
            let mins = (diff / 60.0) as u32;
            format!("{mins}m ago")
        } else if diff < 86400.0 {
            let hours = (diff / 3600.0) as u32;
            format!("{hours}h ago")
        } else {
            let days = (diff / 86400.0) as u32;
            format!("{days}d ago")
        }
    }
}

/// Get current Unix timestamp in seconds (WASM-compatible).
fn now_unix_secs() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_message_creation() {
        let user_id = UserId::new_v4();
        let msg = ChatMessage::from_user(user_id, "Alice", "Hello world!");

        assert_eq!(msg.content, "Hello world!");
        assert!(msg.author.is_user(user_id));
        assert!(msg.thread_id.is_none());
    }

    #[test]
    fn test_agent_message() {
        let msg = ChatMessage::from_agent("claude-3", "Here's my analysis...");

        assert!(msg.author.is_agent());
        assert_eq!(msg.author.display_name(), "claude-3");
    }

    #[test]
    fn test_message_preview() {
        let user_id = UserId::new_v4();
        let msg = ChatMessage::from_user(
            user_id,
            "Alice",
            "This is a longer message that should be truncated",
        );

        assert_eq!(msg.preview(20), "This is a longer");
    }
}
