//! Agent Pane - AI-assisted chat as a first-class pane in the viewport.
//!
//! This pane provides a chat interface for interacting with AI agents (Claude, Codex)
//! as a peer to query panes, allowing parallel agent conversations alongside charts.

use egui::{Color32, Key, RichText, ScrollArea, TextEdit, Vec2};

#[cfg(not(target_arch = "wasm32"))]
use enya_ai::{AcpClient, AgentEvent};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use crate::components::overlay::agent_context::{self, AgentCommand, EditorContext};
use crate::components::util::finder_utils::OverlayColors;
use crate::components::util::id_generator::next_id_usize;
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

/// Actions that can result from agent pane interaction
#[derive(Debug, Clone)]
pub enum AgentPaneAction {
    /// No action
    None,
    /// Commands parsed from agent response
    Commands(Vec<AgentCommand>),
}

/// Status of the response
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(dead_code)] // Variants used for tracking internal state
enum ResponseStatus {
    #[default]
    Waiting,
    Thinking,
    Responding,
    Complete,
}

/// Available AI providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiProvider {
    /// Claude Code (Anthropic) - default
    #[default]
    Claude,
    /// Codex (OpenAI)
    Codex,
}

impl AiProvider {
    /// Get the display name for this provider
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
        }
    }
}

/// Available models (varies by provider)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiModel {
    // Claude models
    ClaudeSonnet45,
    ClaudeOpus45,
    ClaudeHaiku45,
    // OpenAI models (GPT-5.2 series)
    Gpt52,
    Gpt52Pro,
    Gpt52Codex,
}

impl AiModel {
    /// Get the display name for this model
    fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeSonnet45 => "Sonnet 4.5",
            Self::ClaudeOpus45 => "Opus 4.5",
            Self::ClaudeHaiku45 => "Haiku 4.5",
            Self::Gpt52 => "GPT-5.2",
            Self::Gpt52Pro => "GPT-5.2 Pro",
            Self::Gpt52Codex => "GPT-5.2 Codex",
        }
    }

    /// Get the API model ID
    #[allow(dead_code)] // Used when sending requests to AcpClient
    fn model_id(self) -> &'static str {
        match self {
            Self::ClaudeSonnet45 => "claude-sonnet-4-5-20250514",
            Self::ClaudeOpus45 => "claude-opus-4-5-20250514",
            Self::ClaudeHaiku45 => "claude-haiku-4-5-20250514",
            Self::Gpt52 => "gpt-5.2-2025-12-11",
            Self::Gpt52Pro => "gpt-5.2-pro-2025-12-11",
            Self::Gpt52Codex => "gpt-5.2-codex",
        }
    }

    /// Get models available for a provider
    fn for_provider(provider: AiProvider) -> &'static [Self] {
        match provider {
            AiProvider::Claude => &[
                Self::ClaudeSonnet45,
                Self::ClaudeOpus45,
                Self::ClaudeHaiku45,
            ],
            AiProvider::Codex => &[Self::Gpt52Codex, Self::Gpt52, Self::Gpt52Pro],
        }
    }

    /// Get the default model for a provider
    fn default_for(provider: AiProvider) -> Self {
        match provider {
            AiProvider::Claude => Self::ClaudeSonnet45,
            AiProvider::Codex => Self::Gpt52Codex,
        }
    }
}

/// An Agent pane for AI-assisted chat in the viewport.
#[allow(dead_code)] // Some fields used only in native builds or for future features
pub struct AgentPane {
    /// Unique identifier for this pane
    id: usize,
    /// Pane name (e.g., "Agent 1")
    name: String,
    /// Current theme
    theme: AppTheme,
    /// Chat message history
    messages: Vec<ChatMessage>,
    /// Current input text
    input_text: String,
    /// Whether we're currently waiting for a response
    is_waiting: bool,
    /// Event receiver for streaming ACP responses
    #[cfg(not(target_arch = "wasm32"))]
    event_receiver: Option<Receiver<AgentEvent>>,
    /// Accumulated response text during streaming
    response_text: String,
    /// Whether the input should be focused
    focus_input: bool,
    /// Scroll to bottom flag
    scroll_to_bottom: bool,
    /// Selected AI provider
    selected_provider: AiProvider,
    /// Selected model for next request
    selected_model: AiModel,
    /// Current response status for UI display
    current_status: ResponseStatus,
    /// Current activities being displayed
    current_activities: Vec<ActivityItem>,
    /// Timestamp when request started (for elapsed time display)
    request_start_time: Option<std::time::Instant>,
    /// Tokio runtime handle for spawning async tasks
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: Option<tokio::runtime::Handle>,
    /// Editor context to inject into prompts
    editor_context: Option<EditorContext>,
    /// Commands parsed from completed responses (drained on next show())
    pending_commands: Vec<AgentCommand>,
}

impl AgentPane {
    /// Create a new agent pane with a tokio runtime handle.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        let provider = AiProvider::default();
        let id = next_id_usize();
        Self {
            id,
            name: format!("Agent {id}"),
            theme: AppTheme::default(),
            messages: Vec::new(),
            input_text: String::new(),
            is_waiting: false,
            event_receiver: None,
            response_text: String::new(),
            focus_input: true,
            scroll_to_bottom: false,
            selected_provider: provider,
            selected_model: AiModel::default_for(provider),
            current_status: ResponseStatus::Complete,
            current_activities: Vec::new(),
            request_start_time: None,
            runtime_handle: Some(runtime_handle),
            editor_context: None,
            pending_commands: Vec::new(),
        }
    }

    /// Create a new agent pane with a custom name.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_name(name: impl Into<String>, runtime_handle: tokio::runtime::Handle) -> Self {
        let mut pane = Self::new(runtime_handle);
        pane.name = name.into();
        pane
    }

    /// Create a new agent pane (WASM version - no runtime needed).
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        let provider = AiProvider::default();
        let id = next_id_usize();
        Self {
            id,
            name: format!("Agent {id}"),
            theme: AppTheme::default(),
            messages: Vec::new(),
            input_text: String::new(),
            is_waiting: false,
            response_text: String::new(),
            focus_input: true,
            scroll_to_bottom: false,
            selected_provider: provider,
            selected_model: AiModel::default_for(provider),
            current_status: ResponseStatus::Complete,
            current_activities: Vec::new(),
            request_start_time: None,
            editor_context: None,
            pending_commands: Vec::new(),
        }
    }

    /// Set the editor context for prompt injection.
    pub fn set_context(&mut self, context: EditorContext) {
        self.editor_context = Some(context);
    }

    /// Get the pane ID.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get the pane name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set the pane name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the AI provider.
    pub fn set_provider(&mut self, provider: AiProvider) {
        if self.selected_provider != provider {
            self.selected_provider = provider;
            self.selected_model = AiModel::default_for(provider);
        }
    }

    /// Poll for pending commands without rendering.
    ///
    /// This is used by the workspace to collect commands from agent panes
    /// after they've been rendered by the tile tree.
    pub fn poll_pending_commands(&mut self) -> Vec<AgentCommand> {
        // Poll streaming to update pending_commands
        self.poll_streaming_response();

        // Drain and return pending commands
        std::mem::take(&mut self.pending_commands)
    }

    /// Render the agent pane.
    ///
    /// Note: Commands are NOT drained here. Use `poll_pending_commands()` to
    /// retrieve pending commands after the pane has been rendered by the tile tree.
    /// This is necessary because the Component trait's show() doesn't return a value.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Poll streaming state (this may populate pending_commands)
        self.poll_streaming_response();

        // Request repaint while timer is running
        if self.request_start_time.is_some() {
            ui.ctx().request_repaint();
        }

        // Handle Escape to stop request
        if self.is_waiting && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.stop_request();
        }

        let colors = OverlayColors::new(self.theme);
        let ctx = ui.ctx().clone();

        ui.vertical(|ui| {
            // Header with provider/model selector
            self.render_header(ui, &colors);

            // Chat area (scrollable, takes most space)
            let available_height = ui.available_height() - 60.0;
            ScrollArea::vertical()
                .id_salt(format!("agent_chat_{}", self.id))
                .max_height(available_height)
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    self.render_messages(ui, &colors);

                    if self.scroll_to_bottom {
                        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                        self.scroll_to_bottom = false;
                    }
                });

            // Input area at bottom
            self.render_input(ui, &colors, &ctx);
        });
    }

    fn render_header(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Icon with accent color
            ui.label(
                RichText::new(egui_nerdfonts::regular::SPARKLE_FILL)
                    .color(colors.accent)
                    .size(14.0),
            );
            ui.add_space(4.0);

            // Title
            ui.label(
                RichText::new(&self.name)
                    .color(colors.text)
                    .size(typography::MD)
                    .strong(),
            );

            // Model selector dropdown
            ui.add_space(4.0);
            let is_disabled = self.is_waiting;
            ui.add_enabled_ui(!is_disabled, |ui| {
                let style = ui.style_mut();
                style.visuals.widgets.inactive.bg_fill = colors.badge_bg;
                style.visuals.widgets.hovered.bg_fill = colors.elevated_bg;
                style.visuals.widgets.active.bg_fill = colors.elevated_bg;
                style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, colors.separator);
                style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);

                egui::ComboBox::from_id_salt(format!("model_selector_{}", self.id))
                    .selected_text(
                        RichText::new(self.selected_model.display_name())
                            .color(colors.muted_text)
                            .size(typography::SM),
                    )
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for &model in AiModel::for_provider(self.selected_provider) {
                            ui.selectable_value(
                                &mut self.selected_model,
                                model,
                                RichText::new(model.display_name())
                                    .color(colors.text)
                                    .size(typography::SM),
                            );
                        }
                    });
            });
        });

        ui.add_space(4.0);

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
    }

    fn render_messages(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        if self.messages.is_empty() && self.current_activities.is_empty() {
            // Empty state
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new(egui_nerdfonts::regular::COMMENT_TEXT)
                        .color(colors.faint_text)
                        .size(28.0),
                );
                ui.add_space(8.0);
                let prompt_text = match self.selected_provider {
                    AiProvider::Claude => "Ask Claude anything",
                    AiProvider::Codex => "Ask Codex anything",
                };
                ui.label(
                    RichText::new(prompt_text)
                        .color(colors.muted_text)
                        .size(typography::MD),
                );
            });
        } else {
            // Render messages with activities after last user message
            let last_user_idx = self
                .messages
                .iter()
                .enumerate()
                .rev()
                .find(|(_, m)| m.role == MessageRole::User)
                .map(|(i, _)| i);

            for (i, message) in self.messages.iter().enumerate() {
                self.render_message(ui, message, colors);
                ui.add_space(4.0);

                // Show activities after the last user message
                if Some(i) == last_user_idx && !self.current_activities.is_empty() {
                    for activity in &self.current_activities {
                        self.render_activity(ui, activity, colors);
                        ui.add_space(2.0);
                    }
                }
            }
        }
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
                self.selected_provider.display_name(),
                palette::accent::PRIMARY,
                Color32::TRANSPARENT,
            ),
            MessageRole::System => ("System", colors.faint_text, Color32::TRANSPARENT),
        };

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.vertical(|ui| {
                // Role label
                ui.label(
                    RichText::new(role_label)
                        .color(role_color)
                        .size(typography::SM)
                        .strong(),
                );
                ui.add_space(2.0);

                // Message content
                if msg_bg != Color32::TRANSPARENT {
                    egui::Frame::NONE
                        .fill(msg_bg)
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.set_max_width(ui.available_width() - 16.0);
                            self.render_message_content(ui, message, colors);
                        });
                } else {
                    ui.horizontal(|ui| {
                        ui.add_space(2.0);
                        ui.vertical(|ui| {
                            ui.set_max_width(ui.available_width() - 12.0);
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
        if !message.content.is_empty() {
            // Strip enya-command blocks from assistant messages
            let display_content = if message.role == MessageRole::Assistant {
                agent_context::strip_command_blocks(&message.content)
            } else {
                message.content.clone()
            };

            if !display_content.is_empty() {
                let normalized = Self::normalize_text(&display_content);
                ui.label(
                    RichText::new(normalized)
                        .color(colors.text)
                        .size(typography::MD),
                );
            }
        }

        // Streaming indicator with timer
        if message.is_streaming {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new().color(colors.accent).size(10.0));
                ui.add_space(4.0);

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

    fn render_activity(&self, ui: &mut egui::Ui, activity: &ActivityItem, colors: &OverlayColors) {
        use egui_nerdfonts::regular;

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
            ui.add_space(12.0);

            if activity.in_progress {
                ui.add(egui::Spinner::new().color(colors.accent).size(10.0));
            } else {
                ui.label(RichText::new(icon).color(icon_color).size(10.0));
            }

            ui.add_space(4.0);
            ui.label(
                RichText::new(label)
                    .color(colors.muted_text)
                    .size(typography::SM),
            );

            if !summary.is_empty() {
                ui.add_space(3.0);
                ui.label(
                    RichText::new(&summary)
                        .color(colors.faint_text)
                        .size(typography::SM),
                );
            }
        });
    }

    fn render_input(&mut self, ui: &mut egui::Ui, colors: &OverlayColors, ctx: &egui::Context) {
        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Input field
            let input_bg = colors.elevated_bg;
            egui::Frame::new()
                .fill(input_bg)
                .corner_radius(4.0)
                .inner_margin(egui::Margin::symmetric(8, 4))
                .stroke(egui::Stroke::new(1.0, colors.separator))
                .show(ui, |ui| {
                    let hint_text = match self.selected_provider {
                        AiProvider::Claude => "Ask Claude...",
                        AiProvider::Codex => "Ask Codex...",
                    };
                    let response = ui.add_sized(
                        Vec2::new(ui.available_width() - 40.0, 18.0),
                        TextEdit::singleline(&mut self.input_text)
                            .hint_text(
                                RichText::new(hint_text)
                                    .color(colors.faint_text)
                                    .size(typography::SM),
                            )
                            .frame(false)
                            .font(typography::proportional(typography::SM)),
                    );

                    // Focus input on first frame
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

            // Send or Stop button
            ui.add_space(4.0);
            if self.is_waiting {
                // Show stop button while waiting
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(egui_nerdfonts::regular::STOP_CIRCLE)
                                .size(14.0)
                                .color(palette::semantic::ERROR),
                        )
                        .frame(false),
                    )
                    .on_hover_text("Stop (Esc)")
                    .clicked()
                {
                    self.stop_request();
                }
            } else {
                // Show send button when idle
                let can_send = !self.input_text.trim().is_empty();
                let send_color = if can_send {
                    colors.accent
                } else {
                    colors.faint_text
                };

                if ui
                    .add_enabled(
                        can_send,
                        egui::Button::new(
                            RichText::new(egui_nerdfonts::regular::SEND)
                                .size(14.0)
                                .color(send_color),
                        )
                        .frame(false),
                    )
                    .on_hover_text("Send (Enter)")
                    .clicked()
                {
                    self.send_message(ctx);
                }
            }

            ui.add_space(4.0);
        });
        ui.add_space(4.0);
    }

    /// Send the current input as a message.
    #[cfg(not(target_arch = "wasm32"))]
    fn send_message(&mut self, _ctx: &egui::Context) {
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

        // Clear input and reset state
        self.input_text.clear();
        self.is_waiting = true;
        self.scroll_to_bottom = true;
        self.request_start_time = Some(std::time::Instant::now());
        self.response_text.clear();
        self.current_status = ResponseStatus::Waiting;
        self.current_activities.clear();

        // Get working directory
        let working_dir = std::env::current_dir().ok();

        // Create client based on selected provider
        let client = match (&self.selected_provider, &self.runtime_handle) {
            (AiProvider::Claude, Some(handle)) => {
                AcpClient::claude_code_with_runtime(handle.clone())
            }
            (AiProvider::Claude, None) => AcpClient::claude_code(),
            (AiProvider::Codex, Some(handle)) => AcpClient::codex_with_runtime(handle.clone()),
            (AiProvider::Codex, None) => AcpClient::codex(),
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

    #[cfg(target_arch = "wasm32")]
    fn send_message(&mut self, _ctx: &egui::Context) {
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: self.input_text.trim().to_string(),
            is_streaming: false,
        });

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "AI agents are not available in the browser.".to_string(),
            is_streaming: false,
        });

        self.input_text.clear();
        self.scroll_to_bottom = true;
    }

    /// Stop the current request and reset state.
    fn stop_request(&mut self) {
        if !self.is_waiting {
            return;
        }

        // Drop the event receiver to stop listening
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.event_receiver = None;
        }

        // Reset waiting state
        self.is_waiting = false;
        self.request_start_time = None;
        self.current_status = ResponseStatus::Complete;
        self.current_activities.clear();

        // Update the last message if it was streaming
        if let Some(last) = self.messages.last_mut() {
            if last.is_streaming {
                last.is_streaming = false;
                // If there's accumulated response text, use it
                if !self.response_text.is_empty() {
                    last.content = std::mem::take(&mut self.response_text);
                    last.content.push_str("\n\n*[Stopped]*");
                } else {
                    last.content = "*[Stopped]*".to_string();
                }
            }
        }

        log::info!("Agent request stopped by user");
    }

    /// Normalize unicode characters that may not render correctly.
    fn normalize_text(text: &str) -> String {
        text.chars()
            .map(|c| match c {
                '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}' => '-',
                '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
                '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
                '\u{2026}' => c,
                '\u{00A0}' => ' ',
                _ => c,
            })
            .collect()
    }

    /// Truncate text for display.
    #[cfg(not(target_arch = "wasm32"))]
    fn truncate_text(text: &str, max_len: usize) -> String {
        let first_line = text.lines().next().unwrap_or(text);
        if first_line.len() > max_len {
            format!("{}...", &first_line[..max_len - 3])
        } else {
            first_line.to_string()
        }
    }

    /// Truncate a file path to show the suffix.
    #[cfg(not(target_arch = "wasm32"))]
    fn truncate_path(path: &str, max_len: usize) -> String {
        if path.len() <= max_len {
            return path.to_string();
        }

        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() <= 1 {
            return format!("...{}", &path[path.len().saturating_sub(max_len - 3)..]);
        }

        let mut result = String::new();
        for part in parts.iter().rev() {
            let candidate = if result.is_empty() {
                part.to_string()
            } else {
                format!("{part}/{result}")
            };

            if candidate.len() + 4 > max_len {
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

    /// Poll the event receiver and update UI state.
    #[cfg(not(target_arch = "wasm32"))]
    fn poll_streaming_response(&mut self) {
        let Some(ref receiver) = self.event_receiver else {
            return;
        };

        while let Ok(event) = receiver.try_recv() {
            match event {
                AgentEvent::TextDelta(text) => {
                    self.response_text.push_str(&text);
                    self.current_status = ResponseStatus::Responding;
                }
                AgentEvent::ThinkingDelta(text) => {
                    self.current_status = ResponseStatus::Thinking;
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
                                activity_type: ActivityType::Thinking(Self::truncate_text(
                                    &text, 60,
                                )),
                                in_progress: true,
                            });
                        }
                    } else {
                        self.current_activities.push(ActivityItem {
                            activity_type: ActivityType::Thinking(Self::truncate_text(&text, 60)),
                            in_progress: true,
                        });
                    }
                }
                AgentEvent::ToolCallStart {
                    name, raw_input, ..
                } => {
                    for activity in &mut self.current_activities {
                        activity.in_progress = false;
                    }

                    let summary = raw_input
                        .as_ref()
                        .and_then(|v| {
                            if let Some(path) = v
                                .get("file_path")
                                .or_else(|| v.get("path"))
                                .and_then(|s| s.as_str())
                            {
                                return Some(Self::truncate_path(path, 50));
                            }
                            v.get("pattern")
                                .or_else(|| v.get("command"))
                                .or_else(|| v.get("description"))
                                .or_else(|| v.get("prompt"))
                                .and_then(|s| s.as_str())
                                .map(|s| Self::truncate_text(s, 50))
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
                    if let Some(last) = self.current_activities.last_mut() {
                        last.in_progress = false;
                    }
                    if is_error {
                        // Optionally add error indicator
                    }
                }
                AgentEvent::Done { .. } => {
                    for activity in &mut self.current_activities {
                        activity.in_progress = false;
                    }
                    self.current_status = ResponseStatus::Complete;

                    if let Some(last) = self.messages.last_mut() {
                        if last.role == MessageRole::Assistant && last.is_streaming {
                            last.content = self.response_text.clone();
                            last.is_streaming = false;
                        }
                    }

                    // Parse commands from the response
                    let commands = agent_context::parse_commands(&self.response_text);
                    if !commands.is_empty() {
                        log::info!("Parsed {} commands from agent response", commands.len());
                        self.pending_commands.extend(commands);
                    }

                    self.is_waiting = false;
                    self.request_start_time = None;
                    self.event_receiver = None;
                    return;
                }
                AgentEvent::Error(e) => {
                    for activity in &mut self.current_activities {
                        activity.in_progress = false;
                    }
                    self.current_activities.push(ActivityItem {
                        activity_type: ActivityType::Error(e.to_string()),
                        in_progress: false,
                    });
                    self.current_status = ResponseStatus::Complete;

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

    #[cfg(target_arch = "wasm32")]
    fn poll_streaming_response(&mut self) {
        // No-op on WASM
    }
}

/// Implement Component trait so AgentPane can be used in the tile tree.
impl crate::components::Component for AgentPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        AgentPane::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        AgentPane::set_theme(self, theme);
    }

    fn set_api_key(&mut self, _key: &str) {
        // Not needed for agent pane
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed for agent pane
    }

    fn label(&self) -> egui::RichText {
        let icon = egui_nerdfonts::regular::SPARKLE_FILL;
        egui::RichText::new(format!("{icon} {}", self.name))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
