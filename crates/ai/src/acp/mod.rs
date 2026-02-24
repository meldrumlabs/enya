//! Agent Client Protocol (ACP) integration.
//!
//! This module provides a client implementation for connecting to ACP-compatible
//! AI coding agents like Claude Code and Codex.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Editor (Enya)                           │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │               AcpClient                               │  │
//! │  │  - Spawns agent process                               │  │
//! │  │  - Implements Client trait                            │  │
//! │  │  - Converts ACP events → AgentEvent                   │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! │                          │                                   │
//! │                JSON-RPC 2.0 over stdio                      │
//! └──────────────────────────┼───────────────────────────────────┘
//!                            │
//!              ┌─────────────┴─────────────┐
//!              │                           │
//!              ▼                           ▼
//! ┌───────────────┐  ┌───────────────┐
//! │  Claude Code  │  │    Codex      │
//! └───────────────┘  └───────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use enya_ai::acp::{AcpClient, AgentConfig};
//!
//! // Configure the agent
//! let config = AgentConfig::claude_code();
//!
//! // Create client and connect
//! let client = AcpClient::new(config);
//! let rx = client.prompt("Help me understand this code").await?;
//!
//! // Process events
//! while let Some(event) = rx.recv().await {
//!     match event {
//!         AgentEvent::TextDelta(text) => print!("{}", text),
//!         AgentEvent::Done { .. } => break,
//!         _ => {}
//!     }
//! }
//! ```

mod client;
mod config;
mod persistent;
mod protocol;

pub use client::AcpClient;
pub use config::{AgentConfig, AgentKind};
pub use persistent::PersistentAcpClient;
