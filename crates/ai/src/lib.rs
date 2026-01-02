//! AI agent integration for Enya.
//!
//! This crate provides LLM provider clients and an agent framework
//! for building AI-powered features in the Enya editor.
//!
//! # Architecture
//!
//! - [`acp`]: Agent Client Protocol for universal agent communication
//! - [`provider`]: Direct LLM provider clients (Anthropic, OpenAI)
//! - [`tool`]: Tool trait and execution context
//! - [`types`]: Core types (messages, events, errors)
//!
//! # Using ACP (Recommended)
//!
//! The Agent Client Protocol allows connecting to any ACP-compatible agent
//! like Claude Code, Gemini CLI, or Codex:
//!
//! ```ignore
//! use enya_ai::{AcpClient, AgentEvent};
//!
//! // Create a client for Claude Code
//! let client = AcpClient::claude_code();
//!
//! // Send a prompt and receive streaming events
//! let rx = client.prompt("Help me understand this code", None);
//!
//! while let Ok(event) = rx.try_recv() {
//!     match event {
//!         AgentEvent::TextDelta(text) => print!("{}", text),
//!         AgentEvent::ToolCallStart { name, .. } => println!("[{name}]"),
//!         AgentEvent::Done { .. } => break,
//!         _ => {}
//!     }
//! }
//! ```
//!
//! # Using Direct Provider API
//!
//! For direct API access without the CLI:
//!
//! ```ignore
//! use enya_ai::{Provider, Message, AgentEvent};
//!
//! let provider = Provider::anthropic("sk-...", "claude-sonnet-4-20250514");
//! let rx = provider.stream("You are helpful", &messages, &tools);
//!
//! while let Ok(event) = rx.try_recv() {
//!     match event {
//!         AgentEvent::TextDelta(text) => print!("{}", text),
//!         AgentEvent::Done { .. } => break,
//!         _ => {}
//!     }
//! }
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)] // OpenAI, PromQL etc don't need backticks
#![allow(clippy::missing_errors_doc)] // Tool errors are self-explanatory
#![allow(clippy::must_use_candidate)] // stream() receivers are obvious

pub mod acp;
pub mod provider;
pub mod tool;
pub mod types;

// Re-exports
pub use acp::{AcpClient, AgentConfig, AgentKind};
pub use provider::Provider;
pub use tool::{AgentTool, ToolCategory, ToolContext, ToolOutput};
pub use types::{AgentError, AgentEvent, Message, Role};
