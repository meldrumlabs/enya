//! OpenAI API client with SSE streaming.
//!
//! Implements the Chat Completions API: https://platform.openai.com/docs/api-reference/chat

use std::io::BufRead;
use std::sync::mpsc::{self, Receiver, SyncSender};

use serde::{Deserialize, Serialize};

use crate::types::{
    AgentError, AgentEvent, ContentBlock, Message, MessageContent, Role, StopReason, TokenUsage,
    ToolDefinition,
};

const API_URL: &str = "https://api.openai.com/v1/chat/completions";
const MAX_TOKENS: u32 = 8192;

/// OpenAI API client.
#[derive(Clone)]
pub struct OpenAIClient {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

impl OpenAIClient {
    /// Create a new client.
    #[must_use]
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: None,
        }
    }

    /// Create a client with a custom base URL (for OpenAI-compatible APIs).
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
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
        let url = self.base_url.clone().unwrap_or_else(|| API_URL.to_string());

        // Spawn blocking work on the runtime's thread pool
        // ureq is blocking, so we use spawn_blocking via a dedicated thread
        tokio::spawn(async move {
            let result =
                tokio::task::spawn_blocking(move || stream_request(&url, &api_key, &request, &tx))
                    .await;

            if let Err(e) = result {
                tracing::error!("Stream task panicked: {e}");
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
        let url = self.base_url.clone().unwrap_or_else(|| API_URL.to_string());

        std::thread::spawn(move || {
            if let Err(e) = stream_request(&url, &api_key, &request, &tx) {
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
        let mut api_messages = vec![ApiMessage {
            role: "system".to_string(),
            content: Some(system.to_string()),
            tool_calls: None,
            tool_call_id: None,
        }];

        for msg in messages {
            api_messages.push(msg.into());
        }

        let api_tools: Vec<ApiTool> = tools.iter().map(Into::into).collect();

        Request {
            model: self.model.clone(),
            max_tokens: Some(MAX_TOKENS),
            messages: api_messages,
            tools: if api_tools.is_empty() {
                None
            } else {
                Some(api_tools)
            },
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
        }
    }
}

fn stream_request(
    url: &str,
    api_key: &str,
    request: &Request,
    tx: &SyncSender<AgentEvent>,
) -> Result<(), AgentError> {
    let body = serde_json::to_string(request).map_err(|e| AgentError::Parse(e.to_string()))?;
    let auth = format!("Bearer {api_key}");
    let headers = [("Authorization", auth.as_str())];
    super::http::streaming_post(url, &headers, &body, parse_response, tx)
}

fn parse_response(mut body: ureq::Body, tx: &SyncSender<AgentEvent>) -> Result<(), AgentError> {
    let reader = std::io::BufReader::new(body.as_reader());
    parse_sse_stream(reader, tx)
}

fn parse_sse_stream<R: BufRead>(reader: R, tx: &SyncSender<AgentEvent>) -> Result<(), AgentError> {
    // Track tool calls being built (OpenAI streams them in pieces)
    let mut tool_calls: rustc_hash::FxHashMap<u32, ToolCallBuilder> =
        rustc_hash::FxHashMap::default();

    // Track usage across chunks (only populated on final chunk)
    let mut final_usage: Option<TokenUsage> = None;

    for line in reader.lines() {
        let line = line.map_err(|e| AgentError::Http(e.to_string()))?;

        // SSE format: "data: {...}" or "data: [DONE]"
        let Some(data) = line
            .strip_prefix("data: ")
            .or_else(|| line.strip_prefix("data:"))
        else {
            continue;
        };

        let data = data.trim();

        if data == "[DONE]" {
            break;
        }

        if data.is_empty() {
            continue;
        }

        // Parse the chunk
        let chunk: SseChunk = match serde_json::from_str(data) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to parse SSE chunk: {e}");
                continue;
            }
        };

        // Capture usage if present (appears on final chunk)
        if let Some(ref usage) = chunk.usage {
            final_usage = Some(TokenUsage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            });
        }

        // Process choices
        for choice in chunk.choices {
            let delta = choice.delta;

            // Text content
            if let Some(content) = delta.content {
                if !content.is_empty() {
                    let _ = tx.send(AgentEvent::TextDelta(content));
                }
            }

            // Tool calls
            if let Some(tool_call_deltas) = delta.tool_calls {
                for tc_delta in tool_call_deltas {
                    let idx = tc_delta.index;

                    // Get or create builder
                    let builder = tool_calls.entry(idx).or_insert_with(|| ToolCallBuilder {
                        id: String::new(),
                        name: String::new(),
                        arguments: String::new(),
                        started: false,
                    });

                    // Update ID if present
                    if let Some(id) = tc_delta.id {
                        builder.id = id;
                    }

                    // Update name if present
                    if let Some(ref func) = tc_delta.function {
                        if let Some(ref name) = func.name {
                            builder.name = name.clone();
                        }
                        if let Some(ref args) = func.arguments {
                            builder.arguments.push_str(args);

                            // Send input delta
                            if !builder.id.is_empty() {
                                let _ = tx.send(AgentEvent::ToolCallInputDelta {
                                    id: builder.id.clone(),
                                    delta: args.clone(),
                                });
                            }
                        }
                    }

                    // Emit start event once we have id and name
                    if !builder.started && !builder.id.is_empty() && !builder.name.is_empty() {
                        builder.started = true;
                        let _ = tx.send(AgentEvent::ToolCallStart {
                            id: builder.id.clone(),
                            name: builder.name.clone(),
                            raw_input: None,
                        });
                    }
                }
            }

            // Check for finish reason
            if let Some(finish_reason) = choice.finish_reason {
                // Finalize any pending tool calls
                for builder in tool_calls.values() {
                    if builder.started {
                        let input: serde_json::Value = serde_json::from_str(&builder.arguments)
                            .unwrap_or(serde_json::Value::Null);
                        let _ = tx.send(AgentEvent::ToolCallReady {
                            id: builder.id.clone(),
                            input,
                        });
                    }
                }
                tool_calls.clear();

                let stop_reason = match finish_reason.as_str() {
                    "tool_calls" => StopReason::ToolUse,
                    "length" => StopReason::MaxTokens,
                    _ => StopReason::EndTurn, // "stop" and others
                };

                let _ = tx.send(AgentEvent::Done {
                    stop_reason,
                    usage: final_usage,
                });
            }
        }
    }

    Ok(())
}

struct ToolCallBuilder {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

// --- Request types ---

#[derive(Serialize)]
struct Request {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct ApiToolCall {
    id: String,
    r#type: String,
    function: ApiFunctionCall,
}

#[derive(Serialize)]
struct ApiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct ApiTool {
    r#type: String,
    function: ApiFunction,
}

#[derive(Serialize)]
struct ApiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

impl From<&Message> for ApiMessage {
    fn from(msg: &Message) -> Self {
        match &msg.content {
            MessageContent::Text(s) => Self {
                role: match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => "system",
                }
                .to_string(),
                content: Some(s.clone()),
                tool_calls: None,
                tool_call_id: None,
            },
            MessageContent::Blocks(blocks) => {
                // Check if this is a tool result message
                if let Some(ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    ..
                }) = blocks.first()
                {
                    return Self {
                        role: "tool".to_string(),
                        content: Some(content.clone()),
                        tool_calls: None,
                        tool_call_id: Some(tool_use_id.clone()),
                    };
                }

                // Check if assistant message with tool calls
                let tool_calls: Vec<ApiToolCall> = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::ToolUse { id, name, input } => Some(ApiToolCall {
                            id: id.clone(),
                            r#type: "function".to_string(),
                            function: ApiFunctionCall {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        }),
                        _ => None,
                    })
                    .collect();

                // Get text content
                let text_content: String = blocks
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");

                Self {
                    role: match msg.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::System => "system",
                    }
                    .to_string(),
                    content: if text_content.is_empty() {
                        None
                    } else {
                        Some(text_content)
                    },
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                }
            }
        }
    }
}

impl From<&ToolDefinition> for ApiTool {
    fn from(tool: &ToolDefinition) -> Self {
        Self {
            r#type: "function".to_string(),
            function: ApiFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            },
        }
    }
}

// --- SSE response types ---

#[derive(Deserialize)]
struct SseChunk {
    choices: Vec<SseChoice>,
    usage: Option<SseUsage>,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: SseDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct SseDelta {
    content: Option<String>,
    tool_calls: Option<Vec<SseToolCallDelta>>,
}

#[derive(Deserialize)]
struct SseToolCallDelta {
    index: u32,
    id: Option<String>,
    function: Option<SseFunctionDelta>,
}

#[derive(Deserialize)]
struct SseFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct SseUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_request() {
        let client = OpenAIClient::new("test-key", "gpt-4o");
        let messages = vec![Message::user("Hello")];
        let tools = vec![];

        let request = client.build_request("You are helpful", &messages, &tools);

        assert_eq!(request.model, "gpt-4o");
        assert_eq!(request.messages.len(), 2); // system + user
        assert_eq!(request.messages[0].role, "system");
        assert!(request.tools.is_none());
        assert!(request.stream);
    }

    #[test]
    fn test_parse_text_delta() {
        let data = r#"{"id":"chatcmpl-123","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk: SseChunk = serde_json::from_str(data).unwrap();

        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_build_request_with_tools() {
        let client = OpenAIClient::new("key", "gpt-4o");
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
        assert_eq!(api_tools[0].function.name, "search");
        assert_eq!(api_tools[0].r#type, "function");
    }

    #[test]
    fn test_build_request_prepends_system() {
        let client = OpenAIClient::new("key", "gpt-4o");
        let messages = vec![Message::user("hi"), Message::assistant("hello")];

        let request = client.build_request("system prompt", &messages, &[]);

        // system + user + assistant = 3
        assert_eq!(request.messages.len(), 3);
        assert_eq!(request.messages[0].role, "system");
        assert_eq!(request.messages[0].content, Some("system prompt".into()));
    }

    #[test]
    fn test_build_request_stream_options() {
        let client = OpenAIClient::new("key", "gpt-4o");
        let request = client.build_request("sys", &[], &[]);

        assert!(request.stream);
        assert!(request.stream_options.is_some());
        assert!(request.stream_options.unwrap().include_usage);
    }

    #[test]
    fn test_with_base_url() {
        let client = OpenAIClient::new("key", "model").with_base_url("https://custom.api.com/v1");
        assert_eq!(client.base_url, Some("https://custom.api.com/v1".into()));
    }

    #[test]
    fn test_parse_chunk_with_finish_reason() {
        let data = r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let chunk: SseChunk = serde_json::from_str(data).unwrap();

        assert_eq!(chunk.choices[0].finish_reason, Some("stop".to_string()));
    }

    #[test]
    fn test_parse_chunk_with_tool_call() {
        let data = r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"search","arguments":""}}]},"finish_reason":null}]}"#;
        let chunk: SseChunk = serde_json::from_str(data).unwrap();

        let tc = &chunk.choices[0].delta.tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.id, Some("call_1".to_string()));
        assert_eq!(tc.function.as_ref().unwrap().name, Some("search".into()));
    }

    #[test]
    fn test_parse_chunk_with_usage() {
        let data = r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":50}}"#;
        let chunk: SseChunk = serde_json::from_str(data).unwrap();

        let usage = chunk.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
    }

    #[test]
    fn test_parse_sse_stream_text() {
        let input = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\ndata: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\ndata: [DONE]\n";
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
    fn test_parse_sse_stream_done_marker() {
        let input = "data: [DONE]\n";
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let reader = std::io::Cursor::new(input);
        parse_sse_stream(reader, &tx).unwrap();
        drop(tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_sse_stream_skips_empty_and_non_data() {
        let input = "\n: comment\nevent: message\n\ndata: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"},\"finish_reason\":null}]}\ndata: [DONE]\n";
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let reader = std::io::Cursor::new(input);
        parse_sse_stream(reader, &tx).unwrap();
        drop(tx);

        let events: Vec<_> = rx.try_iter().collect();
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], AgentEvent::TextDelta(t) if t == "ok"));
    }

    #[test]
    fn test_parse_sse_stream_tool_calls_finish_reason() {
        let input = "\
data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"search\",\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"q\\\":\\\"test\\\"}\"}}]},\"finish_reason\":null}]}\n\
data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\
data: [DONE]\n";

        let (tx, rx) = std::sync::mpsc::sync_channel(32);
        let reader = std::io::Cursor::new(input);
        parse_sse_stream(reader, &tx).unwrap();
        drop(tx);

        let events: Vec<_> = rx.try_iter().collect();
        // ToolCallStart, InputDelta, InputDelta (second chunk), ToolCallReady, Done
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolCallStart { name, .. } if name == "search"))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            AgentEvent::Done {
                stop_reason: StopReason::ToolUse,
                ..
            }
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::ToolCallReady { .. }))
        );
    }

    #[test]
    fn test_api_message_from_text() {
        let msg = Message::user("hello");
        let api_msg: ApiMessage = (&msg).into();
        assert_eq!(api_msg.role, "user");
        assert_eq!(api_msg.content, Some("hello".into()));
        assert!(api_msg.tool_calls.is_none());
        assert!(api_msg.tool_call_id.is_none());
    }

    #[test]
    fn test_api_message_from_tool_result() {
        let msg = Message {
            role: Role::User,
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: "result data".to_string(),
                is_error: None,
            }]),
        };
        let api_msg: ApiMessage = (&msg).into();
        assert_eq!(api_msg.role, "tool");
        assert_eq!(api_msg.content, Some("result data".into()));
        assert_eq!(api_msg.tool_call_id, Some("call_1".into()));
    }

    #[test]
    fn test_api_message_from_assistant_with_tool_calls() {
        let msg = Message {
            role: Role::Assistant,
            content: MessageContent::Blocks(vec![
                ContentBlock::Text {
                    text: "Let me search".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "call_1".to_string(),
                    name: "search".to_string(),
                    input: serde_json::json!({"q": "test"}),
                },
            ]),
        };
        let api_msg: ApiMessage = (&msg).into();
        assert_eq!(api_msg.role, "assistant");
        assert_eq!(api_msg.content, Some("Let me search".into()));
        let tc = api_msg.tool_calls.unwrap();
        assert_eq!(tc.len(), 1);
        assert_eq!(tc[0].id, "call_1");
        assert_eq!(tc[0].function.name, "search");
    }

    #[test]
    fn test_api_tool_from_definition() {
        let def = ToolDefinition {
            name: "query".to_string(),
            description: "Run query".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let api_tool: ApiTool = (&def).into();
        assert_eq!(api_tool.r#type, "function");
        assert_eq!(api_tool.function.name, "query");
        assert_eq!(api_tool.function.description, "Run query");
    }
}
