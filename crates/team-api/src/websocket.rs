//! WebSocket client for real-time events.
//!
//! Uses `ewebsock` which works on both native and WASM platforms.

use ewebsock::{WsEvent, WsMessage, WsReceiver, WsSender};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::types::{TeamEvent, TeamId};

/// WebSocket connection state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsConnectionState {
    /// Not connected.
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Connected and ready.
    Connected,
    /// Connection failed.
    Failed { error: String },
    /// Reconnecting after disconnect.
    Reconnecting { attempt: u32 },
}

/// Message from client to server over WebSocket.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Ping to keep connection alive.
    Ping,
    /// Start typing indicator.
    StartTyping { thread_id: Uuid },
    /// Stop typing indicator.
    StopTyping { thread_id: Uuid },
}

/// Message from server to client over WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Pong response.
    Pong,
    /// Real-time event.
    Event(Box<TeamEvent>),
    /// Error message.
    Error { message: String },
}

/// WebSocket client for real-time team events.
pub struct RealtimeClient {
    /// WebSocket sender (if connected).
    sender: Option<WsSender>,
    /// WebSocket receiver (if connected).
    receiver: Option<WsReceiver>,
    /// Current connection state.
    state: WsConnectionState,
    /// Buffered events from server.
    event_buffer: VecDeque<TeamEvent>,
    /// Server URL for reconnection.
    server_url: Option<String>,
    /// Auth token for reconnection.
    auth_token: Option<String>,
    /// Team ID for reconnection.
    team_id: Option<TeamId>,
    /// Reconnection attempt count.
    reconnect_attempts: u32,
    /// Max reconnection attempts before giving up.
    max_reconnect_attempts: u32,
}

impl Default for RealtimeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RealtimeClient {
    /// Create a new realtime client.
    pub fn new() -> Self {
        Self {
            sender: None,
            receiver: None,
            state: WsConnectionState::Disconnected,
            event_buffer: VecDeque::new(),
            server_url: None,
            auth_token: None,
            team_id: None,
            reconnect_attempts: 0,
            max_reconnect_attempts: 5,
        }
    }

    /// Get the current connection state.
    pub fn state(&self) -> &WsConnectionState {
        &self.state
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        matches!(self.state, WsConnectionState::Connected)
    }

    /// Connect to the WebSocket server.
    ///
    /// The server URL should be the base HTTP URL (e.g., "http://localhost:8080").
    /// It will be converted to a WebSocket URL (ws:// or wss://).
    pub fn connect(&mut self, server_url: &str, auth_token: &str, team_id: TeamId) {
        // Store for reconnection
        self.server_url = Some(server_url.to_string());
        self.auth_token = Some(auth_token.to_string());
        self.team_id = Some(team_id);
        self.reconnect_attempts = 0;

        self.do_connect();
    }

    /// Internal connection logic.
    fn do_connect(&mut self) {
        let Some(ref server_url) = self.server_url else {
            return;
        };
        let Some(ref auth_token) = self.auth_token else {
            return;
        };
        let Some(team_id) = self.team_id else {
            return;
        };

        // Convert HTTP URL to WebSocket URL
        let ws_url = if server_url.starts_with("https://") {
            server_url.replace("https://", "wss://")
        } else if server_url.starts_with("http://") {
            server_url.replace("http://", "ws://")
        } else {
            format!("ws://{server_url}")
        };

        // Build WebSocket URL with query params
        let ws_url = format!("{ws_url}/realtime?token={auth_token}&team_id={team_id}");

        self.state = WsConnectionState::Connecting;

        // ewebsock connection options
        let options = ewebsock::Options::default();

        match ewebsock::connect(&ws_url, options) {
            Ok((sender, receiver)) => {
                self.sender = Some(sender);
                self.receiver = Some(receiver);
                // State will be updated to Connected when we receive the first message
                // or in poll() when connection is established
                log::info!("WebSocket connecting to {ws_url}");
            }
            Err(e) => {
                log::error!("WebSocket connection failed: {e}");
                self.state = WsConnectionState::Failed {
                    error: e.to_string(),
                };
            }
        }
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        self.sender = None;
        self.receiver = None;
        self.state = WsConnectionState::Disconnected;
        self.event_buffer.clear();
        // Keep server_url/auth_token/team_id for potential reconnection
    }

    /// Poll for events from the WebSocket.
    ///
    /// Call this every frame. Returns any events received from the server.
    pub fn poll(&mut self) -> Vec<TeamEvent> {
        let mut events = Vec::new();
        let mut should_disconnect = false;

        // Poll the receiver for messages
        if let Some(receiver) = &self.receiver {
            while let Some(event) = receiver.try_recv() {
                match event {
                    WsEvent::Opened => {
                        log::info!("WebSocket connected");
                        self.state = WsConnectionState::Connected;
                        self.reconnect_attempts = 0;
                    }
                    WsEvent::Message(msg) => {
                        if let WsMessage::Text(text) = msg {
                            match serde_json::from_str::<ServerMessage>(&text) {
                                Ok(ServerMessage::Event(event)) => {
                                    events.push(*event);
                                }
                                Ok(ServerMessage::Pong) => {
                                    // Pong received, connection is alive
                                }
                                Ok(ServerMessage::Error { message }) => {
                                    log::error!("WebSocket server error: {message}");
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse WebSocket message: {e}");
                                }
                            }
                        }
                    }
                    WsEvent::Error(e) => {
                        log::error!("WebSocket error: {e}");
                        should_disconnect = true;
                    }
                    WsEvent::Closed => {
                        log::info!("WebSocket closed");
                        should_disconnect = true;
                    }
                }
            }
        }

        // Handle disconnect after releasing the borrow
        if should_disconnect {
            self.handle_disconnect();
        }

        // Drain any buffered events
        events.extend(self.event_buffer.drain(..));

        events
    }

    /// Handle disconnection with automatic reconnection.
    fn handle_disconnect(&mut self) {
        self.sender = None;
        self.receiver = None;

        if self.reconnect_attempts < self.max_reconnect_attempts {
            self.reconnect_attempts += 1;
            self.state = WsConnectionState::Reconnecting {
                attempt: self.reconnect_attempts,
            };
            log::info!(
                "WebSocket disconnected, reconnecting (attempt {}/{})",
                self.reconnect_attempts,
                self.max_reconnect_attempts
            );
            // TODO: Add exponential backoff delay
            self.do_connect();
        } else {
            self.state = WsConnectionState::Failed {
                error: "Max reconnection attempts exceeded".to_string(),
            };
            log::error!(
                "WebSocket reconnection failed after {} attempts",
                self.max_reconnect_attempts
            );
        }
    }

    /// Send a ping to keep the connection alive.
    pub fn send_ping(&mut self) {
        self.send_message(&ClientMessage::Ping);
    }

    /// Send a typing indicator.
    pub fn send_start_typing(&mut self, thread_id: Uuid) {
        self.send_message(&ClientMessage::StartTyping { thread_id });
    }

    /// Send a stop typing indicator.
    pub fn send_stop_typing(&mut self, thread_id: Uuid) {
        self.send_message(&ClientMessage::StopTyping { thread_id });
    }

    /// Send a message over the WebSocket.
    fn send_message(&mut self, message: &ClientMessage) {
        if let Some(ref mut sender) = self.sender {
            if let Ok(json) = serde_json::to_string(message) {
                sender.send(WsMessage::Text(json));
            }
        }
    }

    /// Force a reconnection attempt.
    pub fn reconnect(&mut self) {
        self.reconnect_attempts = 0;
        self.do_connect();
    }

    /// Set the maximum number of reconnection attempts.
    pub fn set_max_reconnect_attempts(&mut self, max: u32) {
        self.max_reconnect_attempts = max;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = RealtimeClient::new();
        assert!(!client.is_connected());
        assert!(matches!(client.state(), WsConnectionState::Disconnected));
    }

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::Ping;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ping\""));

        let msg = ClientMessage::StartTyping {
            thread_id: Uuid::nil(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"start_typing\""));
    }
}
