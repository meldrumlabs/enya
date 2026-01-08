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
/// let mut manager = TeamManager::new();
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
    /// Auth token stored for creating AuthResponse.
    pending_auth_token: Option<String>,
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
}

/// Cached annotations with timestamp.
struct CachedAnnotations {
    annotations: Vec<Annotation>,
    /// Timestamp for cache expiration (to be used for staleness checks).
    #[allow(dead_code)]
    fetched_at: u64,
}

/// A pending API request.
struct PendingFetch<T: Send + 'static> {
    promise: Promise<TeamApiResult<T>>,
    started_at: u64,
}

impl Default for TeamManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TeamManager {
    /// Create a new team manager.
    pub fn new() -> Self {
        Self {
            client: None,
            status: TeamConnectionStatus::Disconnected,
            current_team_id: None,
            pending_auth: None,
            pending_auth_token: None,
            annotation_cache: FxHashMap::default(),
            pending_annotations: FxHashMap::default(),
            pending_messages: VecDeque::new(),
            event_buffer: VecDeque::new(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
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

    /// Get the current user if connected.
    pub fn current_user(&self) -> Option<&User> {
        self.status.current_user()
    }

    /// Get the current team if connected.
    pub fn current_team(&self) -> Option<&Team> {
        self.status.current_team()
    }

    /// Connect to a team server with an auth token.
    ///
    /// This initiates authentication and fetches user/team info.
    pub fn connect(&mut self, server_url: &str, auth_token: &str, ctx: &egui::Context) {
        // Create client
        let mut client = TeamClient::new(server_url);
        client.set_auth_token(auth_token);

        // Start auth check - fetch current user
        self.status = TeamConnectionStatus::Connecting;
        self.pending_auth = Some(client.get_current_user(ctx));
        self.pending_auth_token = Some(auth_token.to_string());

        self.client = Some(client);
    }

    /// Connect using OAuth code exchange.
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
        self.client = None;
        self.status = TeamConnectionStatus::Disconnected;
        self.current_team_id = None;
        self.pending_auth = None;
        self.pending_auth_token = None;
        self.annotation_cache.clear();
        self.pending_annotations.clear();
        self.pending_messages.clear();
        self.event_buffer.clear();
    }

    /// Select a team to work with.
    pub fn select_team(&mut self, team_id: TeamId) {
        self.current_team_id = Some(team_id);
        // Clear caches when switching teams
        self.annotation_cache.clear();
    }

    /// Poll for completed requests and return any events.
    ///
    /// Call this every frame from the editor's update loop.
    pub fn poll(&mut self) -> Vec<TeamEvent> {
        let now = now_unix_secs();
        let mut events = Vec::new();

        // Poll authentication
        if let Some(ref promise) = self.pending_auth {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(user) => {
                        // Create a default personal team for the user
                        let team = Team {
                            id: uuid::Uuid::new_v4(),
                            name: format!("{}'s Team", user.display_name),
                            members: vec![user.clone()],
                        };

                        self.current_team_id = Some(team.id);
                        self.status = TeamConnectionStatus::Connected {
                            user: user.clone(),
                            team,
                        };
                    }
                    Err(e) => {
                        self.status = TeamConnectionStatus::Failed {
                            error: e.to_string(),
                        };
                    }
                }
                self.pending_auth = None;
                self.pending_auth_token = None;
            }
        }

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
    }

    /// Push an event to the buffer (for WebSocket events).
    pub fn push_event(&mut self, event: TeamEvent) {
        self.event_buffer.push_back(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_initial_state() {
        let manager = TeamManager::new();
        assert!(!manager.is_connected());
        assert!(manager.current_user().is_none());
        assert!(manager.current_team().is_none());
    }

    #[test]
    fn test_manager_disconnect() {
        let mut manager = TeamManager::new();
        manager.current_team_id = Some(uuid::Uuid::new_v4());
        manager.disconnect();

        assert!(manager.current_team_id.is_none());
        assert!(matches!(manager.status, TeamConnectionStatus::Disconnected));
    }
}
