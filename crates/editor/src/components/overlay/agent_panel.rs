//! Agent Panel - Claude Code integration for AI-assisted metrics exploration.
//!
//! Provides a chat interface to interact with Claude Code CLI, with streaming
//! responses displayed in real-time. Styled with the Obsidian Glass design system.

use egui::{Color32, Key, RichText, ScrollArea, TextEdit, Vec2};

#[cfg(not(target_arch = "wasm32"))]
use enya_ai::{AcpClient, AgentEvent};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use crate::components::util::finder_utils::OverlayColors;
use crate::components::util::{
    ActivityItem, ActivityType, AiModel, AiProvider, MessageRole, ResponseStatus, normalize_unicode,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{truncate_first_line, truncate_path_suffix};
use crate::ui::palette;
use crate::ui::theme::AppTheme;
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

/// Result of showing the agent panel
#[derive(Debug, Clone)]
pub enum AgentPanelResult {
    /// No action needed
    None,
    /// Panel was closed
    Closed,
    /// Commands parsed from agent response
    Commands(Vec<super::agent_context::AgentCommand>),
}

/// The agent panel component for AI-assisted chat
#[allow(dead_code)] // Some fields used only in native builds or for future features
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
    /// Event receiver for streaming ACP responses
    #[cfg(not(target_arch = "wasm32"))]
    event_receiver: Option<Receiver<AgentEvent>>,
    /// Accumulated response text during streaming
    response_text: String,
    /// Whether the input should be focused
    focus_input: bool,
    /// Scroll to bottom flag
    scroll_to_bottom: bool,
    /// Current model being used (display name)
    current_model: Option<String>,
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
    editor_context: Option<super::agent_context::EditorContext>,
    /// Commands parsed from completed responses (drained on next show())
    pending_commands: Vec<super::agent_context::AgentCommand>,
    /// Whether to auto-submit the input on next show() call
    pending_submit: bool,
}

impl AgentPanel {
    /// Create a new agent panel with a tokio runtime handle.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        let provider = AiProvider::default();
        Self {
            is_open: false,
            theme: AppTheme::default(),
            messages: Vec::new(),
            input_text: String::new(),
            is_waiting: false,
            event_receiver: None,
            response_text: String::new(),
            focus_input: false,
            scroll_to_bottom: false,
            current_model: None,
            selected_provider: provider,
            selected_model: AiModel::default_for(provider),
            current_status: ResponseStatus::Complete,
            current_activities: Vec::new(),
            request_start_time: None,
            runtime_handle: Some(runtime_handle),
            editor_context: None,
            pending_commands: Vec::new(),
            pending_submit: false,
        }
    }

    /// Create a new agent panel (WASM version - no runtime needed).
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        let provider = AiProvider::default();
        Self {
            is_open: false,
            theme: AppTheme::default(),
            messages: Vec::new(),
            input_text: String::new(),
            is_waiting: false,
            response_text: String::new(),
            focus_input: false,
            scroll_to_bottom: false,
            current_model: None,
            selected_provider: provider,
            selected_model: AiModel::default_for(provider),
            current_status: ResponseStatus::Complete,
            current_activities: Vec::new(),
            request_start_time: None,
            editor_context: None,
            pending_commands: Vec::new(),
            pending_submit: false,
        }
    }

    /// Set the editor context for prompt injection.
    ///
    /// The context provides the agent with information about available metrics,
    /// connection status, indexed codebase, and current dashboard state.
    pub fn set_context(&mut self, context: super::agent_context::EditorContext) {
        self.editor_context = Some(context);
    }

    /// Clear the editor context.
    pub fn clear_context(&mut self) {
        self.editor_context = None;
    }

    /// Set the AI provider
    pub fn set_provider(&mut self, provider: AiProvider) {
        if self.selected_provider != provider {
            self.selected_provider = provider;
            // Reset to default model for the new provider
            self.selected_model = AiModel::default_for(provider);
        }
    }

    /// Get the current provider
    pub fn provider(&self) -> AiProvider {
        self.selected_provider
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

    /// Submit a query programmatically (e.g., from Agent Input Bar)
    ///
    /// Opens the panel if not already open and queues the message for sending.
    pub fn submit_query(&mut self, query: &str) {
        if query.trim().is_empty() {
            return;
        }
        self.input_text = query.to_string();
        self.is_open = true;
        self.pending_submit = true;
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

    /// Check if the panel is currently waiting for a response
    pub fn is_waiting(&self) -> bool {
        self.is_waiting
    }

    /// Get the current response status
    pub fn response_status(&self) -> ResponseStatus {
        self.current_status
    }

    /// Get the current response text (may be partial during streaming)
    pub fn response_text(&self) -> &str {
        &self.response_text
    }

    /// Get the current activities (tool uses, thinking, etc.)
    pub fn activities(&self) -> &[ActivityItem] {
        &self.current_activities
    }

    /// Get the last completed response text (from messages, not streaming)
    pub fn last_response(&self) -> Option<&str> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant && !m.is_streaming)
            .map(|m| m.content.as_str())
    }

    /// Show the panel as a side panel. Returns the result.
    pub fn show(&mut self, ctx: &egui::Context) -> AgentPanelResult {
        if !self.is_open {
            // Even when closed, check for pending commands
            if !self.pending_commands.is_empty() {
                let commands = std::mem::take(&mut self.pending_commands);
                return AgentPanelResult::Commands(commands);
            }
            return AgentPanelResult::None;
        }

        // Poll streaming state
        self.poll_streaming_response();

        // Handle pending submit from external submit_query() call
        if self.pending_submit && !self.is_waiting {
            self.pending_submit = false;
            self.send_message(ctx);
        }

        // Request repaint while timer is running (to update elapsed time)
        if self.request_start_time.is_some() {
            ctx.request_repaint();
        }

        // Check for pending commands to return
        let mut result = if !self.pending_commands.is_empty() {
            let commands = std::mem::take(&mut self.pending_commands);
            AgentPanelResult::Commands(commands)
        } else {
            AgentPanelResult::None
        };

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

        if matches!(result, AgentPanelResult::Closed) {
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

            // Title - shows current provider
            ui.label(
                RichText::new(self.selected_provider.display_name())
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
                style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, colors.separator);
                style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(4);

                egui::ComboBox::from_id_salt("model_selector")
                    .selected_text(
                        RichText::new(self.selected_model.display_name())
                            .color(text_muted)
                            .size(typography::SM),
                    )
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for &model in AiModel::for_provider(self.selected_provider) {
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
                        let prompt_text = match self.selected_provider {
                            AiProvider::Claude => "Ask Claude anything",
                            AiProvider::Codex => "Ask Codex anything",
                        };
                        ui.label(
                            RichText::new(prompt_text)
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
                    let hint_text = match self.selected_provider {
                        AiProvider::Claude => "Ask Claude...",
                        AiProvider::Codex => "Ask Codex...",
                    };
                    let response = ui.add_sized(
                        Vec2::new(ui.available_width() - 50.0, 20.0),
                        TextEdit::singleline(&mut self.input_text)
                            .hint_text(
                                RichText::new(hint_text)
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
                self.selected_provider.display_name(),
                palette::accent::PRIMARY,
                Color32::TRANSPARENT, // No background for assistant - cleaner
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
            // For assistant messages, strip enya-command blocks from display
            let display_content = if message.role == MessageRole::Assistant {
                super::agent_context::strip_command_blocks(&message.content)
            } else {
                message.content.clone()
            };

            if !display_content.is_empty() {
                // Normalize unicode characters that may not render in our font
                let normalized = normalize_unicode(&display_content);
                ui.label(
                    RichText::new(normalized)
                        .color(colors.text)
                        .size(typography::MD),
                );
            }
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
        self.current_model = Some(self.selected_model.display_name().to_string());

        // Get working directory
        let working_dir = std::env::current_dir().ok();

        // Create client based on selected provider, with runtime handle for async spawning
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

    /// Poll the event receiver and update UI state
    #[cfg(not(target_arch = "wasm32"))]
    fn poll_streaming_response(&mut self) {
        let Some(ref receiver) = self.event_receiver else {
            return;
        };

        // Process all available events
        while let Ok(event) = receiver.try_recv() {
            match event {
                AgentEvent::TextDelta(text) => {
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

    /// Poll streaming state (WASM stub)
    #[cfg(target_arch = "wasm32")]
    fn poll_streaming_response(&mut self) {
        // No-op on WASM
    }
}
