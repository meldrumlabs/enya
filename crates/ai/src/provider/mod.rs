//! LLM provider clients.
//!
//! Supports Anthropic (Claude) and OpenAI APIs with streaming.

mod anthropic;
mod openai;

pub use anthropic::AnthropicClient;
pub use openai::OpenAIClient;

use std::sync::mpsc::Receiver;

use crate::types::{AgentEvent, Message, ToolDefinition};

/// An LLM provider.
#[derive(Clone)]
pub enum Provider {
    Anthropic(AnthropicClient),
    OpenAI(OpenAIClient),
}

impl Provider {
    /// Create an Anthropic (Claude) provider.
    #[must_use]
    pub fn anthropic(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Anthropic(AnthropicClient::new(api_key, model))
    }

    /// Create an OpenAI provider.
    #[must_use]
    pub fn openai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::OpenAI(OpenAIClient::new(api_key, model))
    }

    /// Start a streaming chat completion.
    ///
    /// Spawns a blocking task that streams events through the returned channel.
    /// Poll the receiver in your UI loop.
    ///
    /// # Panics
    ///
    /// Panics if called outside of a tokio runtime context.
    ///
    /// # Arguments
    /// * `system` - System prompt
    /// * `messages` - Conversation history
    /// * `tools` - Available tools
    ///
    /// # Returns
    /// A receiver that yields `AgentEvent`s as they arrive.
    pub fn stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Receiver<AgentEvent> {
        match self {
            Self::Anthropic(client) => client.stream(system, messages, tools),
            Self::OpenAI(client) => client.stream(system, messages, tools),
        }
    }

    /// Get the model name.
    #[must_use]
    pub fn model(&self) -> &str {
        match self {
            Self::Anthropic(client) => &client.model,
            Self::OpenAI(client) => &client.model,
        }
    }
}
