//! Team collaboration state management.
//!
//! This module provides a decoupled interface to team collaboration features.
//! It wraps the `enya-team-api` crate and provides:
//!
//! - Optional team connectivity (editor works without team features)
//! - Connection state management
//! - Team member presence tracking
//! - Polling interface compatible with egui's immediate mode
//!
//! # Design
//!
//! The team functionality is designed to be non-invasive:
//! - When not configured, `TeamState::default()` returns a disconnected state
//! - All team UI components check `is_connected()` before rendering
//! - The editor continues to function normally without team features

use enya_team_api::{Team, TeamConnectionStatus, TeamEvent, TeamManager, User, UserId};

use crate::chat::ChatState;
use crate::components::TeamMember;
use crate::components::widget::team_menu::MemberPresence;
use crate::components::widget::team_status::TeamStatusInfo;

/// Demo mode info for testing the UI without a backend.
#[derive(Debug, Clone)]
struct DemoTeamInfo {
    team_name: String,
    #[allow(dead_code)]
    current_user_id: UserId,
}

/// Configuration for team collaboration.
#[derive(Debug, Clone, Default)]
pub struct TeamConfig {
    /// Base URL of the team server (e.g., "https://api.enya.dev").
    pub server_url: Option<String>,
    /// Authentication token (obtained via OAuth).
    pub auth_token: Option<String>,
}

impl TeamConfig {
    /// Check if team features are configured (both server URL and token required).
    pub fn is_configured(&self) -> bool {
        self.server_url.is_some() && self.auth_token.is_some()
    }
}

/// Decoupled team collaboration state for the editor.
///
/// This wraps `TeamManager` and provides a clean interface that:
/// - Works when team features are disabled (returns default/empty values)
/// - Handles async operations via polling
/// - Tracks member presence and unread notifications
pub struct TeamState {
    /// Team manager.
    manager: TeamManager,
    /// Whether team features are enabled.
    enabled: bool,
    /// Cached team members with presence info.
    members: Vec<TeamMember>,
    /// Unread notification count.
    unread_count: usize,
    /// Demo mode info (for testing UI without backend).
    demo_mode: Option<DemoTeamInfo>,
    /// Chat state for channels, threads, and messages.
    chat_state: ChatState,
}

impl Default for TeamState {
    fn default() -> Self {
        Self {
            manager: TeamManager::new(),
            enabled: false,
            members: Vec::new(),
            unread_count: 0,
            demo_mode: None,
            chat_state: ChatState::new(),
        }
    }
}

impl TeamState {
    /// Create a new team state with the given configuration.
    ///
    /// If server URL and auth token are provided, automatically connects.
    pub fn new(config: TeamConfig, ctx: &egui::Context) -> Self {
        let mut state = Self {
            manager: TeamManager::new(),
            enabled: config.is_configured(),
            members: Vec::new(),
            unread_count: 0,
            demo_mode: None,
            chat_state: ChatState::new(),
        };

        // Auto-connect if configured
        if let (Some(server_url), Some(auth_token)) = (&config.server_url, &config.auth_token) {
            state.manager.connect(server_url, auth_token, ctx);
        }

        state
    }

    /// Create team state without auto-connecting.
    pub fn new_disabled() -> Self {
        Self::default()
    }

    /// Create a demo team state with mock data for testing the UI.
    ///
    /// This bypasses the real TeamManager and populates with fake team members
    /// so we can test the team UI without a backend.
    pub fn new_demo() -> Self {
        let mut state = Self::default();
        state.enable_demo_mode();
        state
    }

    /// Check if running in demo mode.
    pub fn is_demo(&self) -> bool {
        self.demo_mode.is_some()
    }

    /// Enable demo mode with mock data for testing the UI.
    ///
    /// This can be called on an existing TeamState to switch it to demo mode.
    pub fn enable_demo_mode(&mut self) {
        // Create mock users
        let users = vec![
            User {
                id: UserId::new_v4(),
                display_name: "Alice Chen".to_string(),
                email: Some("alice@example.com".to_string()),
                avatar_url: None,
            },
            User {
                id: UserId::new_v4(),
                display_name: "Bob Smith".to_string(),
                email: Some("bob@example.com".to_string()),
                avatar_url: None,
            },
            User {
                id: UserId::new_v4(),
                display_name: "Carol Davis".to_string(),
                email: Some("carol@example.com".to_string()),
                avatar_url: None,
            },
            User {
                id: UserId::new_v4(),
                display_name: "You".to_string(),
                email: Some("you@example.com".to_string()),
                avatar_url: None,
            },
        ];

        let current_user_id = users[3].id;

        // Create team members with varying presence
        self.members = vec![
            TeamMember {
                user: users[0].clone(),
                presence: MemberPresence::Online,
                viewing: Some("P99 Latency".to_string()),
                is_self: false,
            },
            TeamMember {
                user: users[1].clone(),
                presence: MemberPresence::Idle,
                viewing: None,
                is_self: false,
            },
            TeamMember {
                user: users[2].clone(),
                presence: MemberPresence::Offline,
                viewing: None,
                is_self: false,
            },
            TeamMember {
                user: users[3].clone(),
                presence: MemberPresence::Online,
                viewing: None,
                is_self: true,
            },
        ];

        self.enabled = true;
        self.unread_count = 2;
        self.demo_mode = Some(DemoTeamInfo {
            team_name: "Acme SRE".to_string(),
            current_user_id,
        });

        // Initialize chat state with demo data
        // Pass user IDs so messages are linked to the same users as team members
        let demo_users: Vec<(UserId, &str)> = users
            .iter()
            .map(|u| (u.id, u.display_name.as_str()))
            .collect();
        self.chat_state = ChatState::new_demo(&demo_users);
    }

    /// Disable demo mode and return to disconnected state.
    pub fn disable_demo_mode(&mut self) {
        self.demo_mode = None;
        self.members.clear();
        self.unread_count = 0;
        self.enabled = false;
        self.chat_state = ChatState::new();
    }

    /// Toggle demo mode on/off.
    pub fn toggle_demo_mode(&mut self) {
        if self.is_demo() {
            self.disable_demo_mode();
        } else {
            self.enable_demo_mode();
        }
    }

    /// Check if team features are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Check if connected to a team.
    pub fn is_connected(&self) -> bool {
        self.demo_mode.is_some() || self.manager.is_connected()
    }

    /// Get the current connection status.
    pub fn connection_status(&self) -> &TeamConnectionStatus {
        self.manager.status()
    }

    /// Get the current user (if connected).
    pub fn current_user(&self) -> Option<&User> {
        self.manager.current_user()
    }

    /// Get the current team (if connected).
    pub fn current_team(&self) -> Option<&Team> {
        self.manager.current_team()
    }

    /// Get the current user ID (if connected).
    pub fn current_user_id(&self) -> Option<UserId> {
        self.current_user().map(|u| u.id)
    }

    /// Get team members with presence info.
    pub fn members(&self) -> &[TeamMember] {
        &self.members
    }

    /// Get the chat state for channels, threads, and messages.
    pub fn chat_state(&self) -> &ChatState {
        &self.chat_state
    }

    /// Get mutable access to the chat state.
    pub fn chat_state_mut(&mut self) -> &mut ChatState {
        &mut self.chat_state
    }

    /// Get online member count.
    pub fn online_count(&self) -> usize {
        self.members
            .iter()
            .filter(|m| matches!(m.presence, MemberPresence::Online | MemberPresence::Idle))
            .count()
    }

    /// Get unread notification count.
    pub fn unread_count(&self) -> usize {
        self.unread_count
    }

    /// Build status info for UI display.
    /// Returns None if not connected (team UI should be hidden).
    pub fn status_info(&self) -> Option<TeamStatusInfo> {
        if !self.is_connected() {
            return None;
        }

        // In demo mode, build status info from demo data
        if let Some(ref demo) = self.demo_mode {
            return Some(TeamStatusInfo {
                is_connected: true,
                team_name: Some(demo.team_name.clone()),
                user_name: Some("You".to_string()),
                online_count: self.online_count(),
                unread_count: self.unread_count,
            });
        }

        Some(TeamStatusInfo::from_status(
            self.manager.status(),
            self.online_count(),
            self.unread_count,
        ))
    }

    /// Connect to a team server with credentials.
    pub fn connect(&mut self, server_url: &str, auth_token: &str, ctx: &egui::Context) {
        self.enabled = true;
        self.manager.connect(server_url, auth_token, ctx);
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.manager.disconnect();
        self.members.clear();
        self.unread_count = 0;
    }

    /// Poll for updates (call each frame).
    ///
    /// This handles:
    /// - Connection state changes
    /// - Real-time events (mentions, presence changes)
    /// - Updating member list and unread count
    pub fn poll(&mut self, _ctx: &egui::Context) {
        if !self.enabled {
            return;
        }

        // Poll the manager for events
        let events = self.manager.poll();

        // Process events
        for event in events {
            match event {
                TeamEvent::Mentioned { .. } => {
                    self.unread_count += 1;
                }
                TeamEvent::MessageReceived { .. } => {
                    // Could increment unread if in a subscribed thread
                }
                TeamEvent::PresenceChanged { user_id, online } => {
                    // Update presence for the user
                    if let Some(member) = self.members.iter_mut().find(|m| m.user.id == user_id) {
                        member.presence = if online {
                            MemberPresence::Online
                        } else {
                            MemberPresence::Offline
                        };
                    }
                }
                TeamEvent::AnnotationCreated { .. }
                | TeamEvent::AnnotationUpdated { .. }
                | TeamEvent::AnnotationDeleted { .. }
                | TeamEvent::UserTyping { .. }
                | TeamEvent::ViewShared { .. }
                | TeamEvent::MemberJoined { .. }
                | TeamEvent::MemberLeft { .. } => {
                    // These don't affect presence or unread count
                    // TODO: Handle MemberJoined/Left to update member list
                }
            }
        }

        // If we just connected and don't have members, populate from team
        if self.is_connected() && self.members.is_empty() {
            if let Some(team) = self.current_team() {
                let current_id = self.current_user_id();
                self.members = team
                    .members
                    .iter()
                    .map(|user| TeamMember {
                        user: user.clone(),
                        presence: MemberPresence::Online, // Default to online
                        viewing: None,
                        is_self: current_id == Some(user.id),
                    })
                    .collect();
            }
        }
    }

    /// Mark all notifications as read.
    pub fn mark_read(&mut self) {
        self.unread_count = 0;
    }

    /// Share the current workspace view with the team.
    pub fn share_view(&mut self, workspace_url: &str, ctx: &egui::Context) {
        if self.is_connected() {
            self.manager.share_view(workspace_url, ctx);
            log::info!("Sharing workspace view with team");
        }
    }

    /// Get the underlying manager for advanced operations.
    /// Use sparingly - prefer using TeamState methods.
    pub fn manager(&self) -> &TeamManager {
        &self.manager
    }

    /// Get mutable access to the underlying manager.
    pub fn manager_mut(&mut self) -> &mut TeamManager {
        &mut self.manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state_is_disconnected() {
        let state = TeamState::default();
        assert!(!state.is_enabled());
        assert!(!state.is_connected());
        assert!(state.members().is_empty());
        assert_eq!(state.online_count(), 0);
        assert!(state.status_info().is_none());
    }

    #[test]
    fn test_config_is_configured() {
        let config = TeamConfig::default();
        assert!(!config.is_configured());

        let config = TeamConfig {
            server_url: Some("https://api.enya.dev".into()),
            auth_token: None,
        };
        assert!(!config.is_configured()); // Both required

        let config = TeamConfig {
            server_url: Some("https://api.enya.dev".into()),
            auth_token: Some("token".into()),
        };
        assert!(config.is_configured());
    }

    #[test]
    fn test_disabled_state() {
        let state = TeamState::new_disabled();
        assert!(!state.is_enabled());
        assert!(!state.is_connected());
    }
}
