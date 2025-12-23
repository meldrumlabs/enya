//! AI agent integration for Enya.
//!
//! This crate provides LLM provider clients and an agent framework
//! for building AI-powered features in the Enya editor.
//!
//! # Architecture
//!
//! - [`provider`]: LLM provider clients (Anthropic, OpenAI)
//! - [`tool`]: Tool trait and execution context
//! - [`types`]: Core types (messages, events, errors)
//!
//! # Example
//!
//! ```ignore
//! use enya_ai::{Provider, Message, AgentEvent};
//!
//! // Assumes a tokio runtime is available (created at app startup)
//! // Create a provider
//! let provider = Provider::anthropic("sk-...", "claude-sonnet-4-20250514");
//!
//! // Start streaming (spawns a tokio task)
//! let rx = provider.stream("You are helpful", &messages, &tools);
//!
//! // Poll for events in your UI loop
//! while let Ok(event) = rx.try_recv() {
//!     match event {
//!         AgentEvent::TextDelta(text) => print!("{}", text),
//!         AgentEvent::ToolCallReady { id, input } => {
//!             println!("[tool call ready]");
//!         }
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

pub mod provider;
pub mod tool;
pub mod types;

// Re-exports
pub use provider::Provider;
pub use tool::{AgentTool, ToolCategory, ToolContext, ToolOutput};
pub use types::{AgentError, AgentEvent, Message, Role};
