//! Core types for the AI agent system.

use serde::{Deserialize, Serialize};

/// Role in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System prompt / instructions
    System,
    /// User message
    User,
    /// Assistant (model) response
    Assistant,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

impl Message {
    /// Create a user message with text content.
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create an assistant message with text content.
    #[must_use]
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create a system message.
    #[must_use]
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: MessageContent::Text(text.into()),
        }
    }
}

/// Content of a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content
    Text(String),
    /// Multiple content blocks (text, images, tool results, etc.)
    Blocks(Vec<ContentBlock>),
}

/// A content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content
    Text { text: String },
    /// Tool use request from the model
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool result from execution
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Events emitted by the agent during streaming.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Text chunk from the model.
    TextDelta(String),

    /// Model is reasoning/thinking (extended thinking, chain of thought).
    ThinkingDelta(String),

    /// Model wants to call a tool - starting.
    ToolCallStart {
        /// Unique ID for this tool call
        id: String,
        /// Tool name
        name: String,
        /// Raw input JSON (for extracting summaries)
        raw_input: Option<serde_json::Value>,
    },

    /// Tool call input is being streamed.
    ToolCallInputDelta {
        /// Tool call ID
        id: String,
        /// JSON input fragment
        delta: String,
    },

    /// Tool call input is complete.
    ToolCallReady {
        /// Tool call ID
        id: String,
        /// Complete parsed input
        input: serde_json::Value,
    },

    /// Tool execution completed.
    ToolResult {
        /// Tool call ID
        id: String,
        /// Tool output
        output: String,
        /// Whether the tool errored
        is_error: bool,
    },

    /// Model response is complete.
    Done {
        /// Stop reason
        stop_reason: StopReason,
        /// Token usage
        usage: Option<TokenUsage>,
    },

    /// An error occurred.
    Error(AgentError),
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Natural end of response
    EndTurn,
    /// Model wants to use tools
    ToolUse,
    /// Hit max tokens
    MaxTokens,
    /// Hit a stop sequence
    StopSequence,
}

/// Token usage statistics.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Errors that can occur in the agent system.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AgentError {
    /// HTTP request failed
    #[error("HTTP error: {0}")]
    Http(String),

    /// Failed to parse response
    #[error("Parse error: {0}")]
    Parse(String),

    /// API returned an error
    #[error("API error: {message}")]
    Api {
        message: String,
        #[source]
        kind: Option<ApiErrorKind>,
    },

    /// Rate limited
    #[error("Rate limited, retry after {retry_after_secs:?}s")]
    RateLimited { retry_after_secs: Option<u64> },

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Tool execution failed
    #[error("Tool error: {0}")]
    Tool(String),

    /// Provider not configured
    #[error("Provider not configured: {0}")]
    NotConfigured(String),
}

/// Specific API error types.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ApiErrorKind {
    #[error("invalid request")]
    InvalidRequest,
    #[error("context window exceeded")]
    ContextWindowExceeded,
    #[error("content filtered")]
    ContentFiltered,
    #[error("server error")]
    ServerError,
}

/// Tool definition for the LLM.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
