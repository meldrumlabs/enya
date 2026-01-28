//! Streaming and send logic for the agent panel.
//!
//! Extracted to consolidate platform-specific (`#[cfg]`) code in one place.
//! All methods here are `impl AgentPanel` — Rust allows splitting impl blocks across modules.

use super::agent_panel::{AgentPanel, ChatMessage};
use crate::components::util::{MessageRole, ResponseStatus};

#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{
    ActivityItem, ActivityType, truncate_first_line, truncate_path_suffix,
};
#[cfg(not(target_arch = "wasm32"))]
use enya_ai::{AcpClient, AgentEvent};

impl AgentPanel {
    /// Send the current input as a message (native: spawns streaming via ACP client).
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn send_message(&mut self, _ctx: &egui::Context) {
        let prompt = self.input_text.trim().to_string();
        if prompt.is_empty() {
            return;
        }

        // Ensure we have an active conversation thread
        self.ensure_active_thread();

        // Add user message
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.clone(),
            is_streaming: false,
            inline_blocks: Vec::new(),
        });

        // Add placeholder for assistant response
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            is_streaming: true,
            inline_blocks: Vec::new(),
        });

        // Clear input and reset state
        self.input_text.clear();
        self.is_waiting = true;
        self.scroll_to_bottom = true;
        self.request_start_time = Some(std::time::Instant::now());
        self.response_text.clear();
        self.stream_settled_len = 0;
        self.stream_fade_start = None;
        self.current_status = ResponseStatus::Waiting;
        self.current_activities.clear();
        self.current_model = Some(self.selected_model.display_name().to_string());

        // Get working directory
        let working_dir = std::env::current_dir().ok();

        // Create client based on selected provider, with runtime handle for async spawning
        let client = match (&self.selected_provider, &self.runtime_handle) {
            (super::super::util::AiProvider::Claude, Some(handle)) => {
                AcpClient::claude_code_with_runtime(handle.clone())
            }
            (super::super::util::AiProvider::Claude, None) => AcpClient::claude_code(),
            (super::super::util::AiProvider::Codex, Some(handle)) => {
                AcpClient::codex_with_runtime(handle.clone())
            }
            (super::super::util::AiProvider::Codex, None) => AcpClient::codex(),
        };

        // Build system context if available
        let system_context = self
            .editor_context
            .as_ref()
            .map(|ctx| ctx.to_prompt_block());

        let receiver = client.prompt_with_context(
            prompt,
            working_dir,
            Some(self.selected_model.model_id()),
            system_context.as_deref(),
        );

        self.event_receiver = Some(receiver);
    }

    /// Send message (WASM stub: CLI not available in browser).
    #[cfg(target_arch = "wasm32")]
    pub(super) fn send_message(&mut self, _ctx: &egui::Context) {
        self.ensure_active_thread();

        // Add user message
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: self.input_text.trim().to_string(),
            is_streaming: false,
            inline_blocks: Vec::new(),
        });

        // WASM: Claude CLI not available
        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "Claude Code CLI is not available in the browser.".to_string(),
            is_streaming: false,
            inline_blocks: Vec::new(),
        });

        self.input_text.clear();
        self.scroll_to_bottom = true;
    }

    /// Cancel the current request.
    pub(super) fn cancel_request(&mut self) {
        // Drop the event receiver to stop processing
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.event_receiver = None;
        }

        // Update the last assistant message to indicate it was stopped
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.role == MessageRole::Assistant && last_msg.is_streaming {
                last_msg.is_streaming = false;
                if last_msg.content.is_empty() {
                    last_msg.content = "(Generation stopped)".to_string();
                } else {
                    last_msg.content.push_str("\n\n_(Generation stopped)_");
                }
            }
        }

        // Reset state
        self.is_waiting = false;
        self.response_text.clear();
        self.current_status = ResponseStatus::Complete;
        self.current_activities.clear();
        self.request_start_time = None;
    }

    /// Poll the event receiver and update UI state (native: processes ACP events).
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_streaming_response(&mut self) {
        let Some(ref receiver) = self.event_receiver else {
            return;
        };

        // Process all available events
        while let Ok(event) = receiver.try_recv() {
            match event {
                AgentEvent::TextDelta(text) => {
                    // Track settled position for fade-in animation
                    self.stream_settled_len = self.response_text.len();
                    self.stream_fade_start = Some(crate::util::Instant::now());
                    self.response_text.push_str(&text);
                    self.current_status = ResponseStatus::Responding;
                }
                AgentEvent::ThinkingDelta(text) => {
                    self.current_status = ResponseStatus::Thinking;
                    // Update or create thinking activity
                    if let Some(last) = self.current_activities.last_mut() {
                        if let ActivityType::Thinking(ref mut thinking_text) = last.activity_type {
                            if thinking_text.len() < 60 {
                                thinking_text.push_str(&text);
                                if thinking_text.len() > 60 {
                                    thinking_text.truncate(57);
                                    thinking_text.push_str("...");
                                }
                            }
                        } else {
                            self.current_activities.push(ActivityItem {
                                activity_type: ActivityType::Thinking(truncate_first_line(
                                    &text, 60,
                                )),
                                in_progress: true,
                            });
                        }
                    } else {
                        self.current_activities.push(ActivityItem {
                            activity_type: ActivityType::Thinking(truncate_first_line(&text, 60)),
                            in_progress: true,
                        });
                    }
                }
                AgentEvent::ToolCallStart {
                    name, raw_input, ..
                } => {
                    // Mark previous activities as complete
                    for activity in &mut self.current_activities {
                        activity.in_progress = false;
                    }

                    // Extract summary from raw_input
                    let summary = raw_input
                        .as_ref()
                        .and_then(|v| {
                            // Check for file path fields first (use path truncation)
                            if let Some(path) = v
                                .get("file_path")
                                .or_else(|| v.get("path"))
                                .and_then(|s| s.as_str())
                            {
                                return Some(truncate_path_suffix(path, 50));
                            }
                            // Other fields use regular text truncation
                            v.get("pattern")
                                .or_else(|| v.get("command"))
                                .or_else(|| v.get("description"))
                                .or_else(|| v.get("prompt"))
                                .and_then(|s| s.as_str())
                                .map(|s| truncate_first_line(s, 50))
                        })
                        .unwrap_or_default();

                    self.current_activities.push(ActivityItem {
                        activity_type: ActivityType::ToolUse {
                            tool: name,
                            summary,
                        },
                        in_progress: true,
                    });
                }
                AgentEvent::ToolResult { is_error, .. } => {
                    // Mark last tool activity as complete
                    if let Some(last) = self.current_activities.last_mut() {
                        last.in_progress = false;
                    }
                    if is_error {
                        // Optionally add error indicator
                    }
                }
                AgentEvent::Done { .. } => {
                    // Mark all activities as complete
                    for activity in &mut self.current_activities {
                        activity.in_progress = false;
                    }
                    self.current_status = ResponseStatus::Complete;

                    // Update last message
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == MessageRole::Assistant && last.is_streaming {
                            last.content = self.response_text.clone();
                            last.is_streaming = false;
                        }
                    }

                    // Parse commands from the response
                    let commands = super::agent_context::parse_commands(&self.response_text);
                    if !commands.is_empty() {
                        log::info!("Parsed {} commands from agent response", commands.len());
                        self.pending_commands.extend(commands);
                    }

                    self.is_waiting = false;
                    self.request_start_time = None;
                    self.event_receiver = None;

                    // Save conversation thread to disk
                    self.sync_messages_to_thread();
                    return;
                }
                AgentEvent::Error(e) => {
                    // Mark all activities as complete
                    for activity in &mut self.current_activities {
                        activity.in_progress = false;
                    }
                    self.current_activities.push(ActivityItem {
                        activity_type: ActivityType::Error(e.to_string()),
                        in_progress: false,
                    });
                    self.current_status = ResponseStatus::Complete;

                    // Update last message with error
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == MessageRole::Assistant && last.is_streaming {
                            if self.response_text.is_empty() {
                                last.content = format!("Error: {e}");
                            } else {
                                last.content = self.response_text.clone();
                            }
                            last.is_streaming = false;
                        }
                    }

                    self.is_waiting = false;
                    self.request_start_time = None;
                    self.event_receiver = None;
                    return;
                }
                _ => {}
            }
        }

        // Update last message with current response text
        if let Some(last) = self.messages.last_mut() {
            if last.role == MessageRole::Assistant && last.is_streaming {
                last.content = self.response_text.clone();
            }
        }
    }

    /// Poll streaming state (WASM stub - no-op).
    #[cfg(target_arch = "wasm32")]
    pub(super) fn poll_streaming_response(&mut self) {
        // No-op on WASM
    }
}
