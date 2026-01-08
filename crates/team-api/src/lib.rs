//! Team collaboration API types and client for Enya.
//!
//! This crate provides the types and client interface for team collaboration features:
//! - Annotations pinned to chart timestamps
//! - Threaded discussions with @mentions
//! - Real-time updates via WebSocket
//! - War room mode for incident collaboration
//!
//! # Architecture
//!
//! The [`TeamClient`] provides a promise-based async interface that works with egui's
//! immediate mode rendering. HTTP requests return [`Promise`] objects that can be
//! polled each frame.
//!
//! # Example
//!
//! ```ignore
//! use enya_team_api::{TeamManager, TeamConnectionStatus};
//!
//! let mut manager = TeamManager::new();
//!
//! // Connect to team server
//! manager.connect("https://api.enya.dev", "auth_token", &ctx);
//!
//! // In your update loop
//! for event in manager.poll() {
//!     match event {
//!         TeamEvent::Mentioned { message, .. } => {
//!             // Show notification
//!         }
//!         _ => {}
//!     }
//! }
//! ```

pub mod client;
pub mod error;
pub mod manager;
pub mod promise;
pub mod types;

pub use client::TeamClient;
pub use error::{TeamApiError, TeamApiResult};
pub use manager::TeamManager;
pub use poll_promise::Promise;
pub use promise::promise_channel;
pub use types::*;

/// Get the current Unix timestamp in seconds.
/// Works on both native and WASM platforms.
#[inline]
pub fn now_unix_secs() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}
