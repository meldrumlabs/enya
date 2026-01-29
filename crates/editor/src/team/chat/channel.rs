//! Channel data model for team chat.
//!
//! Channels are organized containers for conversations, similar to Slack or Zed.
//! Each channel can contain multiple threads.

use uuid::Uuid;

/// Unique identifier for a channel (matches API type).
pub type ChannelId = Uuid;

/// The kind/category of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelKind {
    /// General discussion channel.
    #[default]
    General,
    /// Incident response channel.
    Incidents,
    /// Deployment-related discussions.
    Deployments,
    /// Alerts and monitoring.
    Alerts,
    /// Custom channel.
    Custom,
}

impl ChannelKind {
    /// Get the icon for this channel kind.
    pub fn icon(&self) -> &'static str {
        use egui_nerdfonts::regular;
        match self {
            Self::General => regular::COMMENT,
            Self::Incidents => regular::ALERT,
            Self::Deployments => regular::ROCKET,
            Self::Alerts => regular::BELL,
            Self::Custom => regular::HASH,
        }
    }

    /// Get the default color for this channel kind.
    pub fn color(&self) -> egui::Color32 {
        use crate::ui::palette;
        match self {
            Self::General => palette::semantic::INFO,
            Self::Incidents => palette::semantic::ERROR,
            Self::Deployments => palette::semantic::SUCCESS,
            Self::Alerts => palette::semantic::WARNING,
            Self::Custom => palette::semantic::INFO,
        }
    }

    /// Convert from API ChannelKind.
    pub fn from_api(kind: enya_team_api::ChannelKind) -> Self {
        match kind {
            enya_team_api::ChannelKind::General => Self::General,
            enya_team_api::ChannelKind::Incidents => Self::Incidents,
            enya_team_api::ChannelKind::Deployments => Self::Deployments,
            enya_team_api::ChannelKind::Alerts => Self::Alerts,
            enya_team_api::ChannelKind::Custom => Self::Custom,
        }
    }

    /// Convert to API ChannelKind.
    pub fn to_api(&self) -> enya_team_api::ChannelKind {
        match self {
            Self::General => enya_team_api::ChannelKind::General,
            Self::Incidents => enya_team_api::ChannelKind::Incidents,
            Self::Deployments => enya_team_api::ChannelKind::Deployments,
            Self::Alerts => enya_team_api::ChannelKind::Alerts,
            Self::Custom => enya_team_api::ChannelKind::Custom,
        }
    }
}

/// A chat channel containing conversations.
#[derive(Debug, Clone)]
pub struct Channel {
    /// Unique identifier.
    pub id: ChannelId,
    /// Display name (e.g., "general", "incidents").
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Channel kind/category.
    pub kind: ChannelKind,
    /// Unread message count.
    pub unread_count: usize,
    /// Whether this channel is muted.
    pub is_muted: bool,
    /// Whether this channel is collapsed in the UI.
    pub is_collapsed: bool,
    /// Creation timestamp (Unix seconds).
    pub created_at: f64,
}

impl Channel {
    /// Create a new channel with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            kind: ChannelKind::General,
            unread_count: 0,
            is_muted: false,
            is_collapsed: false,
            created_at: now_unix_secs(),
        }
    }

    /// Create a channel from API data.
    pub fn from_api(api_channel: &enya_team_api::Channel) -> Self {
        Self {
            id: api_channel.id,
            name: api_channel.name.clone(),
            description: api_channel.description.clone(),
            kind: ChannelKind::from_api(api_channel.kind),
            unread_count: 0,
            is_muted: false,
            is_collapsed: false,
            created_at: api_channel.created_at as f64,
        }
    }

    /// Create a channel with a specific kind.
    pub fn with_kind(mut self, kind: ChannelKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the unread count.
    pub fn with_unread(mut self, count: usize) -> Self {
        self.unread_count = count;
        self
    }

    /// Check if channel has unread messages.
    pub fn has_unread(&self) -> bool {
        self.unread_count > 0 && !self.is_muted
    }

    /// Mark all messages as read.
    pub fn mark_read(&mut self) {
        self.unread_count = 0;
    }

    /// Toggle muted state.
    pub fn toggle_mute(&mut self) {
        self.is_muted = !self.is_muted;
    }

    /// Toggle collapsed state.
    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
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
    fn test_channel_id_uniqueness() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_channel_creation() {
        let channel = Channel::new("general")
            .with_kind(ChannelKind::General)
            .with_description("General discussion")
            .with_unread(5);

        assert_eq!(channel.name, "general");
        assert_eq!(channel.kind, ChannelKind::General);
        assert!(channel.has_unread());
        assert_eq!(channel.unread_count, 5);
    }

    #[test]
    fn test_channel_mute() {
        let mut channel = Channel::new("alerts").with_unread(10);
        assert!(channel.has_unread());

        channel.toggle_mute();
        assert!(!channel.has_unread()); // Muted channels don't show unread
    }
}
