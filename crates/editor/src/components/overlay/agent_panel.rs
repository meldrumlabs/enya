//! Agent Panel - Claude Code integration for AI-assisted metrics exploration.
//!
//! Provides a chat interface to interact with Claude Code CLI, with streaming
//! responses displayed in real-time. Styled with the Obsidian Glass design system.

use egui::{Color32, Key, RichText, ScrollArea, TextEdit, Vec2};
use parking_lot::Mutex;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::io::{BufRead, BufReader, Write};
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

/// Available Claude models
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClaudeModel {
    /// Claude Sonnet 4.5 - balanced speed and capability
    #[default]
    Sonnet45,
    /// Claude Opus 4.5 - most capable
    Opus45,
    /// Claude Haiku 4.5 - fastest
    Haiku45,
}

impl ClaudeModel {
    /// Get the display name for this model
    fn display_name(self) -> &'static str {
        match self {
            Self::Sonnet45 => "Sonnet 4.5",
            Self::Opus45 => "Opus 4.5",
            Self::Haiku45 => "Haiku 4.5",
        }
    }

    /// Get the API model ID
    fn model_id(self) -> &'static str {
        match self {
            Self::Sonnet45 => "claude-sonnet-4-5-20250514",
            Self::Opus45 => "claude-opus-4-5-20250514",
            Self::Haiku45 => "claude-haiku-4-5-20250514",
        }
    }
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
    /// Current model being used (display name)
    current_model: Option<String>,
    /// Selected model for next request
    selected_model: ClaudeModel,
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
            selected_model: ClaudeModel::default(),
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

            // Model selector dropdown
            ui.add_space(6.0);
            let is_disabled = self.is_waiting;
            ui.add_enabled_ui(!is_disabled, |ui| {
                // Style the combo box to match the panel theme
                let style = ui.style_mut();
                style.visuals.widgets.inactive.bg_fill = colors.badge_bg;
                style.visuals.widgets.hovered.bg_fill = colors.elevated_bg;
                style.visuals.widgets.active.bg_fill = colors.elevated_bg;
                style.visuals.widgets.inactive.fg_stroke =
                    egui::Stroke::new(1.0, colors.separator);
                style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);

                egui::ComboBox::from_id_salt("model_selector")
                    .selected_text(
                        RichText::new(self.selected_model.display_name())
                            .color(text_muted)
                            .size(typography::SM),
                    )
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for model in [ClaudeModel::Sonnet45, ClaudeModel::Opus45, ClaudeModel::Haiku45] {
                            ui.selectable_value(
                                &mut self.selected_model,
                                model,
                                RichText::new(model.display_name())
                                    .color(text_primary)
                                    .size(typography::SM),
                            );
                        }
                    });
            });

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
                        format!("{elapsed:.0}s")
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
        // Set the model we're requesting (will be updated if server reports different)
        let model_display = self.selected_model.display_name().to_string();
        let model_id = self.selected_model.model_id().to_string();
        *self.streaming_state.lock() = Some(StreamingState {
            response_text: String::new(),
            is_complete: false,
            error: None,
            status: ResponseStatus::Waiting,
            model: Some(model_display.clone()),
            activities: Vec::new(),
        });
        self.current_status = ResponseStatus::Waiting;
        self.current_activities.clear();
        self.current_model = Some(model_display);

        // Spawn Claude Code ACP adapter process
        let streaming_state = Arc::clone(&self.streaming_state);
        let ctx_clone = ctx.clone();
        let _session_id = self.session_id.clone();

        std::thread::spawn(move || {
            // Use the @zed-industries/claude-code-acp npm package
            // This wraps Claude Code with ACP protocol support using the Claude Agent SDK.
            // Authentication is inherited from the Claude CLI - if you have Claude Max
            // subscription and have run `claude /login`, it will use that.
            // If ANTHROPIC_API_KEY is set, it will use API billing instead.
            // -y auto-confirms the npx install prompt (same as avante.nvim)
            let mut cmd = Command::new("npx");
            cmd.arg("-y")
                .arg("@zed-industries/claude-code-acp")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            log::debug!("Spawning claude-code-acp: {cmd:?}");

            match cmd.spawn() {
                Ok(mut child) => {
                    // Get handles
                    let stdin = child.stdin.take();
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();

                    // Spawn a thread to read stderr and log it
                    if let Some(stderr) = stderr {
                        std::thread::spawn(move || {
                            let reader = BufReader::new(stderr);
                            for line in reader.lines().map_while(Result::ok) {
                                log::warn!("claude-code-acp stderr: {line}");
                            }
                        });
                    }

                    if let (Some(mut stdin), Some(stdout)) = (stdin, stdout) {
                        let reader = BufReader::new(stdout);

                        // Run ACP session
                        if let Err(e) = Self::run_acp_session(
                            &mut stdin,
                            reader,
                            &prompt,
                            &model_id,
                            &streaming_state,
                            &ctx_clone,
                        ) {
                            let mut state = streaming_state.lock();
                            if let Some(ref mut s) = *state {
                                s.error = Some(format!("ACP error: {e}"));
                                s.is_complete = true;
                            }
                        }
                    }

                    // Clean up
                    let _ = child.kill();
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
                        s.error = Some(format!("Failed to start claude-code-acp: {e}"));
                        s.is_complete = true;
                    }
                    ctx_clone.request_repaint();
                }
            }
        });
    }

    /// Run the ACP session protocol
    #[cfg(not(target_arch = "wasm32"))]
    fn run_acp_session(
        stdin: &mut std::process::ChildStdin,
        reader: BufReader<std::process::ChildStdout>,
        prompt: &str,
        model_id: &str,
        streaming_state: &Arc<Mutex<Option<StreamingState>>>,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        use std::io::Lines;

        let mut lines: Lines<BufReader<std::process::ChildStdout>> = reader.lines();

        // Helper to send JSON-RPC message
        let send_msg =
            |stdin: &mut std::process::ChildStdin, msg: &serde_json::Value| -> Result<(), String> {
                writeln!(stdin, "{msg}").map_err(|e| format!("Write error: {e}"))?;
                stdin.flush().map_err(|e| format!("Flush error: {e}"))
            };

        // Helper to read response
        let read_line =
            |lines: &mut Lines<BufReader<std::process::ChildStdout>>| -> Result<serde_json::Value, String> {
                let line = lines
                    .next()
                    .ok_or("No response")?
                    .map_err(|e| format!("Read error: {e}"))?;
                log::trace!("ACP recv: {line}");
                serde_json::from_str(&line).map_err(|e| format!("Parse error: {e}"))
            };

        // 1. Send initialize
        // Note: protocolVersion must be a number (1), not a string
        let init_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientInfo": {
                    "name": "Enya",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "clientCapabilities": {
                    "terminal": true
                }
            }
        });
        send_msg(stdin, &init_msg)?;
        let _ = read_line(&mut lines)?; // Read init response

        // 2. Create session
        // Pass Claude Code options via _meta.claudeCode.options to configure the model
        // and enable extended thinking. The adapter uses Claude Max subscription by default
        // when ANTHROPIC_API_KEY is not set.
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let session_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "session/new",
            "params": {
                "cwd": cwd,
                "mcpServers": [],
                "_meta": {
                    "claudeCode": {
                        "options": {
                            "model": model_id
                        }
                    }
                }
            }
        });
        send_msg(stdin, &session_msg)?;
        let session_resp = read_line(&mut lines)?;

        // Extract session ID
        let session_id = session_resp
            .get("result")
            .and_then(|r| r.get("sessionId"))
            .and_then(|s| s.as_str())
            .unwrap_or("default")
            .to_string();

        log::debug!("ACP session created: {session_id}");

        // 3. Send prompt
        // Note: prompt must be an array at params.prompt, not params.content
        let prompt_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "session/prompt",
            "params": {
                "sessionId": session_id,
                "prompt": [{"type": "text", "text": prompt}]
            }
        });
        send_msg(stdin, &prompt_msg)?;

        // 4. Read streaming responses
        for line_result in lines {
            match line_result {
                Ok(line) => {
                    log::debug!("ACP message: {line}");
                    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
                        // Process the ACP message
                        let should_stop = Self::process_acp_event(&msg, streaming_state, ctx);
                        if should_stop {
                            break;
                        }
                    }
                }
                Err(e) => {
                    return Err(format!("Read error: {e}"));
                }
            }
        }

        Ok(())
    }

    /// Process an ACP event and return true if streaming is complete
    #[cfg(not(target_arch = "wasm32"))]
    fn process_acp_event(
        msg: &serde_json::Value,
        streaming_state: &Arc<Mutex<Option<StreamingState>>>,
        ctx: &egui::Context,
    ) -> bool {
        // Check for notifications (session/update)
        if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
            if method == "session/update" {
                if let Some(params) = msg.get("params") {
                    Self::process_session_update(params, streaming_state, ctx);
                }
            }
            return false;
        }

        // Check for prompt response (completion) - id: 3 is our prompt request
        if msg.get("id") == Some(&serde_json::json!(3)) {
            if let Some(result) = msg.get("result") {
                let mut state = streaming_state.lock();
                if let Some(ref mut s) = *state {
                    // Mark all activities as complete
                    for activity in &mut s.activities {
                        activity.in_progress = false;
                    }
                    s.is_complete = true;
                    s.status = ResponseStatus::Complete;
                }
                ctx.request_repaint();

                // Check stop reason
                if result.get("stopReason").is_some() {
                    return true;
                }
            }

            // Check for error response
            if let Some(error) = msg.get("error") {
                let error_msg = error
                    .get("message")
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
                return true;
            }
        }

        false
    }

    /// Process a session/update notification from ACP
    #[cfg(not(target_arch = "wasm32"))]
    fn process_session_update(
        params: &serde_json::Value,
        streaming_state: &Arc<Mutex<Option<StreamingState>>>,
        ctx: &egui::Context,
    ) {
        let Some(update) = params.get("update") else {
            return;
        };

        // Get the session update type
        let Some(update_type) = update.get("sessionUpdate").and_then(|u| u.as_str()) else {
            return;
        };

        let mut state = streaming_state.lock();
        let Some(ref mut s) = *state else {
            return;
        };

        log::debug!("ACP session update: {update_type}");

        match update_type {
            "agent_message_chunk" => {
                // Text delta from the agent
                if let Some(content) = update.get("content") {
                    if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                        s.response_text.push_str(text);
                        s.status = ResponseStatus::Responding;
                    }
                }
            }
            "agent_thought_chunk" => {
                log::debug!("Got thinking chunk");
                // Thinking delta
                if let Some(content) = update.get("content") {
                    if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
                        s.status = ResponseStatus::Thinking;
                        // Update or create thinking activity
                        if let Some(last) = s.activities.last_mut() {
                            if let ActivityType::Thinking(ref mut thinking_text) =
                                last.activity_type
                            {
                                if thinking_text.len() < 60 {
                                    thinking_text.push_str(text);
                                    if thinking_text.len() > 60 {
                                        thinking_text.truncate(57);
                                        thinking_text.push_str("...");
                                    }
                                }
                            } else {
                                // Not a thinking activity, create new one
                                s.activities.push(ActivityItem {
                                    activity_type: ActivityType::Thinking(Self::truncate_text(
                                        text, 60,
                                    )),
                                    in_progress: true,
                                });
                            }
                        } else {
                            s.activities.push(ActivityItem {
                                activity_type: ActivityType::Thinking(Self::truncate_text(
                                    text, 60,
                                )),
                                in_progress: true,
                            });
                        }
                    }
                }
            }
            "tool_call" => {
                log::debug!("Got tool_call: {update}");
                // Tool call started - check multiple possible locations for tool name
                let tool_name = update
                    .get("name")
                    .and_then(|n| n.as_str())
                    .or_else(|| {
                        // Also check _meta.claudeCode.toolName
                        update
                            .get("_meta")
                            .and_then(|m| m.get("claudeCode"))
                            .and_then(|c| c.get("toolName"))
                            .and_then(|n| n.as_str())
                    })
                    .or_else(|| {
                        // Also check for 'title' field from toolInfoFromToolUse
                        update.get("title").and_then(|t| t.as_str())
                    });

                // Try to extract a summary from the tool input
                // rawInput can be either a JSON object or a JSON string
                let summary = update
                    .get("rawInput")
                    .and_then(|r| {
                        // Handle both object and string forms
                        if r.is_object() {
                            Some(r.clone())
                        } else {
                            r.as_str()
                                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                        }
                    })
                    .and_then(|v| {
                        // Check for file path fields first (use path truncation)
                        if let Some(path) = v
                            .get("file_path")
                            .or_else(|| v.get("path"))
                            .and_then(|s| s.as_str())
                        {
                            return Some(Self::truncate_path(path, 50));
                        }
                        // Other fields use regular text truncation
                        v.get("pattern")
                            .or_else(|| v.get("command"))
                            .or_else(|| v.get("description"))
                            .or_else(|| v.get("prompt"))
                            .and_then(|s| s.as_str())
                            .map(|s| Self::truncate_text(s, 50))
                    })
                    .or_else(|| {
                        // Fall back to title if no specific field found
                        update
                            .get("title")
                            .and_then(|t| t.as_str())
                            .map(String::from)
                    })
                    .unwrap_or_default();

                if let Some(name) = tool_name {
                    log::debug!("Tool name: {name}, summary: {summary}");
                    // Mark previous activities as complete
                    for activity in &mut s.activities {
                        activity.in_progress = false;
                    }
                    s.activities.push(ActivityItem {
                        activity_type: ActivityType::ToolUse {
                            tool: name.to_string(),
                            summary,
                        },
                        in_progress: true,
                    });
                } else {
                    log::warn!("tool_call missing name field");
                }
            }
            "tool_call_update" => {
                // Tool call completed or errored
                if let Some(status) = update.get("status").and_then(|st| st.as_str()) {
                    if status == "completed" || status == "error" {
                        // Mark tool as complete
                        if let Some(last) = s.activities.last_mut() {
                            last.in_progress = false;
                        }
                    }
                }
            }
            _ => {}
        }

        ctx.request_repaint();
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

    /// Truncate a file path to show the suffix (filename with some parent context)
    #[cfg(not(target_arch = "wasm32"))]
    fn truncate_path(path: &str, max_len: usize) -> String {
        if path.len() <= max_len {
            return path.to_string();
        }

        // Try to show as much of the path suffix as possible
        // e.g., ".../components/overlay/agent_panel.rs"
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() <= 1 {
            // No slashes, just truncate normally
            return format!("...{}", &path[path.len().saturating_sub(max_len - 3)..]);
        }

        // Start from the filename and add parent directories until we hit the limit
        let mut result = String::new();
        for part in parts.iter().rev() {
            let candidate = if result.is_empty() {
                part.to_string()
            } else {
                format!("{part}/{result}")
            };

            if candidate.len() + 4 > max_len {
                // Adding this part would exceed the limit
                break;
            }
            result = candidate;
        }

        if result.len() < path.len() {
            format!(".../{result}")
        } else {
            result
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
