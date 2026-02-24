//! Typed JSON-RPC 2.0 protocol structs for ACP communication.

use serde::{Deserialize, Serialize};

// -- JSON-RPC request envelope --

/// A JSON-RPC 2.0 request message.
#[derive(Debug, Serialize)]
pub(super) struct RpcRequest<P: Serialize> {
    pub jsonrpc: &'static str,
    pub id: u32,
    pub method: &'static str,
    pub params: P,
}

impl<P: Serialize> RpcRequest<P> {
    pub(super) fn new(id: u32, method: &'static str, params: P) -> Self {
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
pub(super) struct InitializeParams {
    pub protocol_version: u32,
    pub client_info: ClientInfo,
    pub client_capabilities: ClientCapabilities,
}

#[derive(Debug, Serialize)]
pub(super) struct ClientInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct ClientCapabilities {
    pub terminal: bool,
}

// -- Session New --

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionNewParams {
    pub cwd: String,
    pub mcp_servers: Vec<()>,
    #[serde(rename = "_meta")]
    pub meta: SessionMeta,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionMeta {
    pub claude_code: ClaudeCodeMeta,
}

#[derive(Debug, Serialize)]
pub(super) struct ClaudeCodeMeta {
    pub options: ClaudeCodeOptions,
}

#[derive(Debug, Serialize)]
pub(super) struct ClaudeCodeOptions {
    pub model: String,
}

// -- Session Prompt --

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionPromptParams {
    pub session_id: String,
    pub prompt: Vec<PromptContent>,
}

#[derive(Debug, Serialize)]
pub(super) struct PromptContent {
    #[serde(rename = "type")]
    pub content_type: &'static str,
    pub text: String,
}

// -- Response types --

/// A JSON-RPC 2.0 response envelope.
#[derive(Debug, Deserialize)]
pub(super) struct RpcResponse<R> {
    #[allow(dead_code)]
    pub id: Option<serde_json::Value>,
    pub result: Option<R>,
    pub error: Option<RpcErrorObject>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Deserialize)]
pub(super) struct RpcErrorObject {
    pub code: i32,
    pub message: String,
}

/// Result from the `session/new` method.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionNewResult {
    pub session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_request_serialization() {
        let req = RpcRequest::new(
            1,
            "initialize",
            InitializeParams {
                protocol_version: 1,
                client_info: ClientInfo {
                    name: "Enya",
                    version: "0.1.0",
                },
                client_capabilities: ClientCapabilities { terminal: true },
            },
        );

        let json = serde_json::to_string(&req).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "initialize");
        assert_eq!(parsed["params"]["protocolVersion"], 1);
        assert_eq!(parsed["params"]["clientInfo"]["name"], "Enya");
        assert_eq!(parsed["params"]["clientCapabilities"]["terminal"], true);
    }

    #[test]
    fn session_new_params_serialization() {
        let params = SessionNewParams {
            cwd: "/home/user".to_string(),
            mcp_servers: vec![],
            meta: SessionMeta {
                claude_code: ClaudeCodeMeta {
                    options: ClaudeCodeOptions {
                        model: "claude-sonnet-4-5-20250514".to_string(),
                    },
                },
            },
        };

        let json = serde_json::to_string(&params).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["cwd"], "/home/user");
        assert_eq!(parsed["mcpServers"], serde_json::json!([]));
        assert_eq!(
            parsed["_meta"]["claudeCode"]["options"]["model"],
            "claude-sonnet-4-5-20250514"
        );
    }

    #[test]
    fn session_prompt_params_serialization() {
        let params = SessionPromptParams {
            session_id: "sess_123".to_string(),
            prompt: vec![PromptContent {
                content_type: "text",
                text: "Hello agent".to_string(),
            }],
        };

        let json = serde_json::to_string(&params).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["sessionId"], "sess_123");
        assert_eq!(parsed["prompt"][0]["type"], "text");
        assert_eq!(parsed["prompt"][0]["text"], "Hello agent");
    }

    #[test]
    fn rpc_response_deserialization() {
        let json = r#"{"id":1,"result":{"sessionId":"sess_abc"}}"#;
        let resp: RpcResponse<SessionNewResult> = serde_json::from_str(json).unwrap();

        assert!(resp.error.is_none());
        assert_eq!(
            resp.result.unwrap().session_id,
            Some("sess_abc".to_string())
        );
    }

    #[test]
    fn rpc_response_with_error() {
        let json = r#"{"id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let resp: RpcResponse<SessionNewResult> = serde_json::from_str(json).unwrap();

        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn rpc_response_with_null_session_id() {
        let json = r#"{"id":2,"result":{}}"#;
        let resp: RpcResponse<SessionNewResult> = serde_json::from_str(json).unwrap();

        assert!(resp.result.unwrap().session_id.is_none());
    }

    #[test]
    fn prompt_content_serialization() {
        let content = PromptContent {
            content_type: "text",
            text: "What is the CPU usage?".to_string(),
        };
        let json = serde_json::to_string(&content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "text");
        assert_eq!(parsed["text"], "What is the CPU usage?");
    }
}
