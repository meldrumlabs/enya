//! Team API state manager.
//!
//! Manages connection state, caches, and provides a high-level interface
//! for team collaboration features.

use poll_promise::Promise;
use rustc_hash::FxHashMap;
use std::collections::VecDeque;

use crate::client::TeamClient;
use crate::error::{TeamApiError, TeamApiResult};
use crate::now_unix_secs;
use crate::types::*;
use crate::websocket::{RealtimeClient, WsConnectionState};

/// Default timeout for API requests in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Manages team API state and provides high-level operations.
///
/// Similar to `QueryManager` in enya-client, this provides a polling-based
/// interface that works with egui's immediate mode rendering.
///
/// # Example
///
/// ```ignore
/// // Create with runtime handle (native) or default (WASM)
/// let mut manager = TeamManager::new(runtime_handle);
///
/// // Connect to server
/// manager.connect("https://api.enya.dev", "auth_token", &ctx);
///
/// // In update loop
/// for event in manager.poll() {
///     // Handle real-time events
/// }
///
/// // Get annotations (cached)
/// let annotations = manager.get_annotations("query_fingerprint", &ctx);
/// ```
pub struct TeamManager {
    /// HTTP client.
    client: Option<TeamClient>,
    /// Current connection status.
    status: TeamConnectionStatus,
    /// Current team ID (selected team).
    current_team_id: Option<TeamId>,
    /// Pending authentication promise (fetches current user).
    pending_auth: Option<Promise<TeamApiResult<User>>>,
    /// Auth token stored for WebSocket connection after auth.
    pending_auth_token: Option<String>,
    /// Pending teams fetch (after user auth succeeds).
    pending_teams: Option<Promise<TeamApiResult<Vec<Team>>>>,
    /// Authenticated user (stored while fetching teams).
    pending_user: Option<User>,
    /// Cached annotations by query fingerprint.
    annotation_cache: FxHashMap<String, CachedAnnotations>,
    /// Pending annotation fetches.
    pending_annotations: FxHashMap<String, PendingFetch<Vec<Annotation>>>,
    /// Pending message sends.
    pending_messages: VecDeque<PendingFetch<Message>>,
    /// Buffered events from WebSocket.
    event_buffer: VecDeque<TeamEvent>,
    /// Request timeout in seconds.
    timeout_secs: u64,
    /// Cached channels for current team.
    channel_cache: Option<CachedChannels>,
    /// Pending channel fetch.
    pending_channels: Option<PendingFetch<Vec<Channel>>>,
    /// Pending channel creates (we poll these to add to cache when done).
    pending_channel_creates: VecDeque<PendingFetch<Channel>>,
    /// Cached threads by channel ID.
    thread_cache: FxHashMap<ChannelId, CachedThreads>,
    /// Pending thread fetches by channel ID.
    pending_threads: FxHashMap<ChannelId, PendingFetch<Vec<ChatThread>>>,
    /// Cached messages by thread ID.
    message_cache: FxHashMap<ThreadId, CachedMessages>,
    /// Pending message fetches by thread ID.
    pending_thread_messages: FxHashMap<ThreadId, PendingFetch<Vec<Message>>>,
    /// WebSocket client for real-time events.
    realtime: RealtimeClient,
    /// Server URL for WebSocket reconnection.
    server_url: Option<String>,
    /// Tokio runtime handle for spawning async tasks (native only).
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: Option<tokio::runtime::Handle>,
}

/// Cached annotations with timestamp.
struct CachedAnnotations {
    annotations: Vec<Annotation>,
    /// Timestamp for cache expiration (to be used for staleness checks).
    #[allow(dead_code)]
    fetched_at: u64,
}

/// Cached channels with timestamp.
struct CachedChannels {
    channels: Vec<Channel>,
    #[allow(dead_code)]
    fetched_at: u64,
}

/// Cached threads with timestamp.
struct CachedThreads {
    threads: Vec<ChatThread>,
    #[allow(dead_code)]
    fetched_at: u64,
}

/// Cached messages with timestamp.
struct CachedMessages {
    messages: Vec<Message>,
    #[allow(dead_code)]
    fetched_at: u64,
}

/// A pending API request.
struct PendingFetch<T: Send + 'static> {
    promise: Promise<TeamApiResult<T>>,
    started_at: u64,
}

impl TeamManager {
    /// Create a new team manager (native).
    ///
    /// Requires a tokio runtime handle for spawning async HTTP requests.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            client: None,
            status: TeamConnectionStatus::Disconnected,
            current_team_id: None,
            pending_auth: None,
            pending_auth_token: None,
            pending_teams: None,
            pending_user: None,
            annotation_cache: FxHashMap::default(),
            pending_annotations: FxHashMap::default(),
            pending_messages: VecDeque::new(),
            event_buffer: VecDeque::new(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            channel_cache: None,
            pending_channels: None,
            pending_channel_creates: VecDeque::new(),
            thread_cache: FxHashMap::default(),
            pending_threads: FxHashMap::default(),
            message_cache: FxHashMap::default(),
            pending_thread_messages: FxHashMap::default(),
            realtime: RealtimeClient::new(),
            server_url: None,
            runtime_handle: Some(runtime_handle),
        }
    }

    /// Create a new team manager without a runtime (for demo/offline mode).
    ///
    /// This manager cannot make HTTP requests but can be used for demo mode.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_offline() -> Self {
        Self {
            client: None,
            status: TeamConnectionStatus::Disconnected,
            current_team_id: None,
            pending_auth: None,
            pending_auth_token: None,
            pending_teams: None,
            pending_user: None,
            annotation_cache: FxHashMap::default(),
            pending_annotations: FxHashMap::default(),
            pending_messages: VecDeque::new(),
            event_buffer: VecDeque::new(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            channel_cache: None,
            pending_channels: None,
            pending_channel_creates: VecDeque::new(),
            thread_cache: FxHashMap::default(),
            pending_threads: FxHashMap::default(),
            message_cache: FxHashMap::default(),
            pending_thread_messages: FxHashMap::default(),
            realtime: RealtimeClient::new(),
            server_url: None,
            runtime_handle: None,
        }
    }

    /// Create a new team manager (WASM).
    ///
    /// On WASM, no runtime handle is needed.
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        Self {
            client: None,
            status: TeamConnectionStatus::Disconnected,
            current_team_id: None,
            pending_auth: None,
            pending_auth_token: None,
            pending_teams: None,
            pending_user: None,
            annotation_cache: FxHashMap::default(),
            pending_annotations: FxHashMap::default(),
            pending_messages: VecDeque::new(),
            event_buffer: VecDeque::new(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            channel_cache: None,
            pending_channels: None,
            pending_channel_creates: VecDeque::new(),
            thread_cache: FxHashMap::default(),
            pending_threads: FxHashMap::default(),
            message_cache: FxHashMap::default(),
            pending_thread_messages: FxHashMap::default(),
            realtime: RealtimeClient::new(),
            server_url: None,
        }
    }

    /// Set the runtime handle (native only).
    ///
    /// This allows setting the runtime handle after construction,
    /// useful when the runtime isn't available at TeamManager creation time.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_runtime_handle(&mut self, handle: tokio::runtime::Handle) {
        self.runtime_handle = Some(handle);
    }

    /// Get the current connection status.
    pub fn status(&self) -> &TeamConnectionStatus {
        &self.status
    }

    /// Get the current team ID.
    pub fn current_team_id(&self) -> Option<TeamId> {
        self.current_team_id
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.status.is_connected()
    }

    /// Get the WebSocket connection state.
    pub fn ws_state(&self) -> &WsConnectionState {
        self.realtime.state()
    }

    /// Check if WebSocket is connected.
    pub fn is_ws_connected(&self) -> bool {
        self.realtime.is_connected()
    }

    /// Get the current user if connected.
    pub fn current_user(&self) -> Option<&User> {
        self.status.current_user()
    }

    /// Get the current team if connected.
    pub fn current_team(&self) -> Option<&Team> {
        self.status.current_team()
    }

    /// Connect to a team server with an auth token (native).
    ///
    /// This initiates authentication and fetches user/team info.
    /// After auth succeeds, the WebSocket connection is established.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn connect(&mut self, server_url: &str, auth_token: &str, ctx: &egui::Context) {
        // Check for runtime handle
        let Some(ref runtime_handle) = self.runtime_handle else {
            self.status = TeamConnectionStatus::Failed {
                error: "No runtime handle available. Call set_runtime_handle first.".to_string(),
            };
            return;
        };

        // Store URL and token for WebSocket connection after auth
        self.server_url = Some(server_url.to_string());

        // Create client with runtime handle
        let mut client = TeamClient::new(server_url, runtime_handle.clone());
        client.set_auth_token(auth_token);

        // Start auth check - fetch current user
        self.status = TeamConnectionStatus::Connecting;
        self.pending_auth = Some(client.get_current_user(ctx));
        self.pending_auth_token = Some(auth_token.to_string());

        self.client = Some(client);
    }

    /// Connect to a team server with an auth token (WASM).
    ///
    /// This initiates authentication and fetches user/team info.
    /// After auth succeeds, the WebSocket connection is established.
    #[cfg(target_arch = "wasm32")]
    pub fn connect(&mut self, server_url: &str, auth_token: &str, ctx: &egui::Context) {
        // Store URL and token for WebSocket connection after auth
        self.server_url = Some(server_url.to_string());

        // Create client (WASM doesn't need runtime handle)
        let mut client = TeamClient::new(server_url);
        client.set_auth_token(auth_token);

        // Start auth check - fetch current user
        self.status = TeamConnectionStatus::Connecting;
        self.pending_auth = Some(client.get_current_user(ctx));
        self.pending_auth_token = Some(auth_token.to_string());

        self.client = Some(client);
    }

    /// Connect using OAuth code exchange (native).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn connect_with_oauth(
        &mut self,
        server_url: &str,
        provider: OAuthProvider,
        code: &str,
        ctx: &egui::Context,
    ) {
        // Check for runtime handle
        let Some(ref runtime_handle) = self.runtime_handle else {
            self.status = TeamConnectionStatus::Failed {
                error: "No runtime handle available. Call set_runtime_handle first.".to_string(),
            };
            return;
        };

        let client = TeamClient::new(server_url, runtime_handle.clone());

        self.status = TeamConnectionStatus::Connecting;
        // For OAuth, we use get_current_user after the OAuth flow completes on the server
        // The actual OAuth exchange happens server-side
        self.pending_auth = Some(client.get_current_user(ctx));
        self.pending_auth_token = None; // Token will come from OAuth response
        self.client = Some(client);

        // Note: In a real implementation, we'd first call exchange_oauth_code,
        // then set the token, then call get_current_user
        let _ = (provider, code); // Suppress unused warnings for now
    }

    /// Connect using OAuth code exchange (WASM).
    #[cfg(target_arch = "wasm32")]
    pub fn connect_with_oauth(
        &mut self,
        server_url: &str,
        provider: OAuthProvider,
        code: &str,
        ctx: &egui::Context,
    ) {
        let client = TeamClient::new(server_url);

        self.status = TeamConnectionStatus::Connecting;
        // For OAuth, we use get_current_user after the OAuth flow completes on the server
        // The actual OAuth exchange happens server-side
        self.pending_auth = Some(client.get_current_user(ctx));
        self.pending_auth_token = None; // Token will come from OAuth response
        self.client = Some(client);

        // Note: In a real implementation, we'd first call exchange_oauth_code,
        // then set the token, then call get_current_user
        let _ = (provider, code); // Suppress unused warnings for now
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.realtime.disconnect();
        self.client = None;
        self.status = TeamConnectionStatus::Disconnected;
        self.current_team_id = None;
        self.pending_auth = None;
        self.pending_auth_token = None;
        self.pending_teams = None;
        self.pending_user = None;
        self.server_url = None;
        self.annotation_cache.clear();
        self.pending_annotations.clear();
        self.pending_messages.clear();
        self.event_buffer.clear();
        self.channel_cache = None;
        self.pending_channels = None;
        self.pending_channel_creates.clear();
        self.thread_cache.clear();
        self.pending_threads.clear();
        self.message_cache.clear();
        self.pending_thread_messages.clear();
    }

    /// Select a team to work with.
    pub fn select_team(&mut self, team_id: TeamId) {
        let old_team_id = self.current_team_id;
        self.current_team_id = Some(team_id);

        // Clear caches when switching teams
        self.annotation_cache.clear();
        self.channel_cache = None;
        self.thread_cache.clear();
        self.message_cache.clear();

        // Reconnect WebSocket with new team if team changed
        if old_team_id != Some(team_id) {
            if let Some(ref client) = self.client {
                if let Some(ref server_url) = self.server_url {
                    if let Some(token) = client.auth_token() {
                        self.realtime.disconnect();
                        self.realtime.connect(server_url, token, team_id);
                    }
                }
            }
        }
    }

    /// Poll for completed requests and return any events.
    ///
    /// Call this every frame from the editor's update loop.
    pub fn poll(&mut self) -> Vec<TeamEvent> {
        let now = now_unix_secs();
        let mut events = Vec::new();

        // Poll authentication (phase 1: get user)
        if let Some(ref promise) = self.pending_auth {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(user) => {
                        // Store user and start fetching teams
                        self.pending_user = Some(user.clone());

                        // Start fetching teams
                        if let Some(ref client) = self.client {
                            self.pending_teams = Some(client.list_teams(&egui::Context::default()));
                        }
                    }
                    Err(e) => {
                        self.status = TeamConnectionStatus::Failed {
                            error: e.to_string(),
                        };
                        self.pending_auth_token = None;
                    }
                }
                self.pending_auth = None;
            }
        }

        // Poll teams fetch (phase 2: get teams after user auth)
        if let Some(ref promise) = self.pending_teams {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(teams) => {
                        if let Some(user) = self.pending_user.take() {
                            // Use the first team (or create a fallback if no teams)
                            let team = teams.first().cloned().unwrap_or_else(|| Team {
                                id: uuid::Uuid::new_v4(),
                                name: format!("{}'s Team", user.display_name),
                                members: vec![user.clone()],
                            });

                            let team_id = team.id;
                            self.current_team_id = Some(team_id);
                            self.status = TeamConnectionStatus::Connected {
                                user: user.clone(),
                                team,
                            };

                            // Connect WebSocket now that we have the real team ID
                            if let (Some(server_url), Some(auth_token)) =
                                (&self.server_url, &self.pending_auth_token)
                            {
                                self.realtime.connect(server_url, auth_token, team_id);
                            }
                        }
                    }
                    Err(e) => {
                        self.status = TeamConnectionStatus::Failed {
                            error: e.to_string(),
                        };
                        self.pending_user = None;
                    }
                }
                self.pending_teams = None;
                self.pending_auth_token = None;
            }
        }

        // Poll WebSocket for real-time events
        events.extend(self.realtime.poll());

        // Poll annotation fetches
        let mut completed_annotations = Vec::new();
        for (fingerprint, pending) in &self.pending_annotations {
            if let Some(result) = pending.promise.ready() {
                completed_annotations.push((fingerprint.clone(), result.clone()));
            } else if now.saturating_sub(pending.started_at) >= self.timeout_secs {
                completed_annotations.push((
                    fingerprint.clone(),
                    Err(TeamApiError::Timeout {
                        elapsed_secs: now.saturating_sub(pending.started_at),
                    }),
                ));
            }
        }

        for (fingerprint, result) in completed_annotations {
            self.pending_annotations.remove(&fingerprint);
            if let Ok(annotations) = result {
                self.annotation_cache.insert(
                    fingerprint,
                    CachedAnnotations {
                        annotations,
                        fetched_at: now,
                    },
                );
            }
        }

        // Poll message sends
        let mut completed_messages = Vec::new();
        while let Some(pending) = self.pending_messages.front() {
            if let Some(result) = pending.promise.ready() {
                completed_messages.push(result.clone());
                self.pending_messages.pop_front();
            } else if now.saturating_sub(pending.started_at) >= self.timeout_secs {
                self.pending_messages.pop_front();
                // Could emit an error event here
            } else {
                break;
            }
        }

        // Poll channel fetch
        if let Some(ref pending) = self.pending_channels {
            if let Some(result) = pending.promise.ready() {
                if let Ok(channels) = result {
                    self.channel_cache = Some(CachedChannels {
                        channels: channels.clone(),
                        fetched_at: now,
                    });
                }
                self.pending_channels = None;
            } else if now.saturating_sub(pending.started_at) >= self.timeout_secs {
                self.pending_channels = None;
            }
        }

        // Poll pending channel creates - collect completed ones first
        let mut created_channels: Vec<Channel> = Vec::new();
        let mut to_remove = 0usize;
        for pending in &self.pending_channel_creates {
            if let Some(result) = pending.promise.ready() {
                match result {
                    Ok(channel) => {
                        log::info!("Channel created successfully: {}", channel.name);
                        created_channels.push(channel.clone());
                    }
                    Err(e) => {
                        log::error!("Failed to create channel: {e}");
                    }
                }
                to_remove += 1;
            } else if now.saturating_sub(pending.started_at) >= self.timeout_secs {
                log::error!("Channel create timed out");
                to_remove += 1;
            } else {
                break;
            }
        }
        // Remove completed creates
        for _ in 0..to_remove {
            self.pending_channel_creates.pop_front();
        }
        // Add to cache and emit events
        for channel in created_channels {
            self.add_channel_to_cache(channel.clone());
            events.push(TeamEvent::ChannelCreated { channel });
        }

        // Poll thread fetches
        let mut completed_threads = Vec::new();
        for (channel_id, pending) in &self.pending_threads {
            if let Some(result) = pending.promise.ready() {
                completed_threads.push((*channel_id, result.clone()));
            } else if now.saturating_sub(pending.started_at) >= self.timeout_secs {
                completed_threads.push((
                    *channel_id,
                    Err(TeamApiError::Timeout {
                        elapsed_secs: now.saturating_sub(pending.started_at),
                    }),
                ));
            }
        }

        for (channel_id, result) in completed_threads {
            self.pending_threads.remove(&channel_id);
            if let Ok(threads) = result {
                self.thread_cache.insert(
                    channel_id,
                    CachedThreads {
                        threads,
                        fetched_at: now,
                    },
                );
            }
        }

        // Poll thread message fetches
        let mut completed_thread_messages = Vec::new();
        for (thread_id, pending) in &self.pending_thread_messages {
            if let Some(result) = pending.promise.ready() {
                completed_thread_messages.push((*thread_id, result.clone()));
            } else if now.saturating_sub(pending.started_at) >= self.timeout_secs {
                completed_thread_messages.push((
                    *thread_id,
                    Err(TeamApiError::Timeout {
                        elapsed_secs: now.saturating_sub(pending.started_at),
                    }),
                ));
            }
        }

        for (thread_id, result) in completed_thread_messages {
            self.pending_thread_messages.remove(&thread_id);
            if let Ok(messages) = result {
                self.message_cache.insert(
                    thread_id,
                    CachedMessages {
                        messages,
                        fetched_at: now,
                    },
                );
            }
        }

        // Drain event buffer
        events.extend(self.event_buffer.drain(..));

        events
    }

    /// Get annotations for a query fingerprint.
    ///
    /// Returns cached annotations if available, otherwise initiates a fetch.
    /// Returns an empty slice while fetching.
    pub fn get_annotations(
        &mut self,
        query_fingerprint: &str,
        ctx: &egui::Context,
    ) -> &[Annotation] {
        // Return cached if available
        if let Some(cached) = self.annotation_cache.get(query_fingerprint) {
            return &cached.annotations;
        }

        // Check if already fetching
        if self.pending_annotations.contains_key(query_fingerprint) {
            return &[];
        }

        // Initiate fetch if connected
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let promise = client.list_annotations(team_id, query_fingerprint, ctx);
            self.pending_annotations.insert(
                query_fingerprint.to_string(),
                PendingFetch {
                    promise,
                    started_at: now_unix_secs(),
                },
            );
        }

        &[]
    }

    /// Check if annotations are being fetched for a query.
    pub fn is_fetching_annotations(&self, query_fingerprint: &str) -> bool {
        self.pending_annotations.contains_key(query_fingerprint)
    }

    /// Create a new annotation.
    pub fn create_annotation(&mut self, annotation: NewAnnotation, ctx: &egui::Context) {
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let fingerprint = annotation.query_fingerprint.clone();
            let _promise = client.create_annotation(team_id, &annotation, ctx);

            // Invalidate cache for this fingerprint so it will be re-fetched
            self.annotation_cache.remove(&fingerprint);
        }
    }

    /// Delete an annotation.
    pub fn delete_annotation(
        &mut self,
        annotation_id: AnnotationId,
        query_fingerprint: &str,
        ctx: &egui::Context,
    ) {
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let _promise = client.delete_annotation(team_id, annotation_id, ctx);
            // Invalidate cache
            self.annotation_cache.remove(query_fingerprint);
        }
    }

    /// Send a message to a thread.
    pub fn send_message(&mut self, thread_id: ThreadId, content: &str, ctx: &egui::Context) {
        if let Some(client) = &self.client {
            let message = NewMessage {
                content: content.to_string(),
                inline_chart: None,
            };
            let promise = client.send_message(thread_id, &message, ctx);
            self.pending_messages.push_back(PendingFetch {
                promise,
                started_at: now_unix_secs(),
            });
        }
    }

    /// Share current view with team (war room).
    pub fn share_view(&mut self, workspace_url: &str, ctx: &egui::Context) {
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let _promise = client.share_view(team_id, workspace_url, ctx);
        }
    }

    /// Invalidate annotation cache for a query.
    pub fn invalidate_annotations(&mut self, query_fingerprint: &str) {
        self.annotation_cache.remove(query_fingerprint);
    }

    /// Clear all caches.
    pub fn clear_caches(&mut self) {
        self.annotation_cache.clear();
        self.channel_cache = None;
        self.thread_cache.clear();
        self.message_cache.clear();
    }

    /// Push an event to the buffer (for WebSocket events).
    pub fn push_event(&mut self, event: TeamEvent) {
        self.event_buffer.push_back(event);
    }

    /// Send a typing indicator for a thread.
    pub fn send_start_typing(&mut self, thread_id: ThreadId) {
        self.realtime.send_start_typing(thread_id);
    }

    /// Stop typing indicator for a thread.
    pub fn send_stop_typing(&mut self, thread_id: ThreadId) {
        self.realtime.send_stop_typing(thread_id);
    }

    /// Force reconnect the WebSocket.
    pub fn reconnect_websocket(&mut self) {
        self.realtime.reconnect();
    }

    /// Send a ping to keep WebSocket alive.
    pub fn send_ws_ping(&mut self) {
        self.realtime.send_ping();
    }

    // =========================================================================
    // Channel methods
    // =========================================================================

    /// Get channels for the current team.
    ///
    /// Returns cached channels if available, otherwise initiates a fetch.
    /// Returns an empty slice while fetching.
    pub fn get_channels(&mut self, ctx: &egui::Context) -> &[Channel] {
        // Return cached if available
        if let Some(ref cached) = self.channel_cache {
            return &cached.channels;
        }

        // Check if already fetching
        if self.pending_channels.is_some() {
            return &[];
        }

        // Initiate fetch if connected
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let promise = client.list_channels(team_id, ctx);
            self.pending_channels = Some(PendingFetch {
                promise,
                started_at: now_unix_secs(),
            });
        }

        &[]
    }

    /// Check if channels are being fetched.
    pub fn is_fetching_channels(&self) -> bool {
        self.pending_channels.is_some()
    }

    /// Create a new channel.
    pub fn create_channel(&mut self, channel: &NewChannel, ctx: &egui::Context) {
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let promise = client.create_channel(team_id, channel, ctx);
            // Track the promise so we can poll it and add to cache when done
            self.pending_channel_creates.push_back(PendingFetch {
                promise,
                started_at: now_unix_secs(),
            });
        }
    }

    /// Invalidate channel cache.
    pub fn invalidate_channels(&mut self) {
        self.channel_cache = None;
    }

    // =========================================================================
    // Thread methods
    // =========================================================================

    /// Get threads for a channel.
    ///
    /// Returns cached threads if available, otherwise initiates a fetch.
    /// Returns an empty slice while fetching.
    pub fn get_threads(&mut self, channel_id: ChannelId, ctx: &egui::Context) -> &[ChatThread] {
        // Return cached if available
        if let Some(cached) = self.thread_cache.get(&channel_id) {
            return &cached.threads;
        }

        // Check if already fetching
        if self.pending_threads.contains_key(&channel_id) {
            return &[];
        }

        // Initiate fetch if connected
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let promise = client.list_channel_threads(team_id, channel_id, ctx);
            self.pending_threads.insert(
                channel_id,
                PendingFetch {
                    promise,
                    started_at: now_unix_secs(),
                },
            );
        }

        &[]
    }

    /// Check if threads are being fetched for a channel.
    pub fn is_fetching_threads(&self, channel_id: ChannelId) -> bool {
        self.pending_threads.contains_key(&channel_id)
    }

    /// Create a new thread in a channel.
    pub fn create_thread(
        &mut self,
        channel_id: ChannelId,
        thread: &NewThread,
        ctx: &egui::Context,
    ) {
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let _promise = client.create_thread(team_id, channel_id, thread, ctx);
            // Invalidate thread cache for this channel
            self.thread_cache.remove(&channel_id);
        }
    }

    /// Invalidate thread cache for a channel.
    pub fn invalidate_threads(&mut self, channel_id: ChannelId) {
        self.thread_cache.remove(&channel_id);
    }

    // =========================================================================
    // Thread message methods
    // =========================================================================

    /// Get messages for a thread.
    ///
    /// Returns cached messages if available, otherwise initiates a fetch.
    /// Returns an empty slice while fetching.
    pub fn get_thread_messages(
        &mut self,
        channel_id: ChannelId,
        thread_id: ThreadId,
        ctx: &egui::Context,
    ) -> &[Message] {
        // Return cached if available
        if let Some(cached) = self.message_cache.get(&thread_id) {
            return &cached.messages;
        }

        // Check if already fetching
        if self.pending_thread_messages.contains_key(&thread_id) {
            return &[];
        }

        // Initiate fetch if connected
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let promise = client.list_channel_messages(team_id, channel_id, thread_id, ctx);
            self.pending_thread_messages.insert(
                thread_id,
                PendingFetch {
                    promise,
                    started_at: now_unix_secs(),
                },
            );
        }

        &[]
    }

    /// Check if messages are being fetched for a thread.
    pub fn is_fetching_thread_messages(&self, thread_id: ThreadId) -> bool {
        self.pending_thread_messages.contains_key(&thread_id)
    }

    /// Send a message to a channel thread.
    pub fn send_channel_message(
        &mut self,
        channel_id: ChannelId,
        thread_id: ThreadId,
        message: &NewMessage,
        ctx: &egui::Context,
    ) {
        if let (Some(client), Some(team_id)) = (&self.client, self.current_team_id) {
            let promise = client.send_channel_message(team_id, channel_id, thread_id, message, ctx);
            self.pending_messages.push_back(PendingFetch {
                promise,
                started_at: now_unix_secs(),
            });
            // Invalidate message cache for this thread
            self.message_cache.remove(&thread_id);
        }
    }

    /// Invalidate message cache for a thread.
    pub fn invalidate_thread_messages(&mut self, thread_id: ThreadId) {
        self.message_cache.remove(&thread_id);
    }

    /// Add a channel to the cache (for real-time events).
    /// Checks for duplicates by ID before adding.
    pub fn add_channel_to_cache(&mut self, channel: Channel) {
        if let Some(ref mut cached) = self.channel_cache {
            // Don't add if already exists (prevent duplicates)
            if !cached.channels.iter().any(|c| c.id == channel.id) {
                cached.channels.push(channel);
            }
        }
    }

    /// Add a thread to the cache (for real-time events).
    /// Checks for duplicates by ID before adding.
    pub fn add_thread_to_cache(&mut self, thread: ChatThread) {
        if let Some(cached) = self.thread_cache.get_mut(&thread.channel_id) {
            // Don't add if already exists (prevent duplicates)
            if !cached.threads.iter().any(|t| t.id == thread.id) {
                cached.threads.push(thread);
            }
        }
    }

    /// Add a message to the cache (for real-time events).
    pub fn add_message_to_cache(&mut self, thread_id: ThreadId, message: Message) {
        if let Some(cached) = self.message_cache.get_mut(&thread_id) {
            cached.messages.push(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_initial_state() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let manager = TeamManager::new(rt.handle().clone());
        assert!(!manager.is_connected());
        assert!(manager.current_user().is_none());
        assert!(manager.current_team().is_none());
    }

    #[test]
    fn test_manager_disconnect() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut manager = TeamManager::new(rt.handle().clone());
        manager.current_team_id = Some(uuid::Uuid::new_v4());
        manager.disconnect();

        assert!(manager.current_team_id.is_none());
        assert!(matches!(manager.status, TeamConnectionStatus::Disconnected));
    }

    #[test]
    fn test_manager_offline_mode() {
        let manager = TeamManager::new_offline();
        assert!(!manager.is_connected());
        assert!(manager.runtime_handle.is_none());
    }
}
