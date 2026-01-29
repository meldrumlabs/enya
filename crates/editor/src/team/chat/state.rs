//! Chat state management for channels, threads, and messages.
//!
//! This module provides state management for team chat, including:
//! - Channel management (create, select, mute)
//! - Thread tracking (active, resolved)
//! - Message storage and retrieval
//! - Demo data for testing

use enya_team_api::UserId;

use super::chat_view::{
    BarData, InlineBarChart, InlineChart, InlineStat, InlineTable, InlineVisualization, StatTrend,
};
use super::thread::ThreadPriority;
use super::{
    Channel, ChannelId, ChannelKind, ChatMessage, MessageId, Thread, ThreadId, ThreadStatus,
};
use crate::components::pane::time_series_chart::{DataPoint, Series};

/// State management for team chat.
#[derive(Debug, Clone, Default)]
pub struct ChatState {
    /// All channels.
    channels: Vec<Channel>,
    /// All threads.
    threads: Vec<Thread>,
    /// All messages.
    messages: Vec<ChatMessage>,
    /// Currently selected channel.
    selected_channel: Option<ChannelId>,
    /// Currently selected thread.
    selected_thread: Option<ThreadId>,
    /// Whether demo mode is enabled.
    demo_mode: bool,
}

impl ChatState {
    /// Create a new empty chat state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a chat state with demo data.
    pub fn new_demo(demo_users: &[(UserId, &str)]) -> Self {
        let mut state = Self::new();
        state.populate_demo_data(demo_users);
        state
    }

    /// Get all channels.
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// Get all threads.
    pub fn threads(&self) -> &[Thread] {
        &self.threads
    }

    /// Get active threads (for the threads-first panel).
    pub fn active_threads(&self) -> Vec<&Thread> {
        self.threads
            .iter()
            .filter(|t| {
                t.status == ThreadStatus::Active
                    && (t.has_unread() || t.is_pinned || t.priority == ThreadPriority::Critical)
            })
            .collect()
    }

    /// Get all messages.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Get messages for a specific channel (root messages only).
    pub fn channel_messages(&self, channel_id: ChannelId) -> Vec<&ChatMessage> {
        // First, find threads in this channel
        let thread_ids: Vec<ThreadId> = self
            .threads
            .iter()
            .filter(|t| t.channel_id == channel_id)
            .map(|t| t.id)
            .collect();

        // Return messages that are either:
        // 1. Root messages (thread_id is None) - these need to be associated with a thread root
        // 2. Or we need to find root messages by their IDs
        self.messages
            .iter()
            .filter(|m| {
                m.thread_id.is_none()
                    || thread_ids
                        .iter()
                        .any(|tid| m.thread_id == Some(*tid) && self.is_root_message(m.id, *tid))
            })
            .collect()
    }

    /// Check if a message is the root of a thread.
    fn is_root_message(&self, _message_id: MessageId, _thread_id: ThreadId) -> bool {
        // For now, just check if thread_id is None (root messages)
        false
    }

    /// Get messages in a specific thread.
    pub fn thread_messages(&self, thread_id: ThreadId) -> Vec<&ChatMessage> {
        self.messages
            .iter()
            .filter(|m| m.thread_id == Some(thread_id))
            .collect()
    }

    /// Get the selected channel.
    pub fn selected_channel(&self) -> Option<ChannelId> {
        self.selected_channel
    }

    /// Get the selected thread.
    pub fn selected_thread(&self) -> Option<ThreadId> {
        self.selected_thread
    }

    /// Select a channel.
    pub fn select_channel(&mut self, id: ChannelId) {
        self.selected_channel = Some(id);
        self.selected_thread = None;

        // Mark channel as read
        if let Some(channel) = self.channels.iter_mut().find(|c| c.id == id) {
            channel.mark_read();
        }
    }

    /// Select a thread.
    pub fn select_thread(&mut self, id: ThreadId) {
        self.selected_thread = Some(id);

        // Mark thread as read
        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == id) {
            thread.mark_read();
        }
    }

    /// Add a channel.
    pub fn add_channel(&mut self, channel: Channel) {
        self.channels.push(channel);
    }

    /// Add a thread.
    pub fn add_thread(&mut self, thread: Thread) {
        self.threads.push(thread);
    }

    /// Add a message.
    pub fn add_message(&mut self, message: ChatMessage) {
        // Update thread reply count if applicable
        if let Some(thread_id) = message.thread_id {
            if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                thread.add_reply();
            }
        }

        // Update channel unread count
        // (In a real app, we'd check if the user authored this message)
        self.messages.push(message);
    }

    /// Check if demo mode is enabled.
    pub fn is_demo(&self) -> bool {
        self.demo_mode
    }

    /// Populate with demo data for testing.
    pub fn populate_demo_data(&mut self, users: &[(UserId, &str)]) {
        self.demo_mode = true;

        // Create demo channels
        let general = Channel::new("general")
            .with_kind(ChannelKind::General)
            .with_description("General team discussion");

        let incidents = Channel::new("incidents")
            .with_kind(ChannelKind::Incidents)
            .with_description("Incident response and tracking")
            .with_unread(2);

        let deployments = Channel::new("deployments")
            .with_kind(ChannelKind::Deployments)
            .with_description("Deployment notifications")
            .with_unread(1);

        let alerts = Channel::new("alerts")
            .with_kind(ChannelKind::Alerts)
            .with_description("System alerts and monitoring");

        self.channels = vec![general, incidents.clone(), deployments.clone(), alerts];

        // Get user IDs (or use defaults)
        let (alice_id, alice_name) = users
            .first()
            .copied()
            .unwrap_or_else(|| (UserId::new_v4(), "Alice"));
        let (bob_id, bob_name) = users
            .get(1)
            .copied()
            .unwrap_or_else(|| (UserId::new_v4(), "Bob"));
        let (you_id, you_name) = users
            .get(3)
            .copied()
            .unwrap_or_else(|| (UserId::new_v4(), "You"));

        // Create demo messages and threads
        // Create P99 latency spike data (snapshot at share time)
        let now = now_unix_secs();
        let latency_series = Series {
            name: "p99_latency".to_string(),
            tags: Default::default(),
            points: vec![
                DataPoint {
                    timestamp: now - 3600.0,
                    value: 120.0,
                },
                DataPoint {
                    timestamp: now - 3000.0,
                    value: 125.0,
                },
                DataPoint {
                    timestamp: now - 2400.0,
                    value: 118.0,
                },
                DataPoint {
                    timestamp: now - 1800.0,
                    value: 450.0, // Spike starts
                },
                DataPoint {
                    timestamp: now - 1200.0,
                    value: 520.0, // Peak
                },
                DataPoint {
                    timestamp: now - 600.0,
                    value: 490.0,
                },
                DataPoint {
                    timestamp: now - 300.0,
                    value: 180.0, // Recovery
                },
                DataPoint {
                    timestamp: now,
                    value: 130.0,
                },
            ],
            color: None,
        };

        let incident_msg = ChatMessage::from_user(
            alice_id,
            alice_name,
            "P99 latency spike detected on api-gateway. Investigating...",
        )
        .with_inline_chart(
            InlineChart::new("P99 Latency (ms)")
                .with_series(latency_series)
                .with_height(150.0),
        );
        let incident_thread = Thread::new(incidents.id, &incident_msg, "P99 latency spike")
            .with_priority(ThreadPriority::Critical);
        let incident_thread_id = incident_thread.id;

        let deploy_msg = ChatMessage::from_user(
            bob_id,
            bob_name,
            "Deploy v2.3.1 to staging complete. Ready for review.",
        );
        let deploy_thread = Thread::new(deployments.id, &deploy_msg, "Deploy v2.3.1 review")
            .with_priority(ThreadPriority::High);
        let deploy_thread_id = deploy_thread.id;

        // Add threads
        let mut incident_thread = incident_thread;
        incident_thread.reply_count = 3;
        incident_thread.unread_count = 2;
        incident_thread.participant_count = 3;
        incident_thread.is_pinned = true;

        let mut deploy_thread = deploy_thread;
        deploy_thread.reply_count = 1;
        deploy_thread.unread_count = 1;
        deploy_thread.participant_count = 2;

        self.threads = vec![incident_thread, deploy_thread];

        // Add messages
        self.messages = vec![
            incident_msg,
            ChatMessage::from_user(bob_id, bob_name, "Seeing elevated error rates too. Checking logs.")
                .in_thread(incident_thread_id),
            // Agent response with stat card and error breakdown table
            ChatMessage::from_agent("claude-3", "Based on the metrics, the latency spike correlates with increased traffic from the EU region. The database connection pool may be saturated.")
                .in_thread(incident_thread_id)
                .with_visualization(InlineVisualization::Stat(
                    InlineStat::new("Current P99", "520ms")
                        .with_previous("125ms")
                        .with_trend(StatTrend::Up)
                        .with_subtitle("Last 15 minutes"),
                ))
                .with_visualization(InlineVisualization::Table(
                    InlineTable::new(
                        "Error Breakdown",
                        vec![
                            "Error Type".to_string(),
                            "Count".to_string(),
                            "% of Total".to_string(),
                        ],
                    )
                    .with_row(vec![
                        "Connection Timeout".to_string(),
                        "1,234".to_string(),
                        "45%".to_string(),
                    ])
                    .with_row(vec![
                        "Query Timeout".to_string(),
                        "856".to_string(),
                        "31%".to_string(),
                    ])
                    .with_row(vec![
                        "Pool Exhausted".to_string(),
                        "421".to_string(),
                        "15%".to_string(),
                    ])
                    .with_row(vec![
                        "Other".to_string(),
                        "247".to_string(),
                        "9%".to_string(),
                    ]),
                ))
                .with_visualization(InlineVisualization::BarChart(
                    InlineBarChart::new("Traffic by Region")
                        .with_bar(BarData::new("EU-West", 4250.0))
                        .with_bar(BarData::new("US-East", 2100.0))
                        .with_bar(BarData::new("US-West", 1800.0))
                        .with_bar(BarData::new("AP-South", 950.0)),
                )),
            ChatMessage::from_user(you_id, you_name, "Scaling up db replicas now")
                .in_thread(incident_thread_id),
            deploy_msg,
            ChatMessage::from_user(alice_id, alice_name, "LGTM! Approved for prod.")
                .in_thread(deploy_thread_id),
        ];
    }

    /// Find a channel by name.
    pub fn find_channel_by_name(&self, name: &str) -> Option<&Channel> {
        self.channels.iter().find(|c| c.name == name)
    }

    /// Get a channel by ID.
    pub fn get_channel(&self, id: ChannelId) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == id)
    }

    /// Get a thread by ID.
    pub fn get_thread(&self, id: ThreadId) -> Option<&Thread> {
        self.threads.iter().find(|t| t.id == id)
    }

    // =========================================================================
    // Sync methods for API data
    // =========================================================================

    /// Sync channels from API data.
    /// Replaces existing channels with new data from the server.
    pub fn sync_channels(&mut self, api_channels: &[enya_team_api::Channel]) {
        // Keep selected channel if it still exists
        let selected = self.selected_channel;

        self.channels = api_channels.iter().map(Channel::from_api).collect();

        // Restore selection if the channel still exists
        if let Some(id) = selected {
            if self.channels.iter().any(|c| c.id == id) {
                self.selected_channel = Some(id);
            } else {
                self.selected_channel = None;
            }
        }
    }

    /// Sync threads for a channel from API data.
    pub fn sync_threads(
        &mut self,
        channel_id: ChannelId,
        api_threads: &[enya_team_api::ChatThread],
    ) {
        // Remove existing threads for this channel
        self.threads.retain(|t| t.channel_id != channel_id);

        // Add new threads
        for api_thread in api_threads {
            // Use a nil UUID as placeholder for root_message_id since API doesn't provide it
            let thread = Thread::from_api(api_thread, uuid::Uuid::nil());
            self.threads.push(thread);
        }
    }

    /// Sync messages for a thread from API data.
    pub fn sync_thread_messages(
        &mut self,
        thread_id: ThreadId,
        api_messages: &[enya_team_api::Message],
        get_author_name: impl Fn(UserId) -> String,
    ) {
        // Remove existing messages for this thread
        self.messages.retain(|m| m.thread_id != Some(thread_id));

        // Add new messages
        for api_msg in api_messages {
            let author_name = get_author_name(api_msg.author_id);
            let msg = ChatMessage::from_api(api_msg, &author_name, Some(thread_id));
            self.messages.push(msg);
        }
    }

    /// Add a single channel from API (for real-time events).
    pub fn add_channel_from_api(&mut self, api_channel: &enya_team_api::Channel) {
        // Only add if not already present
        if !self.channels.iter().any(|c| c.id == api_channel.id) {
            self.channels.push(Channel::from_api(api_channel));
        }
    }

    /// Add a single thread from API (for real-time events).
    pub fn add_thread_from_api(&mut self, api_thread: &enya_team_api::ChatThread) {
        // Only add if not already present
        if !self.threads.iter().any(|t| t.id == api_thread.id) {
            let thread = Thread::from_api(api_thread, uuid::Uuid::nil());
            self.threads.push(thread);
        }
    }

    /// Add a single message from API (for real-time events).
    pub fn add_message_from_api(
        &mut self,
        api_message: &enya_team_api::Message,
        author_name: &str,
        thread_id: ThreadId,
    ) {
        // Only add if not already present
        if !self.messages.iter().any(|m| m.id == api_message.id) {
            let msg = ChatMessage::from_api(api_message, author_name, Some(thread_id));

            // Update thread reply count
            if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                thread.add_reply();
            }

            self.messages.push(msg);
        }
    }

    /// Clear all non-demo data (for disconnection).
    pub fn clear(&mut self) {
        if !self.demo_mode {
            self.channels.clear();
            self.threads.clear();
            self.messages.clear();
            self.selected_channel = None;
            self.selected_thread = None;
        }
    }

    /// Get threads for a specific channel.
    pub fn channel_threads(&self, channel_id: ChannelId) -> Vec<&Thread> {
        self.threads
            .iter()
            .filter(|t| t.channel_id == channel_id)
            .collect()
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
    fn test_chat_state_creation() {
        let state = ChatState::new();
        assert!(state.channels().is_empty());
        assert!(state.threads().is_empty());
        assert!(state.messages().is_empty());
    }

    #[test]
    fn test_demo_data() {
        let users = vec![
            (UserId::new_v4(), "Alice"),
            (UserId::new_v4(), "Bob"),
            (UserId::new_v4(), "Carol"),
            (UserId::new_v4(), "You"),
        ];
        let state = ChatState::new_demo(&users);

        assert!(state.is_demo());
        assert!(!state.channels().is_empty());
        assert!(!state.threads().is_empty());
        assert!(!state.messages().is_empty());
    }

    #[test]
    fn test_channel_selection() {
        let state = ChatState::new_demo(&[]);
        let mut state = state;

        if let Some(channel) = state.channels().first() {
            let id = channel.id;
            state.select_channel(id);
            assert_eq!(state.selected_channel(), Some(id));
        }
    }
}
