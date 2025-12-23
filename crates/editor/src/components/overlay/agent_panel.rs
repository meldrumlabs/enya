//! Agent Panel - Claude Code integration for AI-assisted metrics exploration.
//!
//! Provides a chat interface to interact with Claude Code CLI, with streaming
//! responses displayed in real-time. Styled with the Obsidian Glass design system.

use egui::{Color32, Key, RichText, ScrollArea, TextEdit, Vec2};
use parking_lot::Mutex;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::io::{BufRead, BufReader};
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};

use crate::components::util::finder_utils::OverlayColors;
use crate::theme::AppTheme;
use crate::ui::palette;
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

/// Type of activity in the agent log
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityType {
    /// Claude is thinking (with optional thinking text)
    Thinking(String),
    /// Tool usage (tool name, summary)
    ToolUse { tool: String, summary: String },
    /// Error message
    Error(String),
    /// Final text response
    Response(String),
}

/// An activity item in the agent log
#[derive(Debug, Clone)]
pub struct ActivityItem {
    /// The type of activity
    pub activity_type: ActivityType,
    /// Whether this activity is still in progress
    pub in_progress: bool,
}

/// Result of showing the agent panel
#[derive(Debug, Clone, PartialEq)]
pub enum AgentPanelResult {
    None,
    Closed,
}

/// Status of the Claude response
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum ResponseStatus {
    #[default]
    Waiting,
    Thinking,
    Responding,
    Complete,
}

/// Streaming state for Claude CLI responses
struct StreamingState {
    /// Accumulated response text
    response_text: String,
    /// Whether streaming is complete
    is_complete: bool,
    /// Any error that occurred
    error: Option<String>,
    /// Current response status
    status: ResponseStatus,
    /// Model being used (extracted from stream events)
    model: Option<String>,
    /// Activity log items
    activities: Vec<ActivityItem>,
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
    /// Current model being used
    current_model: Option<String>,
    /// Current response status for UI display
    current_status: ResponseStatus,
    /// Current activities being displayed
    current_activities: Vec<ActivityItem>,
    /// Timestamp when request started (for elapsed time display)
    request_start_time: Option<std::time::Instant>,
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
            current_model: None,
            current_status: ResponseStatus::Complete,
            current_activities: Vec::new(),
            request_start_time: None,
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

        // Request repaint while timer is running (to update elapsed time)
        if self.request_start_time.is_some() {
            ctx.request_repaint();
        }

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
        // Use frosted glass style matching other overlays
        let (bg, border) = match self.theme {
            AppTheme::Light => (
                Color32::from_rgba_unmultiplied(255, 255, 255, 250),
                palette::light_border::DEFAULT,
            ),
            AppTheme::Dark => (
                Color32::from_rgba_unmultiplied(15, 15, 15, 250), // Slightly darker than SURFACE
                palette::border::SUBTLE,
            ),
        };

        egui::Frame::NONE
            .fill(bg)
            .stroke(egui::Stroke::new(1.0, border))
            .inner_margin(egui::Margin::same(0))
    }

    fn render_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let colors = OverlayColors::new(self.theme);
        let accent = colors.accent;
        let text_primary = colors.text;
        let text_muted = colors.muted_text;
        let text_faint = colors.faint_text;

        // Header - compact and clean
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);

            // Icon with accent color
            ui.label(
                RichText::new(egui_nerdfonts::regular::ROBOT)
                    .color(accent)
                    .size(16.0),
            );
            ui.add_space(6.0);

            // Title
            ui.label(
                RichText::new("Claude")
                    .color(text_primary)
                    .size(typography::LG)
                    .strong(),
            );

            // Show model name if available (subtle badge style)
            if let Some(ref model) = self.current_model {
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(colors.badge_bg)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(model)
                                .color(text_muted)
                                .size(typography::SM),
                        );
                    });
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(14.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(egui_nerdfonts::regular::CLOSE)
                                .size(14.0)
                                .color(text_faint),
                        )
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
            egui::Stroke::new(1.0, colors.separator),
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

                if self.messages.is_empty() && self.current_activities.is_empty() {
                    // Empty state - minimal and elegant
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label(
                            RichText::new(egui_nerdfonts::regular::COMMENT_TEXT)
                                .color(text_faint)
                                .size(32.0),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Ask Claude anything")
                                .color(text_muted)
                                .size(typography::LG),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Try: \"Help me understand this dashboard\"")
                                .color(text_faint)
                                .size(typography::MD)
                                .italics(),
                        );
                    });
                } else {
                    // Render messages in order, inserting activities after the LAST user message
                    let last_user_idx = self
                        .messages
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, m)| m.role == MessageRole::User)
                        .map(|(i, _)| i);

                    for (i, message) in self.messages.iter().enumerate() {
                        self.render_message(ui, message, &colors);
                        ui.add_space(6.0);

                        // Show activities right after the last user message
                        if Some(i) == last_user_idx && !self.current_activities.is_empty() {
                            ui.add_space(4.0);
                            for activity in &self.current_activities {
                                self.render_activity(ui, activity, &colors);
                                ui.add_space(3.0);
                            }
                        }
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
            egui::Stroke::new(1.0, colors.separator),
        );

        // Input area - cleaner styling
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);

            // Input field with elevated background
            let input_bg = colors.elevated_bg;
            egui::Frame::new()
                .fill(input_bg)
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .stroke(egui::Stroke::new(1.0, colors.separator))
                .show(ui, |ui| {
                    let response = ui.add_sized(
                        Vec2::new(ui.available_width() - 50.0, 20.0),
                        TextEdit::singleline(&mut self.input_text)
                            .hint_text(
                                RichText::new("Ask Claude...")
                                    .color(text_faint)
                                    .size(typography::MD),
                            )
                            .frame(false)
                            .font(typography::proportional(typography::MD)),
                    );

                    // Focus input on open
                    if self.focus_input {
                        response.request_focus();
                        self.focus_input = false;
                    }

                    // Handle Enter to send
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        let can_send = !self.input_text.trim().is_empty() && !self.is_waiting;
                        if can_send {
                            self.send_message(ctx);
                        }
                    }
                });

            // Send button - accent colored when active
            ui.add_space(6.0);
            let can_send = !self.input_text.trim().is_empty() && !self.is_waiting;
            let send_color = if can_send { accent } else { text_faint };

            if ui
                .add_enabled(
                    can_send,
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::SEND)
                            .size(16.0)
                            .color(send_color),
                    )
                    .frame(false),
                )
                .on_hover_text("Send (Enter)")
                .clicked()
            {
                self.send_message(ctx);
            }

            ui.add_space(10.0);
        });
        ui.add_space(8.0);
    }

    fn render_message(&self, ui: &mut egui::Ui, message: &ChatMessage, colors: &OverlayColors) {
        let (role_label, role_color, msg_bg) = match message.role {
            MessageRole::User => (
                "You",
                colors.accent,
                match self.theme {
                    AppTheme::Light => palette::light_bg::ELEVATED,
                    AppTheme::Dark => palette::bg::ELEVATED,
                },
            ),
            MessageRole::Assistant => (
                "Claude",
                palette::accent::PRIMARY, // Emerald for Claude
                Color32::TRANSPARENT,     // No background for assistant - cleaner
            ),
            MessageRole::System => ("System", colors.faint_text, Color32::TRANSPARENT),
        };

        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.vertical(|ui| {
                // Role label - small and subtle
                ui.label(
                    RichText::new(role_label)
                        .color(role_color)
                        .size(typography::SM)
                        .strong(),
                );
                ui.add_space(3.0);

                // Message content
                if msg_bg != Color32::TRANSPARENT {
                    // User messages get a subtle background
                    egui::Frame::NONE
                        .fill(msg_bg)
                        .corner_radius(8.0)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_max_width(ui.available_width() - 28.0);
                            self.render_message_content(ui, message, colors);
                        });
                } else {
                    // Assistant messages - just text, no frame
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(2.0);
                        ui.vertical(|ui| {
                            ui.set_max_width(ui.available_width() - 20.0);
                            self.render_message_content(ui, message, colors);
                        });
                    });
                }
            });
        });
    }

    fn render_message_content(
        &self,
        ui: &mut egui::Ui,
        message: &ChatMessage,
        colors: &OverlayColors,
    ) {
        // Show content if we have any
        if !message.content.is_empty() {
            ui.label(
                RichText::new(&message.content)
                    .color(colors.text)
                    .size(typography::MD),
            );
        }

        // Streaming indicator with timer
        if message.is_streaming {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().color(colors.accent).size(12.0));
                ui.add_space(6.0);

                // Show elapsed time
                if let Some(start) = self.request_start_time {
                    let elapsed = start.elapsed().as_secs_f32();
                    let time_str = if elapsed < 10.0 {
                        format!("{elapsed:.1}s")
                    } else {
                        format!("{:.0}s", elapsed)
                    };
                    ui.label(
                        RichText::new(time_str)
                            .color(colors.accent)
                            .size(typography::SM),
                    );
                }
            });
        }
    }

    /// Render an activity item (Claude Code style)
    fn render_activity(&self, ui: &mut egui::Ui, activity: &ActivityItem, colors: &OverlayColors) {
        use egui_nerdfonts::regular;

        // Get icon and color based on activity type
        let (icon, label, summary, icon_color) = match &activity.activity_type {
            ActivityType::Thinking(text) => (
                regular::LIGHTBULB,
                "Thinking",
                text.clone(),
                colors.muted_text,
            ),
            ActivityType::ToolUse { tool, summary } => {
                let icon = match tool.as_str() {
                    "Edit" => regular::PENCIL,
                    "Write" => regular::FILE_DOCUMENT,
                    "Read" => regular::EYE,
                    "Bash" => regular::TERMINAL,
                    "Grep" => regular::MAGNIFY,
                    "Glob" => regular::FILE_SEARCH,
                    "Task" => regular::COG,
                    "WebFetch" | "WebSearch" => regular::WEB,
                    _ => regular::CUBE,
                };
                (icon, tool.as_str(), summary.clone(), colors.accent)
            }
            ActivityType::Error(msg) => (
                regular::CLOSE_CIRCLE,
                "Error",
                msg.clone(),
                palette::semantic::ERROR,
            ),
            ActivityType::Response(_) => {
                return;
            }
        };

        ui.horizontal(|ui| {
            ui.add_space(18.0);

            // Compact activity row with subtle styling
            if activity.in_progress {
                ui.add(egui::Spinner::new().color(colors.accent).size(12.0));
            } else {
                ui.label(RichText::new(icon).color(icon_color).size(12.0));
            }

            ui.add_space(6.0);

            // Label - smaller and muted
            ui.label(
                RichText::new(label)
                    .color(colors.muted_text)
                    .size(typography::SM),
            );

            // Summary text
            if !summary.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(&summary)
                        .color(colors.faint_text)
                        .size(typography::SM),
                );
            }
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
        self.request_start_time = Some(std::time::Instant::now());

        // Reset streaming state
        *self.streaming_state.lock() = Some(StreamingState {
            response_text: String::new(),
            is_complete: false,
            error: None,
            status: ResponseStatus::Waiting,
            model: None,
            activities: Vec::new(),
        });
        self.current_status = ResponseStatus::Waiting;
        self.current_activities.clear();

        // Spawn Claude CLI process
        let streaming_state = Arc::clone(&self.streaming_state);
        let ctx_clone = ctx.clone();
        let session_id = self.session_id.clone();

        std::thread::spawn(move || {
            // Build command - use the full path to claude from the same location
            // that is used when running from terminal
            let claude_path = std::env::var("HOME")
                .map(|home| {
                    format!("{home}/Library/Application Support/com.conductor.app/./bin/claude")
                })
                .unwrap_or_else(|_| "claude".to_string());

            let mut cmd = Command::new(&claude_path);
            // Clear Claude Code SDK environment variables that would cause
            // the CLI to use API key billing instead of Claude Max subscription
            cmd.env_remove("CLAUDECODE")
                .env_remove("CLAUDE_CODE_ENTRYPOINT")
                .env_remove("CLAUDE_AGENT_SDK_VERSION")
                .env_remove("ANTHROPIC_API_KEY")
                .arg("-p")
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

            log::debug!("Spawning claude CLI: {cmd:?}");

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
            "system" => {
                // Init event - extract model info
                let mut state = streaming_state.lock();
                if let Some(ref mut s) = *state {
                    // Extract model from the system event
                    if let Some(model) = json.get("model").and_then(|m| m.as_str()) {
                        s.model = Some(Self::format_model_name(model));
                    }
                    s.status = ResponseStatus::Thinking;
                }
                ctx.request_repaint();
            }
            "assistant" => {
                // Extract content from message - can include thinking and tool_use
                if let Some(message) = json.get("message") {
                    // Try to get model from message if not already set
                    if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
                        let mut state = streaming_state.lock();
                        if let Some(ref mut s) = *state {
                            if s.model.is_none() {
                                s.model = Some(Self::format_model_name(model));
                            }
                        }
                    }

                    if let Some(content) = message.get("content") {
                        if let Some(blocks) = content.as_array() {
                            let mut state = streaming_state.lock();
                            if let Some(ref mut s) = *state {
                                // Mark any in-progress activities as complete
                                for activity in &mut s.activities {
                                    activity.in_progress = false;
                                }

                                for block in blocks {
                                    let block_type =
                                        block.get("type").and_then(|t| t.as_str()).unwrap_or("");

                                    match block_type {
                                        "thinking" => {
                                            // Extract thinking text
                                            if let Some(thinking) =
                                                block.get("thinking").and_then(|t| t.as_str())
                                            {
                                                // Truncate thinking text for display
                                                let summary = Self::truncate_text(thinking, 60);
                                                s.activities.push(ActivityItem {
                                                    activity_type: ActivityType::Thinking(summary),
                                                    in_progress: false,
                                                });
                                            }
                                        }
                                        "tool_use" => {
                                            // Extract tool name and input
                                            let tool_name = block
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("Unknown");
                                            let input = block.get("input");
                                            let summary = Self::format_tool_summary(tool_name, input);
                                            s.activities.push(ActivityItem {
                                                activity_type: ActivityType::ToolUse {
                                                    tool: tool_name.to_string(),
                                                    summary,
                                                },
                                                in_progress: false,
                                            });
                                        }
                                        "text" => {
                                            // Final text response
                                            if let Some(text) =
                                                block.get("text").and_then(|t| t.as_str())
                                            {
                                                s.response_text = text.to_string();
                                                s.status = ResponseStatus::Responding;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            ctx.request_repaint();
                        }
                    }
                }
            }
            "content_block_start" => {
                // New content block starting - could be thinking or tool_use
                if let Some(content_block) = json.get("content_block") {
                    let block_type = content_block
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    let mut state = streaming_state.lock();
                    if let Some(ref mut s) = *state {
                        match block_type {
                            "thinking" => {
                                s.status = ResponseStatus::Thinking;
                                s.activities.push(ActivityItem {
                                    activity_type: ActivityType::Thinking(String::new()),
                                    in_progress: true,
                                });
                            }
                            "tool_use" => {
                                let tool_name = content_block
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("Unknown");
                                s.activities.push(ActivityItem {
                                    activity_type: ActivityType::ToolUse {
                                        tool: tool_name.to_string(),
                                        summary: String::new(),
                                    },
                                    in_progress: true,
                                });
                            }
                            "text" => {
                                s.status = ResponseStatus::Responding;
                            }
                            _ => {}
                        }
                    }
                }
                ctx.request_repaint();
            }
            "content_block_delta" => {
                // Update to existing content block
                if let Some(delta) = json.get("delta") {
                    let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    let mut state = streaming_state.lock();
                    if let Some(ref mut s) = *state {
                        match delta_type {
                            "thinking_delta" => {
                                // Update thinking text in last thinking activity
                                if let Some(thinking) =
                                    delta.get("thinking").and_then(|t| t.as_str())
                                {
                                    if let Some(last) = s.activities.last_mut() {
                                        if let ActivityType::Thinking(ref mut text) =
                                            last.activity_type
                                        {
                                            // Only keep first part for display
                                            if text.len() < 60 {
                                                text.push_str(thinking);
                                                if text.len() > 60 {
                                                    text.truncate(57);
                                                    text.push_str("...");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            "input_json_delta" => {
                                // Tool input being streamed - update summary
                                if let Some(partial) =
                                    delta.get("partial_json").and_then(|p| p.as_str())
                                {
                                    if let Some(last) = s.activities.last_mut() {
                                        if let ActivityType::ToolUse {
                                            ref mut summary, ..
                                        } = last.activity_type
                                        {
                                            if summary.len() < 50 {
                                                summary.push_str(partial);
                                                if summary.len() > 50 {
                                                    summary.truncate(47);
                                                    summary.push_str("...");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            "text_delta" => {
                                // Text response streaming
                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                    s.response_text.push_str(text);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                ctx.request_repaint();
            }
            "content_block_stop" => {
                // Content block finished
                let mut state = streaming_state.lock();
                if let Some(ref mut s) = *state {
                    // Mark last activity as complete
                    if let Some(last) = s.activities.last_mut() {
                        last.in_progress = false;
                    }
                }
                ctx.request_repaint();
            }
            "result" => {
                // Check for errors first
                let is_error = json
                    .get("is_error")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);

                let mut state = streaming_state.lock();
                if let Some(ref mut s) = *state {
                    if is_error {
                        if let Some(result_text) = json.get("result").and_then(|r| r.as_str()) {
                            s.error = Some(result_text.to_string());
                            s.activities.push(ActivityItem {
                                activity_type: ActivityType::Error(result_text.to_string()),
                                in_progress: false,
                            });
                        }
                    } else if let Some(result_text) = json.get("result").and_then(|r| r.as_str()) {
                        if s.response_text.is_empty() {
                            // Add final response as activity
                            s.activities.push(ActivityItem {
                                activity_type: ActivityType::Response(result_text.to_string()),
                                in_progress: false,
                            });
                            s.response_text = result_text.to_string();
                        }
                    }
                    s.is_complete = true;
                    s.status = ResponseStatus::Complete;
                }
                ctx.request_repaint();
            }
            "error" => {
                // Handle error events
                let error_msg = json
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");

                let mut state = streaming_state.lock();
                if let Some(ref mut s) = *state {
                    s.error = Some(error_msg.to_string());
                    s.activities.push(ActivityItem {
                        activity_type: ActivityType::Error(error_msg.to_string()),
                        in_progress: false,
                    });
                    s.is_complete = true;
                    s.status = ResponseStatus::Complete;
                }
                ctx.request_repaint();
            }
            _ => {}
        }
    }

    /// Truncate text for display
    #[cfg(not(target_arch = "wasm32"))]
    fn truncate_text(text: &str, max_len: usize) -> String {
        // Take first line only, and truncate
        let first_line = text.lines().next().unwrap_or(text);
        if first_line.len() > max_len {
            format!("{}...", &first_line[..max_len - 3])
        } else {
            first_line.to_string()
        }
    }

    /// Format a summary for tool usage
    #[cfg(not(target_arch = "wasm32"))]
    fn format_tool_summary(tool_name: &str, input: Option<&serde_json::Value>) -> String {
        let Some(input) = input else {
            return String::new();
        };

        match tool_name {
            "Edit" | "Write" | "Read" => {
                // Show file path
                input
                    .get("file_path")
                    .and_then(|p| p.as_str())
                    .map(|p| {
                        // Show just filename or last path component
                        p.rsplit('/').next().unwrap_or(p).to_string()
                    })
                    .unwrap_or_default()
            }
            "Bash" => {
                // Show command (truncated)
                input
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| Self::truncate_text(c, 40))
                    .unwrap_or_default()
            }
            "Grep" | "Glob" => {
                // Show pattern
                input
                    .get("pattern")
                    .and_then(|p| p.as_str())
                    .map(|p| Self::truncate_text(p, 40))
                    .unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Format model name for display (e.g., "claude-sonnet-4-20250514" -> "Sonnet 4")
    #[cfg(not(target_arch = "wasm32"))]
    fn format_model_name(model: &str) -> String {
        // Extract the model family and version
        let model_lower = model.to_lowercase();
        if model_lower.contains("opus") {
            if model_lower.contains("4-5") || model_lower.contains("4.5") {
                "Opus 4.5".to_string()
            } else if model_lower.contains("4") {
                "Opus 4".to_string()
            } else {
                "Opus".to_string()
            }
        } else if model_lower.contains("sonnet") {
            if model_lower.contains("4") {
                "Sonnet 4".to_string()
            } else if model_lower.contains("3.5") || model_lower.contains("3-5") {
                "Sonnet 3.5".to_string()
            } else {
                "Sonnet".to_string()
            }
        } else if model_lower.contains("haiku") {
            if model_lower.contains("3.5") || model_lower.contains("3-5") {
                "Haiku 3.5".to_string()
            } else {
                "Haiku".to_string()
            }
        } else {
            // Return a shortened version of the model name
            model.to_string()
        }
    }

    /// Poll streaming state and update messages
    fn poll_streaming_response(&mut self) {
        let state = self.streaming_state.lock();
        if let Some(ref s) = *state {
            // Update model and status on the panel
            if let Some(ref model) = s.model {
                self.current_model = Some(model.clone());
            }
            self.current_status = s.status;

            // Sync activities from streaming state
            self.current_activities = s.activities.clone();

            // Update the last assistant message
            if let Some(last) = self.messages.last_mut() {
                if last.role == MessageRole::Assistant && last.is_streaming {
                    last.content = s.response_text.clone();

                    if s.is_complete {
                        last.is_streaming = false;
                        self.is_waiting = false;
                        self.request_start_time = None;

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
