//! ACP client implementation.
//!
//! This module provides a client wrapper for connecting to ACP-compatible
//! AI coding agents via JSON-RPC 2.0 over stdio.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, info, trace, warn};

use super::config::AgentConfig;
use super::protocol::{
    ClaudeCodeMeta, ClaudeCodeOptions, ClientCapabilities, ClientInfo, InitializeParams,
    PromptContent, RpcRequest, RpcResponse, SessionMeta, SessionNewParams, SessionNewResult,
    SessionPromptParams,
};
use crate::types::{AgentError, AgentEvent, StopReason};

/// Client name sent to agents during initialization.
const CLIENT_NAME: &str = "Enya";
/// Client version sent to agents.
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// ACP client for connecting to AI coding agents.
///
/// This client spawns an agent subprocess and communicates with it using
/// the Agent Client Protocol over stdio.
///
/// # Supported Agents
///
/// - Claude Code (`claude --acp`)
/// - Gemini CLI (`gemini --acp`)
/// - Codex (`codex --acp`)
/// - Any ACP-compatible agent
pub struct AcpClient {
    config: AgentConfig,
    /// Optional tokio runtime handle for spawning tasks.
    /// If not provided, uses the current runtime context (must be in a tokio context).
    runtime: Option<tokio::runtime::Handle>,
}

impl AcpClient {
    /// Create a new ACP client with the given configuration.
    #[must_use]
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            runtime: None,
        }
    }

    /// Create a new ACP client with a specific runtime handle.
    ///
    /// Use this when you need to spawn tasks from outside a tokio runtime context.
    #[must_use]
    pub fn with_runtime(config: AgentConfig, runtime: tokio::runtime::Handle) -> Self {
        Self {
            config,
            runtime: Some(runtime),
        }
    }

    /// Create a new ACP client configured for Claude Code.
    #[must_use]
    pub fn claude_code() -> Self {
        Self::new(AgentConfig::claude_code())
    }

    /// Create a new ACP client configured for Claude Code with a runtime handle.
    #[must_use]
    pub fn claude_code_with_runtime(runtime: tokio::runtime::Handle) -> Self {
        Self::with_runtime(AgentConfig::claude_code(), runtime)
    }

    /// Create a new ACP client configured for Gemini CLI.
    #[must_use]
    pub fn gemini_cli() -> Self {
        Self::new(AgentConfig::gemini_cli())
    }

    /// Create a new ACP client configured for Codex.
    #[must_use]
    pub fn codex() -> Self {
        Self::new(AgentConfig::codex())
    }

    /// Create a new ACP client configured for Codex with a runtime handle.
    #[must_use]
    pub fn codex_with_runtime(runtime: tokio::runtime::Handle) -> Self {
        Self::with_runtime(AgentConfig::codex(), runtime)
    }

    /// Send a prompt to the agent and receive streaming events.
    ///
    /// This spawns the agent subprocess, initializes it, creates a session,
    /// and sends the prompt. Events are streamed back through the returned channel.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's message to send to the agent
    /// * `working_dir` - Optional working directory for the agent
    ///
    /// # Returns
    ///
    /// A receiver that yields `AgentEvent`s as the agent responds.
    pub fn prompt(
        &self,
        prompt: impl Into<String>,
        working_dir: Option<PathBuf>,
    ) -> Receiver<AgentEvent> {
        self.prompt_with_model(prompt, working_dir, None)
    }

    /// Send a prompt to the agent with a specific model.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's message to send to the agent
    /// * `working_dir` - Optional working directory for the agent
    /// * `model` - Optional model ID (e.g., "claude-sonnet-4-5-20250514")
    ///
    /// # Returns
    ///
    /// A receiver that yields `AgentEvent`s as the agent responds.
    pub fn prompt_with_model(
        &self,
        prompt: impl Into<String>,
        working_dir: Option<PathBuf>,
        model: Option<&str>,
    ) -> Receiver<AgentEvent> {
        self.prompt_with_context(prompt, working_dir, model, None)
    }

    /// Send a prompt to the agent with a system context.
    ///
    /// The system context is prepended to the prompt to provide additional
    /// information to the agent (e.g., available metrics, connection status).
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's message to send to the agent
    /// * `working_dir` - Optional working directory for the agent
    /// * `model` - Optional model ID (e.g., "claude-sonnet-4-5-20250514")
    /// * `system_context` - Optional system context to prepend to the prompt
    ///
    /// # Returns
    ///
    /// A receiver that yields `AgentEvent`s as the agent responds.
    pub fn prompt_with_context(
        &self,
        prompt: impl Into<String>,
        working_dir: Option<PathBuf>,
        model: Option<&str>,
        system_context: Option<&str>,
    ) -> Receiver<AgentEvent> {
        let (tx, rx) = mpsc::sync_channel(256);
        let prompt = prompt.into();
        let model = model.map(String::from);
        let system_context = system_context.map(String::from);

        let config = if let Some(dir) = working_dir {
            self.config.clone().with_working_dir(dir)
        } else {
            self.config.clone()
        };

        // Spawn a tokio task to handle the async ACP connection
        // Use the provided runtime handle if available, otherwise use current context
        let future = async move {
            if let Err(e) = run_acp_session(
                &config,
                &prompt,
                model.as_deref(),
                system_context.as_deref(),
                tx.clone(),
            )
            .await
            {
                let _ = tx.send(AgentEvent::Error(e));
            }
        };

        if let Some(ref handle) = self.runtime {
            handle.spawn(future);
        } else {
            tokio::spawn(future);
        }

        rx
    }

    /// Check if an agent is available.
    ///
    /// Returns `true` if the agent command exists and can be executed.
    pub fn is_available(&self) -> bool {
        std::process::Command::new(&self.config.command)
            .arg("--version")
            .output()
            .is_ok()
    }

    /// Get the agent configuration.
    #[must_use]
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Get the agent kind.
    #[must_use]
    pub fn kind(&self) -> super::config::AgentKind {
        self.config.kind
    }
}

/// Spawn the agent process with the given configuration.
fn spawn_agent(config: &AgentConfig) -> Result<Child, AgentError> {
    let mut cmd = Command::new(&config.command);

    for arg in &config.args {
        cmd.arg(arg);
    }

    if let Some(ref dir) = config.working_dir {
        cmd.current_dir(dir);
    }

    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    for key in &config.env_remove {
        cmd.env_remove(key);
    }

    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit());

    cmd.spawn()
        .map_err(|e| AgentError::Process(format!("failed to spawn '{}': {e}", config.command)))
}

/// Default model when none is specified.
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250514";

/// Run a complete ACP session over JSON-RPC 2.0.
///
/// Spawns the agent process, performs the initialize handshake, creates
/// a session, sends the prompt, and streams responses back through the
/// channel.
#[allow(clippy::too_many_lines)]
async fn run_acp_session(
    config: &AgentConfig,
    prompt: &str,
    model: Option<&str>,
    system_context: Option<&str>,
    tx: SyncSender<AgentEvent>,
) -> Result<(), AgentError> {
    let model_id = model.unwrap_or(DEFAULT_MODEL);
    info!(
        agent = config.kind.display_name(),
        command = %config.command,
        model = model_id,
        "starting ACP session"
    );

    // Spawn the agent process with the model set via env var and CLI arg
    let config = config
        .clone()
        .with_env("ANTHROPIC_MODEL", model_id)
        .with_arg("--model")
        .with_arg(model_id);
    let mut child = spawn_agent(&config)?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AgentError::Process("failed to get agent stdin".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AgentError::Process("failed to get agent stdout".into()))?;

    let mut reader = BufReader::new(stdout).lines();
    let mut writer = tokio::io::BufWriter::new(stdin);

    // Send initialization
    let init_msg = RpcRequest::new(
        1,
        "initialize",
        InitializeParams {
            protocol_version: 1,
            client_info: ClientInfo {
                name: CLIENT_NAME,
                version: CLIENT_VERSION,
            },
            client_capabilities: ClientCapabilities { terminal: true },
        },
    );
    send_message(&mut writer, &init_msg).await?;
    read_response(&mut reader, "init").await?;

    // Create session
    let cwd = config
        .working_dir
        .as_ref()
        .map_or_else(|| ".".to_string(), |p| p.display().to_string());
    let session_msg = RpcRequest::new(
        2,
        "session/new",
        SessionNewParams {
            cwd,
            mcp_servers: vec![],
            meta: SessionMeta {
                claude_code: ClaudeCodeMeta {
                    options: ClaudeCodeOptions {
                        model: model_id.to_string(),
                    },
                },
            },
        },
    );
    send_message(&mut writer, &session_msg).await?;
    let session_id = read_session_id(&mut reader).await?;

    // Send prompt (system context is prepended if provided)
    let full_prompt = if let Some(ctx) = system_context {
        format!("{ctx}\n\n---\n\n{prompt}")
    } else {
        prompt.to_string()
    };

    let prompt_msg = RpcRequest::new(
        3,
        "session/prompt",
        SessionPromptParams {
            session_id,
            prompt: vec![PromptContent {
                content_type: "text",
                text: full_prompt,
            }],
        },
    );
    send_message(&mut writer, &prompt_msg).await?;

    // Read streaming responses
    read_streaming_responses(&mut reader, &tx).await?;

    // Clean up
    let _ = child.kill().await;
    Ok(())
}

/// Send a typed JSON-RPC message to the agent.
async fn send_message<T: serde::Serialize>(
    writer: &mut tokio::io::BufWriter<tokio::process::ChildStdin>,
    msg: &T,
) -> Result<(), AgentError> {
    let json = serde_json::to_string(msg)
        .map_err(|e| AgentError::Process(format!("failed to serialize message: {e}")))?;
    writer
        .write_all(json.as_bytes())
        .await
        .map_err(|e| AgentError::Process(format!("failed to write: {e}")))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|e| AgentError::Process(format!("failed to write: {e}")))?;
    writer
        .flush()
        .await
        .map_err(|e| AgentError::Process(format!("failed to flush: {e}")))
}

/// Read a response from the agent.
async fn read_response(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    label: &str,
) -> Result<Option<String>, AgentError> {
    if let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| AgentError::Process(format!("failed to read: {e}")))?
    {
        debug!(label, "response: {line}");
        Ok(Some(line))
    } else {
        Ok(None)
    }
}

/// Read and extract session ID from response.
async fn read_session_id(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
) -> Result<String, AgentError> {
    if let Some(line) = read_response(reader, "session").await? {
        if let Ok(resp) = serde_json::from_str::<RpcResponse<SessionNewResult>>(&line) {
            if let Some(ref error) = resp.error {
                warn!(code = error.code, message = %error.message, "session/new returned error");
            }
            if let Some(result) = resp.result {
                if let Some(id) = result.session_id {
                    return Ok(id);
                }
            }
            warn!("session/new response missing sessionId, using fallback");
        }
    } else {
        warn!("no response from session/new, using fallback");
    }
    Ok("default".to_string())
}

/// Read streaming responses until completion.
async fn read_streaming_responses(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    tx: &SyncSender<AgentEvent>,
) -> Result<(), AgentError> {
    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| AgentError::Process(format!("failed to read: {e}")))?
    {
        trace!("ACP message: {line}");

        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            // Check for notifications
            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                if method == "session/update" {
                    if let Some(params) = msg.get("params") {
                        process_session_update(params, tx);
                    }
                }
            }
            // Check for prompt response (completion)
            if msg.get("id") == Some(&serde_json::json!(3)) {
                if let Some(result) = msg.get("result") {
                    if let Some(reason) = result.get("stopReason").and_then(|r| r.as_str()) {
                        let stop_reason = match reason {
                            "tool_use" => StopReason::ToolUse,
                            "max_tokens" => StopReason::MaxTokens,
                            "stop_sequence" => StopReason::StopSequence,
                            "end_turn" => StopReason::EndTurn,
                            other => {
                                debug!(
                                    reason = other,
                                    "unknown stop reason, defaulting to EndTurn"
                                );
                                StopReason::EndTurn
                            }
                        };
                        let _ = tx.send(AgentEvent::Done {
                            stop_reason,
                            usage: None,
                        });
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Extract a tool call or update ID from the update payload.
///
/// Tries `toolCallId` first, then falls back to `id`.
fn extract_tool_id(update: &serde_json::Value) -> String {
    if let Some(id) = update.get("toolCallId").and_then(|i| i.as_str()) {
        return id.to_string();
    }
    if let Some(id) = update.get("id").and_then(|i| i.as_str()) {
        debug!("tool ID resolved from 'id' field instead of 'toolCallId'");
        return id.to_string();
    }
    warn!("tool call/update missing toolCallId and id, using 'unknown'");
    "unknown".to_string()
}

/// Process a session update notification and emit appropriate events.
fn process_session_update(params: &serde_json::Value, tx: &SyncSender<AgentEvent>) {
    let Some(update) = params.get("update") else {
        return;
    };

    let Some(update_type) = update.get("sessionUpdate").and_then(|u| u.as_str()) else {
        return;
    };

    match update_type {
        "agent_message_chunk" => {
            if let Some(content) = update.get("content") {
                if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                    let _ = tx.send(AgentEvent::TextDelta(text.to_string()));
                }
            }
        }
        "agent_thought_chunk" => {
            if let Some(content) = update.get("content") {
                if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                    let _ = tx.send(AgentEvent::ThinkingDelta(text.to_string()));
                }
            }
        }
        "tool_call" => {
            let id = extract_tool_id(update);

            // Extract tool name — check multiple locations for cross-agent compat
            let name = if let Some(n) = update.get("name").and_then(|n| n.as_str()) {
                n.to_string()
            } else if let Some(n) = update
                .get("_meta")
                .and_then(|m| m.get("claudeCode"))
                .and_then(|c| c.get("toolName"))
                .and_then(|n| n.as_str())
            {
                debug!(
                    name = n,
                    "tool name resolved from _meta.claudeCode.toolName"
                );
                n.to_string()
            } else if let Some(n) = update.get("title").and_then(|t| t.as_str()) {
                debug!(name = n, "tool name resolved from title field");
                n.to_string()
            } else {
                warn!("tool_call missing name in all locations, using 'unknown'");
                "unknown".to_string()
            };

            // rawInput can be either a JSON object or a JSON string
            let raw_input = update.get("rawInput").and_then(|r| {
                if r.is_object() {
                    Some(r.clone())
                } else if let Some(s) = r.as_str() {
                    serde_json::from_str::<serde_json::Value>(s).ok()
                } else {
                    None
                }
            });

            let _ = tx.send(AgentEvent::ToolCallStart {
                id,
                name,
                raw_input,
            });
        }
        "tool_call_update" => {
            let id = extract_tool_id(update);

            if let Some(status) = update.get("status").and_then(|s| s.as_str()) {
                if status == "completed" || status == "error" {
                    let result = update
                        .get("result")
                        .and_then(|r| r.as_str())
                        .unwrap_or("")
                        .to_string();
                    let _ = tx.send(AgentEvent::ToolResult {
                        id,
                        output: result,
                        is_error: status == "error",
                    });
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_claude_code() {
        let config = AgentConfig::claude_code();
        // Uses npx to run the @zed-industries/claude-code-acp package
        assert_eq!(config.command, "npx");
        assert!(
            config
                .args
                .contains(&"@zed-industries/claude-code-acp".to_string())
        );
    }

    #[test]
    fn test_acp_client_creation() {
        let client = AcpClient::claude_code();
        assert_eq!(
            client.config().kind,
            super::super::config::AgentKind::ClaudeCode
        );
    }

    #[test]
    fn test_acp_client_gemini() {
        let client = AcpClient::gemini_cli();
        assert_eq!(
            client.config().kind,
            super::super::config::AgentKind::GeminiCli
        );
    }
}
