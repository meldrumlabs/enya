//! Anthropic (Claude) API client with SSE streaming.
//!
//! Implements the Messages API: https://docs.anthropic.com/en/api/messages

use std::io::BufRead;
use std::sync::mpsc::{self, Receiver, SyncSender};

use serde::{Deserialize, Serialize};

use crate::types::{
    AgentError, AgentEvent, ContentBlock, Message, MessageContent, Role, StopReason, TokenUsage,
    ToolDefinition,
};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 8192;

/// Anthropic API client.
#[derive(Clone)]
pub struct AnthropicClient {
    pub api_key: String,
    pub model: String,
}

impl AnthropicClient {
    /// Create a new client.
    #[must_use]
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Start a streaming chat completion.
    ///
    /// Spawns a task that streams events through the returned channel.
    /// Poll the receiver in your UI loop.
    ///
    /// # Panics
    ///
    /// Panics if called outside of a tokio runtime context.
    pub fn stream(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Receiver<AgentEvent> {
        let (tx, rx) = mpsc::sync_channel(256);

        let request = self.build_request(system, messages, tools);
        let api_key = self.api_key.clone();

        // Spawn blocking work on the runtime's thread pool
        // ureq is blocking, so we use spawn_blocking via a dedicated thread
        tokio::spawn(async move {
            let result =
                tokio::task::spawn_blocking(move || stream_request(&api_key, &request, &tx)).await;

            if let Err(e) = result {
                log::error!("Stream task panicked: {e}");
            }
        });

        rx
    }

    /// Start a streaming chat completion using a raw thread.
    ///
    /// Use this when you don't have an async runtime available.
    /// Prefer `stream()` with an `AsyncRuntime` when possible.
    pub fn stream_blocking(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Receiver<AgentEvent> {
        let (tx, rx) = mpsc::sync_channel(256);

        let request = self.build_request(system, messages, tools);
        let api_key = self.api_key.clone();

        std::thread::spawn(move || {
            if let Err(e) = stream_request(&api_key, &request, &tx) {
                let _ = tx.send(AgentEvent::Error(e));
            }
        });

        rx
    }

    fn build_request(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Request {
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System) // System goes in separate field
            .map(Into::into)
            .collect();

        let api_tools: Vec<ApiTool> = tools.iter().map(Into::into).collect();

        Request {
            model: self.model.clone(),
            max_tokens: MAX_TOKENS,
            system: system.to_string(),
            messages: api_messages,
            tools: if api_tools.is_empty() {
                None
            } else {
                Some(api_tools)
            },
            stream: true,
        }
    }
}

fn stream_request(
    api_key: &str,
    request: &Request,
    tx: &SyncSender<AgentEvent>,
) -> Result<(), AgentError> {
    let body = serde_json::to_string(request).map_err(|e| AgentError::Parse(e.to_string()))?;
    let headers = [("x-api-key", api_key), ("anthropic-version", API_VERSION)];
    super::http::streaming_post(API_URL, &headers, &body, parse_response, tx)
}

fn parse_response(mut body: ureq::Body, tx: &SyncSender<AgentEvent>) -> Result<(), AgentError> {
    let reader = std::io::BufReader::new(body.as_reader());
    parse_sse_stream(reader, tx)
}

fn parse_sse_stream<R: BufRead>(reader: R, tx: &SyncSender<AgentEvent>) -> Result<(), AgentError> {
    let mut current_tool_id: Option<String> = None;
    let mut current_tool_name: Option<String> = None;
    let mut current_tool_input = String::new();

    for line in reader.lines() {
        let line = line.map_err(|e| AgentError::Http(e.to_string()))?;

        // SSE format: "data: {...}"
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };

        // Parse the event
        let event: SseEvent = match serde_json::from_str(data) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Failed to parse SSE event: {e}");
                continue;
            }
        };

        match event {
            SseEvent::ContentBlockStart { content_block, .. } => {
                match content_block {
                    SseContentBlock::Text { .. } => {
                        // Text block starting, wait for deltas
                    }
                    SseContentBlock::ToolUse { id, name, .. } => {
                        current_tool_id = Some(id.clone());
                        current_tool_name = Some(name.clone());
                        current_tool_input.clear();
                        let _ = tx.send(AgentEvent::ToolCallStart {
                            id,
                            name,
                            raw_input: None,
                        });
                    }
                }
            }

            SseEvent::ContentBlockDelta { delta, .. } => match delta {
                SseDelta::TextDelta { text } => {
                    let _ = tx.send(AgentEvent::TextDelta(text));
                }
                SseDelta::InputJsonDelta { partial_json } => {
                    if let Some(ref id) = current_tool_id {
                        current_tool_input.push_str(&partial_json);
                        let _ = tx.send(AgentEvent::ToolCallInputDelta {
                            id: id.clone(),
                            delta: partial_json,
                        });
                    }
                }
                SseDelta::ThinkingDelta { thinking } => {
                    let _ = tx.send(AgentEvent::ThinkingDelta(thinking));
                }
            },

            SseEvent::ContentBlockStop { .. } => {
                // If we were building a tool call, emit the ready event
                if let (Some(id), Some(_name)) = (current_tool_id.take(), current_tool_name.take())
                {
                    let input: serde_json::Value = serde_json::from_str(&current_tool_input)
                        .unwrap_or(serde_json::Value::Null);
                    let _ = tx.send(AgentEvent::ToolCallReady { id, input });
                    current_tool_input.clear();
                }
            }

            SseEvent::MessageDelta { delta, usage } => {
                let stop_reason = match delta.stop_reason.as_deref() {
                    Some("tool_use") => StopReason::ToolUse,
                    Some("max_tokens") => StopReason::MaxTokens,
                    Some("stop_sequence") => StopReason::StopSequence,
                    _ => StopReason::EndTurn, // "end_turn" and others
                };

                let token_usage = usage.map(|u| TokenUsage {
                    input_tokens: u.input_tokens.unwrap_or(0),
                    output_tokens: u.output_tokens.unwrap_or(0),
                });

                let _ = tx.send(AgentEvent::Done {
                    stop_reason,
                    usage: token_usage,
                });
            }

            SseEvent::Error { error } => {
                return Err(AgentError::Api {
                    message: error.message,
                    kind: None,
                });
            }

            // MessageStart, MessageStop, Ping - no action needed
            SseEvent::MessageStart { .. } | SseEvent::MessageStop | SseEvent::Ping => {}
        }
    }

    Ok(())
}

// --- Request types ---

#[derive(Serialize)]
struct Request {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    stream: bool,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: ApiContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ApiContent {
    Text(String),
    Blocks(Vec<ApiContentBlock>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Serialize)]
struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

impl From<&Message> for ApiMessage {
    fn from(msg: &Message) -> Self {
        let role = match msg.role {
            Role::User | Role::System => "user", // System shouldn't happen, it's separate
            Role::Assistant => "assistant",
        };

        let content = match &msg.content {
            MessageContent::Text(s) => ApiContent::Text(s.clone()),
            MessageContent::Blocks(blocks) => {
                let api_blocks: Vec<ApiContentBlock> = blocks
                    .iter()
                    .map(|b| match b {
                        ContentBlock::Text { text } => ApiContentBlock::Text { text: text.clone() },
                        ContentBlock::ToolUse { id, name, input } => ApiContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        },
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } => ApiContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: content.clone(),
                        },
                    })
                    .collect();
                ApiContent::Blocks(api_blocks)
            }
        };

        Self {
            role: role.to_string(),
            content,
        }
    }
}

impl From<&ToolDefinition> for ApiTool {
    fn from(tool: &ToolDefinition) -> Self {
        Self {
            name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: tool.input_schema.clone(),
        }
    }
}

// --- SSE response types ---

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SseEvent {
    MessageStart {
        #[allow(dead_code)]
        message: SseMessage,
    },
    ContentBlockStart {
        #[allow(dead_code)]
        index: u32,
        content_block: SseContentBlock,
    },
    ContentBlockDelta {
        #[allow(dead_code)]
        index: u32,
        delta: SseDelta,
    },
    ContentBlockStop {
        #[allow(dead_code)]
        index: u32,
    },
    MessageDelta {
        delta: SseMessageDelta,
        usage: Option<SseUsage>,
    },
    MessageStop,
    Error {
        error: SseError,
    },
    Ping,
}

#[derive(Deserialize)]
struct SseMessage {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    model: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SseContentBlock {
    Text {
        #[allow(dead_code)]
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        #[allow(dead_code)]
        input: serde_json::Value,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)] // API uses these names
enum SseDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
}

#[derive(Deserialize)]
struct SseMessageDelta {
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct SseUsage {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct SseError {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_request() {
        let client = AnthropicClient::new("test-key", "claude-sonnet-4-20250514");
        let messages = vec![Message::user("Hello")];
        let tools = vec![];

        let request = client.build_request("You are helpful", &messages, &tools);

        assert_eq!(request.model, "claude-sonnet-4-20250514");
        assert_eq!(request.system, "You are helpful");
        assert_eq!(request.messages.len(), 1);
        assert!(request.tools.is_none());
        assert!(request.stream);
    }

    #[test]
    fn test_parse_text_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();

        match event {
            SseEvent::ContentBlockDelta {
                delta: SseDelta::TextDelta { text },
                ..
            } => {
                assert_eq!(text, "Hello");
            }
            _ => panic!("Expected TextDelta"),
        }
    }

    #[test]
    fn test_build_request_with_tools() {
        let client = AnthropicClient::new("key", "claude-sonnet-4-20250514");
        let messages = vec![Message::user("help")];
        let tools = vec![ToolDefinition {
            name: "search".to_string(),
            description: "Search things".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }];

        let request = client.build_request("Be helpful", &messages, &tools);

        assert!(request.tools.is_some());
        let api_tools = request.tools.unwrap();
        assert_eq!(api_tools.len(), 1);
        assert_eq!(api_tools[0].name, "search");
    }

    #[test]
    fn test_build_request_filters_system_messages() {
        let client = AnthropicClient::new("key", "claude-sonnet-4-20250514");
        let messages = vec![
            Message::system("system prompt"),
            Message::user("hello"),
            Message::assistant("hi"),
        ];

        let request = client.build_request("separate system", &messages, &[]);

        // System messages should be filtered out (system goes in separate field)
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.system, "separate system");
    }

    #[test]
    fn test_parse_message_start() {
        let data = r#"{"type":"message_start","message":{"id":"msg_1","model":"claude-3"}}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        assert!(matches!(event, SseEvent::MessageStart { .. }));
    }

    #[test]
    fn test_parse_message_stop() {
        let data = r#"{"type":"message_stop"}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        assert!(matches!(event, SseEvent::MessageStop));
    }

    #[test]
    fn test_parse_ping() {
        let data = r#"{"type":"ping"}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        assert!(matches!(event, SseEvent::Ping));
    }

    #[test]
    fn test_parse_content_block_start_tool_use() {
        let data = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"search","input":{}}}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        match event {
            SseEvent::ContentBlockStart {
                content_block: SseContentBlock::ToolUse { id, name, .. },
                ..
            } => {
                assert_eq!(id, "toolu_1");
                assert_eq!(name, "search");
            }
            _ => panic!("Expected ToolUse content block start"),
        }
    }

    #[test]
    fn test_parse_input_json_delta() {
        let data = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"query\":"}}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        match event {
            SseEvent::ContentBlockDelta {
                delta: SseDelta::InputJsonDelta { partial_json },
                ..
            } => {
                assert_eq!(partial_json, "{\"query\":");
            }
            _ => panic!("Expected InputJsonDelta"),
        }
    }

    #[test]
    fn test_parse_thinking_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me think..."}}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        match event {
            SseEvent::ContentBlockDelta {
                delta: SseDelta::ThinkingDelta { thinking },
                ..
            } => {
                assert_eq!(thinking, "Let me think...");
            }
            _ => panic!("Expected ThinkingDelta"),
        }
    }

    #[test]
    fn test_parse_message_delta_end_turn() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":100,"output_tokens":50}}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        match event {
            SseEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("end_turn"));
                let u = usage.unwrap();
                assert_eq!(u.input_tokens, Some(100));
                assert_eq!(u.output_tokens, Some(50));
            }
            _ => panic!("Expected MessageDelta"),
        }
    }

    #[test]
    fn test_parse_error_event() {
        let data = r#"{"type":"error","error":{"message":"overloaded"}}"#;
        let event: SseEvent = serde_json::from_str(data).unwrap();
        match event {
            SseEvent::Error { error } => {
                assert_eq!(error.message, "overloaded");
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_parse_sse_stream_text() {
        let input = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}\n";
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let reader = std::io::Cursor::new(input);
        parse_sse_stream(reader, &tx).unwrap();
        drop(tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], AgentEvent::TextDelta(t) if t == "Hi"));
        assert!(matches!(
            &events[1],
            AgentEvent::Done {
                stop_reason: StopReason::EndTurn,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_sse_stream_skips_non_data_lines() {
        let input = ": comment\nevent: message\ndata: {\"type\":\"message_stop\"}\n";
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let reader = std::io::Cursor::new(input);
        parse_sse_stream(reader, &tx).unwrap();
        drop(tx);

        let events: Vec<_> = rx.try_iter().collect();
        // message_stop doesn't emit an AgentEvent
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_sse_stream_tool_call_lifecycle() {
        let input = "\
data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"search\",\"input\":{}}}\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"q\\\":\"}}\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"test\\\"}\"}}\n\
data: {\"type\":\"content_block_stop\",\"index\":1}\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"}}\n";

        let (tx, rx) = std::sync::mpsc::sync_channel(32);
        let reader = std::io::Cursor::new(input);
        parse_sse_stream(reader, &tx).unwrap();
        drop(tx);

        let events: Vec<_> = rx.try_iter().collect();
        // Should see: ToolCallStart, InputDelta x2, ToolCallReady, Done
        assert!(matches!(
            &events[0],
            AgentEvent::ToolCallStart { name, .. } if name == "search"
        ));
        assert!(matches!(&events[1], AgentEvent::ToolCallInputDelta { .. }));
        assert!(matches!(&events[2], AgentEvent::ToolCallInputDelta { .. }));
        assert!(matches!(&events[3], AgentEvent::ToolCallReady { id, .. } if id == "t1"));
        assert!(matches!(
            &events[4],
            AgentEvent::Done {
                stop_reason: StopReason::ToolUse,
                ..
            }
        ));
    }

    #[test]
    fn test_api_message_from_text_message() {
        let msg = Message::user("hello");
        let api_msg: ApiMessage = (&msg).into();
        assert_eq!(api_msg.role, "user");
        assert!(matches!(api_msg.content, ApiContent::Text(s) if s == "hello"));
    }

    #[test]
    fn test_api_message_from_blocks_message() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "result".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "t1".to_string(),
                    name: "search".to_string(),
                    input: serde_json::json!({"q": "test"}),
                },
            ]),
        };
        let api_msg: ApiMessage = (&msg).into();
        assert_eq!(api_msg.role, "assistant");
        assert!(matches!(api_msg.content, ApiContent::Blocks(blocks) if blocks.len() == 2));
    }
}
