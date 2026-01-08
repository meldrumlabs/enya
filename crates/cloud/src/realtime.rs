//! Real-time WebSocket server.

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use enya_team_api::TeamEvent;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::auth::jwt;
use crate::error::ApiError;
use crate::state::AppState;

/// Real-time event with routing info.
#[derive(Debug, Clone)]
pub struct RealtimeEvent {
    /// Target team ID (for filtering).
    pub team_id: Uuid,
    /// Target user IDs (None = broadcast to team).
    pub target_users: Option<Vec<Uuid>>,
    /// The event payload.
    pub event: TeamEvent,
}

impl RealtimeEvent {
    /// Create a team broadcast event.
    pub fn broadcast(team_id: Uuid, event: TeamEvent) -> Self {
        Self {
            team_id,
            target_users: None,
            event,
        }
    }

    /// Create a targeted event for specific users.
    pub fn targeted(team_id: Uuid, users: Vec<Uuid>, event: TeamEvent) -> Self {
        Self {
            team_id,
            target_users: Some(users),
            event,
        }
    }

    /// Check if event should be sent to a specific user in a team.
    pub fn should_send_to(&self, team_id: Uuid, user_id: Uuid) -> bool {
        if self.team_id != team_id {
            return false;
        }

        match &self.target_users {
            None => true, // Broadcast to all team members
            Some(users) => users.contains(&user_id),
        }
    }
}

/// Query parameters for WebSocket connection.
#[derive(Debug, Deserialize)]
pub struct WsParams {
    /// Authentication token.
    pub token: String,
    /// Team ID to subscribe to.
    pub team_id: Uuid,
}

/// Message from client over WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ClientMessage {
    /// Ping to keep connection alive.
    Ping,
    /// Start typing indicator.
    StartTyping { thread_id: Uuid },
    /// Stop typing indicator.
    StopTyping { thread_id: Uuid },
}

/// Message to client over WebSocket.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ServerMessage {
    /// Pong response.
    Pong,
    /// Real-time event.
    Event(TeamEvent),
    /// Error message.
    Error { message: String },
}

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<WsParams>,
) -> Result<impl IntoResponse, ApiError> {
    // Verify token
    let claims = jwt::verify_token(&params.token, &state.config.jwt_secret)?;
    let user_id = claims.user_id();

    // Verify team membership
    crate::auth::middleware::require_team_member(&state, params.team_id, user_id).await?;

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, user_id, params.team_id)))
}

/// Handle an individual WebSocket connection.
async fn handle_socket(socket: WebSocket, state: AppState, user_id: Uuid, team_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast channel
    let mut rx = state.subscribe_realtime();

    tracing::info!("WebSocket connected: user={user_id}, team={team_id}");

    // Spawn task to forward broadcast events to this client
    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if event.should_send_to(team_id, user_id) {
                        let msg = ServerMessage::Event(event.event);
                        let json = serde_json::to_string(&msg).unwrap();
                        if sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("WebSocket lagged {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Handle incoming messages from client
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::Ping => {
                            // Pong is handled by the send task
                        }
                        ClientMessage::StartTyping { thread_id } => {
                            // Broadcast typing indicator
                            state.broadcast(RealtimeEvent::broadcast(
                                team_id,
                                TeamEvent::UserTyping { thread_id, user_id },
                            ));
                        }
                        ClientMessage::StopTyping { .. } => {
                            // Could broadcast stop typing, but usually not needed
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Err(e) => {
                tracing::error!("WebSocket error: {e}");
                break;
            }
            _ => {}
        }
    }

    // Clean up
    send_task.abort();
    tracing::info!("WebSocket disconnected: user={user_id}, team={team_id}");
}
