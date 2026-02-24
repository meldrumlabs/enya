//! Core types for the AI agent system.

use serde::{Deserialize, Serialize};

/// Role in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

    /// Agent process spawn or I/O error
    #[error("Process error: {0}")]
    Process(String),

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

#[cfg(test)]
mod tests {
    use super::*;

    // -- Role --

    #[test]
    fn role_serialization() {
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn role_deserialization() {
        let role: Role = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(role, Role::User);
        let role: Role = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(role, Role::System);
        let role: Role = serde_json::from_str("\"assistant\"").unwrap();
        assert_eq!(role, Role::Assistant);
    }

    // -- Message constructors --

    #[test]
    fn message_user() {
        let msg = Message::user("hello");
        assert_eq!(msg.role, Role::User);
        match msg.content {
            MessageContent::Text(ref s) => assert_eq!(s, "hello"),
            MessageContent::Blocks(_) => panic!("expected Text content"),
        }
    }

    #[test]
    fn message_assistant() {
        let msg = Message::assistant("response");
        assert_eq!(msg.role, Role::Assistant);
        match msg.content {
            MessageContent::Text(ref s) => assert_eq!(s, "response"),
            MessageContent::Blocks(_) => panic!("expected Text content"),
        }
    }

    #[test]
    fn message_system() {
        let msg = Message::system("you are helpful");
        assert_eq!(msg.role, Role::System);
        match msg.content {
            MessageContent::Text(ref s) => assert_eq!(s, "you are helpful"),
            MessageContent::Blocks(_) => panic!("expected Text content"),
        }
    }

    #[test]
    fn message_accepts_owned_string() {
        let msg = Message::user(String::from("owned"));
        match msg.content {
            MessageContent::Text(ref s) => assert_eq!(s, "owned"),
            MessageContent::Blocks(_) => panic!("expected Text content"),
        }
    }

    // -- MessageContent serialization --

    #[test]
    fn text_content_serializes_as_string() {
        let content = MessageContent::Text("hello".to_string());
        let json = serde_json::to_string(&content).unwrap();
        assert_eq!(json, "\"hello\"");
    }

    #[test]
    fn blocks_content_serializes_as_array() {
        let content = MessageContent::Blocks(vec![ContentBlock::Text {
            text: "hi".to_string(),
        }]);
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.starts_with('['));
        assert!(json.contains("\"type\":\"text\""));
    }

    // -- ContentBlock --

    #[test]
    fn content_block_text_roundtrip() {
        let block = ContentBlock::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        match parsed {
            ContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected Text block"),
        }
    }

    #[test]
    fn content_block_tool_use_roundtrip() {
        let block = ContentBlock::ToolUse {
            id: "call_1".to_string(),
            name: "search".to_string(),
            input: serde_json::json!({"query": "cpu"}),
        };
        let json = serde_json::to_string(&block).unwrap();
        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        match parsed {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "search");
                assert_eq!(input["query"], "cpu");
            }
            _ => panic!("expected ToolUse block"),
        }
    }

    #[test]
    fn content_block_tool_result_roundtrip() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "call_1".to_string(),
            content: "found 5 results".to_string(),
            is_error: Some(false),
        };
        let json = serde_json::to_string(&block).unwrap();
        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        match parsed {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "call_1");
                assert_eq!(content, "found 5 results");
                assert_eq!(is_error, Some(false));
            }
            _ => panic!("expected ToolResult block"),
        }
    }

    #[test]
    fn content_block_tool_result_omits_none_is_error() {
        let block = ContentBlock::ToolResult {
            tool_use_id: "call_1".to_string(),
            content: "ok".to_string(),
            is_error: None,
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(!json.contains("is_error"));
    }

    // -- Message full roundtrip --

    #[test]
    fn message_text_roundtrip() {
        let msg = Message::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.role, Role::User);
        match parsed.content {
            MessageContent::Text(s) => assert_eq!(s, "test"),
            MessageContent::Blocks(_) => panic!("expected Text"),
        }
    }

    #[test]
    fn message_blocks_roundtrip() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "I'll search".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "query".to_string(),
                    input: serde_json::json!({}),
                },
            ]),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.role, Role::Assistant);
        match parsed.content {
            MessageContent::Blocks(blocks) => assert_eq!(blocks.len(), 2),
            MessageContent::Text(_) => panic!("expected Blocks"),
        }
    }

    // -- AgentError display --

    #[test]
    fn agent_error_display_messages() {
        assert_eq!(
            AgentError::Http("timeout".into()).to_string(),
            "HTTP error: timeout"
        );
        assert_eq!(
            AgentError::Process("spawn".into()).to_string(),
            "Process error: spawn"
        );
        assert_eq!(
            AgentError::Parse("bad json".into()).to_string(),
            "Parse error: bad json"
        );
        assert_eq!(
            AgentError::Auth("bad key".into()).to_string(),
            "Authentication failed: bad key"
        );
        assert_eq!(
            AgentError::Tool("exec failed".into()).to_string(),
            "Tool error: exec failed"
        );
        assert_eq!(
            AgentError::NotConfigured("missing".into()).to_string(),
            "Provider not configured: missing"
        );
    }

    #[test]
    fn agent_error_api_display() {
        let err = AgentError::Api {
            message: "overloaded".into(),
            kind: Some(ApiErrorKind::ServerError),
        };
        assert!(err.to_string().contains("overloaded"));
    }

    #[test]
    fn agent_error_rate_limited_display() {
        let err = AgentError::RateLimited {
            retry_after_secs: Some(60),
        };
        let display = err.to_string();
        assert!(display.contains("60"));
    }

    // -- ApiErrorKind --

    #[test]
    fn api_error_kind_display() {
        assert_eq!(ApiErrorKind::InvalidRequest.to_string(), "invalid request");
        assert_eq!(
            ApiErrorKind::ContextWindowExceeded.to_string(),
            "context window exceeded"
        );
        assert_eq!(
            ApiErrorKind::ContentFiltered.to_string(),
            "content filtered"
        );
        assert_eq!(ApiErrorKind::ServerError.to_string(), "server error");
    }

    // -- TokenUsage --

    #[test]
    fn token_usage_default_is_zero() {
        let usage = TokenUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
    }

    // -- StopReason --

    #[test]
    fn stop_reason_equality() {
        assert_eq!(StopReason::EndTurn, StopReason::EndTurn);
        assert_eq!(StopReason::ToolUse, StopReason::ToolUse);
        assert_ne!(StopReason::EndTurn, StopReason::MaxTokens);
        assert_ne!(StopReason::ToolUse, StopReason::StopSequence);
    }

    // -- ToolDefinition --

    #[test]
    fn tool_definition_serialization() {
        let tool = ToolDefinition {
            name: "query_metrics".to_string(),
            description: "Query Prometheus".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "expr": { "type": "string" }
                },
                "required": ["expr"]
            }),
        };
        let json = serde_json::to_string(&tool).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "query_metrics");
        assert_eq!(parsed["description"], "Query Prometheus");
        assert_eq!(parsed["input_schema"]["type"], "object");
        assert!(
            parsed["input_schema"]["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("expr"))
        );
    }
}
