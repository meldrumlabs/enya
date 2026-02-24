//! Agent Input Bar - A persistent input bar for Agent mode interactions.
//!
//! This component provides a lightweight input surface for AI agent interactions,
//! appearing above the status line when Agent mode is active. It supports:
//! - Natural language input with real-time suggestions
//! - Quick command keys (w, y, c, r, e, f, s, h)
//! - Response display with expandable content
//! - Context awareness (selected panes from visual mode)
//! - Direct AI integration (no side panel dependency)

use egui::{Color32, Key, RichText, ScrollArea, TextEdit};
use egui_tiles::TileId;

#[cfg(not(target_arch = "wasm32"))]
use enya_ai::{AgentConfig, AgentEvent, PersistentAcpClient};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::overlay::AgentCommand;
use crate::components::overlay::MentionPopup;
use crate::components::overlay::SlashCommandPopup;
#[cfg(not(target_arch = "wasm32"))]
use crate::components::overlay::{parse_commands, strip_command_blocks};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::ActivityType;
use crate::components::util::finder_utils::{OverlayColors, OverlayStyle};
use crate::components::util::{ActivityItem, ConversationHandoff, HandoffContextPane};

/// State of the agent input bar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentInputState {
    /// Ready for input, showing placeholder and quick key hints
    #[default]
    Ready,
    /// User is typing in the input field
    Typing,
    /// AI is processing the request
    Processing,
    /// Showing AI response
    Response,
}

/// A quick command that can be triggered with a single key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickCommand {
    /// w - What's wrong? (triage)
    WhatsWrong,
    /// y - Why? (root cause)
    Why,
    /// c - Compare (to baseline)
    Compare,
    /// r - Related (show correlated metrics)
    Related,
    /// e - Explain (focused element)
    Explain,
    /// f - Fix (remediation suggestions)
    Fix,
    /// s - Summarize (incident summary)
    Summarize,
    /// h - History (past similar incidents)
    History,
}

impl QuickCommand {
    /// Get the prompt text for this command
    pub fn prompt(&self) -> &'static str {
        match self {
            Self::WhatsWrong => "What's wrong? Analyze the current state and identify any issues.",
            Self::Why => "Why is this happening? Investigate the root cause.",
            Self::Compare => "Compare the current state to the baseline or a previous time period.",
            Self::Related => "Show related metrics that might be correlated.",
            Self::Explain => "Explain what this metric or chart is showing.",
            Self::Fix => "How can this be fixed? Suggest remediation steps.",
            Self::Summarize => "Summarize the current incident or situation.",
            Self::History => "Has this happened before? Show historical patterns.",
        }
    }

    /// Get the display label for this command
    pub fn label(&self) -> &'static str {
        match self {
            Self::WhatsWrong => "What's wrong?",
            Self::Why => "Why?",
            Self::Compare => "Compare",
            Self::Related => "Related",
            Self::Explain => "Explain",
            Self::Fix => "Fix",
            Self::Summarize => "Summarize",
            Self::History => "History",
        }
    }
}

/// Result from showing the agent input bar
#[derive(Debug, Clone, Default)]
pub struct AgentInputBarResult {
    /// Whether the user wants to exit agent mode
    pub exit_requested: bool,
    /// A query to send to the AI agent
    pub query: Option<String>,
    /// Quick command triggered
    pub quick_command: Option<QuickCommand>,
    /// Request to add a pane to context
    pub add_pane_to_context: bool,
    /// Request to remove focused pane from context
    pub remove_pane_from_context: bool,
    /// Request to clear context
    pub clear_context: bool,
    /// Undo last action
    pub undo_requested: bool,
    /// Enya commands parsed from AI response (e.g., create_pane, set_time_range)
    pub commands: Vec<AgentCommand>,
    /// Request to open the conversation in a full agent pane (Tab key handoff)
    pub open_in_pane: bool,
}

/// Context pane information for display
#[derive(Debug, Clone)]
pub struct ContextPane {
    /// Tile ID for the pane
    pub tile_id: TileId,
    /// Display name
    pub name: String,
}

/// Agent Input Bar component
pub struct AgentInputBar {
    /// Current state
    state: AgentInputState,
    /// Input text
    input: String,
    /// Whether the input field should be focused
    focus_input: bool,
    /// Current theme
    theme: AppTheme,
    /// Panes in context (from visual mode selection)
    context_panes: Vec<ContextPane>,
    /// Estimated system context size in characters (for token usage indicator)
    context_char_count: usize,
    /// Current AI provider name (e.g., "Claude", "Codex")
    provider_name: String,
    /// Last query sent to the AI (for handoff)
    last_query: String,
    /// Last response text (for display)
    response_text: String,
    /// Display text (response with command blocks stripped)
    display_text: String,
    /// Whether the response is expanded (for long responses)
    response_expanded: bool,
    /// Processing status message
    processing_status: String,
    /// Processing elapsed time
    processing_start: Option<crate::util::Instant>,
    /// Current activities (tool use, thinking, etc.)
    activities: Vec<ActivityItem>,
    /// Last action that can be undone
    can_undo: bool,
    /// Pending commands parsed from AI response
    pending_commands: Vec<AgentCommand>,
    /// Number of commands applied in the last response (for badge display)
    applied_command_count: usize,
    /// Inline content blocks generated by commands (for handoff to agent panel)
    inline_blocks: Vec<crate::components::InlineContent>,
    /// @ mention popup state for metric selection
    mention_popup: MentionPopup,
    /// / slash command popup state
    slash_command_popup: SlashCommandPopup,
    /// Previous input text (for detecting @ insertion)
    prev_input: String,
    /// Whether to move cursor to end of input on next frame
    cursor_to_end: bool,
    /// Last text edit rect (for positioning popups above cursor)
    text_edit_rect: Option<egui::Rect>,
    /// Previous state (for transition animations)
    prev_state: AgentInputState,
    /// Transition progress (0.0 = just changed, 1.0 = fully settled)
    transition_t: f32,
    /// Event receiver for streaming AI responses (native only)
    #[cfg(not(target_arch = "wasm32"))]
    event_receiver: Option<Receiver<AgentEvent>>,
    /// Persistent ACP client that keeps a warm subprocess across prompts
    #[cfg(not(target_arch = "wasm32"))]
    persistent_client: Option<PersistentAcpClient>,
    /// Selected AI model ID (e.g., "claude-sonnet-4-5-20250514")
    #[cfg(not(target_arch = "wasm32"))]
    selected_model: Option<String>,
    /// Current AI provider (for detecting provider changes)
    #[cfg(not(target_arch = "wasm32"))]
    selected_provider: crate::components::util::AiProvider,
    /// Tokio runtime handle (kept for recreating the persistent client on provider change)
    #[cfg(not(target_arch = "wasm32"))]
    runtime_handle: Option<tokio::runtime::Handle>,
}

impl Default for AgentInputBar {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentInputBar {
    /// Create a new agent input bar
    pub fn new() -> Self {
        Self {
            state: AgentInputState::Ready,
            input: String::new(),
            focus_input: true,
            theme: AppTheme::default(),
            context_panes: Vec::new(),
            context_char_count: 0,
            provider_name: "Claude".to_string(),
            last_query: String::new(),
            response_text: String::new(),
            display_text: String::new(),
            response_expanded: false,
            processing_status: String::new(),
            processing_start: None,
            activities: Vec::new(),
            can_undo: false,
            pending_commands: Vec::new(),
            applied_command_count: 0,
            inline_blocks: Vec::new(),
            mention_popup: MentionPopup::new(),
            slash_command_popup: SlashCommandPopup::new(),
            prev_input: String::new(),
            cursor_to_end: false,
            text_edit_rect: None,
            prev_state: AgentInputState::Ready,
            transition_t: 1.0,
            #[cfg(not(target_arch = "wasm32"))]
            event_receiver: None,
            #[cfg(not(target_arch = "wasm32"))]
            persistent_client: None,
            #[cfg(not(target_arch = "wasm32"))]
            selected_model: None,
            #[cfg(not(target_arch = "wasm32"))]
            selected_provider: crate::components::util::AiProvider::Claude,
            #[cfg(not(target_arch = "wasm32"))]
            runtime_handle: None,
        }
    }

    /// Create a new agent input bar with a tokio runtime handle
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_with_runtime(runtime_handle: &tokio::runtime::Handle) -> Self {
        Self {
            state: AgentInputState::Ready,
            input: String::new(),
            focus_input: true,
            theme: AppTheme::default(),
            context_panes: Vec::new(),
            context_char_count: 0,
            provider_name: "Claude".to_string(),
            last_query: String::new(),
            response_text: String::new(),
            display_text: String::new(),
            response_expanded: false,
            processing_status: String::new(),
            processing_start: None,
            activities: Vec::new(),
            can_undo: false,
            pending_commands: Vec::new(),
            applied_command_count: 0,
            inline_blocks: Vec::new(),
            mention_popup: MentionPopup::new(),
            slash_command_popup: SlashCommandPopup::new(),
            prev_input: String::new(),
            cursor_to_end: false,
            text_edit_rect: None,
            prev_state: AgentInputState::Ready,
            transition_t: 1.0,
            event_receiver: None,
            persistent_client: None, // Created on first set_provider_and_model call
            selected_model: None,
            selected_provider: crate::components::util::AiProvider::Claude,
            runtime_handle: Some(runtime_handle.clone()),
        }
    }

    /// Set the current theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.slash_command_popup.set_theme(theme);
        self.mention_popup.set_theme(theme);
    }

    /// Set the current AI provider name (e.g., "Claude", "Codex")
    pub fn set_provider_name(&mut self, name: &str) {
        self.provider_name = name.to_string();
    }

    /// Update the AI provider and model.
    ///
    /// If the provider changes, the persistent client is recreated with the
    /// new agent config (e.g., switching from Claude Code to Codex).
    /// The model is passed to each prompt so the ACP agent uses the right one.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_provider_and_model(
        &mut self,
        provider: crate::components::util::AiProvider,
        model: Option<String>,
    ) {
        use crate::components::util::AiProvider;

        log::debug!(
            "set_provider_and_model: provider={}, model={:?}",
            provider.display_name(),
            model,
        );

        self.provider_name = provider.display_name().to_string();

        // Create or recreate persistent client when provider changes
        // (also creates the initial client on first call after startup)
        if provider != self.selected_provider || self.persistent_client.is_none() {
            let new_config = match provider {
                AiProvider::Claude => AgentConfig::claude_code(),
                AiProvider::Codex => AgentConfig::codex(),
            };

            if let Some(handle) = &self.runtime_handle {
                log::debug!(
                    "creating persistent ACP client for provider: {}",
                    provider.display_name()
                );
                self.persistent_client = Some(PersistentAcpClient::new(new_config, handle));
            }

            self.selected_provider = provider;
        }

        self.selected_model = model;
    }

    /// Pre-warm the agent subprocess so the first prompt is fast.
    ///
    /// Call this when the user enters agent mode (before they type anything).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn warmup(&self) {
        if let Some(client) = &self.persistent_client {
            client.warmup();
        }
    }

    /// Set available metrics for @ mention autocomplete
    pub fn set_available_metrics(&mut self, metrics: Vec<String>) {
        self.mention_popup.set_metrics(metrics.clone());
        self.slash_command_popup.set_available_metrics(metrics);
    }

    /// Set the estimated system context size in characters (for token usage indicator)
    pub fn set_context_char_count(&mut self, chars: usize) {
        self.context_char_count = chars;
    }

    /// Open the slash command popup
    pub fn open_slash_commands(&mut self) {
        self.slash_command_popup.open();
    }

    /// Check if the slash command popup is open
    pub fn is_slash_commands_open(&self) -> bool {
        self.slash_command_popup.is_open()
    }

    /// Set the context panes (from visual mode selection)
    pub fn set_context_panes(&mut self, panes: Vec<ContextPane>) {
        self.context_panes = panes;
    }

    /// Add a pane to context
    pub fn add_context_pane(&mut self, pane: ContextPane) {
        if !self.context_panes.iter().any(|p| p.tile_id == pane.tile_id) {
            self.context_panes.push(pane);
        }
    }

    /// Remove a pane from context
    pub fn remove_context_pane(&mut self, tile_id: TileId) {
        self.context_panes.retain(|p| p.tile_id != tile_id);
    }

    /// Clear all context panes
    pub fn clear_context(&mut self) {
        self.context_panes.clear();
    }

    /// Get the context pane tile IDs
    pub fn context_pane_ids(&self) -> Vec<TileId> {
        self.context_panes.iter().map(|p| p.tile_id).collect()
    }

    /// Take any pending commands that were parsed from AI responses.
    ///
    /// This allows the caller to process commands immediately after poll()
    /// rather than waiting for show() to be called.
    pub fn take_pending_commands(&mut self) -> Vec<AgentCommand> {
        std::mem::take(&mut self.pending_commands)
    }

    /// Get the display text (response with command blocks stripped).
    ///
    /// Used to check if the agent sent explanatory text along with commands.
    pub fn display_text(&self) -> &str {
        &self.display_text
    }

    /// Export the current conversation state for handoff to an agent pane.
    ///
    /// This is used when the user presses Tab in response state to continue
    /// the conversation in a full agent pane. Returns `None` if there's no
    /// conversation to export (e.g., not in response state or empty query).
    pub fn export_for_handoff(&self) -> Option<ConversationHandoff> {
        // Only allow handoff from response state with actual content
        if self.state != AgentInputState::Response {
            return None;
        }

        if self.last_query.is_empty() && self.response_text.is_empty() {
            return None;
        }

        Some(ConversationHandoff {
            query: self.last_query.clone(),
            response: self.response_text.clone(),
            display_text: self.display_text.clone(),
            context_panes: self
                .context_panes
                .iter()
                .map(|p| HandoffContextPane {
                    tile_id: p.tile_id,
                    name: p.name.clone(),
                })
                .collect(),
            activities: self.activities.clone(),
            inline_blocks: self.inline_blocks.clone(),
        })
    }

    /// Add inline content to be included on handoff.
    ///
    /// Since the input bar doesn't render inline content (it's a compact overlay),
    /// this content is stored and transferred to the agent panel on handoff.
    pub fn add_inline_content(&mut self, content: crate::components::InlineContent) {
        self.inline_blocks.push(content);
        log::debug!("Added inline content to agent input bar for handoff");
    }

    /// Reset to ready state (for entering agent mode)
    pub fn reset(&mut self) {
        self.state = AgentInputState::Ready;
        self.input.clear();
        self.prev_input.clear();
        self.focus_input = true;
        self.last_query.clear();
        self.response_text.clear();
        self.display_text.clear();
        self.response_expanded = false;
        self.processing_status.clear();
        self.processing_start = None;
        self.activities.clear();
        self.can_undo = false;
        self.pending_commands.clear();
        self.applied_command_count = 0;
        self.inline_blocks.clear();
        self.prev_state = AgentInputState::Ready;
        self.transition_t = 1.0;
        self.mention_popup.close();
        self.slash_command_popup.close();
    }

    /// Reset and start in typing mode (for direct entry via `aa`)
    pub fn reset_to_typing(&mut self) {
        self.reset();
        self.state = AgentInputState::Typing;
    }

    /// Start processing a request
    pub fn start_processing(&mut self, status: &str) {
        self.state = AgentInputState::Processing;
        self.processing_status = status.to_string();
        self.processing_start = Some(crate::util::Instant::now());
        self.activities.clear();
    }

    /// Update processing status
    pub fn update_processing_status(&mut self, status: &str) {
        self.processing_status = status.to_string();
    }

    /// Add an activity item
    pub fn add_activity(&mut self, activity: ActivityItem) {
        self.activities.push(activity);
    }

    /// Set activities (replaces existing)
    pub fn set_activities(&mut self, activities: Vec<ActivityItem>) {
        self.activities = activities;
    }

    /// Set the response and transition to response state
    pub fn set_response(&mut self, response: String, can_undo: bool) {
        self.state = AgentInputState::Response;
        self.response_text = response;
        self.can_undo = can_undo;
        self.processing_start = None;
        self.activities.clear();
    }

    /// Clear response and return to ready state
    pub fn clear_response(&mut self) {
        self.state = AgentInputState::Ready;
        self.response_text.clear();
        self.display_text.clear();
        self.response_expanded = false;
        self.can_undo = false;
        self.input.clear();
        self.focus_input = true;
    }

    /// Get the current state
    pub fn state(&self) -> AgentInputState {
        self.state
    }

    /// Tick the state transition animation.
    ///
    /// Detects state changes and returns the current fade-in alpha (0.0→1.0).
    /// Call once per frame before rendering content.
    fn tick_transition(&mut self, ctx: &egui::Context) -> f32 {
        if self.state != self.prev_state {
            self.prev_state = self.state;
            self.transition_t = 0.0;
        }
        if self.transition_t < 1.0 {
            // Animate from 0→1 over ~150ms
            let dt = ctx.input(|i| i.stable_dt).min(0.05);
            self.transition_t = (self.transition_t + dt / 0.15).min(1.0);
            ctx.request_repaint();
        }
        // Ease-out cubic for smooth deceleration
        let t = self.transition_t;
        1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
    }

    /// Show the agent input bar
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) -> AgentInputBarResult {
        let transition_alpha = self.tick_transition(ui.ctx());
        let mut result = AgentInputBarResult::default();
        let colors = OverlayColors::new(self.theme);
        let style = OverlayStyle::frosted_glass(self.theme);

        // Calculate height based on state
        let base_height = 44.0;
        let expanded_height = if self.response_expanded {
            200.0
        } else {
            base_height
        };
        // Extra height for multi-line input content
        let input_lines = self.input.lines().count().max(1);
        let multiline_extra = if input_lines > 1 {
            (input_lines - 1) as f32 * 16.0
        } else {
            0.0
        };

        let height = match self.state {
            AgentInputState::Ready => base_height + multiline_extra,
            AgentInputState::Typing => {
                // Only add extra height for suggestions when there's input
                if self.input.is_empty() {
                    base_height
                } else {
                    base_height + 80.0 + multiline_extra
                }
            }
            AgentInputState::Processing => base_height + 24.0,
            AgentInputState::Response => {
                if self.response_text.len() > 100 || self.response_text.contains('\n') {
                    expanded_height + 60.0
                } else {
                    base_height + 20.0
                }
            }
        };

        // Accent for Agent mode
        let accent = self.theme.accent_primary();

        // Inner glow color for premium glass effect (Custom variant handles plugin colors internally)
        let inner_glow = self.theme.overlay_highlight();

        // Subtle bottom shadow glow (accent)
        let bottom_glow = self
            .theme
            .backdrop_accent_glow()
            .unwrap_or(Color32::TRANSPARENT);

        // Max width for compact, centered input bar (OpenCode-style)
        let max_width = 680.0;
        let available_width = ui.available_width();
        let bar_width = available_width.min(max_width);

        // Create frame with premium glass styling
        let frame = style
            .frame()
            .inner_margin(egui::Margin::symmetric(16, 10))
            .corner_radius(14.0) // Slightly more rounded for premium feel
            .shadow(egui::epaint::Shadow {
                offset: [0, 6],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(60), // Subtle shadow for depth
            });

        // Center the input bar horizontally
        let centered_response = ui.allocate_ui_with_layout(
            egui::vec2(available_width, height + 30.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                let frame_response = frame.show(ui, |ui| {
                    ui.set_min_height(height);
                    ui.set_width(bar_width);

                    ui.vertical(|ui| {
                        // Top row: Provider badge + Context + Input
                        ui.horizontal(|ui| {
                            // Provider badge with accent
                            let badge_bg = accent.gamma_multiply(0.18);

                            egui::Frame::new()
                                .fill(badge_bg)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        // Show provider logo based on name
                                        let logo_size = typography::SM + 2.0;
                                        let provider_lower = self.provider_name.to_lowercase();
                                        if provider_lower.contains("claude") {
                                            ui.add(
                                                egui::Image::new(egui::include_image!(
                                                    "../../../assets/claude.png"
                                                ))
                                                .tint(accent)
                                                .max_size(egui::vec2(logo_size, logo_size)),
                                            );
                                        } else if provider_lower.contains("openai")
                                            || provider_lower.contains("codex")
                                            || provider_lower.contains("gpt")
                                        {
                                            ui.add(
                                                egui::Image::new(egui::include_image!(
                                                    "../../../assets/openai.png"
                                                ))
                                                .tint(accent)
                                                .max_size(egui::vec2(logo_size, logo_size)),
                                            );
                                        }
                                        ui.label(
                                            RichText::new(&self.provider_name)
                                                .color(accent)
                                                .size(typography::SM)
                                                .strong(),
                                        );
                                    });
                                });

                            ui.add_space(12.0);

                            // Context panes indicator (if any)
                            if !self.context_panes.is_empty() {
                                let context_text = if self.context_panes.len() == 1 {
                                    self.context_panes[0].name.clone()
                                } else if self.context_panes.len() <= 3 {
                                    self.context_panes
                                        .iter()
                                        .map(|p| p.name.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                } else {
                                    format!(
                                        "{}, +{} more",
                                        self.context_panes[0].name,
                                        self.context_panes.len() - 1
                                    )
                                };

                                // Context badge
                                let ctx_badge_bg = colors.badge_bg.gamma_multiply(0.9);

                                egui::Frame::new()
                                    .fill(ctx_badge_bg)
                                    .corner_radius(4.0)
                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(semantic_icons::nav::PANES)
                                                    .color(colors.muted_text)
                                                    .size(typography::SM),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(&context_text)
                                                    .color(colors.muted_text)
                                                    .size(typography::SM),
                                            );
                                        });
                                    });

                                ui.add_space(4.0);

                                // Minimal context buttons
                                let btn_color = colors.faint_text;
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("+")
                                                .color(btn_color)
                                                .size(typography::SM),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Add focused pane")
                                    .clicked()
                                {
                                    result.add_pane_to_context = true;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("−")
                                                .color(btn_color)
                                                .size(typography::SM),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Remove focused pane")
                                    .clicked()
                                {
                                    result.remove_pane_from_context = true;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("×")
                                                .color(btn_color)
                                                .size(typography::SM),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Clear context")
                                    .clicked()
                                {
                                    result.clear_context = true;
                                }

                                ui.add_space(8.0);

                                // Subtle separator
                                ui.label(
                                    RichText::new("·")
                                        .color(colors.separator)
                                        .size(typography::MD),
                                );
                                ui.add_space(8.0);
                            }

                            // Intercept bare Enter before TextEdit to use for submission.
                            // Shift+Enter will pass through to multiline TextEdit as newline.
                            let mut enter_to_submit = false;
                            if matches!(
                                self.state,
                                AgentInputState::Typing | AgentInputState::Ready
                            ) {
                                ui.ctx().input_mut(|input| {
                                    if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                                        enter_to_submit = true;
                                    }
                                });
                            }

                            // Fade-in on state transitions
                            ui.set_opacity(transition_alpha);

                            // Track if stop was requested
                            let mut stop_requested = false;

                            // Main content based on state
                            match self.state {
                                AgentInputState::Ready => {
                                    self.show_ready_state(ui, &colors, &mut result);
                                }
                                AgentInputState::Typing => {
                                    self.show_typing_state(ui, &colors, &mut result);
                                }
                                AgentInputState::Processing => {
                                    stop_requested =
                                        self.show_processing_state(ui, &colors, accent);
                                }
                                AgentInputState::Response => {
                                    self.show_response_state(ui, &colors, &mut result);
                                }
                            }

                            // Handle stop request
                            if stop_requested {
                                self.stop_generation();
                            }

                            // Submit on bare Enter (intercepted before TextEdit)
                            if enter_to_submit && !self.input.is_empty() {
                                self.last_query = self.input.clone();
                                result.query = Some(self.input.clone());
                                self.input.clear();
                                self.prev_input.clear();
                                self.state = AgentInputState::Ready;
                                self.focus_input = true;
                            }
                        });

                        // Additional rows for expanded content
                        if self.state == AgentInputState::Typing && !self.input.is_empty() {
                            ui.add_space(8.0);
                            self.show_suggestions(ui, &colors);
                        }

                        if self.state == AgentInputState::Processing && !self.activities.is_empty()
                        {
                            ui.add_space(4.0);
                            self.show_activities(ui, &colors);
                        }

                        if self.state == AgentInputState::Response && self.response_expanded {
                            ui.add_space(8.0);
                            self.show_expanded_response(ui, &colors);
                        }
                    });
                });

                // Draw premium glass effects
                let rect = frame_response.response.rect;
                if style.inner_highlight().is_some() {
                    // Top edge highlight for glass reflection
                    let highlight_rect = egui::Rect::from_min_size(
                        rect.left_top() + egui::vec2(1.0, 1.0),
                        egui::vec2(rect.width() - 2.0, 1.5),
                    );
                    ui.painter().rect_filled(highlight_rect, 12.0, inner_glow);

                    // Subtle bottom edge glow (emerald accent in dark mode)
                    if bottom_glow != Color32::TRANSPARENT {
                        let bottom_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.left() + 1.0, rect.bottom() - 2.0),
                            egui::vec2(rect.width() - 2.0, 1.0),
                        );
                        ui.painter().rect_filled(bottom_rect, 12.0, bottom_glow);
                    }
                }

                rect
            },
        );

        // Check for / and @ triggers (order matters - slash first, then mention, then update prev_input)
        self.check_input_triggers();

        // Calculate cursor position for popup alignment
        let cursor_x = self.calculate_cursor_x(ui);
        let rect = centered_response.inner;

        // Show mention popup if active (positioned above cursor) - also outside for Area rendering
        if self.mention_popup.active {
            self.show_mention_popup(ui, &colors, rect, cursor_x);
        }

        // Show slash command popup if active (positioned above cursor)
        if self.slash_command_popup.active {
            self.slash_command_popup.show(ui, rect, cursor_x);
        }

        // Handle keyboard input
        self.handle_keyboard(ui.ctx(), &mut result);

        // Drain any pending commands into the result
        if !self.pending_commands.is_empty() {
            result.commands = std::mem::take(&mut self.pending_commands);
        }

        result
    }

    /// Show the agent input bar in inline mode (for status line embedding)
    /// Returns the input bar result with commands and exit request
    #[profiling::function]
    pub fn show_inline(&mut self, ui: &mut egui::Ui) -> AgentInputBarResult {
        let transition_alpha = self.tick_transition(ui.ctx());
        let mut result = AgentInputBarResult::default();
        let colors = OverlayColors::new(self.theme);
        let accent = self.theme.accent_primary();

        // Match status line height
        let height = 26.0;

        // Separator after the mode badge (provider is now shown there)
        let separator_width = 20.0;
        let (sep_rect, _) =
            ui.allocate_exact_size(egui::vec2(separator_width, height), egui::Sense::hover());
        if ui.is_rect_visible(sep_rect) {
            let line_color = self.theme.text_tertiary().gamma_multiply(0.25);
            ui.painter().vline(
                sep_rect.center().x,
                egui::Rangef::new(sep_rect.min.y + 6.0, sep_rect.max.y - 6.0),
                egui::Stroke::new(1.0, line_color),
            );
        }

        // Fade-in on state transitions
        ui.set_opacity(transition_alpha);

        // Track if stop was requested
        let mut stop_requested = false;

        // Different UI based on state
        match self.state {
            AgentInputState::Ready | AgentInputState::Typing => {
                // Text input - clean placeholder that hints at capabilities
                let hint_text = "Ask anything...  / commands  @ metrics";
                let max_input_width = 500.0;
                let available = ui.available_width() - 50.0;
                let input_width = available.min(max_input_width).max(200.0);

                let text_edit_id = ui.make_persistent_id("agent_input_inline");
                let response = ui.add(
                    TextEdit::singleline(&mut self.input)
                        .id(text_edit_id)
                        .hint_text(hint_text)
                        .desired_width(input_width)
                        .font(typography::proportional(typography::MD))
                        .text_color(colors.text)
                        .frame(false),
                );

                // Store the text edit rect for popup positioning
                self.text_edit_rect = Some(response.rect);

                if self.focus_input {
                    response.request_focus();
                    self.focus_input = false;
                }

                // Transition states based on input
                if !self.input.is_empty() && self.state == AgentInputState::Ready {
                    self.state = AgentInputState::Typing;
                }

                // Handle Enter to submit
                if response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && !self.input.is_empty()
                {
                    // Submit the query
                    self.last_query = self.input.clone();
                    result.query = Some(self.input.clone());
                    self.input.clear();
                    self.state = AgentInputState::Ready;
                    self.focus_input = true;
                }

                // Move cursor to end if requested
                if self.cursor_to_end {
                    self.cursor_to_end = false;
                    if let Some(mut state) =
                        egui::text_edit::TextEditState::load(ui.ctx(), text_edit_id)
                    {
                        let ccursor = egui::text::CCursor::new(self.input.len());
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                        state.store(ui.ctx(), text_edit_id);
                    }
                }

                // Token usage indicator (only when typing)
                if !self.input.is_empty() {
                    // Estimate: ~4 chars per token
                    let total_chars = self.input.len() + self.context_char_count;
                    let estimated_tokens = total_chars / 4;
                    let token_str = if estimated_tokens >= 1000 {
                        format!("~{:.1}k tokens", estimated_tokens as f32 / 1000.0)
                    } else {
                        format!("~{estimated_tokens} tokens")
                    };
                    ui.label(
                        RichText::new(token_str)
                            .color(colors.muted_text.gamma_multiply(0.5))
                            .size(typography::XS),
                    );
                }
            }
            AgentInputState::Processing => {
                // Minimal display: braille spinner + elapsed time + stop hint
                self.render_pulsing_dots(ui, accent);
                ui.add_space(8.0);

                // Elapsed time
                let elapsed = self
                    .processing_start
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);
                ui.label(
                    RichText::new(format!("{elapsed}s"))
                        .color(colors.muted_text)
                        .size(typography::SM),
                );

                ui.add_space(16.0);

                // Stop hint - same style as "Esc clear" in Response state
                if Self::render_key_hint_clickable(ui, "Esc", "stop", accent, colors.muted_text) {
                    stop_requested = true;
                }

                // Request repaint for animation and elapsed time
                ui.ctx().request_repaint();
            }
            AgentInputState::Response => {
                // Success indicator
                ui.label(
                    RichText::new(semantic_icons::status::SUCCESS)
                        .color(accent)
                        .size(typography::MD),
                );
                ui.add_space(6.0);

                // Show command badge if actions were applied, otherwise text summary
                if self.applied_command_count > 0 {
                    let n = self.applied_command_count;
                    let label = if n == 1 {
                        "1 action applied".to_string()
                    } else {
                        format!("{n} actions applied")
                    };
                    ui.label(RichText::new(label).color(accent).size(typography::MD));
                } else {
                    let summary = if !self.display_text.is_empty() {
                        let first_line = self.display_text.lines().next().unwrap_or("");
                        if first_line.len() > 50 {
                            format!("{}...", &first_line[..47])
                        } else if first_line.is_empty() {
                            "Response ready".to_string()
                        } else {
                            first_line.to_string()
                        }
                    } else {
                        "Response ready".to_string()
                    };
                    ui.label(
                        RichText::new(summary)
                            .color(colors.text)
                            .size(typography::MD),
                    );
                }

                ui.add_space(16.0);

                // Keyboard hints
                Self::render_key_hint(ui, "Tab", "expand", accent, colors.muted_text);
                ui.add_space(10.0);
                Self::render_key_hint(ui, "Esc", "clear", accent, colors.muted_text);

                // Handle Tab key to open in panel
                ui.ctx().input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, Key::Tab) {
                        result.open_in_pane = true;
                    }
                });
            }
        }

        // Check for / and @ triggers
        self.check_input_triggers();

        // Calculate cursor position for popup alignment
        let cursor_x = self.calculate_cursor_x(ui);

        // Get rect for popup positioning (use text_edit_rect or fallback)
        let rect = self
            .text_edit_rect
            .unwrap_or_else(|| ui.available_rect_before_wrap());

        // Show mention popup if active
        if self.mention_popup.active {
            self.show_mention_popup(ui, &colors, rect, cursor_x);
        }

        // Show slash command popup if active
        if self.slash_command_popup.active {
            self.slash_command_popup.show(ui, rect, cursor_x);
        }

        // Handle all keyboard input including popup navigation
        self.handle_keyboard(ui.ctx(), &mut result);

        // For inline mode, also handle Escape to clear, stop, or exit when no popup is active
        if !self.mention_popup.active && !self.slash_command_popup.active {
            ui.ctx().input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    if self.state == AgentInputState::Processing {
                        stop_requested = true;
                    } else if !self.input.is_empty() {
                        self.input.clear();
                    } else {
                        result.exit_requested = true;
                    }
                }
            });
        }

        // Handle stop request
        if stop_requested {
            self.stop_generation();
        }

        // Drain any pending commands into the result
        if !self.pending_commands.is_empty() {
            result.commands = std::mem::take(&mut self.pending_commands);
        }

        result
    }

    /// Render a keyboard hint like "Tab expand" with proper styling
    fn render_key_hint(
        ui: &mut egui::Ui,
        key: &str,
        action: &str,
        key_color: Color32,
        action_color: Color32,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Key in a subtle badge style
            ui.label(
                RichText::new(key)
                    .color(key_color)
                    .size(typography::SM)
                    .strong(),
            );
            // Action text
            ui.label(
                RichText::new(action)
                    .color(action_color)
                    .size(typography::SM),
            );
        });
    }

    /// Render a clickable keyboard hint - returns true if clicked
    fn render_key_hint_clickable(
        ui: &mut egui::Ui,
        key: &str,
        action: &str,
        key_color: Color32,
        action_color: Color32,
    ) -> bool {
        let response = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // Key in a subtle badge style
                ui.label(
                    RichText::new(key)
                        .color(key_color)
                        .size(typography::SM)
                        .strong(),
                );
                // Action text
                ui.label(
                    RichText::new(action)
                        .color(action_color)
                        .size(typography::SM),
                );
            })
            .response;

        response.interact(egui::Sense::click()).clicked()
    }

    /// Check for / and @ triggers in the input text.
    /// Both use prev_input to detect changes, so we check both before updating prev_input.
    fn check_input_triggers(&mut self) {
        let input_len = self.input.len();
        let prev_len = self.prev_input.len();

        if input_len > prev_len {
            // Character(s) were added
            let new_chars = &self.input[prev_len..];

            // Check for / slash command trigger (only if mention popup is not active)
            if !self.mention_popup.active {
                if new_chars.contains('/') && !self.slash_command_popup.active {
                    // Only trigger if / is at the start or after a space
                    if let Some(slash_pos) = self.input.rfind('/') {
                        let is_at_start = slash_pos == 0;
                        let is_after_space =
                            slash_pos > 0 && self.input.chars().nth(slash_pos - 1) == Some(' ');

                        if is_at_start || is_after_space {
                            self.slash_command_popup.start(slash_pos);
                        }
                    }
                } else if self.slash_command_popup.active {
                    // Update query: extract text after /
                    let slash_pos = self.slash_command_popup.get_slash_position();
                    if slash_pos < self.input.len() {
                        let query = &self.input[slash_pos + 1..];
                        // Close if there's a space (command was completed) or newline
                        if query.contains(' ') || query.contains('\n') {
                            self.slash_command_popup.close();
                        } else {
                            self.slash_command_popup.set_query(query);
                        }
                    }
                }
            }

            // Check for @ mention trigger (only if slash popup is not active)
            if !self.slash_command_popup.active {
                if new_chars.contains('@') {
                    // Find position of the new @
                    if let Some(at_pos) = self.input.rfind('@') {
                        self.mention_popup.start(at_pos);
                    }
                } else if self.mention_popup.active {
                    // Update query: extract text after @
                    let query = &self.input[self.mention_popup.get_at_position() + 1..];
                    // Close if there's a space or the @ was deleted
                    if query.contains(' ') || query.contains('\n') {
                        self.mention_popup.close();
                    } else {
                        self.mention_popup.set_query(query);
                    }
                }
            }
        } else if input_len < prev_len {
            // Character(s) were deleted
            if self.slash_command_popup.active {
                let slash_pos = self.slash_command_popup.get_slash_position();
                if self.input.len() <= slash_pos {
                    // The / was deleted
                    self.slash_command_popup.close();
                } else {
                    // Update query
                    let query = &self.input[slash_pos + 1..];
                    self.slash_command_popup.set_query(query);
                }
            }

            if self.mention_popup.active {
                if self.input.len() <= self.mention_popup.get_at_position() {
                    // The @ was deleted
                    self.mention_popup.close();
                } else {
                    // Update query
                    let query = &self.input[self.mention_popup.get_at_position() + 1..];
                    self.mention_popup.set_query(query);
                }
            }
        }

        // Update prev_input AFTER both checks
        self.prev_input = self.input.clone();
    }

    /// Calculate approximate cursor X position based on trigger character position
    fn calculate_cursor_x(&self, _ui: &egui::Ui) -> Option<f32> {
        let text_edit_rect = self.text_edit_rect?;

        // Determine which trigger position to use
        let char_pos = if self.slash_command_popup.active {
            self.slash_command_popup.get_slash_position()
        } else if self.mention_popup.active {
            self.mention_popup.get_at_position()
        } else {
            return None;
        };

        // Approximate character width for proportional font at MD size (~14px)
        // This is an estimate; actual width varies per character
        let char_width = 8.5;

        // Calculate X position: text_edit left + (char_pos * char_width)
        let cursor_x = text_edit_rect.left() + (char_pos as f32 * char_width);

        Some(cursor_x)
    }

    /// Show the mention popup for selecting metrics
    fn show_mention_popup(
        &self,
        ui: &mut egui::Ui,
        _colors: &OverlayColors,
        input_rect: egui::Rect,
        cursor_x: Option<f32>,
    ) {
        self.mention_popup.show(ui, input_rect, cursor_x);
    }

    fn show_ready_state(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        _result: &mut AgentInputBarResult,
    ) {
        // Placeholder with quick key hints
        let hint_text = "Ask a question...  /commands  @metrics";

        // Text input that looks like placeholder (multiline for Shift+Enter support)
        let response = ui.add(
            TextEdit::multiline(&mut self.input)
                .hint_text(hint_text)
                .desired_width(ui.available_width() - 20.0)
                .font(typography::proportional(typography::MD))
                .text_color(colors.text)
                .frame(false)
                .desired_rows(1),
        );

        // Store the text edit rect for popup positioning
        self.text_edit_rect = Some(response.rect);

        if self.focus_input {
            response.request_focus();
            self.focus_input = false;
        }

        // Transition to typing if user starts typing
        if !self.input.is_empty() {
            self.state = AgentInputState::Typing;
        }
    }

    fn show_typing_state(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        _result: &mut AgentInputBarResult,
    ) {
        let hint_text = "Ask a question...  /commands  @metrics";
        let text_edit_id = ui.make_persistent_id("agent_input_typing");
        let response = ui.add(
            TextEdit::multiline(&mut self.input)
                .id(text_edit_id)
                .hint_text(hint_text)
                .desired_width(ui.available_width() - 60.0)
                .font(typography::proportional(typography::MD))
                .text_color(colors.text)
                .frame(false)
                .desired_rows(1),
        );

        // Store the text edit rect for popup positioning
        self.text_edit_rect = Some(response.rect);

        if self.focus_input {
            response.request_focus();
            self.focus_input = false;
        }

        // Move cursor to end if requested (after metric selection)
        if self.cursor_to_end {
            self.cursor_to_end = false;
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), text_edit_id) {
                let ccursor = egui::text::CCursor::new(self.input.len());
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                state.store(ui.ctx(), text_edit_id);
            }
        }

        // Token usage indicator (right-aligned, only when typing)
        if !self.input.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Estimate: ~4 chars per token
                let total_chars = self.input.len() + self.context_char_count;
                let estimated_tokens = total_chars / 4;
                let token_str = if estimated_tokens >= 1000 {
                    format!("~{:.1}k tokens", estimated_tokens as f32 / 1000.0)
                } else {
                    format!("~{estimated_tokens} tokens")
                };
                ui.label(
                    RichText::new(token_str)
                        .color(colors.muted_text.gamma_multiply(0.5))
                        .size(typography::XS),
                );
            });
        }
    }

    fn show_processing_state(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        accent: Color32,
    ) -> bool {
        let mut stop_requested = false;

        // Animated pulsing dots (Amp-style)
        self.render_pulsing_dots(ui, accent);

        ui.add_space(8.0);

        // Status text with elapsed time
        let elapsed = self
            .processing_start
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);

        // Show live response preview if text is streaming, otherwise status
        if !self.response_text.is_empty() {
            let preview = streaming_preview(&self.response_text, 80);
            ui.label(
                RichText::new(preview)
                    .color(colors.text.gamma_multiply(0.85))
                    .size(typography::MD),
            );
        } else {
            ui.label(
                RichText::new(format!("{elapsed}s"))
                    .color(colors.muted_text)
                    .size(typography::SM),
            );
        }

        // Stop hint (right-aligned) - same style as inline mode
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if Self::render_key_hint_clickable(ui, "Esc", "stop", accent, colors.muted_text) {
                stop_requested = true;
            }
        });

        // Request repaint to update elapsed time and animation
        ui.ctx().request_repaint();

        stop_requested
    }

    /// Render braille spinner for the processing state.
    /// Uses the classic terminal spinner pattern: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
    fn render_pulsing_dots(&self, ui: &mut egui::Ui, color: Color32) {
        const BRAILLE_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

        let time = ui.ctx().input(|i| i.time);
        // 10 frames per second for smooth but not too fast spinning
        let frame_index = ((time * 10.0) as usize) % BRAILLE_FRAMES.len();
        let spinner_char = BRAILLE_FRAMES[frame_index];

        ui.label(
            RichText::new(spinner_char.to_string())
                .color(color)
                .size(typography::MD),
        );
    }

    fn show_response_state(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        result: &mut AgentInputBarResult,
    ) {
        // Use display_text (with command blocks stripped) for display
        let display = &self.display_text;

        // Show command badge or response preview
        if self.applied_command_count > 0 {
            let accent = self.theme.accent_primary();
            let n = self.applied_command_count;
            let badge = if n == 1 {
                "1 action applied".to_string()
            } else {
                format!("{n} actions applied")
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(
                    RichText::new(semantic_icons::status::SUCCESS)
                        .color(accent)
                        .size(typography::MD),
                );
                ui.label(RichText::new(badge).color(accent).size(typography::MD));
                // Show truncated text summary alongside if there's display text
                if !display.trim().is_empty() {
                    ui.label(
                        RichText::new("—")
                            .color(colors.muted_text)
                            .size(typography::SM),
                    );
                    let preview = if let Some(first_line) = display.lines().next() {
                        if first_line.len() > 50 {
                            format!("{}...", &first_line[..47])
                        } else {
                            first_line.to_string()
                        }
                    } else {
                        String::new()
                    };
                    if !preview.is_empty() {
                        ui.label(
                            RichText::new(preview)
                                .color(colors.muted_text)
                                .size(typography::SM),
                        );
                    }
                }
            });
        } else {
            let preview = if display.len() > 80 {
                format!("{}...", &display[..77])
            } else if let Some(first_line) = display.lines().next() {
                if display.lines().count() > 1 {
                    format!("{first_line}...")
                } else {
                    first_line.to_string()
                }
            } else {
                display.clone()
            };

            ui.label(
                RichText::new(format!("{} {preview}", semantic_icons::status::SUCCESS))
                    .color(colors.text)
                    .size(typography::MD),
            );
        }

        // Extract accent before closure to avoid borrow issues
        let accent = self.theme.accent_primary();

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Tab hint for opening in panel - show with accent color
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new("Tab").color(accent).size(typography::SM));
                ui.label(
                    RichText::new("open in panel")
                        .color(colors.muted_text)
                        .size(typography::SM),
                );
            });

            ui.add_space(12.0);

            // Undo button if available
            if self.can_undo
                && ui
                    .small_button(RichText::new("u: undo").color(colors.muted_text))
                    .clicked()
            {
                result.undo_requested = true;
            }

            // Expand/collapse for long responses
            if display.len() > 100 || display.contains('\n') {
                let expand_text = if self.response_expanded {
                    "collapse"
                } else {
                    "expand"
                };
                if ui
                    .small_button(RichText::new(expand_text).color(colors.muted_text))
                    .clicked()
                {
                    self.response_expanded = !self.response_expanded;
                }
            }
        });
    }

    fn show_suggestions(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        // Premium suggestion pills with hover effects
        let suggestions = [
            ("/investigate", "@metric why is it spiking?"),
            ("/query", "show me p99 latency"),
            ("/diff", "@cpu compare to yesterday"),
        ];

        // Accent for pill highlights
        let pill_accent = self.theme.accent_primary();

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            ui.label(
                RichText::new("Try:")
                    .color(colors.muted_text)
                    .size(typography::SM),
            );

            for (cmd, rest) in suggestions.iter() {
                // Calculate text width for pill sizing
                let text = format!("{cmd} {rest}");
                let font = typography::proportional(typography::SM);
                let galley =
                    ui.painter()
                        .layout_no_wrap(text.clone(), font.clone(), colors.faint_text);
                let pill_width = galley.size().x + 16.0;

                let (pill_rect, pill_response) =
                    ui.allocate_exact_size(egui::vec2(pill_width, 22.0), egui::Sense::click());

                let is_hovered = pill_response.hovered();

                // Background pill with subtle hover effect
                let bg_color = if is_hovered {
                    pill_accent.gamma_multiply(0.12)
                } else {
                    colors.elevated_bg.gamma_multiply(0.6)
                };

                let border_color = if is_hovered {
                    pill_accent.gamma_multiply(0.3)
                } else {
                    colors.separator.gamma_multiply(0.5)
                };

                ui.painter().rect_filled(pill_rect, 6.0, bg_color);
                ui.painter().rect_stroke(
                    pill_rect,
                    6.0,
                    egui::Stroke::new(1.0, border_color),
                    egui::StrokeKind::Inside,
                );

                // Draw text with syntax highlighting
                let text_start = pill_rect.left_center() + egui::vec2(8.0, 0.0);
                let cmd_color = if is_hovered {
                    pill_accent
                } else {
                    pill_accent.gamma_multiply(0.7)
                };
                let rest_color = if is_hovered {
                    colors.text.gamma_multiply(0.8)
                } else {
                    colors.faint_text
                };

                // Draw command part
                let cmd_galley =
                    ui.painter()
                        .layout_no_wrap(cmd.to_string(), font.clone(), cmd_color);
                ui.painter().galley(
                    egui::pos2(text_start.x, text_start.y - cmd_galley.size().y / 2.0),
                    cmd_galley.clone(),
                    cmd_color,
                );

                // Draw rest part
                let rest_galley =
                    ui.painter()
                        .layout_no_wrap(format!(" {rest}"), font.clone(), rest_color);
                ui.painter().galley(
                    egui::pos2(
                        text_start.x + cmd_galley.size().x,
                        text_start.y - rest_galley.size().y / 2.0,
                    ),
                    rest_galley,
                    rest_color,
                );

                // Click to insert suggestion
                if pill_response.clicked() {
                    self.input = format!("{cmd} ");
                    self.focus_input = true;
                }

                // Hover tooltip
                pill_response.on_hover_text("Click to use this command");
            }
        });
    }

    fn show_activities(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        // Only show the most recent activity to keep the UI compact
        if let Some(activity) = self.activities.last() {
            ui.horizontal(|ui| {
                ui.add_space(24.0);

                let (icon, text) = match &activity.activity_type {
                    crate::components::util::ActivityType::Thinking(text) => {
                        (semantic_icons::status::LOADING, text.clone())
                    }
                    crate::components::util::ActivityType::ToolUse { tool, summary } => {
                        let icon = match tool.as_str() {
                            "Read" => semantic_icons::file::GENERIC,
                            "Grep" | "Glob" => semantic_icons::action::SEARCH,
                            "Bash" => semantic_icons::file::CODE,
                            "Edit" | "Write" => semantic_icons::action::EDIT,
                            "Task" => semantic_icons::action::ROBOT,
                            "WebFetch" | "WebSearch" => semantic_icons::action::LINK,
                            _ => semantic_icons::action::TOOL,
                        };
                        (icon, format!("{tool}: {summary}"))
                    }
                    crate::components::util::ActivityType::Error(msg) => {
                        (semantic_icons::diagnostic::ERROR, msg.clone())
                    }
                    crate::components::util::ActivityType::Response(text) => {
                        (semantic_icons::status::SUCCESS, text.clone())
                    }
                    crate::components::util::ActivityType::EditorAction {
                        description,
                        success,
                    } => {
                        let icon = if *success {
                            semantic_icons::status::SUCCESS
                        } else {
                            semantic_icons::diagnostic::ERROR
                        };
                        (icon, description.clone())
                    }
                };

                // Use accent color for activities while processing (even if individual
                // activity is complete), muted for errors
                let color = match &activity.activity_type {
                    crate::components::util::ActivityType::Error(_) => colors.muted_text,
                    _ if activity.in_progress => colors.accent,
                    _ if self.state == AgentInputState::Processing => {
                        // Still processing, so show completed activities in a slightly muted accent
                        colors.accent.gamma_multiply(0.7)
                    }
                    _ => colors.muted_text,
                };

                ui.label(RichText::new(icon).color(color).size(typography::SM));
                ui.add_space(4.0);
                ui.label(RichText::new(text).color(color).size(typography::SM));
            });
        }
    }

    fn show_expanded_response(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            ui.label(
                RichText::new(&self.display_text)
                    .color(colors.text)
                    .size(typography::MD),
            );
        });
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context, result: &mut AgentInputBarResult) {
        // Handle slash command popup keyboard input first (like @ mentions)
        if self.slash_command_popup.active {
            let mut handled = false;
            ctx.input_mut(|input| {
                // Navigate up
                if input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                    || input.consume_key(egui::Modifiers::CTRL, Key::P)
                    || input.consume_key(egui::Modifiers::CTRL, Key::K)
                {
                    self.slash_command_popup.select_prev();
                    handled = true;
                }
                // Navigate down
                else if input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                    || input.consume_key(egui::Modifiers::CTRL, Key::N)
                    || input.consume_key(egui::Modifiers::CTRL, Key::J)
                {
                    self.slash_command_popup.select_next();
                    handled = true;
                }
                // Select with Enter or Tab
                else if input.consume_key(egui::Modifiers::NONE, Key::Enter)
                    || input.consume_key(egui::Modifiers::NONE, Key::Tab)
                {
                    if let Some(cmd) = self.slash_command_popup.selected() {
                        // Replace /query with the selected command + space
                        let slash_pos = self.slash_command_popup.get_slash_position();
                        let cmd_name = cmd.name;

                        // Build new input: text before / + /command + space
                        let prefix = &self.input[..slash_pos];
                        self.input = format!("{prefix}/{cmd_name} ");
                        self.prev_input = self.input.clone();
                    }
                    self.slash_command_popup.close();
                    // Re-focus the input field after selection and move cursor to end
                    self.focus_input = true;
                    self.cursor_to_end = true;
                    handled = true;
                }
                // Cancel with Escape
                else if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    self.slash_command_popup.close();
                    handled = true;
                }
            });

            if handled {
                return;
            }
        }

        // Handle mention popup keyboard input
        if self.mention_popup.active {
            let mut handled = false;
            ctx.input_mut(|input| {
                // Navigate up
                if input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                    || input.consume_key(egui::Modifiers::CTRL, Key::P)
                    || input.consume_key(egui::Modifiers::CTRL, Key::K)
                {
                    self.mention_popup.select_prev();
                    handled = true;
                }
                // Navigate down
                else if input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                    || input.consume_key(egui::Modifiers::CTRL, Key::N)
                    || input.consume_key(egui::Modifiers::CTRL, Key::J)
                {
                    self.mention_popup.select_next();
                    handled = true;
                }
                // Select with Enter or Tab
                else if input.consume_key(egui::Modifiers::NONE, Key::Enter)
                    || input.consume_key(egui::Modifiers::NONE, Key::Tab)
                {
                    if let Some(metric) = self.mention_popup.selected() {
                        // Replace @query with the selected metric
                        let at_pos = self.mention_popup.get_at_position();
                        let metric_name = metric.to_string();

                        // Build new input: text before @ + metric + space
                        let prefix = &self.input[..at_pos];
                        self.input = format!("{prefix}{metric_name} ");
                        self.prev_input = self.input.clone();
                    }
                    self.mention_popup.close();
                    // Re-focus the input field after selection and move cursor to end
                    self.focus_input = true;
                    self.cursor_to_end = true;
                    handled = true;
                }
                // Cancel with Escape
                else if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    self.mention_popup.close();
                    handled = true;
                }
            });

            if handled {
                return;
            }
        }

        ctx.input_mut(|input| {
            match self.state {
                AgentInputState::Ready => {
                    // Quick commands (only when not typing)
                    if self.input.is_empty() {
                        if input.consume_key(egui::Modifiers::NONE, Key::W) {
                            result.quick_command = Some(QuickCommand::WhatsWrong);
                        } else if input.consume_key(egui::Modifiers::NONE, Key::Y) {
                            result.quick_command = Some(QuickCommand::Why);
                        } else if input.consume_key(egui::Modifiers::NONE, Key::C) {
                            result.quick_command = Some(QuickCommand::Compare);
                        } else if input.consume_key(egui::Modifiers::NONE, Key::R) {
                            result.quick_command = Some(QuickCommand::Related);
                        } else if input.consume_key(egui::Modifiers::NONE, Key::E) {
                            result.quick_command = Some(QuickCommand::Explain);
                        } else if input.consume_key(egui::Modifiers::NONE, Key::F) {
                            result.quick_command = Some(QuickCommand::Fix);
                        } else if input.consume_key(egui::Modifiers::NONE, Key::S) {
                            result.quick_command = Some(QuickCommand::Summarize);
                        } else if input.consume_key(egui::Modifiers::NONE, Key::H) {
                            result.quick_command = Some(QuickCommand::History);
                        }
                    }

                    // Escape to exit
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                        result.exit_requested = true;
                    }
                }
                AgentInputState::Typing => {
                    // Enter to send (only when mention popup is not active)
                    if input.consume_key(egui::Modifiers::NONE, Key::Enter)
                        && !self.input.is_empty()
                    {
                        result.query = Some(self.input.clone());
                        self.input.clear();
                        self.prev_input.clear();
                    }

                    // Escape to clear input / exit
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                        if self.input.is_empty() {
                            result.exit_requested = true;
                        } else {
                            self.input.clear();
                            self.prev_input.clear();
                            self.state = AgentInputState::Ready;
                        }
                    }
                }
                AgentInputState::Processing => {
                    // Escape to stop generation
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                        self.stop_generation();
                    }
                }
                AgentInputState::Response => {
                    // Tab to open in agent pane (handoff)
                    if input.consume_key(egui::Modifiers::NONE, Key::Tab) {
                        result.open_in_pane = true;
                    }

                    // Enter to continue (new query)
                    if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                        self.clear_response();
                    }

                    // Escape to exit
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                        result.exit_requested = true;
                    }

                    // u to undo
                    if input.consume_key(egui::Modifiers::NONE, Key::U) && self.can_undo {
                        result.undo_requested = true;
                    }

                    // Quick follow-up commands
                    if input.consume_key(egui::Modifiers::NONE, Key::Y) {
                        result.quick_command = Some(QuickCommand::Why);
                    } else if input.consume_key(egui::Modifiers::NONE, Key::F) {
                        result.quick_command = Some(QuickCommand::Fix);
                    } else if input.consume_key(egui::Modifiers::NONE, Key::R) {
                        result.quick_command = Some(QuickCommand::Related);
                    } else if input.consume_key(egui::Modifiers::NONE, Key::C) {
                        result.quick_command = Some(QuickCommand::Compare);
                    } else if input.consume_key(egui::Modifiers::NONE, Key::S) {
                        result.quick_command = Some(QuickCommand::Summarize);
                    }

                    // +/- for context manipulation
                    if input.consume_key(egui::Modifiers::SHIFT, Key::Equals) {
                        // + key
                        result.add_pane_to_context = true;
                    }
                    if input.consume_key(egui::Modifiers::NONE, Key::Minus) {
                        result.remove_pane_from_context = true;
                    }
                    if input.consume_key(egui::Modifiers::NONE, Key::Num0) {
                        result.clear_context = true;
                    }
                }
            }
        });
    }

    /// Send a query to the AI agent
    ///
    /// The `system_context` parameter is passed as a system prompt to the AI,
    /// providing context about the editor state and available commands.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn send_query(&mut self, query: &str, system_context: Option<&str>) {
        // Store the query for potential handoff
        self.last_query = query.to_string();

        // Transition to processing state
        self.state = AgentInputState::Processing;
        self.processing_status = "Sending to agent...".to_string();
        self.processing_start = Some(crate::util::Instant::now());
        self.activities.clear();
        self.response_text.clear();
        self.display_text.clear();

        // Get working directory
        let working_dir = std::env::current_dir().ok();

        // Resolve the effective model: use selected model, or fall back to
        // the provider's default from the manifest.
        let effective_model = self.selected_model.clone().or_else(|| {
            crate::components::util::ProviderManifest::default_model_id_for(self.selected_provider)
        });

        // Send via persistent client (reuses warm subprocess)
        if let Some(client) = &self.persistent_client {
            let receiver = client.prompt_with_context(
                query,
                working_dir,
                effective_model.as_deref(),
                system_context,
            );
            self.event_receiver = Some(receiver);
        } else {
            log::error!("no persistent ACP client available");
            self.state = AgentInputState::Response;
            self.response_text = "AI agent not available (no runtime)".to_string();
            self.display_text = self.response_text.clone();
        }
    }

    /// Send a query (WASM version - not supported)
    #[cfg(target_arch = "wasm32")]
    pub fn send_query(&mut self, query: &str, _context: Option<&str>) {
        self.last_query = query.to_string();
        self.state = AgentInputState::Response;
        self.response_text = "Claude Code CLI is not available in the browser.".to_string();
        self.display_text = self.response_text.clone();
        self.can_undo = false;
    }

    /// Poll for streaming AI responses and update state
    #[cfg(not(target_arch = "wasm32"))]
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(ref receiver) = self.event_receiver else {
            return;
        };

        // Process all available events
        let mut should_clear_receiver = false;
        while let Ok(event) = receiver.try_recv() {
            match event {
                AgentEvent::TextDelta(text) => {
                    self.response_text.push_str(&text);
                    self.processing_status = "Responding...".to_string();
                }
                AgentEvent::ThinkingDelta(text) => {
                    self.processing_status = "Thinking...".to_string();
                    // Update or create thinking activity
                    if let Some(last) = self.activities.last_mut() {
                        if let ActivityType::Thinking(ref mut thinking_text) = last.activity_type {
                            if thinking_text.len() < 60 {
                                thinking_text.push_str(&text);
                                if thinking_text.len() > 60 {
                                    thinking_text.truncate(57);
                                    thinking_text.push_str("...");
                                }
                            }
                        } else {
                            self.activities.push(ActivityItem {
                                activity_type: ActivityType::Thinking(truncate_text(&text, 60)),
                                in_progress: true,
                            });
                        }
                    } else {
                        self.activities.push(ActivityItem {
                            activity_type: ActivityType::Thinking(truncate_text(&text, 60)),
                            in_progress: true,
                        });
                    }
                }
                AgentEvent::ToolCallStart {
                    name,
                    raw_input,
                    id,
                } => {
                    // Normalize tool name (strip mcp__acp__ prefix)
                    let display_name = normalize_tool_name(&name);
                    self.processing_status = format!("Using {display_name}...");
                    // Extract a summary from the input (may be empty initially)
                    let input_str = raw_input
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    let summary = if input_str.is_empty() || input_str == "{}" {
                        "...".to_string()
                    } else {
                        extract_tool_summary(display_name, &input_str)
                    };
                    self.activities.push(ActivityItem {
                        activity_type: ActivityType::ToolUse {
                            tool: display_name.to_string(),
                            summary,
                        },
                        in_progress: true,
                    });
                    // Store the ID for later updates
                    let _ = id; // Used for matching in ToolCallReady
                }
                AgentEvent::ToolCallInputDelta { .. } => {
                    // Input is being streamed, ignore
                }
                AgentEvent::ToolCallReady { input, .. } => {
                    // Update the last activity with complete input
                    if let Some(last) = self.activities.last_mut() {
                        if let ActivityType::ToolUse { tool, summary } = &mut last.activity_type {
                            let new_summary = extract_tool_summary(tool, &input.to_string());
                            if new_summary != "{}" && !new_summary.is_empty() {
                                *summary = new_summary;
                            }
                        }
                    }
                }
                AgentEvent::ToolResult { .. } => {
                    // Mark last tool use as complete
                    if let Some(last) = self.activities.last_mut() {
                        last.in_progress = false;
                    }
                }
                AgentEvent::Done { .. } => {
                    // Mark all activities as complete
                    for activity in &mut self.activities {
                        activity.in_progress = false;
                    }

                    // Parse Enya commands from the response
                    log::debug!(
                        "Agent response complete. Response text ({} chars): {}",
                        self.response_text.len(),
                        if self.response_text.len() > 500 {
                            format!("{}...", &self.response_text[..500])
                        } else {
                            self.response_text.clone()
                        }
                    );
                    let commands = parse_commands(&self.response_text);
                    log::debug!(
                        "Parsed {} enya-command(s) from agent response",
                        commands.len()
                    );
                    self.applied_command_count = commands.len();
                    if !commands.is_empty() {
                        for cmd in &commands {
                            log::info!("Agent command: {cmd:?}");
                        }
                        self.pending_commands.extend(commands);
                        // Request repaint to ensure commands are processed on next frame
                        ctx.request_repaint();
                    }

                    // Strip command blocks from display text
                    self.display_text = strip_command_blocks(&self.response_text);

                    // Transition to response state
                    self.state = AgentInputState::Response;
                    self.processing_start = None;
                    self.can_undo = false; // TODO: implement undo

                    should_clear_receiver = true;
                }
                AgentEvent::Error(e) => {
                    self.activities.push(ActivityItem {
                        activity_type: ActivityType::Error(e.to_string()),
                        in_progress: false,
                    });
                    self.state = AgentInputState::Response;
                    self.response_text = format!("Error: {e}");
                    self.display_text = self.response_text.clone();
                    self.processing_start = None;
                    should_clear_receiver = true;
                }
            }
        }

        if should_clear_receiver {
            self.event_receiver = None;
        }

        // Request repaint while processing
        if self.event_receiver.is_some() || self.processing_start.is_some() {
            ctx.request_repaint();
        }
    }

    /// Poll for streaming AI responses (WASM version - no-op)
    #[cfg(target_arch = "wasm32")]
    pub fn poll(&mut self, _ctx: &egui::Context) {
        // No streaming in WASM
    }

    /// Check if we're currently waiting for a response
    pub fn is_waiting(&self) -> bool {
        self.state == AgentInputState::Processing
    }

    /// Stop the current generation and return to Ready state.
    /// This drops the event receiver (soft cancel - background task may continue).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn stop_generation(&mut self) {
        if self.state == AgentInputState::Processing {
            self.event_receiver = None;
            self.processing_start = None;
            self.processing_status.clear();
            self.activities.clear();
            // Keep any partial response text that was received
            if !self.response_text.is_empty() {
                self.response_text.push_str("\n\n*(generation stopped)*");
                self.display_text = self.response_text.clone();
                self.state = AgentInputState::Response;
            } else {
                self.state = AgentInputState::Ready;
            }
        }
    }

    /// Stop the current generation (WASM version - no-op)
    #[cfg(target_arch = "wasm32")]
    pub fn stop_generation(&mut self) {
        self.state = AgentInputState::Ready;
    }
}

/// Truncate text to a maximum length, adding ellipsis if needed
#[cfg(not(target_arch = "wasm32"))]
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}

/// Normalize tool names by stripping common prefixes (e.g., mcp__acp__)
#[cfg(not(target_arch = "wasm32"))]
fn normalize_tool_name(name: &str) -> &str {
    // Strip mcp__acp__ prefix if present
    name.strip_prefix("mcp__acp__")
        .or_else(|| name.strip_prefix("mcp__"))
        .unwrap_or(name)
}

/// Extract a summary from tool input for display
#[cfg(not(target_arch = "wasm32"))]
fn extract_tool_summary(tool: &str, input: &str) -> String {
    // Try to parse as JSON and extract relevant fields
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(input) {
        // Handle both normalized and full tool names
        let tool_base = normalize_tool_name(tool);
        match tool_base {
            "Read" => {
                if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                    // Get just the filename
                    return path.rsplit('/').next().unwrap_or(path).to_string();
                }
            }
            "Grep" => {
                if let Some(pattern) = json.get("pattern").and_then(|v| v.as_str()) {
                    return truncate_text(pattern, 30);
                }
            }
            "Bash" => {
                if let Some(cmd) = json.get("command").and_then(|v| v.as_str()) {
                    // Show first meaningful part of command
                    let cmd_preview = cmd.lines().next().unwrap_or(cmd);
                    return truncate_text(cmd_preview, 40);
                }
                // Also check for description field
                if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                    return truncate_text(desc, 40);
                }
            }
            "Edit" | "Write" => {
                if let Some(path) = json.get("file_path").and_then(|v| v.as_str()) {
                    return path.rsplit('/').next().unwrap_or(path).to_string();
                }
            }
            "Glob" => {
                if let Some(pattern) = json.get("pattern").and_then(|v| v.as_str()) {
                    return truncate_text(pattern, 30);
                }
            }
            "Task" => {
                if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                    return truncate_text(desc, 40);
                }
            }
            "WebFetch" | "WebSearch" => {
                if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
                    return truncate_text(url, 40);
                }
                if let Some(query) = json.get("query").and_then(|v| v.as_str()) {
                    return truncate_text(query, 40);
                }
            }
            _ => {}
        }
    }

    // For non-JSON or unrecognized, just truncate the input
    if input.is_empty() || input == "{}" {
        "...".to_string()
    } else {
        truncate_text(input, 30)
    }
}

/// Extract a preview from streaming response text.
///
/// Prefers the last non-empty line (most recent content), truncated to `max_len`.
/// Strips markdown formatting artifacts for cleaner display.
fn streaming_preview(text: &str, max_len: usize) -> String {
    // Get the last non-empty line (most recent meaningful content)
    let line = text
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();

    if line.is_empty() {
        return "...".to_string();
    }

    // Strip leading markdown artifacts (bullet points, headers)
    let cleaned = line.trim_start_matches(['#', '-', '*', '>']).trim_start();

    let display = if cleaned.is_empty() { line } else { cleaned };

    if display.len() <= max_len {
        display.to_string()
    } else {
        format!("{}...", &display[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_trigger_at_start() {
        let mut bar = AgentInputBar::new();
        bar.input = "/".to_string();
        bar.prev_input = String::new();

        bar.check_input_triggers();

        assert!(
            bar.slash_command_popup.active,
            "Slash popup should be active after typing / at start"
        );
        assert_eq!(
            bar.slash_command_popup.get_slash_position(),
            0,
            "Slash position should be 0"
        );
    }

    #[test]
    fn test_slash_trigger_after_space() {
        let mut bar = AgentInputBar::new();
        bar.input = "hello /".to_string();
        bar.prev_input = "hello ".to_string();

        bar.check_input_triggers();

        assert!(
            bar.slash_command_popup.active,
            "Slash popup should be active after typing / after space"
        );
        assert_eq!(
            bar.slash_command_popup.get_slash_position(),
            6,
            "Slash position should be 6"
        );
    }

    #[test]
    fn test_slash_no_trigger_mid_word() {
        let mut bar = AgentInputBar::new();
        bar.input = "hello/".to_string();
        bar.prev_input = "hello".to_string();

        bar.check_input_triggers();

        assert!(
            !bar.slash_command_popup.active,
            "Slash popup should NOT be active when / is in middle of word"
        );
    }

    #[test]
    fn test_mention_trigger() {
        let mut bar = AgentInputBar::new();
        bar.input = "@".to_string();
        bar.prev_input = String::new();

        bar.check_input_triggers();

        assert!(
            bar.mention_popup.active,
            "Mention popup should be active after typing @"
        );
    }
}
