//! Typed JSON-RPC 2.0 protocol structs for ACP communication.

use serde::{Deserialize, Serialize};

// -- JSON-RPC request envelope --

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Serialize)]
pub struct RpcRequest<P: Serialize> {
    pub jsonrpc: &'static str,
    pub id: u32,
    pub method: &'static str,
    pub params: P,
}

impl<P: Serialize> RpcRequest<P> {
    pub fn new(id: u32, method: &'static str, params: P) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}

// -- Initialize --

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_info: ClientInfo,
    pub client_capabilities: ClientCapabilities,
}

#[derive(Debug, Serialize)]
pub struct ClientInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ClientCapabilities {
    pub terminal: bool,
}

// -- Session New --

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewParams {
    pub cwd: String,
    pub mcp_servers: Vec<()>,
    #[serde(rename = "_meta")]
    pub meta: SessionMeta,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub claude_code: ClaudeCodeMeta,
}

#[derive(Debug, Serialize)]
pub struct ClaudeCodeMeta {
    pub options: ClaudeCodeOptions,
}

#[derive(Debug, Serialize)]
pub struct ClaudeCodeOptions {
    pub model: String,
}

// -- Session Prompt --

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
    pub session_id: String,
    pub prompt: Vec<PromptContent>,
}

#[derive(Debug, Serialize)]
pub struct PromptContent {
    #[serde(rename = "type")]
    pub content_type: &'static str,
    pub text: String,
}

// -- Response types --

/// A JSON-RPC 2.0 response envelope.
#[derive(Debug, Deserialize)]
pub struct RpcResponse<R> {
    #[allow(dead_code)]
    pub id: Option<serde_json::Value>,
    pub result: Option<R>,
    pub error: Option<RpcErrorObject>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Deserialize)]
pub struct RpcErrorObject {
    pub code: i32,
    pub message: String,
}

/// Result from the `session/new` method.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewResult {
    pub session_id: Option<String>,
}
