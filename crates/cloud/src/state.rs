//! Application state.

use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::config::Config;
use crate::realtime::RealtimeEvent;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Configuration.
    pub config: Arc<Config>,
    /// Database connection pool.
    pub db: PgPool,
    /// Broadcast channel for real-time events.
    pub realtime_tx: broadcast::Sender<RealtimeEvent>,
}

impl AppState {
    /// Create a new application state.
    pub fn new(config: Config, db: PgPool) -> Self {
        let (realtime_tx, _) = broadcast::channel(1024);

        Self {
            config: Arc::new(config),
            db,
            realtime_tx,
        }
    }

    /// Subscribe to real-time events.
    pub fn subscribe_realtime(&self) -> broadcast::Receiver<RealtimeEvent> {
        self.realtime_tx.subscribe()
    }

    /// Broadcast a real-time event.
    pub fn broadcast(&self, event: RealtimeEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.realtime_tx.send(event);
    }
}
