//! Thread data model for focused discussions.
//!
//! Threads allow branching conversations from a channel message,
//! keeping discussions organized and reducing noise.

use uuid::Uuid;

use super::{ChannelId, ChatMessage, MessageId};

/// Unique identifier for a thread (matches API type).
pub type ThreadId = Uuid;

/// Status of a thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThreadStatus {
    /// Thread is active/open.
    #[default]
    Active,
    /// Thread is resolved/closed.
    Resolved,
    /// Thread is archived (old but not resolved).
    Archived,
}

impl ThreadStatus {
    /// Get the icon for this status.
    pub fn icon(&self) -> &'static str {
        use egui_nerdfonts::regular;
        match self {
            Self::Active => regular::COMMENT,
            Self::Resolved => regular::CHECK,
            Self::Archived => regular::ARCHIVE,
        }
    }

    /// Get the label for this status.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Resolved => "Resolved",
            Self::Archived => "Archived",
        }
    }
}

/// A conversation thread within a channel.
#[derive(Debug, Clone)]
pub struct Thread {
    /// Unique identifier.
    pub id: ThreadId,
    /// Channel this thread belongs to.
    pub channel_id: ChannelId,
    /// The root message that started this thread.
    pub root_message_id: MessageId,
    /// Thread title (often first line of root message).
    pub title: String,
    /// Thread status.
    pub status: ThreadStatus,
    /// Number of replies in this thread.
    pub reply_count: usize,
    /// Unread reply count.
    pub unread_count: usize,
    /// IDs of participants in this thread.
    pub participant_count: usize,
    /// When the thread was created (Unix seconds).
    pub created_at: f64,
    /// When the thread was last updated (Unix seconds).
    pub last_activity_at: f64,
    /// Whether this thread is pinned/important.
    pub is_pinned: bool,
    /// Priority indicator (for incident threads).
    pub priority: ThreadPriority,
}

/// Priority level for threads (especially incidents).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThreadPriority {
    /// Normal priority.
    #[default]
    Normal,
    /// High priority / important.
    High,
    /// Critical / incident.
    Critical,
}

impl ThreadPriority {
    /// Get the icon for this priority.
    pub fn icon(&self) -> &'static str {
        use egui_nerdfonts::regular;
        match self {
            Self::Normal => regular::COMMENT,
            Self::High => regular::BOOKMARK,
            Self::Critical => regular::FIRE,
        }
    }

    /// Get the color for this priority.
    pub fn color(&self) -> egui::Color32 {
        use crate::ui::palette;
        match self {
            Self::Normal => palette::semantic::INFO,
            Self::High => palette::semantic::WARNING,
            Self::Critical => palette::semantic::ERROR,
        }
    }
}

impl Thread {
    /// Create a new thread from a root message.
    pub fn new(
        channel_id: ChannelId,
        root_message: &ChatMessage,
        title: impl Into<String>,
    ) -> Self {
        let now = now_unix_secs();
        Self {
            id: Uuid::new_v4(),
            channel_id,
            root_message_id: root_message.id,
            title: title.into(),
            status: ThreadStatus::Active,
            reply_count: 0,
            unread_count: 0,
            participant_count: 1,
            created_at: now,
            last_activity_at: now,
            is_pinned: false,
            priority: ThreadPriority::Normal,
        }
    }

    /// Create a thread from API data.
    pub fn from_api(api_thread: &enya_team_api::ChatThread, root_message_id: MessageId) -> Self {
        Self {
            id: api_thread.id,
            channel_id: api_thread.channel_id,
            root_message_id,
            title: api_thread.title.clone(),
            status: if api_thread.resolved {
                ThreadStatus::Resolved
            } else {
                ThreadStatus::Active
            },
            reply_count: api_thread.message_count as usize,
            unread_count: 0,
            participant_count: 1, // API doesn't track this yet
            created_at: api_thread.created_at as f64,
            last_activity_at: api_thread
                .last_message_at
                .map(|t| t as f64)
                .unwrap_or(api_thread.created_at as f64),
            is_pinned: false,
            priority: ThreadPriority::Normal,
        }
    }

    /// Set the thread priority.
    pub fn with_priority(mut self, priority: ThreadPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Pin this thread.
    pub fn pin(&mut self) {
        self.is_pinned = true;
    }

    /// Unpin this thread.
    pub fn unpin(&mut self) {
        self.is_pinned = false;
    }

    /// Mark thread as resolved.
    pub fn resolve(&mut self) {
        self.status = ThreadStatus::Resolved;
    }

    /// Reopen a resolved thread.
    pub fn reopen(&mut self) {
        self.status = ThreadStatus::Active;
    }

    /// Archive the thread.
    pub fn archive(&mut self) {
        self.status = ThreadStatus::Archived;
    }

    /// Check if thread has unread messages.
    pub fn has_unread(&self) -> bool {
        self.unread_count > 0
    }

    /// Mark all messages as read.
    pub fn mark_read(&mut self) {
        self.unread_count = 0;
    }

    /// Add a reply to this thread.
    pub fn add_reply(&mut self) {
        self.reply_count += 1;
        self.unread_count += 1;
        self.last_activity_at = now_unix_secs();
    }

    /// Format last activity for display.
    pub fn relative_activity(&self) -> String {
        let now = now_unix_secs();
        let diff = now - self.last_activity_at;

        if diff < 60.0 {
            "just now".to_string()
        } else if diff < 3600.0 {
            let mins = (diff / 60.0) as u32;
            format!("{mins}m")
        } else if diff < 86400.0 {
            let hours = (diff / 3600.0) as u32;
            format!("{hours}h")
        } else {
            let days = (diff / 86400.0) as u32;
            format!("{days}d")
        }
    }

    /// Get a summary for display (e.g., "3 replies").
    pub fn reply_summary(&self) -> String {
        match self.reply_count {
            0 => "No replies".to_string(),
            1 => "1 reply".to_string(),
            n => format!("{n} replies"),
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
    use enya_team_api::UserId;

    use super::*;

    #[test]
    fn test_thread_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_thread_creation() {
        let channel_id = Uuid::new_v4();
        let msg = ChatMessage::from_user(UserId::new_v4(), "Alice", "P99 latency spike detected");
        let thread = Thread::new(channel_id, &msg, "P99 incident");

        assert_eq!(thread.title, "P99 incident");
        assert_eq!(thread.status, ThreadStatus::Active);
        assert_eq!(thread.reply_count, 0);
    }

    #[test]
    fn test_thread_replies() {
        let channel_id = Uuid::new_v4();
        let msg = ChatMessage::from_user(UserId::new_v4(), "Alice", "Starting investigation");
        let mut thread = Thread::new(channel_id, &msg, "Investigation");

        thread.add_reply();
        thread.add_reply();

        assert_eq!(thread.reply_count, 2);
        assert_eq!(thread.unread_count, 2);
        assert_eq!(thread.reply_summary(), "2 replies");
    }

    #[test]
    fn test_thread_resolve() {
        let channel_id = Uuid::new_v4();
        let msg = ChatMessage::from_user(UserId::new_v4(), "Alice", "Issue found");
        let mut thread = Thread::new(channel_id, &msg, "Bug report");

        assert_eq!(thread.status, ThreadStatus::Active);

        thread.resolve();
        assert_eq!(thread.status, ThreadStatus::Resolved);

        thread.reopen();
        assert_eq!(thread.status, ThreadStatus::Active);
    }
}
