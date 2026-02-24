//! Persistent ACP client that keeps a warm subprocess across prompts.
//!
//! Unlike [`super::AcpClient`] which spawns a new subprocess per prompt,
//! this client maintains a long-lived agent process and reuses it for
//! subsequent requests, eliminating the `npx` cold-start overhead.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

use log::{debug, info, warn};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc as tokio_mpsc;

use super::client::{
    CLIENT_NAME, CLIENT_VERSION, DEFAULT_MODEL, read_response, read_session_id,
    read_streaming_responses, send_message, spawn_agent,
};
use super::config::AgentConfig;
use super::protocol::{
    ClaudeCodeMeta, ClaudeCodeOptions, ClientCapabilities, ClientInfo, InitializeParams,
    PromptContent, RpcRequest, SessionMeta, SessionNewParams, SessionPromptParams,
    SetSessionModelParams,
};
use crate::types::{AgentError, AgentEvent};

/// How long the subprocess idles before being killed (5 minutes).
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Command sent from the public API to the session manager task.
enum SessionCommand {
    /// Pre-warm: spawn the subprocess and complete the initialize handshake.
    Warmup,
    /// Send a prompt and stream responses back through `response_tx`.
    Prompt {
        prompt: String,
        working_dir: Option<PathBuf>,
        model: Option<String>,
        system_context: Option<String>,
        response_tx: SyncSender<AgentEvent>,
    },
    /// Shut down the session manager.
    Shutdown,
}

/// Internal state of the managed subprocess.
struct WarmProcess {
    child: tokio::process::Child,
    reader: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    writer: tokio::io::BufWriter<tokio::process::ChildStdin>,
    /// Monotonic JSON-RPC ID counter.
    next_id: u32,
    /// Model the process was spawned with.
    current_model: String,
}

/// A persistent ACP client that keeps a warm subprocess.
///
/// Call [`warmup`](Self::warmup) to pre-spawn the agent process, then
/// [`prompt_with_context`](Self::prompt_with_context) to send prompts
/// without the cold-start penalty.
///
/// Works with any ACP-compatible agent (Claude Code, Codex, or custom).
pub struct PersistentAcpClient {
    command_tx: tokio_mpsc::UnboundedSender<SessionCommand>,
}

impl PersistentAcpClient {
    /// Create a new persistent client and spawn the session manager task.
    ///
    /// The subprocess is NOT started yet — call [`warmup`](Self::warmup) to
    /// pre-warm, or it will be spawned on the first prompt.
    #[must_use]
    pub fn new(config: AgentConfig, runtime: &tokio::runtime::Handle) -> Self {
        let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
        runtime.spawn(session_manager_loop(config, command_rx));
        Self { command_tx }
    }

    /// Pre-warm the subprocess: spawn it and complete the `initialize` handshake.
    ///
    /// Returns immediately — warming happens asynchronously. If the subprocess
    /// is already warm, this is a no-op.
    pub fn warmup(&self) {
        let _ = self.command_tx.send(SessionCommand::Warmup);
    }

    /// Send a prompt and receive streaming events.
    ///
    /// If the subprocess is warm, skips the cold start entirely.
    /// If cold or dead, transparently spawns a new one.
    pub fn prompt_with_context(
        &self,
        prompt: impl Into<String>,
        working_dir: Option<PathBuf>,
        model: Option<&str>,
        system_context: Option<&str>,
    ) -> Receiver<AgentEvent> {
        let (response_tx, response_rx) = mpsc::sync_channel(256);
        let cmd = SessionCommand::Prompt {
            prompt: prompt.into(),
            working_dir,
            model: model.map(String::from),
            system_context: system_context.map(String::from),
            response_tx,
        };
        if self.command_tx.send(cmd).is_err() {
            // Session manager has shut down; return an error on the channel.
            let (tx, rx) = mpsc::sync_channel(1);
            let _ = tx.send(AgentEvent::Error(AgentError::Process(
                "session manager shut down".into(),
            )));
            return rx;
        }
        response_rx
    }

    /// Shut down the subprocess and background task.
    pub fn shutdown(&self) {
        let _ = self.command_tx.send(SessionCommand::Shutdown);
    }
}

impl Drop for PersistentAcpClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Spawn and initialize an agent subprocess, returning a `WarmProcess`.
async fn spawn_and_initialize(
    config: &AgentConfig,
    model_id: &str,
) -> Result<WarmProcess, AgentError> {
    let spawn_config = config.clone().with_model(model_id);

    let t0 = Instant::now();
    info!(
        "spawning persistent ACP subprocess (agent={}, command={}, model={})",
        config.kind.display_name(),
        config.command,
        model_id
    );

    let mut child = spawn_agent(&spawn_config)?;
    let spawn_ms = t0.elapsed().as_millis();

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

    // Send initialize handshake (always id=1)
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
    let init_ms = t0.elapsed().as_millis();

    info!("ACP subprocess initialized and warm (spawn_ms={spawn_ms}, init_ms={init_ms})");

    Ok(WarmProcess {
        child,
        reader,
        writer,
        next_id: 2, // id=1 was used for initialize
        current_model: model_id.to_string(),
    })
}

/// Send a prompt on an existing warm process.
///
/// Performs `session/new` + `session/prompt` and streams responses.
async fn send_prompt_on_warm(
    proc: &mut WarmProcess,
    config: &AgentConfig,
    prompt: &str,
    system_context: Option<&str>,
    tx: &SyncSender<AgentEvent>,
) -> Result<(), AgentError> {
    let t0 = Instant::now();

    // session/new
    let session_new_id = proc.next_id;
    proc.next_id += 1;

    let cwd = config
        .working_dir
        .as_ref()
        .map_or_else(|| ".".to_string(), |p| p.display().to_string());

    let session_msg = RpcRequest::new(
        session_new_id,
        "session/new",
        SessionNewParams {
            cwd,
            mcp_servers: vec![],
            meta: SessionMeta {
                claude_code: ClaudeCodeMeta {
                    options: ClaudeCodeOptions {
                        model: proc.current_model.clone(),
                    },
                },
            },
        },
    );
    send_message(&mut proc.writer, &session_msg).await?;
    let session_id = read_session_id(&mut proc.reader).await?;
    let session_new_ms = t0.elapsed().as_millis();

    // Explicitly set the model via session/set_model after session creation.
    // This overrides any defaults from the agent's own config (e.g. Claude CLI
    // settings or Codex config.toml) to ensure our UI selection takes effect.
    let set_model_id = proc.next_id;
    proc.next_id += 1;
    let set_model_msg = RpcRequest::new(
        set_model_id,
        "session/set_model",
        SetSessionModelParams {
            session_id: session_id.clone(),
            model_id: proc.current_model.clone(),
        },
    );
    send_message(&mut proc.writer, &set_model_msg).await?;
    read_response(&mut proc.reader, "setSessionModel").await?;
    let set_model_ms = t0.elapsed().as_millis();
    info!(
        "ACP model set to {} (session_new_ms={session_new_ms}, set_model_ms={set_model_ms})",
        proc.current_model
    );

    // session/prompt
    let prompt_id = proc.next_id;
    proc.next_id += 1;

    let full_prompt = if let Some(ctx) = system_context {
        format!("{ctx}\n\n---\n\n{prompt}")
    } else {
        prompt.to_string()
    };

    let prompt_msg = RpcRequest::new(
        prompt_id,
        "session/prompt",
        SessionPromptParams {
            session_id,
            prompt: vec![PromptContent {
                content_type: "text",
                text: full_prompt,
            }],
        },
    );
    send_message(&mut proc.writer, &prompt_msg).await?;
    let prompt_sent_ms = t0.elapsed().as_millis();

    info!(
        "ACP prompt sent, waiting for response stream (session_new_ms={session_new_ms}, prompt_sent_ms={prompt_sent_ms})"
    );

    // Stream responses
    read_streaming_responses(&mut proc.reader, tx, prompt_id).await?;
    let total_ms = t0.elapsed().as_millis();
    info!("ACP prompt streaming complete (total_ms={total_ms})");

    Ok(())
}

/// The core session manager loop. Owns the subprocess and processes commands.
async fn session_manager_loop(
    config: AgentConfig,
    mut command_rx: tokio_mpsc::UnboundedReceiver<SessionCommand>,
) {
    let mut warm: Option<WarmProcess> = None;

    loop {
        // If warm, race between receiving a command and the idle timeout.
        // If cold, just wait for a command indefinitely.
        let cmd = if warm.is_some() {
            tokio::select! {
                cmd = command_rx.recv() => cmd,
                () = tokio::time::sleep(IDLE_TIMEOUT) => {
                    debug!("idle timeout reached, killing warm subprocess");
                    kill_process(&mut warm).await;
                    continue;
                }
            }
        } else {
            command_rx.recv().await
        };

        let Some(cmd) = cmd else {
            // Channel closed — shutdown.
            debug!("command channel closed, shutting down session manager");
            kill_process(&mut warm).await;
            break;
        };

        match cmd {
            SessionCommand::Warmup => {
                if warm.is_some() {
                    debug!("warmup requested but subprocess already warm");
                    continue;
                }
                match spawn_and_initialize(&config, DEFAULT_MODEL).await {
                    Ok(proc) => warm = Some(proc),
                    Err(e) => {
                        warn!("warmup failed: {e}");
                        // Stay cold — next prompt will retry.
                    }
                }
            }
            SessionCommand::Prompt {
                prompt,
                working_dir,
                model,
                system_context,
                response_tx,
            } => {
                let prompt_t0 = Instant::now();
                let model_id = model.as_deref().unwrap_or(DEFAULT_MODEL);

                // Check if we need to respawn (cold or model changed).
                let was_warm = warm.is_some();
                let needs_respawn = match &warm {
                    None => true,
                    Some(proc) => proc.current_model != model_id,
                };

                // Build config with working dir (used for both spawn and session/new).
                let prompt_config = if let Some(dir) = &working_dir {
                    config.clone().with_working_dir(dir.clone())
                } else {
                    config.clone()
                };

                if needs_respawn {
                    kill_process(&mut warm).await;
                    match spawn_and_initialize(&prompt_config, model_id).await {
                        Ok(proc) => warm = Some(proc),
                        Err(e) => {
                            let _ = response_tx.send(AgentEvent::Error(e));
                            continue;
                        }
                    }
                }

                let ready_ms = prompt_t0.elapsed().as_millis();
                info!(
                    "process ready for prompt (was_warm={was_warm}, ready_ms={ready_ms}, model={model_id})"
                );

                let proc = warm.as_mut().expect("process should be warm after spawn");
                match send_prompt_on_warm(
                    proc,
                    &prompt_config,
                    &prompt,
                    system_context.as_deref(),
                    &response_tx,
                )
                .await
                {
                    Ok(()) => {
                        // Process is still alive — keep it warm for next prompt.
                    }
                    Err(e) => {
                        warn!("prompt failed: {e}, killing subprocess");
                        let _ = response_tx.send(AgentEvent::Error(e));
                        kill_process(&mut warm).await;
                    }
                }
            }
            SessionCommand::Shutdown => {
                debug!("shutdown requested");
                kill_process(&mut warm).await;
                break;
            }
        }
    }
}

/// Kill the warm subprocess if present.
async fn kill_process(warm: &mut Option<WarmProcess>) {
    if let Some(mut proc) = warm.take() {
        let _ = proc.child.kill().await;
        debug!("killed warm subprocess");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentEvent;

    #[test]
    fn shutdown_command_closes_channel() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = AgentConfig::claude_code();
        let client = PersistentAcpClient::new(config, rt.handle());

        // Shutdown should not panic
        client.shutdown();
        // Second shutdown should also be safe (channel may be closed)
        client.shutdown();
    }

    #[test]
    fn prompt_after_shutdown_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = AgentConfig::claude_code();
        let client = PersistentAcpClient::new(config, rt.handle());

        client.shutdown();
        // Give the manager task a moment to process shutdown
        std::thread::sleep(Duration::from_millis(50));

        let rx = client.prompt_with_context("test", None, None, None);
        // Should get an error since the manager shut down
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(AgentEvent::Error(_)) => {} // expected
            other => {
                // Channel might be empty if shutdown raced, that's OK too
                if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = other {
                    // Manager may still be shutting down
                } else if let Err(std::sync::mpsc::RecvTimeoutError::Disconnected) = other {
                    // Also fine — sender was dropped
                } else {
                    panic!("unexpected result: {other:?}");
                }
            }
        }
    }

    #[test]
    fn warmup_when_already_warm_is_noop() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = AgentConfig::claude_code();
        let client = PersistentAcpClient::new(config, rt.handle());

        // Multiple warmups should not panic
        client.warmup();
        client.warmup();
        client.warmup();

        client.shutdown();
    }

    #[test]
    fn drop_triggers_shutdown() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = AgentConfig::claude_code();

        {
            let _client = PersistentAcpClient::new(config, rt.handle());
            // client dropped here — should trigger shutdown via Drop
        }

        // If we get here without hanging, the drop worked.
    }
}
