//! Agent Panel - Claude Code integration for AI-assisted metrics exploration.
//!
//! Provides a chat interface to interact with Claude Code CLI, with streaming
//! responses displayed in real-time.

use egui::{Color32, Key, RichText, ScrollArea, TextEdit, Vec2};
use parking_lot::Mutex;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::io::{BufRead, BufReader};
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::typography;

/// A message in the chat history
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Who sent the message
    pub role: MessageRole,
    /// The message content (may be partial during streaming)
    pub content: String,
    /// Whether this message is still being streamed
    pub is_streaming: bool,
}

/// Role of the message sender
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Result of showing the agent panel
#[derive(Debug, Clone, PartialEq)]
pub enum AgentPanelResult {
    None,
    Closed,
}

/// Streaming state for Claude CLI responses
struct StreamingState {
    /// Accumulated response text
    response_text: String,
    /// Whether streaming is complete
    is_complete: bool,
    /// Any error that occurred
    error: Option<String>,
}

/// The agent panel component for Claude Code chat
pub struct AgentPanel {
    /// Whether the panel is open
    is_open: bool,
    /// Current theme
    theme: AppTheme,
    /// Chat message history
    messages: Vec<ChatMessage>,
    /// Current input text
    input_text: String,
    /// Whether we're currently waiting for a response
    is_waiting: bool,
    /// Shared state for streaming responses (read by UI, written by background thread)
    streaming_state: Arc<Mutex<Option<StreamingState>>>,
    /// Whether the input should be focused
    focus_input: bool,
    /// Scroll to bottom flag
    scroll_to_bottom: bool,
    /// Session ID for continuing conversations
    session_id: Option<String>,
}

impl Default for AgentPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPanel {
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
            messages: Vec::new(),
            input_text: String::new(),
            is_waiting: false,
            streaming_state: Arc::new(Mutex::new(None)),
            focus_input: false,
            scroll_to_bottom: false,
            session_id: None,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the panel
    pub fn open(&mut self) {
        self.is_open = true;
        self.focus_input = true;
    }

    /// Close the panel
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Toggle the panel open/closed
    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Check if the panel is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the panel as a side panel. Returns the result.
    pub fn show(&mut self, ctx: &egui::Context) -> AgentPanelResult {
        if !self.is_open {
            return AgentPanelResult::None;
        }

        // Poll streaming state
        self.poll_streaming_response();

        let mut result = AgentPanelResult::None;

        // Handle keyboard input
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));
        if escape && !self.is_waiting {
            result = AgentPanelResult::Closed;
        }

        // Side panel on the right
        egui::SidePanel::right("agent_panel")
            .resizable(true)
            .default_width(400.0)
            .min_width(300.0)
            .max_width(800.0)
            .frame(self.panel_frame())
            .show(ctx, |ui| {
                self.render_content(ui, ctx);
            });

        if result == AgentPanelResult::Closed {
            self.close();
        }

        result
    }

    fn panel_frame(&self) -> egui::Frame {
        let (bg, border) = match self.theme {
            AppTheme::Light => (palette::light_bg::SURFACE, palette::light_border::DEFAULT),
            AppTheme::Dark => (palette::bg::SURFACE, palette::border::DEFAULT),
        };

        egui::Frame::NONE
            .fill(bg)
            .stroke(egui::Stroke::new(1.0, border))
            .inner_margin(egui::Margin::same(0))
    }

    fn render_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let accent = palette::accent::PRIMARY;
        let text_primary = text_color(self.theme);
        let text_muted = text_primary.gamma_multiply(0.6);
        let separator = match self.theme {
            AppTheme::Light => palette::light_border::SUBTLE,
            AppTheme::Dark => palette::border::SUBTLE,
        };

        // Header
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(semantic_icons::status::INFO)
                    .color(accent)
                    .size(18.0),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("Claude Agent")
                    .color(accent)
                    .size(16.0)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("×").size(18.0).color(text_muted))
                            .frame(false),
                    )
                    .on_hover_text("Close (Esc)")
                    .clicked()
                {
                    self.is_open = false;
                }
            });
        });
        ui.add_space(8.0);

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator),
        );

        // Chat area (scrollable)
        let available_height = ui.available_height() - 80.0; // Reserve space for input
        ScrollArea::vertical()
            .id_salt("agent_chat_scroll")
            .max_height(available_height)
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(12.0);

                if self.messages.is_empty() {
                    // Empty state
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("Ask Claude anything about your metrics")
                                .color(text_muted)
                                .size(14.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Try: \"Help me understand this dashboard\"")
                                .color(text_muted.gamma_multiply(0.7))
                                .size(12.0)
                                .italics(),
                        );
                    });
                } else {
                    // Render messages
                    for message in &self.messages {
                        self.render_message(ui, message);
                        ui.add_space(12.0);
                    }
                }

                // Scroll to bottom if needed
                if self.scroll_to_bottom {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    self.scroll_to_bottom = false;
                }
            });

        // Input separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator),
        );

        // Input area
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);

            // Input field
            let response = ui.add_sized(
                Vec2::new(ui.available_width() - 60.0, 32.0),
                TextEdit::singleline(&mut self.input_text)
                    .hint_text("Ask Claude...")
                    .frame(true)
                    .margin(egui::Margin::symmetric(8, 6))
                    .font(typography::monospace(typography::MD)),
            );

            // Focus input on open
            if self.focus_input {
                response.request_focus();
                self.focus_input = false;
            }

            // Send button
            ui.add_space(4.0);
            let can_send = !self.input_text.trim().is_empty() && !self.is_waiting;
            let send_color = if can_send { accent } else { text_muted };

            if ui
                .add_enabled(
                    can_send,
                    egui::Button::new(RichText::new("→").size(18.0).color(send_color)),
                )
                .on_hover_text("Send (Enter)")
                .clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && can_send)
            {
                self.send_message(ctx);
            }

            ui.add_space(8.0);
        });
        ui.add_space(8.0);

        // Status indicator
        if self.is_waiting {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.spinner();
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Claude is thinking...")
                        .color(text_muted)
                        .size(11.0),
                );
            });
            ui.add_space(4.0);
        }
    }

    fn render_message(&self, ui: &mut egui::Ui, message: &ChatMessage) {
        let text_primary = text_color(self.theme);
        let text_muted = text_primary.gamma_multiply(0.6);

        let (role_label, role_color, msg_bg) = match message.role {
            MessageRole::User => (
                "You",
                palette::accent::PRIMARY,
                match self.theme {
                    AppTheme::Light => Color32::from_rgba_unmultiplied(59, 130, 246, 20), // blue tint
                    AppTheme::Dark => Color32::from_rgba_unmultiplied(59, 130, 246, 30),
                },
            ),
            MessageRole::Assistant => (
                "Claude",
                palette::semantic::SUCCESS,
                match self.theme {
                    AppTheme::Light => Color32::from_rgba_unmultiplied(34, 197, 94, 15), // green tint
                    AppTheme::Dark => Color32::from_rgba_unmultiplied(34, 197, 94, 20),
                },
            ),
            MessageRole::System => ("System", text_muted, Color32::TRANSPARENT),
        };

        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.vertical(|ui| {
                // Role label
                ui.label(
                    RichText::new(role_label)
                        .color(role_color)
                        .size(11.0)
                        .strong(),
                );
                ui.add_space(2.0);

                // Message content with background
                egui::Frame::NONE
                    .fill(msg_bg)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width() - 24.0);

                        // Render content (could add markdown support later)
                        let content = if message.is_streaming && message.content.is_empty() {
                            "..."
                        } else {
                            &message.content
                        };

                        ui.label(RichText::new(content).color(text_primary).size(13.0));

                        // Streaming indicator
                        if message.is_streaming {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.spinner();
                            });
                        }
                    });
            });
        });
    }

    /// Send the current input as a message
    #[cfg(not(target_arch = "wasm32"))]
    fn send_message(&mut self, ctx: &egui::Context) {
        let prompt = self.input_text.trim().to_string();
        if prompt.is_empty() {
            return;
        }

        // Add user message
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.clone(),
            is_streaming: false,
        });

        // Add placeholder for assistant response
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: String::new(),
            is_streaming: true,
        });

        // Clear input
        self.input_text.clear();
        self.is_waiting = true;
        self.scroll_to_bottom = true;

        // Reset streaming state
        *self.streaming_state.lock() = Some(StreamingState {
            response_text: String::new(),
            is_complete: false,
            error: None,
        });

        // Spawn Claude CLI process
        let streaming_state = Arc::clone(&self.streaming_state);
        let ctx_clone = ctx.clone();
        let session_id = self.session_id.clone();

        std::thread::spawn(move || {
            // Build command
            let mut cmd = Command::new("claude");
            cmd.arg("-p")
                .arg("--output-format")
                .arg("stream-json")
                .arg("--verbose")
                .arg(&prompt)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            // Continue session if we have one
            if let Some(ref sid) = session_id {
                cmd.arg("--resume").arg(sid);
            }

            match cmd.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    if let Some(stdout) = stdout {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines() {
                            match line {
                                Ok(line) => {
                                    if let Ok(json) =
                                        serde_json::from_str::<serde_json::Value>(&line)
                                    {
                                        Self::process_stream_event(
                                            &streaming_state,
                                            &json,
                                            &ctx_clone,
                                        );
                                    }
                                }
                                Err(e) => {
                                    let mut state = streaming_state.lock();
                                    if let Some(ref mut s) = *state {
                                        s.error = Some(format!("Read error: {e}"));
                                        s.is_complete = true;
                                    }
                                    break;
                                }
                            }
                        }
                    }

                    // Wait for process to complete
                    let _ = child.wait();

                    // Mark as complete
                    let mut state = streaming_state.lock();
                    if let Some(ref mut s) = *state {
                        s.is_complete = true;
                    }
                    ctx_clone.request_repaint();
                }
                Err(e) => {
                    let mut state = streaming_state.lock();
                    if let Some(ref mut s) = *state {
                        s.error = Some(format!("Failed to start claude: {e}"));
                        s.is_complete = true;
                    }
                    ctx_clone.request_repaint();
                }
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn send_message(&mut self, _ctx: &egui::Context) {
        // Add user message
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: self.input_text.trim().to_string(),
            is_streaming: false,
        });

        // WASM: Claude CLI not available
        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "Claude Code CLI is not available in the browser.".to_string(),
            is_streaming: false,
        });

        self.input_text.clear();
        self.scroll_to_bottom = true;
    }

    /// Process a streaming JSON event from Claude CLI
    #[cfg(not(target_arch = "wasm32"))]
    fn process_stream_event(
        streaming_state: &Arc<Mutex<Option<StreamingState>>>,
        json: &serde_json::Value,
        ctx: &egui::Context,
    ) {
        let event_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match event_type {
            "assistant" => {
                // Extract text from message content
                if let Some(message) = json.get("message") {
                    if let Some(content) = message.get("content") {
                        if let Some(blocks) = content.as_array() {
                            for block in blocks {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    let mut state = streaming_state.lock();
                                    if let Some(ref mut s) = *state {
                                        s.response_text = text.to_string();
                                    }
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                }
            }
            "result" => {
                // Final result - extract session_id for continuation
                if let Some(result_text) = json.get("result").and_then(|r| r.as_str()) {
                    let mut state = streaming_state.lock();
                    if let Some(ref mut s) = *state {
                        if s.response_text.is_empty() {
                            s.response_text = result_text.to_string();
                        }
                        s.is_complete = true;
                    }
                }

                // Check for errors
                if json
                    .get("is_error")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false)
                {
                    let mut state = streaming_state.lock();
                    if let Some(ref mut s) = *state {
                        s.error = Some("Claude returned an error".to_string());
                    }
                }

                ctx.request_repaint();
            }
            "system" => {
                // Init event - extract session_id
                // We'll capture this for session continuation later
            }
            _ => {}
        }
    }

    /// Poll streaming state and update messages
    fn poll_streaming_response(&mut self) {
        let state = self.streaming_state.lock();
        if let Some(ref s) = *state {
            // Update the last assistant message
            if let Some(last) = self.messages.last_mut() {
                if last.role == MessageRole::Assistant && last.is_streaming {
                    last.content = s.response_text.clone();

                    if s.is_complete {
                        last.is_streaming = false;
                        self.is_waiting = false;

                        // Handle error
                        if let Some(ref err) = s.error {
                            if last.content.is_empty() {
                                last.content = format!("Error: {err}");
                            }
                        }
                    }
                }
            }

            // Clear state when complete
            if s.is_complete {
                drop(state);
                *self.streaming_state.lock() = None;
            }
        }
    }
}
