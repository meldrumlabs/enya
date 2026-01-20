//! Agent Panel - Claude Code integration for AI-assisted metrics exploration.
//!
//! Provides a chat interface to interact with Claude Code CLI, with streaming
//! responses displayed in real-time. Styled with the Obsidian Glass design system.

use egui::{Color32, CornerRadius, Key, RichText, ScrollArea, Stroke, TextEdit, Vec2};

#[cfg(not(target_arch = "wasm32"))]
use enya_ai::{AcpClient, AgentEvent};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use crate::chat::ChatColors;
use crate::components::pane::time_series_chart::TimeSeriesChart;
use crate::components::pane::{InlineChart, InlineContent, InlineSearchResults, InlineSource};
use crate::components::util::{
    ActivityItem, ActivityType, AiModel, AiProvider, ConversationHandoff, MessageRole,
    ResponseStatus, ScrollShadowConfig, ScrollState, normalize_unicode, render_scroll_shadows,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{truncate_first_line, truncate_path_suffix};
use crate::components::widget::ThinkingIndicator;
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
    /// Inline content blocks (charts, source previews)
    pub inline_blocks: Vec<InlineContent>,
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
    /// Return focus to the viewport (vim h key pressed)
    ReturnFocusToViewport,
    /// Entered input mode (vim i or Enter pressed) - workspace should release vim focus
    EnteredInputMode,
}

/// The agent panel component for AI-assisted chat
#[allow(dead_code)] // Some fields used only in native builds or for future features
pub struct AgentPanel {
    /// Whether the panel is open
    is_open: bool,
    /// Whether this panel has keyboard focus (for vim-style navigation)
    has_focus: bool,
    /// Skip vim key detection for one frame after gaining focus
    /// (prevents immediate key detection from lingering keypresses)
    skip_vim_keys_once: bool,
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
            has_focus: false,
            skip_vim_keys_once: false,
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
            has_focus: false,
            skip_vim_keys_once: false,
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

    /// Get the current provider name as a string
    pub fn provider_name(&self) -> String {
        match self.selected_provider {
            AiProvider::Claude => "Claude".to_string(),
            AiProvider::Codex => "Codex".to_string(),
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

    /// Import a conversation from the agent input bar (handoff).
    ///
    /// Opens the panel and populates it with the existing conversation,
    /// allowing the user to continue in a persistent side panel.
    pub fn import_from_handoff(&mut self, handoff: ConversationHandoff) {
        // Clear existing conversation state
        self.messages.clear();
        self.current_activities.clear();
        self.response_text.clear();
        self.current_status = ResponseStatus::Complete;
        self.is_waiting = false;

        // Add user message from handoff
        if !handoff.query.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::User,
                content: handoff.query,
                is_streaming: false,
                inline_blocks: Vec::new(),
            });
        }

        // Add assistant response from handoff
        if !handoff.response.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::Assistant,
                content: handoff.display_text,
                is_streaming: false,
                inline_blocks: Vec::new(),
            });
        }

        // Import activities from the handoff
        self.current_activities = handoff.activities;

        // Open the panel and scroll to bottom
        self.is_open = true;
        self.scroll_to_bottom = true;
        self.focus_input = true;

        log::info!(
            "Imported conversation handoff: {} messages",
            self.messages.len()
        );
    }

    /// Add activities from command execution to the current activity list.
    ///
    /// This is used to display feedback when agent commands are executed.
    pub fn add_activities(&mut self, activities: Vec<ActivityItem>) {
        self.current_activities.extend(activities);
    }

    /// Add inline content (chart, source, search results) to the last assistant message.
    ///
    /// This is used by the workspace to inject visualizations into the conversation
    /// after parsing agent commands.
    pub fn add_inline_content(&mut self, content: InlineContent) {
        // Find the last assistant message and add the inline content
        if let Some(msg) = self
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
        {
            msg.inline_blocks.push(content);
            self.scroll_to_bottom = true;
            log::info!("Added inline content to agent panel message");
        } else {
            log::warn!("No assistant message found to inject inline content");
        }
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

    /// Set whether this panel has keyboard focus.
    pub fn set_focus(&mut self, focused: bool) {
        // When gaining focus, set flag to skip vim keys for one frame
        // This prevents lingering keypresses (e.g., 'a' from Space+a) from
        // being detected as vim navigation keys immediately
        if focused && !self.has_focus {
            self.skip_vim_keys_once = true;
        }
        self.has_focus = focused;
    }

    /// Check if this panel has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.has_focus
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
    #[profiling::function]
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

    /// Show the panel within a layout hierarchy (participates in layout flow).
    ///
    /// Unlike `show()` which renders as an overlay, this method renders the panel
    /// as a first-class layout participant. The viewport will shrink to accommodate
    /// the panel when it's open, similar to the channels panel on the left.
    #[profiling::function]
    pub fn show_inside(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> AgentPanelResult {
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

        // CRITICAL: When panel has vim focus, clear any egui widget focus FIRST
        // This prevents TextEdit or other widgets from consuming vim navigation keys
        // Must happen BEFORE we check for keyboard input
        if self.has_focus {
            ui.ctx()
                .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
        }

        // Handle keyboard input when panel has vim focus
        let mut return_focus = false;
        let mut enter_input_mode = false;
        if self.has_focus {
            // Skip vim key detection for one frame after gaining focus
            // This prevents lingering keypresses from being detected immediately
            if self.skip_vim_keys_once {
                self.skip_vim_keys_once = false;
                // Still consume any lingering keys so they don't affect other widgets
                ui.ctx().input_mut(|input| {
                    let _ = input.consume_key(egui::Modifiers::NONE, Key::H);
                    let _ = input.consume_key(egui::Modifiers::NONE, Key::A);
                });
            } else {
                // Panel has vim focus - handle vim navigation keys
                // Use consume_key to ensure keys aren't processed by other widgets
                ui.ctx().input_mut(|input| {
                    // Escape or h or left arrow - return focus to viewport
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape)
                        || input.consume_key(egui::Modifiers::NONE, Key::H)
                        || input.consume_key(egui::Modifiers::NONE, Key::ArrowLeft)
                    {
                        return_focus = true;
                    }
                    // i or Enter - enter insert mode (focus the chat input)
                    else if input.consume_key(egui::Modifiers::NONE, Key::I)
                        || input.consume_key(egui::Modifiers::NONE, Key::Enter)
                    {
                        enter_input_mode = true;
                    }
                });
            }
        }

        if return_focus {
            self.has_focus = false;
            result = AgentPanelResult::ReturnFocusToViewport;
        }

        if enter_input_mode {
            self.has_focus = false;
            self.focus_input = true;
            result = AgentPanelResult::EnteredInputMode;
        }

        // Premium left border for visual anchoring (opposite of channels panel's right border)
        let left_border = self.theme.border_subtle().gamma_multiply(0.6);

        // Side panel on the right - participates in layout (viewport shrinks)
        egui::SidePanel::right("agent_panel")
            .resizable(true)
            .default_width(400.0)
            .min_width(300.0)
            .max_width(800.0)
            .frame(self.panel_frame())
            .show_inside(ui, |ui| {
                // Draw left edge highlight for visual anchoring
                let panel_rect = ui.available_rect_before_wrap();
                ui.painter().vline(
                    panel_rect.left(),
                    panel_rect.y_range(),
                    Stroke::new(1.0, left_border),
                );

                self.render_content(ui, ctx);
            });

        if matches!(result, AgentPanelResult::Closed) {
            self.close();
        }

        result
    }

    fn panel_frame(&self) -> egui::Frame {
        // Premium frosted glass style matching channels panel
        // Use accent border when panel has vim focus
        let bg = self.theme.bg_surface();
        let (border_color, border_width) = if self.has_focus {
            (self.theme.accent_primary(), 2.0)
        } else {
            (self.theme.border_subtle(), 1.0)
        };

        egui::Frame::NONE
            .fill(bg)
            .stroke(Stroke::new(border_width, border_color))
            .inner_margin(egui::Margin::same(0))
            // Add left gap for visual separation from viewport (matches channels panel spacing)
            .outer_margin(egui::Margin {
                left: 8,
                ..Default::default()
            })
    }

    /// Get the chat colors helper for the current theme.
    fn colors(&self) -> ChatColors {
        ChatColors::new(self.theme)
    }

    /// Render a subtle divider between sections.
    fn render_divider(&self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        let rect = ui.available_rect_before_wrap();
        let y = rect.top();
        ui.painter().hline(
            (rect.left() + 12.0)..=(rect.right() - 12.0),
            y,
            Stroke::new(1.0, self.colors().divider()),
        );
        ui.add_space(8.0);
    }

    fn render_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let colors = self.colors();
        let accent = self.theme.accent_primary();
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();

        // Premium header with subtle depth
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Provider logo icon
            let logo_size = 18.0;
            match self.selected_provider {
                AiProvider::Claude => {
                    ui.add(
                        egui::Image::new(egui::include_image!("../../../assets/claude.png"))
                            .tint(accent)
                            .max_size(egui::vec2(logo_size, logo_size)),
                    );
                }
                AiProvider::Codex => {
                    ui.add(
                        egui::Image::new(egui::include_image!("../../../assets/openai.png"))
                            .tint(accent)
                            .max_size(egui::vec2(logo_size, logo_size)),
                    );
                }
            }
            ui.add_space(8.0);

            // Title - shows current provider with strong typography
            ui.label(
                RichText::new(self.selected_provider.display_name())
                    .color(text_primary)
                    .size(typography::LG)
                    .strong(),
            );

            // Model selector dropdown - premium styling
            ui.add_space(8.0);
            let is_disabled = self.is_waiting;
            ui.add_enabled_ui(!is_disabled, |ui| {
                // Style the combo box to match the premium theme
                let style = ui.style_mut();
                style.visuals.widgets.inactive.bg_fill = colors.hover_bg();
                style.visuals.widgets.hovered.bg_fill = colors.selection_bg();
                style.visuals.widgets.active.bg_fill = colors.selection_bg();
                style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, colors.divider());
                style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

                egui::ComboBox::from_id_salt("model_selector")
                    .selected_text(
                        RichText::new(self.selected_model.display_name())
                            .color(text_secondary)
                            .size(typography::SM),
                    )
                    .width(100.0)
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
                ui.add_space(16.0);

                // Close button with hover effect
                let close_btn = ui.add(
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::CLOSE)
                            .size(14.0)
                            .color(text_tertiary),
                    )
                    .frame(false),
                );

                if close_btn.hovered() {
                    let rect = close_btn.rect.expand(4.0);
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(4), colors.hover_bg());
                }

                if close_btn.on_hover_text("Close (Esc)").clicked() {
                    self.is_open = false;
                }

                ui.add_space(4.0);

                // Clear conversation button (only show if there are messages)
                if !self.messages.is_empty() {
                    let clear_btn = ui.add(
                        egui::Button::new(
                            RichText::new(egui_nerdfonts::regular::TRASH_CAN_OUTLINE)
                                .size(14.0)
                                .color(text_tertiary),
                        )
                        .frame(false),
                    );

                    if clear_btn.hovered() {
                        let rect = clear_btn.rect.expand(4.0);
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(4), colors.hover_bg());
                    }

                    if clear_btn.on_hover_text("Clear conversation").clicked() {
                        self.messages.clear();
                        self.current_activities.clear();
                        self.response_text.clear();
                    }
                }
            });
        });
        ui.add_space(10.0);

        // Premium divider
        self.render_divider(ui);

        // Chat area (scrollable) with scroll shadows
        let available_height = ui.available_height() - 90.0; // Reserve space for input
        let scroll_output = ScrollArea::vertical()
            .id_salt("agent_chat_scroll")
            .max_height(available_height)
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(8.0);

                if self.messages.is_empty() && self.current_activities.is_empty() {
                    // Empty state - premium and elegant
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);

                        // Icon with subtle accent
                        ui.label(
                            RichText::new(egui_nerdfonts::regular::COMMENT_TEXT)
                                .color(text_tertiary)
                                .size(36.0),
                        );
                        ui.add_space(16.0);

                        // Primary prompt
                        let prompt_text = match self.selected_provider {
                            AiProvider::Claude => "Ask Claude anything",
                            AiProvider::Codex => "Ask Codex anything",
                        };
                        ui.label(
                            RichText::new(prompt_text)
                                .color(text_secondary)
                                .size(typography::LG),
                        );
                        ui.add_space(8.0);

                        // Suggestion with subtle styling
                        ui.label(
                            RichText::new("Try: \"Help me understand this dashboard\"")
                                .color(text_tertiary)
                                .size(typography::MD)
                                .italics(),
                        );

                        ui.add_space(24.0);

                        // Keyboard shortcuts hint
                        ui.label(
                            RichText::new("i  type  •  h  back")
                                .color(text_tertiary)
                                .size(typography::SM),
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

                    // Iterate by index to avoid borrow conflicts with &mut self methods
                    let message_count = self.messages.len();
                    for i in 0..message_count {
                        // Clone the message to avoid borrow conflicts
                        let message = self.messages[i].clone();
                        self.render_message(ui, &message, &colors);
                        ui.add_space(6.0);

                        // Show activities right after the last user message
                        if Some(i) == last_user_idx && !self.current_activities.is_empty() {
                            ui.add_space(4.0);
                            // Clone activities to avoid borrow conflicts
                            let activities: Vec<_> = self.current_activities.clone();
                            for activity in &activities {
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

        // Render scroll shadows for the chat area
        let scroll_state = ScrollState::from_scroll_output(
            scroll_output.content_size,
            scroll_output.inner_rect,
            scroll_output.state.offset,
        );
        let shadow_config = ScrollShadowConfig::default()
            .with_color(self.theme.bg_surface())
            .with_opacity(0.6);
        render_scroll_shadows(ui, scroll_output.inner_rect, scroll_state, shadow_config);

        // Premium input divider
        self.render_divider(ui);

        // Input area - premium styling
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Input field with elevated background and premium corners
            let input_bg = self.theme.bg_elevated();
            egui::Frame::new()
                .fill(input_bg)
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .stroke(Stroke::new(1.0, colors.divider()))
                .show(ui, |ui| {
                    let hint_text = match self.selected_provider {
                        AiProvider::Claude => "Ask Claude...",
                        AiProvider::Codex => "Ask Codex...",
                    };
                    let response = ui.add_sized(
                        Vec2::new(ui.available_width() - 50.0, 22.0),
                        TextEdit::singleline(&mut self.input_text)
                            .hint_text(
                                RichText::new(hint_text)
                                    .color(text_tertiary)
                                    .size(typography::MD),
                            )
                            .frame(false)
                            .font(typography::proportional(typography::MD)),
                    );

                    // Only focus input when explicitly requested AND panel doesn't have vim focus
                    // This allows vim navigation to work by default, like channels panel
                    if self.focus_input {
                        if !self.has_focus {
                            response.request_focus();
                        }
                        // Always clear the flag regardless of whether we focused
                        self.focus_input = false;
                    }

                    // CRITICAL: If panel has vim focus, ensure the TextEdit doesn't have egui focus
                    // Otherwise the TextEdit will consume vim navigation keys (h, j, k, l)
                    if self.has_focus && response.has_focus() {
                        response.surrender_focus();
                        ui.ctx()
                            .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
                    }

                    // When user clicks on text input, release vim focus
                    // (similar to channels panel split view behavior)
                    if response.clicked_by(egui::PointerButton::Primary) {
                        self.has_focus = false;
                    }

                    // Handle Escape when text input is focused to return vim focus to panel
                    // (matches ChatView pattern in channels panel)
                    if response.has_focus() {
                        let mut should_surrender = false;
                        ui.ctx().input_mut(|input| {
                            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                                should_surrender = true;
                            }
                        });
                        if should_surrender {
                            self.has_focus = true;
                            response.surrender_focus();
                            // Also clear global egui focus so vim keys work immediately
                            ui.ctx()
                                .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
                        }
                    }

                    // Handle Enter to send
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        let can_send = !self.input_text.trim().is_empty() && !self.is_waiting;
                        if can_send {
                            self.send_message(ctx);
                        }
                    }
                });

            // Send or Stop button - premium styling with hover effect
            ui.add_space(8.0);

            if self.is_waiting {
                // Stop button when waiting for response
                let stop_btn = ui.add(
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::STOP_CIRCLE_OUTLINE)
                            .size(16.0)
                            .color(self.theme.semantic_error()),
                    )
                    .frame(false),
                );

                if stop_btn.hovered() {
                    let rect = stop_btn.rect.expand(4.0);
                    ui.painter()
                        .rect_filled(rect, CornerRadius::same(4), colors.hover_bg());
                }

                if stop_btn.on_hover_text("Stop generation").clicked() {
                    self.cancel_request();
                }
            } else {
                // Send button
                let can_send = !self.input_text.trim().is_empty();
                let send_color = if can_send { accent } else { text_tertiary };

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
            }

            ui.add_space(10.0);
        });
        ui.add_space(8.0);
    }

    fn render_message(&mut self, ui: &mut egui::Ui, message: &ChatMessage, colors: &ChatColors) {
        let text_tertiary = self.theme.text_tertiary();
        let accent = self.theme.accent_primary();

        let (role_label, role_color, msg_bg, show_accent_bar) = match message.role {
            MessageRole::User => ("You", accent, colors.own_message_bg(), false),
            MessageRole::Assistant => (
                self.selected_provider.display_name(),
                accent,
                colors.agent_message_bg(),
                true, // Show left accent bar for AI messages
            ),
            MessageRole::System => ("System", text_tertiary, Color32::TRANSPARENT, false),
        };

        // Full-width message row with proper padding
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            ui.vertical(|ui| {
                // Role label with icon and copy button
                let header_response = ui.horizontal(|ui| {
                    match message.role {
                        MessageRole::User => {
                            ui.label(
                                RichText::new(egui_nerdfonts::regular::ACCOUNT)
                                    .color(role_color)
                                    .size(typography::SM),
                            );
                        }
                        MessageRole::Assistant => {
                            // Use provider logo for assistant messages
                            let logo_size = typography::SM;
                            match self.selected_provider {
                                AiProvider::Claude => {
                                    ui.add(
                                        egui::Image::new(egui::include_image!(
                                            "../../../assets/claude.png"
                                        ))
                                        .tint(role_color)
                                        .max_size(egui::vec2(logo_size, logo_size)),
                                    );
                                }
                                AiProvider::Codex => {
                                    ui.add(
                                        egui::Image::new(egui::include_image!(
                                            "../../../assets/openai.png"
                                        ))
                                        .tint(role_color)
                                        .max_size(egui::vec2(logo_size, logo_size)),
                                    );
                                }
                            }
                        }
                        MessageRole::System => {
                            ui.label(
                                RichText::new(egui_nerdfonts::regular::INFORMATION)
                                    .color(role_color)
                                    .size(typography::SM),
                            );
                        }
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(role_label)
                            .color(role_color)
                            .size(typography::SM)
                            .strong(),
                    );
                });

                // Show copy button on hover (to the right of the header)
                let header_rect = header_response.response.rect;
                let hover_area = header_rect.expand2(egui::vec2(100.0, 4.0));
                let is_hovering = ui.rect_contains_pointer(hover_area);

                if is_hovering && !message.content.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(header_rect.width() + 8.0);
                        let copy_btn = ui.add(
                            egui::Button::new(
                                RichText::new(egui_nerdfonts::regular::CONTENT_COPY)
                                    .size(12.0)
                                    .color(text_tertiary),
                            )
                            .frame(false),
                        );

                        if copy_btn.hovered() {
                            let rect = copy_btn.rect.expand(3.0);
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(3),
                                colors.hover_bg(),
                            );
                        }

                        if copy_btn.on_hover_text("Copy message").clicked() {
                            ui.ctx().copy_text(message.content.clone());
                        }
                    });
                }
                ui.add_space(4.0);

                // Message content with premium styling
                let content_response = egui::Frame::new()
                    .fill(msg_bg)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width() - 32.0);
                        self.render_message_content(ui, message, colors);
                    });

                // Draw accent bar for assistant messages
                if show_accent_bar {
                    let rect = content_response.response.rect;
                    let accent_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left(), rect.top() + 4.0),
                        egui::vec2(3.0, rect.height() - 8.0),
                    );
                    ui.painter()
                        .rect_filled(accent_rect, CornerRadius::same(2), accent);
                }
            });
        });
    }

    fn render_message_content(
        &mut self,
        ui: &mut egui::Ui,
        message: &ChatMessage,
        colors: &ChatColors,
    ) {
        let text_primary = self.theme.text_primary();

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
                        .color(text_primary)
                        .size(typography::MD),
                );
            }
        }

        // Render inline content blocks (charts, source, search results)
        for block in &message.inline_blocks {
            ui.add_space(8.0);
            match block {
                InlineContent::Chart(chart) => {
                    self.render_inline_chart(ui, chart, colors);
                }
                InlineContent::Source(source) => {
                    self.render_inline_source(ui, source, colors);
                }
                InlineContent::SearchResults(results) => {
                    self.render_inline_search_results(ui, results, colors);
                }
            }
        }

        // Amp-style thinking indicator with animated pulsing dots
        if message.is_streaming {
            ui.add_space(8.0);

            // Handle start_time type difference between native and WASM
            #[cfg(not(target_arch = "wasm32"))]
            let start_time = self.request_start_time;
            #[cfg(target_arch = "wasm32")]
            let start_time: Option<crate::util::Instant> = None;

            ThinkingIndicator::new(self.theme)
                .with_start_time(start_time)
                .with_status_and_activities(self.current_status, &self.current_activities)
                .show(ui);
        }
    }

    /// Render an inline time series chart within a message.
    fn render_inline_chart(&mut self, ui: &mut egui::Ui, chart: &InlineChart, colors: &ChatColors) {
        let chart_height = chart.height.unwrap_or(120.0);
        let accent = self.theme.accent_primary();
        let text_primary = self.theme.text_primary();
        let text_tertiary = self.theme.text_tertiary();

        // Chart container with premium styling
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                // Title header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(egui_nerdfonts::regular::CHART_LINE)
                            .color(accent)
                            .size(14.0),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&chart.title)
                            .color(text_primary)
                            .size(typography::SM)
                            .strong(),
                    );
                });

                ui.add_space(6.0);

                if chart.series.is_empty() {
                    ui.label(
                        RichText::new("No data")
                            .color(text_tertiary)
                            .size(typography::SM),
                    );
                } else {
                    // Create a TimeSeriesChart for consistent styling
                    let mut ts_chart = TimeSeriesChart::new(&chart.title);
                    ts_chart.set_theme(self.theme);
                    ts_chart.set_show_legend(false); // Compact mode - no legend

                    // Add all series from the inline chart
                    for series in &chart.series {
                        ts_chart.add_series(series.clone());
                    }

                    // Render within constrained height
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), chart_height),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ts_chart.show(ui);
                        },
                    );
                }
            });
    }

    /// Render an inline source code preview within a message.
    fn render_inline_source(&self, ui: &mut egui::Ui, source: &InlineSource, colors: &ChatColors) {
        let accent = self.theme.accent_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();

        // Source container with premium styling
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                // Header with file path and line number
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(egui_nerdfonts::regular::FILE_CODE)
                            .color(accent)
                            .size(14.0),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("{}:{}", source.file_path, source.line))
                            .color(accent)
                            .size(typography::SM)
                            .strong(),
                    );

                    // Language badge with premium styling
                    if !source.language.is_empty() {
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(colors.hover_bg())
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(&source.language)
                                        .color(text_secondary)
                                        .size(typography::XS),
                                );
                            });
                    }
                });

                ui.add_space(8.0);

                // Line number width
                let max_line = source.start_line + source.lines.len();
                let line_num_width = format!("{max_line}").len();

                // Source lines with line numbers and tree-sitter syntax highlighting
                for (i, line) in source.lines.iter().enumerate() {
                    let line_num = source.start_line + i;
                    let is_target = line_num == source.line;

                    let (line_color, bg_color) = if is_target {
                        (palette::semantic::WARNING, self.theme.highlight_line())
                    } else {
                        (text_tertiary, Color32::TRANSPARENT)
                    };

                    let response = ui.horizontal(|ui| {
                        // Line number
                        let prefix = if is_target {
                            format!("{line_num:>line_num_width$} →")
                        } else {
                            format!("{line_num:>line_num_width$}  ")
                        };
                        ui.label(
                            RichText::new(prefix)
                                .color(line_color)
                                .font(typography::monospace(typography::SM)),
                        );
                        ui.add_space(4.0);

                        // Code line with tree-sitter syntax highlighting
                        let job = source
                            .highlight_data
                            .highlight_line(line_num, line, self.theme);
                        ui.label(job);
                    });

                    // Draw background for target line
                    if is_target {
                        let rect = response.response.rect.expand2(egui::vec2(2.0, 1.0));
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(4), bg_color);
                    }
                }
            });
    }

    /// Render inline search results within a message.
    fn render_inline_search_results(
        &self,
        ui: &mut egui::Ui,
        results: &InlineSearchResults,
        colors: &ChatColors,
    ) {
        use egui_nerdfonts::regular;

        let accent = self.theme.accent_primary();
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();

        // Search results container with premium styling
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                // Header with search query
                ui.horizontal(|ui| {
                    ui.label(RichText::new(regular::MAGNIFY).color(accent).size(14.0));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("Search: \"{}\"", results.query))
                            .color(text_primary)
                            .size(typography::SM)
                            .strong(),
                    );

                    // Filter badge with premium styling
                    if results.filter != "all" {
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(colors.hover_bg())
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(&results.filter)
                                        .color(text_secondary)
                                        .size(typography::XS),
                                );
                            });
                    }

                    // Result count
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} results", results.results.len()))
                                .color(text_tertiary)
                                .size(typography::XS),
                        );
                    });
                });

                ui.add_space(8.0);

                // Show up to 5 results in compact format with hover states
                let max_results = 5;
                for (i, result) in results.results.iter().take(max_results).enumerate() {
                    if i > 0 {
                        ui.add_space(4.0);
                    }

                    ui.horizontal(|ui| {
                        // Kind icon
                        let icon = match result.kind.as_str() {
                            "metric" => regular::CHART_LINE,
                            "alert" => regular::ALERT,
                            "commit" => regular::SOURCE_COMMIT,
                            _ => regular::FILE_DOCUMENT,
                        };
                        ui.label(RichText::new(icon).color(text_secondary).size(12.0));
                        ui.add_space(6.0);

                        // Name
                        ui.label(
                            RichText::new(&result.name)
                                .color(text_primary)
                                .size(typography::SM),
                        );

                        // File path (truncated)
                        if !result.file_path.is_empty() {
                            ui.add_space(6.0);
                            let path_display = if result.file_path.len() > 30 {
                                format!("...{}", &result.file_path[result.file_path.len() - 27..])
                            } else {
                                result.file_path.clone()
                            };
                            ui.label(
                                RichText::new(path_display)
                                    .color(text_tertiary)
                                    .size(typography::XS),
                            );
                        }
                    });
                }

                // "More results" indicator
                if results.results.len() > max_results {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "... and {} more",
                            results.results.len() - max_results
                        ))
                        .color(text_tertiary)
                        .size(typography::XS)
                        .italics(),
                    );
                }
            });
    }

    /// Render an activity item with premium row styling (matching channels panel).
    fn render_activity(&self, ui: &mut egui::Ui, activity: &ActivityItem, _colors: &ChatColors) {
        use egui_nerdfonts::regular;

        let accent = self.theme.accent_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();

        // Get icon and color based on activity type
        let (icon, label, summary, icon_color) = match &activity.activity_type {
            ActivityType::Thinking(text) => {
                (regular::LIGHTBULB, "Thinking", text.clone(), text_secondary)
            }
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
                (icon, tool.as_str(), summary.clone(), accent)
            }
            ActivityType::EditorAction {
                description,
                success,
            } => {
                let icon = if *success {
                    regular::CHECK_CIRCLE
                } else {
                    regular::CLOSE_CIRCLE
                };
                let color = if *success {
                    palette::semantic::SUCCESS
                } else {
                    palette::semantic::ERROR
                };
                (icon, "Action", description.clone(), color)
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

        // Premium row with hover background (matching channels panel style)
        let row_height = 28.0;
        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_height),
            egui::Sense::hover(),
        );

        let content_rect = rect.shrink2(egui::vec2(16.0, 2.0));

        // Draw activity content
        let icon_pos = content_rect.left_center() + Vec2::new(4.0, 0.0);
        let label_pos = content_rect.left_center() + Vec2::new(26.0, 0.0);

        // Icon or braille spinner
        if activity.in_progress {
            // Draw braille spinner at icon position
            const BRAILLE_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let time = ui.ctx().input(|i| i.time);
            let frame_index = ((time * 10.0) as usize) % BRAILLE_FRAMES.len();
            let spinner_char = BRAILLE_FRAMES[frame_index];
            ui.painter().text(
                icon_pos,
                egui::Align2::LEFT_CENTER,
                spinner_char.to_string(),
                typography::proportional(typography::SM),
                accent,
            );
        } else {
            ui.painter().text(
                icon_pos,
                egui::Align2::LEFT_CENTER,
                icon,
                typography::proportional(typography::SM),
                icon_color,
            );
        }

        // Label
        ui.painter().text(
            label_pos,
            egui::Align2::LEFT_CENTER,
            label,
            typography::proportional(typography::SM),
            text_secondary,
        );

        // Summary text (if present)
        if !summary.is_empty() {
            let summary_pos = label_pos + Vec2::new(label.len() as f32 * 7.0 + 8.0, 0.0);
            ui.painter().text(
                summary_pos,
                egui::Align2::LEFT_CENTER,
                &summary,
                typography::proportional(typography::SM),
                text_tertiary,
            );
        }
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

    /// Cancel the current request
    fn cancel_request(&mut self) {
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
