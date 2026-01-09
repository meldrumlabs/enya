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

use enya_team_api::{
    Team, TeamConnectionStatus, TeamEvent, TeamManager, User, UserId, WsConnectionState,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::AsyncRuntime;

use crate::chat::{ChannelId, ChatState, ThreadId};
use crate::components::TeamMember;
use crate::components::widget::team_menu::MemberPresence;
use crate::components::widget::team_status::{TeamStatusInfo, WsState};

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
    /// Async runtime for spawning background tasks.
    #[cfg(not(target_arch = "wasm32"))]
    async_runtime: Option<AsyncRuntime>,
}

impl Default for TeamState {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            manager: TeamManager::new_offline(),
            #[cfg(target_arch = "wasm32")]
            manager: TeamManager::new(),
            enabled: false,
            members: Vec::new(),
            unread_count: 0,
            demo_mode: None,
            chat_state: ChatState::new(),
            #[cfg(not(target_arch = "wasm32"))]
            async_runtime: None,
        }
    }
}

impl TeamState {
    /// Create a new team state with the given configuration (native).
    ///
    /// If server URL and auth token are provided, automatically connects.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(config: TeamConfig, async_runtime: AsyncRuntime, ctx: &egui::Context) -> Self {
        let mut manager = TeamManager::new(async_runtime.handle().clone());

        let enabled = config.is_configured();

        // Auto-connect if configured
        if let (Some(server_url), Some(auth_token)) = (&config.server_url, &config.auth_token) {
            manager.connect(server_url, auth_token, ctx);
        }

        Self {
            manager,
            enabled,
            members: Vec::new(),
            unread_count: 0,
            demo_mode: None,
            chat_state: ChatState::new(),
            async_runtime: Some(async_runtime),
        }
    }

    /// Create a new team state with the given configuration (WASM).
    ///
    /// If server URL and auth token are provided, automatically connects.
    #[cfg(target_arch = "wasm32")]
    pub fn new(config: TeamConfig, ctx: &egui::Context) -> Self {
        let mut manager = TeamManager::new();

        let enabled = config.is_configured();

        // Auto-connect if configured
        if let (Some(server_url), Some(auth_token)) = (&config.server_url, &config.auth_token) {
            manager.connect(server_url, auth_token, ctx);
        }

        Self {
            manager,
            enabled,
            members: Vec::new(),
            unread_count: 0,
            demo_mode: None,
            chat_state: ChatState::new(),
        }
    }

    /// Create team state without auto-connecting.
    pub fn new_disabled() -> Self {
        Self::default()
    }

    /// Set the async runtime (native only).
    ///
    /// This allows setting the runtime after construction, which is useful
    /// when the TeamState is created before the runtime is available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_async_runtime(&mut self, runtime: AsyncRuntime) {
        self.manager.set_runtime_handle(runtime.handle().clone());
        self.async_runtime = Some(runtime);
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

    /// Get the WebSocket connection state.
    pub fn ws_state(&self) -> &WsConnectionState {
        self.manager.ws_state()
    }

    /// Check if WebSocket is connected.
    pub fn is_ws_connected(&self) -> bool {
        self.demo_mode.is_some() || self.manager.is_ws_connected()
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
                ws_state: WsState::Connected, // Demo mode simulates connected
            });
        }

        Some(TeamStatusInfo::from_status(
            self.manager.status(),
            self.manager.ws_state(),
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
        self.chat_state.clear();
    }

    /// Poll for updates (call each frame).
    ///
    /// This handles:
    /// - Connection state changes
    /// - Real-time events (mentions, presence changes)
    /// - Syncing channels/threads/messages from TeamManager to ChatState
    /// - Updating member list and unread count
    pub fn poll(&mut self, ctx: &egui::Context) {
        if !self.enabled || self.demo_mode.is_some() {
            return;
        }

        // Poll the manager for events
        let events = self.manager.poll();

        // Process real-time events
        for event in events {
            match event {
                TeamEvent::Mentioned { .. } => {
                    self.unread_count += 1;
                }
                TeamEvent::MessageReceived {
                    thread_id,
                    message,
                    author,
                } => {
                    // Add the message to chat state
                    self.chat_state
                        .add_message_from_api(&message, &author.display_name, thread_id);
                    // Also update manager cache
                    self.manager.add_message_to_cache(thread_id, message);
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
                TeamEvent::ChannelCreated { channel } => {
                    log::info!("Received ChannelCreated event: {}", channel.name);
                    // Add to chat state only (manager cache is already updated by manager.poll()
                    // when the create response arrives, so we don't need to add again)
                    self.chat_state.add_channel_from_api(&channel);
                }
                TeamEvent::ThreadCreated {
                    thread,
                    initial_message,
                    author,
                } => {
                    // Add to chat state
                    self.chat_state.add_thread_from_api(&thread);
                    self.chat_state.add_message_from_api(
                        &initial_message,
                        &author.display_name,
                        thread.id,
                    );
                    // Add to manager cache
                    self.manager.add_thread_to_cache(thread.clone());
                    self.manager
                        .add_message_to_cache(thread.id, initial_message);
                }
                TeamEvent::ThreadResolved {
                    thread_id,
                    channel_id,
                } => {
                    // Invalidate caches to refetch with updated resolved status
                    self.manager.invalidate_threads(channel_id);
                    let _ = thread_id;
                }
                TeamEvent::AnnotationCreated { .. }
                | TeamEvent::AnnotationUpdated { .. }
                | TeamEvent::AnnotationDeleted { .. }
                | TeamEvent::UserTyping { .. }
                | TeamEvent::ViewShared { .. }
                | TeamEvent::MemberJoined { .. }
                | TeamEvent::MemberLeft { .. } => {
                    // These don't affect chat state
                }
            }
        }

        // Sync channels from manager to chat state
        if self.is_connected() {
            let channels = self.manager.get_channels(ctx);
            // Sync if we have channels from API and either:
            // - chat state has no channels (initial sync)
            // - manager has more channels (new channel was created)
            let chat_channel_count = self.chat_state.channels().len();
            if !channels.is_empty()
                && (chat_channel_count == 0 || channels.len() > chat_channel_count)
            {
                log::info!(
                    "Syncing channels: manager has {}, chat state has {}",
                    channels.len(),
                    chat_channel_count
                );
                self.chat_state.sync_channels(channels);
            }

            // Sync threads for selected channel
            if let Some(channel_id) = self.chat_state.selected_channel() {
                let threads = self.manager.get_threads(channel_id, ctx);
                if !threads.is_empty() && self.chat_state.channel_threads(channel_id).is_empty() {
                    self.chat_state.sync_threads(channel_id, threads);
                }
            }

            // Sync messages for selected thread
            if let Some(thread_id) = self.chat_state.selected_thread() {
                if let Some(channel_id) = self.chat_state.selected_channel() {
                    let messages = self.manager.get_thread_messages(channel_id, thread_id, ctx);
                    if !messages.is_empty() && self.chat_state.thread_messages(thread_id).is_empty()
                    {
                        // Build author name lookup from members
                        let members = &self.members;
                        self.chat_state
                            .sync_thread_messages(thread_id, messages, |user_id| {
                                members
                                    .iter()
                                    .find(|m| m.user.id == user_id)
                                    .map(|m| m.user.display_name.clone())
                                    .unwrap_or_else(|| "Unknown".to_string())
                            });
                    }
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

    /// Send a message to the currently selected thread.
    pub fn send_message(&mut self, content: &str, ctx: &egui::Context) {
        if self.demo_mode.is_some() {
            // In demo mode, add message directly to chat state
            if let (Some(thread_id), Some(user)) = (
                self.chat_state.selected_thread(),
                self.demo_mode.as_ref().map(|d| d.current_user_id),
            ) {
                let msg =
                    crate::chat::ChatMessage::from_user(user, "You", content).in_thread(thread_id);
                self.chat_state.add_message(msg);
            }
            return;
        }

        if let (Some(channel_id), Some(thread_id)) = (
            self.chat_state.selected_channel(),
            self.chat_state.selected_thread(),
        ) {
            let message = enya_team_api::NewMessage {
                content: content.to_string(),
                inline_chart: None,
            };
            self.manager
                .send_channel_message(channel_id, thread_id, &message, ctx);
        }
    }

    /// Send typing indicator start.
    pub fn start_typing(&mut self) {
        if self.demo_mode.is_some() {
            return;
        }

        if let Some(thread_id) = self.chat_state.selected_thread() {
            self.manager.send_start_typing(thread_id);
        }
    }

    /// Send typing indicator stop.
    pub fn stop_typing(&mut self) {
        if self.demo_mode.is_some() {
            return;
        }

        if let Some(thread_id) = self.chat_state.selected_thread() {
            self.manager.send_stop_typing(thread_id);
        }
    }

    /// Force refresh channels from the server.
    pub fn refresh_channels(&mut self) {
        self.manager.invalidate_channels();
    }

    /// Force refresh threads for the selected channel.
    pub fn refresh_threads(&mut self) {
        if let Some(channel_id) = self.chat_state.selected_channel() {
            self.manager.invalidate_threads(channel_id);
        }
    }

    /// Force refresh messages for the selected thread.
    pub fn refresh_messages(&mut self) {
        if let Some(thread_id) = self.chat_state.selected_thread() {
            self.manager.invalidate_thread_messages(thread_id);
        }
    }

    /// Select a channel and trigger data fetch.
    pub fn select_channel(&mut self, channel_id: ChannelId, ctx: &egui::Context) {
        self.chat_state.select_channel(channel_id);

        // Pre-fetch threads for the channel
        if self.demo_mode.is_none() {
            let _ = self.manager.get_threads(channel_id, ctx);
        }
    }

    /// Select a thread and trigger data fetch.
    pub fn select_thread(&mut self, thread_id: ThreadId, ctx: &egui::Context) {
        self.chat_state.select_thread(thread_id);

        // Pre-fetch messages for the thread
        if self.demo_mode.is_none() {
            if let Some(channel_id) = self.chat_state.selected_channel() {
                let _ = self.manager.get_thread_messages(channel_id, thread_id, ctx);
            }
        }
    }

    /// Create a new channel.
    pub fn create_channel(&mut self, name: &str, ctx: &egui::Context) {
        use enya_team_api::NewChannel;

        if self.demo_mode.is_some() {
            // In demo mode, create a local channel
            let now = crate::util::now_unix_secs() as f64;
            let channel = crate::chat::Channel {
                id: uuid::Uuid::new_v4(),
                name: name.to_string(),
                description: None,
                kind: crate::chat::ChannelKind::General,
                unread_count: 0,
                is_muted: false,
                is_collapsed: false,
                created_at: now,
            };
            self.chat_state.add_channel(channel);
            log::info!("Created demo channel: {name}");
        } else {
            // In live mode, call the API
            let channel = NewChannel {
                name: name.to_string(),
                description: None,
                kind: enya_team_api::ChannelKind::General,
            };
            self.manager.create_channel(&channel, ctx);
            log::info!("Creating channel via API: {name}");
        }
    }

    /// Create a new thread in a channel.
    pub fn create_thread(&mut self, channel_id: ChannelId, title: &str, ctx: &egui::Context) {
        use crate::chat::thread::ThreadPriority;
        use enya_team_api::NewThread;

        if self.demo_mode.is_some() {
            // In demo mode, create a local thread
            let now = crate::util::now_unix_secs() as f64;
            let thread = crate::chat::Thread {
                id: uuid::Uuid::new_v4(),
                channel_id,
                root_message_id: uuid::Uuid::new_v4(), // Placeholder root message
                title: title.to_string(),
                status: crate::chat::ThreadStatus::Active,
                reply_count: 0,
                unread_count: 0,
                participant_count: 1,
                created_at: now,
                last_activity_at: now,
                is_pinned: false,
                priority: ThreadPriority::Normal,
            };
            self.chat_state.add_thread(thread);
            log::info!("Created demo thread: {title}");
        } else {
            // In live mode, call the API
            let thread = NewThread {
                title: title.to_string(),
                initial_message: title.to_string(), // Use title as initial message
                inline_chart: None,
            };
            self.manager.create_thread(channel_id, &thread, ctx);
            log::info!("Creating thread via API: {title}");
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
