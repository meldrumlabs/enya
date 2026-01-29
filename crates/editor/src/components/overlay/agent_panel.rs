//! Agent Panel - Claude Code integration for AI-assisted metrics exploration.
//!
//! Provides a chat interface to interact with Claude Code CLI, with streaming
//! responses displayed in real-time. Styled with the Obsidian Glass design system.

use egui::{Color32, CornerRadius, Key, RichText, ScrollArea, Stroke, TextEdit, Vec2};

#[cfg(not(target_arch = "wasm32"))]
use enya_ai::AgentEvent;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use super::mention_popup::MentionPopup;
use super::slash_commands::SlashCommandPopup;
use crate::chat::ChatColors;
use crate::components::pane::time_series_chart::TimeSeriesChart;
use crate::components::pane::{
    InlineChart, InlineContent, InlineDiff, InlineDiffLineKind, InlineSearchResults, InlineSource,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::file_opener::{FileOpenerAction, FileOpenerPopup, FileOpenerResult};
use crate::components::util::{
    ActivityItem, ActivityType, AiModel, AiProvider, ConversationHandoff, MessageRole,
    ResponseStatus, ScrollShadowConfig, ScrollState, normalize_unicode, render_scroll_shadows,
};
use crate::components::widget::ThinkingIndicator;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::icons::APP_GHOSTTY;
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
    /// An error occurred (e.g., file not found)
    Error(String),
    /// Open diff viewer with specific commit
    OpenDiffViewer {
        /// Commit hash
        hash: String,
        /// Commit message
        message: String,
    },
}

/// The agent panel component for AI-assisted chat.
///
/// Fields are `pub(super)` to allow the `agent_streaming` sibling module to
/// access them without exposing internals outside the overlay module.
#[allow(dead_code)] // Some fields used only in native builds or for future features
pub struct AgentPanel {
    pub(super) is_open: bool,
    pub(super) has_focus: bool,
    pub(super) skip_vim_keys_once: bool,
    pub(super) theme: AppTheme,
    pub(super) messages: Vec<ChatMessage>,
    pub(super) input_text: String,
    pub(super) is_waiting: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) event_receiver: Option<Receiver<AgentEvent>>,
    pub(super) response_text: String,
    pub(super) focus_input: bool,
    pub(super) scroll_to_bottom: bool,
    pub(super) is_at_bottom: bool,
    pub(super) last_response_len: usize,
    pub(super) stream_settled_len: usize,
    pub(super) stream_fade_start: Option<crate::util::Instant>,
    pub(super) current_model: Option<String>,
    pub(super) selected_provider: AiProvider,
    pub(super) selected_model: AiModel,
    pub(super) current_status: ResponseStatus,
    pub(super) current_activities: Vec<ActivityItem>,
    pub(super) request_start_time: Option<std::time::Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) runtime_handle: Option<tokio::runtime::Handle>,
    pub(super) editor_context: Option<super::agent_context::EditorContext>,
    pub(super) pending_commands: Vec<super::agent_context::AgentCommand>,
    pub(super) pending_submit: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) file_opener: FileOpenerPopup,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) repo_path: Option<std::path::PathBuf>,
    pub(super) selected_message: Option<usize>,
    pub(super) scroll_to_selected: bool,
    pub(super) search_active: bool,
    pub(super) search_query: String,
    pub(super) search_matches: Vec<usize>,
    pub(super) search_match_idx: usize,
    pub(super) conversation_store: super::conversation_store::ConversationStore,
    /// Pending diff viewer request (commit hash, message)
    pending_diff_viewer: Option<(String, String)>,
    /// Whether keyboard input should be disabled (another overlay is on top)
    keyboard_disabled: bool,
    /// Mention popup for @metric autocomplete
    mention_popup: MentionPopup,
    /// Slash command popup for /command autocomplete
    slash_command_popup: SlashCommandPopup,
    /// Previous input text for change detection
    prev_input_text: String,
}

impl AgentPanel {
    /// Shared field initialization for both native and WASM constructors.
    fn new_common() -> Self {
        let provider = AiProvider::default();
        Self {
            is_open: false,
            has_focus: false,
            skip_vim_keys_once: false,
            theme: AppTheme::default(),
            messages: Vec::new(),
            input_text: String::new(),
            is_waiting: false,
            #[cfg(not(target_arch = "wasm32"))]
            event_receiver: None,
            response_text: String::new(),
            focus_input: false,
            scroll_to_bottom: false,
            is_at_bottom: true,
            last_response_len: 0,
            stream_settled_len: 0,
            stream_fade_start: None,
            current_model: None,
            selected_provider: provider,
            selected_model: AiModel::default_for(provider),
            current_status: ResponseStatus::Complete,
            current_activities: Vec::new(),
            request_start_time: None,
            #[cfg(not(target_arch = "wasm32"))]
            runtime_handle: None,
            editor_context: None,
            pending_commands: Vec::new(),
            pending_submit: false,
            #[cfg(not(target_arch = "wasm32"))]
            file_opener: FileOpenerPopup::new(),
            #[cfg(not(target_arch = "wasm32"))]
            repo_path: None,
            selected_message: None,
            scroll_to_selected: false,
            search_active: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_idx: 0,
            conversation_store: super::conversation_store::ConversationStore::new(),
            pending_diff_viewer: None,
            keyboard_disabled: false,
            mention_popup: MentionPopup::new(),
            slash_command_popup: SlashCommandPopup::new(),
            prev_input_text: String::new(),
        }
    }

    /// Create a new agent panel with a tokio runtime handle.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        let mut panel = Self::new_common();
        panel.runtime_handle = Some(runtime_handle);
        panel
    }

    /// Create a new agent panel (WASM version - no runtime needed).
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        Self::new_common()
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

    /// Set whether keyboard input should be disabled (e.g., when diff viewer is open).
    pub fn set_keyboard_disabled(&mut self, disabled: bool) {
        self.keyboard_disabled = disabled;
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

    /// Set the theme (supports Custom variant with plugin colors)
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        #[cfg(not(target_arch = "wasm32"))]
        self.file_opener.set_theme(theme);
        self.mention_popup.set_theme(theme);
        self.slash_command_popup.set_theme(theme);
    }

    /// Set available metrics for @mention autocomplete
    pub fn set_available_metrics(&mut self, metrics: Vec<String>) {
        self.mention_popup.set_metrics(metrics);
    }

    /// Sets the repository root path for computing full file paths.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_repo_path(&mut self, path: Option<std::path::PathBuf>) {
        self.repo_path = path;
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

        // Add assistant response from handoff (with any inline blocks)
        if !handoff.response.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::Assistant,
                content: handoff.display_text,
                is_streaming: false,
                inline_blocks: handoff.inline_blocks,
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

        // Handle keyboard input - use consume_key to prevent multiple processing
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
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

        // Show file opener popup if open (native only)
        #[cfg(not(target_arch = "wasm32"))]
        if self.file_opener.is_open() {
            match self.file_opener.show(ctx, self.theme) {
                FileOpenerResult::Selected(action) => {
                    if let Some(error) = self.handle_file_opener_action(&action, ctx) {
                        return AgentPanelResult::Error(error);
                    }
                }
                FileOpenerResult::Closed | FileOpenerResult::None => {}
            }
        }

        if matches!(result, AgentPanelResult::Closed) {
            self.close();
        }

        // Check for pending diff viewer request
        if let Some((hash, message)) = self.pending_diff_viewer.take() {
            return AgentPanelResult::OpenDiffViewer { hash, message };
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

        // Handle keyboard input when panel has vim focus (and keyboard not disabled)
        let mut return_focus = false;
        let mut enter_input_mode = false;
        let mut yank_text: Option<String> = None;
        if self.has_focus && !self.keyboard_disabled {
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
                    // j or Down - select next message
                    else if input.consume_key(egui::Modifiers::NONE, Key::J)
                        || input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                    {
                        if !self.messages.is_empty() {
                            let next = match self.selected_message {
                                Some(idx) => (idx + 1).min(self.messages.len() - 1),
                                None => 0,
                            };
                            self.selected_message = Some(next);
                            self.scroll_to_selected = true;
                            self.scroll_to_bottom = false; // don't override selection scroll
                        }
                    }
                    // k or Up - select previous message
                    else if input.consume_key(egui::Modifiers::NONE, Key::K)
                        || input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                    {
                        if !self.messages.is_empty() {
                            let prev = match self.selected_message {
                                Some(idx) => idx.saturating_sub(1),
                                None => self.messages.len() - 1,
                            };
                            self.selected_message = Some(prev);
                            self.scroll_to_selected = true;
                        }
                    }
                    // y - yank (copy) selected message content
                    else if input.consume_key(egui::Modifiers::NONE, Key::Y) {
                        if let Some(idx) = self.selected_message {
                            if let Some(msg) = self.messages.get(idx) {
                                yank_text = Some(msg.content.clone());
                            }
                        }
                    }
                    // / - enter search mode
                    else if input.consume_key(egui::Modifiers::NONE, Key::Slash) {
                        self.search_active = true;
                        self.search_query.clear();
                        self.search_matches.clear();
                        self.search_match_idx = 0;
                    }
                    // n - next search match
                    else if input.consume_key(egui::Modifiers::NONE, Key::N) {
                        if !self.search_matches.is_empty() {
                            self.search_match_idx =
                                (self.search_match_idx + 1) % self.search_matches.len();
                            self.selected_message =
                                Some(self.search_matches[self.search_match_idx]);
                            self.scroll_to_selected = true;
                        }
                    }
                    // N (shift+n) - previous search match
                    else if input.consume_key(egui::Modifiers::SHIFT, Key::N) {
                        if !self.search_matches.is_empty() {
                            self.search_match_idx = if self.search_match_idx == 0 {
                                self.search_matches.len() - 1
                            } else {
                                self.search_match_idx - 1
                            };
                            self.selected_message =
                                Some(self.search_matches[self.search_match_idx]);
                            self.scroll_to_selected = true;
                        }
                    }
                    // G - jump to last message
                    else if input.consume_key(egui::Modifiers::SHIFT, Key::G) {
                        if !self.messages.is_empty() {
                            self.selected_message = Some(self.messages.len() - 1);
                            self.scroll_to_selected = true;
                        }
                    }
                    // g g - jump to first message (single g for simplicity)
                    else if input.consume_key(egui::Modifiers::NONE, Key::G)
                        && !self.messages.is_empty()
                    {
                        self.selected_message = Some(0);
                        self.scroll_to_selected = true;
                    }
                    // o - open inline diff in full diff viewer (if selected message has one)
                    else if input.consume_key(egui::Modifiers::NONE, Key::O) {
                        if let Some(idx) = self.selected_message {
                            if let Some(msg) = self.messages.get(idx) {
                                // Find the first inline diff in the message
                                for block in &msg.inline_blocks {
                                    if let InlineContent::Diff(diff) = block {
                                        if diff.commit_hash != "working" {
                                            self.pending_diff_viewer = Some((
                                                diff.commit_hash.clone(),
                                                diff.commit_message.clone(),
                                            ));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
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

        // Copy yanked text to clipboard
        if let Some(text) = yank_text {
            ui.ctx().copy_text(text);
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

        // Show file opener popup if open (native only)
        #[cfg(not(target_arch = "wasm32"))]
        if self.file_opener.is_open() {
            match self.file_opener.show(ctx, self.theme) {
                FileOpenerResult::Selected(action) => {
                    if let Some(error) = self.handle_file_opener_action(&action, ctx) {
                        return AgentPanelResult::Error(error);
                    }
                }
                FileOpenerResult::Closed | FileOpenerResult::None => {}
            }
        }

        if matches!(result, AgentPanelResult::Closed) {
            self.close();
        }

        // Check for pending diff viewer request
        if let Some((hash, message)) = self.pending_diff_viewer.take() {
            return AgentPanelResult::OpenDiffViewer { hash, message };
        }

        result
    }

    /// Handle file opener action. Returns an error message if the action failed.
    #[cfg(not(target_arch = "wasm32"))]
    fn handle_file_opener_action(
        &self,
        action: &FileOpenerAction,
        ctx: &egui::Context,
    ) -> Option<String> {
        match action {
            FileOpenerAction::OpenIn(app) => {
                if let Some(path) = self.file_opener.file_path() {
                    // Compute full path if we have a repo root
                    let full_path = if let Some(ref root) = self.repo_path {
                        root.join(path)
                    } else {
                        path.to_path_buf()
                    };
                    if let Err(e) = app.execute(&full_path) {
                        log::warn!("Failed to open file: {e}");
                        return Some(e);
                    }
                } else {
                    return Some("No file path available".to_string());
                }
            }
            FileOpenerAction::CopyPath => {
                if let Some(path) = self.file_opener.file_path() {
                    let full_path = if let Some(ref root) = self.repo_path {
                        root.join(path)
                    } else {
                        path.to_path_buf()
                    };
                    ctx.copy_text(full_path.display().to_string());
                }
            }
            FileOpenerAction::CopyRelativePath => {
                if let Some(path) = self.file_opener.file_path() {
                    ctx.copy_text(path.display().to_string());
                }
            }
        }
        None
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

    /// Calculate cursor X position for popup alignment.
    fn calculate_popup_cursor_x(&self, input_rect: egui::Rect) -> Option<f32> {
        let char_pos = if self.slash_command_popup.active {
            self.slash_command_popup.get_slash_position()
        } else if self.mention_popup.active {
            self.mention_popup.get_at_position()
        } else {
            return None;
        };

        // Approximate character width for proportional font at MD size
        let char_width = 8.5;
        let cursor_x = input_rect.left() + (char_pos as f32 * char_width);
        Some(cursor_x)
    }

    /// Render a subtle divider between sections.
    fn render_thread_picker(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();
        let accent = self.theme.accent_primary();
        let colors = self.colors();

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Thread icon
            ui.label(
                RichText::new(egui_nerdfonts::regular::COMMENT_TEXT_MULTIPLE_OUTLINE)
                    .color(text_tertiary)
                    .size(typography::SM),
            );
            ui.add_space(4.0);

            // Current thread name (clickable to open picker)
            let thread_name = self
                .conversation_store
                .active_thread()
                .map(|t| t.name.as_str())
                .unwrap_or("No conversation");

            let name_btn = ui.add(
                egui::Button::new(
                    RichText::new(thread_name)
                        .color(text_secondary)
                        .size(typography::SM),
                )
                .frame(false),
            );

            if name_btn.clicked() {
                self.conversation_store.picker_open = !self.conversation_store.picker_open;
                self.conversation_store.renaming = false;
            }

            // Dropdown arrow
            ui.label(
                RichText::new(if self.conversation_store.picker_open {
                    egui_nerdfonts::regular::CHEVRON_UP
                } else {
                    egui_nerdfonts::regular::CHEVRON_DOWN
                })
                .color(text_tertiary)
                .size(typography::XS),
            );

            // Right-aligned: pin + rename buttons for active thread
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                if let Some(idx) = self.conversation_store.active_idx {
                    let is_pinned = self
                        .conversation_store
                        .threads
                        .get(idx)
                        .is_some_and(|t| t.pinned);

                    // Pin button
                    let pin_icon = if is_pinned {
                        egui_nerdfonts::regular::PIN
                    } else {
                        egui_nerdfonts::regular::PIN_OUTLINE
                    };
                    let pin_color = if is_pinned { accent } else { text_tertiary };
                    let pin_btn = ui.add(
                        egui::Button::new(RichText::new(pin_icon).size(12.0).color(pin_color))
                            .frame(false),
                    );
                    if pin_btn.hovered() {
                        let rect = pin_btn.rect.expand(3.0);
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(3), colors.hover_bg());
                    }
                    let pin_tip = if is_pinned { "Unpin" } else { "Pin" };
                    if pin_btn.on_hover_text(pin_tip).clicked() {
                        self.conversation_store.toggle_pin(idx);
                    }

                    // Rename button
                    let rename_btn = ui.add(
                        egui::Button::new(
                            RichText::new(egui_nerdfonts::regular::PENCIL_OUTLINE)
                                .size(12.0)
                                .color(text_tertiary),
                        )
                        .frame(false),
                    );
                    if rename_btn.hovered() {
                        let rect = rename_btn.rect.expand(3.0);
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(3), colors.hover_bg());
                    }
                    if rename_btn.on_hover_text("Rename").clicked() {
                        self.conversation_store.renaming = !self.conversation_store.renaming;
                        if self.conversation_store.renaming {
                            self.conversation_store.rename_buf = self
                                .conversation_store
                                .active_thread()
                                .map(|t| t.name.clone())
                                .unwrap_or_default();
                        }
                    }
                }
            });
        });

        // Rename inline editor
        if self.conversation_store.renaming {
            ui.horizontal(|ui| {
                ui.add_space(36.0);
                let rename_id = ui.id().with("thread_rename");
                let response = ui.add(
                    TextEdit::singleline(&mut self.conversation_store.rename_buf)
                        .id(rename_id)
                        .desired_width(ui.available_width() - 52.0)
                        .font(typography::proportional(typography::SM))
                        .text_color(text_primary),
                );
                if !response.has_focus() {
                    response.request_focus();
                }
                if response.lost_focus() {
                    let new_name = self.conversation_store.rename_buf.trim().to_string();
                    if !new_name.is_empty() {
                        if let Some(thread) = self.conversation_store.active_thread_mut() {
                            thread.name = new_name;
                        }
                        self.conversation_store.save_active();
                    }
                    self.conversation_store.renaming = false;
                }
            });
        }

        // Thread list popup
        if self.conversation_store.picker_open {
            ui.add_space(4.0);
            let picker_bg = self.theme.bg_elevated();
            let border = self.theme.border_subtle();

            egui::Frame::new()
                .fill(picker_bg)
                .corner_radius(CornerRadius::same(6))
                .stroke(Stroke::new(1.0, border))
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width() - 24.0);

                    // "New conversation" item
                    let new_btn = ui.add(
                        egui::Button::new(
                            RichText::new(format!(
                                "{}  New conversation",
                                egui_nerdfonts::regular::PLUS
                            ))
                            .color(accent)
                            .size(typography::SM),
                        )
                        .frame(false)
                        .min_size(Vec2::new(ui.available_width(), 24.0)),
                    );
                    if new_btn.hovered() {
                        ui.painter().rect_filled(
                            new_btn.rect,
                            CornerRadius::same(4),
                            colors.hover_bg(),
                        );
                    }
                    if new_btn.clicked() {
                        self.start_new_conversation();
                        self.conversation_store.picker_open = false;
                    }

                    if !self.conversation_store.threads.is_empty() {
                        // Thin separator
                        ui.add_space(2.0);
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            rect.left()..=rect.right(),
                            rect.top(),
                            Stroke::new(0.5, border),
                        );
                        ui.add_space(2.0);
                    }

                    // Scrollable thread list
                    let max_list_height = 200.0;
                    ScrollArea::vertical()
                        .id_salt("thread_picker_scroll")
                        .max_height(max_list_height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let mut switch_to: Option<usize> = None;
                            let mut delete_idx: Option<usize> = None;

                            for (i, thread) in self.conversation_store.threads.iter().enumerate() {
                                let is_active = self.conversation_store.active_idx == Some(i);
                                let item_bg = if is_active {
                                    colors.selection_bg()
                                } else {
                                    egui::Color32::TRANSPARENT
                                };

                                let frame_response = egui::Frame::new()
                                    .fill(item_bg)
                                    .corner_radius(CornerRadius::same(4))
                                    .inner_margin(egui::Margin::symmetric(6, 3))
                                    .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            // Pin indicator
                                            if thread.pinned {
                                                ui.label(
                                                    RichText::new(egui_nerdfonts::regular::PIN)
                                                        .color(accent)
                                                        .size(typography::XS),
                                                );
                                            }

                                            // Thread name
                                            let name_color = if is_active {
                                                text_primary
                                            } else {
                                                text_secondary
                                            };
                                            ui.label(
                                                RichText::new(&thread.name)
                                                    .color(name_color)
                                                    .size(typography::SM),
                                            );

                                            // Message count
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    // Delete button (on hover)
                                                    let del_btn = ui.add(
                                                        egui::Button::new(
                                                            RichText::new(
                                                                egui_nerdfonts::regular::CLOSE,
                                                            )
                                                            .size(10.0)
                                                            .color(text_tertiary),
                                                        )
                                                        .frame(false),
                                                    );
                                                    if del_btn.on_hover_text("Delete").clicked() {
                                                        delete_idx = Some(i);
                                                    }

                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{}",
                                                            thread.messages.len()
                                                        ))
                                                        .color(text_tertiary)
                                                        .size(typography::XS),
                                                    );
                                                },
                                            );
                                        });
                                    });

                                // Click to switch
                                let item_rect = frame_response.response.rect;
                                let item_response = ui.interact(
                                    item_rect,
                                    ui.id().with("thread_item").with(i),
                                    egui::Sense::click(),
                                );
                                if item_response.clicked() && !is_active {
                                    switch_to = Some(i);
                                }
                                if item_response.hovered() && !is_active {
                                    ui.painter().rect_filled(
                                        item_rect,
                                        CornerRadius::same(4),
                                        colors.hover_bg(),
                                    );
                                }
                            }

                            // Handle actions after iteration
                            if let Some(idx) = delete_idx {
                                let was_active = self.conversation_store.active_idx == Some(idx);
                                self.conversation_store.delete_thread(idx);
                                if was_active {
                                    self.load_thread_messages();
                                }
                            }
                            if let Some(idx) = switch_to {
                                // Save current before switching
                                if !self.messages.is_empty() {
                                    self.sync_messages_to_thread();
                                }
                                self.conversation_store.switch_to(idx);
                                self.load_thread_messages();
                                self.conversation_store.picker_open = false;
                            }
                        });
                });
        }
    }

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

                    if clear_btn.on_hover_text("New conversation").clicked() {
                        self.start_new_conversation();
                    }
                }
            });
        });
        ui.add_space(6.0);

        // Thread picker row
        self.render_thread_picker(ui);

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
                // Capture content area bounds for selection border (excludes scrollbar)
                let content_rect = ui.max_rect();
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
                        let is_selected = self.selected_message == Some(i);

                        let before_y = ui.cursor().min.y;
                        self.render_message(ui, &message, &colors);
                        let after_y = ui.cursor().min.y;

                        // Visual selection highlight for vim j/k navigation
                        if is_selected {
                            // Use content_rect for horizontal bounds (excludes scrollbar)
                            let rect = egui::Rect::from_min_max(
                                egui::pos2(content_rect.left() + 4.0, before_y - 2.0),
                                egui::pos2(content_rect.right() - 4.0, after_y + 2.0),
                            );
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(6),
                                Stroke::new(1.5, self.theme.accent_primary()),
                                egui::StrokeKind::Inside,
                            );
                            if self.scroll_to_selected {
                                ui.scroll_to_rect(rect, Some(egui::Align::Center));
                                self.scroll_to_selected = false;
                            }
                        }
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

                // Auto-scroll during streaming if user is at bottom
                let should_scroll = self.scroll_to_bottom
                    || (self.is_waiting
                        && self.is_at_bottom
                        && self.response_text.len() != self.last_response_len);
                if should_scroll {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    self.scroll_to_bottom = false;
                }
                self.last_response_len = self.response_text.len();
            });

        // Track scroll position to detect if user scrolled away from bottom
        let scroll_state = ScrollState::from_scroll_output(
            scroll_output.content_size,
            scroll_output.inner_rect,
            scroll_output.state.offset,
        );
        self.is_at_bottom = !scroll_state.can_scroll_down;

        // Render scroll shadows
        let shadow_config = ScrollShadowConfig::default()
            .with_color(self.theme.bg_surface())
            .with_opacity(0.6);
        render_scroll_shadows(ui, scroll_output.inner_rect, scroll_state, shadow_config);

        // "Jump to latest" floating button when scrolled up during streaming
        if self.is_waiting && scroll_state.can_scroll_down {
            let btn_width = 120.0;
            let btn_pos = egui::pos2(
                scroll_output.inner_rect.center().x - btn_width / 2.0,
                scroll_output.inner_rect.bottom() - 36.0,
            );
            let btn_rect = egui::Rect::from_min_size(btn_pos, egui::vec2(btn_width, 26.0));

            // Background pill
            ui.painter()
                .rect_filled(btn_rect, CornerRadius::same(13), self.theme.bg_elevated());
            ui.painter().rect_stroke(
                btn_rect,
                CornerRadius::same(13),
                Stroke::new(1.0, self.theme.border_subtle()),
                egui::StrokeKind::Outside,
            );

            // Label + arrow
            let text_galley = ui.painter().layout_no_wrap(
                format!("{} Jump to latest", egui_nerdfonts::regular::ARROW_DOWN),
                typography::proportional(typography::SM),
                accent,
            );
            let text_pos = egui::pos2(
                btn_rect.center().x - text_galley.size().x / 2.0,
                btn_rect.center().y - text_galley.size().y / 2.0,
            );
            ui.painter().galley(text_pos, text_galley, accent);

            // Click interaction
            let btn_response = ui.interact(
                btn_rect,
                ui.id().with("jump_to_latest"),
                egui::Sense::click(),
            );
            if btn_response.clicked() {
                self.scroll_to_bottom = true;
                self.is_at_bottom = true;
            }
            if btn_response.hovered() {
                ui.painter()
                    .rect_filled(btn_rect, CornerRadius::same(13), colors.hover_bg());
                // Re-draw text on hover
                let text_galley = ui.painter().layout_no_wrap(
                    format!("{} Jump to latest", egui_nerdfonts::regular::ARROW_DOWN),
                    typography::proportional(typography::SM),
                    accent,
                );
                ui.painter().galley(text_pos, text_galley, accent);
            }
        }

        // Search bar (vim / mode)
        if self.search_active {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("/")
                        .color(self.theme.accent_primary())
                        .size(typography::SM)
                        .monospace(),
                );
                let search_id = ui.id().with("agent_search");
                let response = ui.add(
                    TextEdit::singleline(&mut self.search_query)
                        .id(search_id)
                        .desired_width(ui.available_width() - 32.0)
                        .font(typography::proportional(typography::SM))
                        .text_color(self.theme.text_primary())
                        .frame(false),
                );
                // Auto-focus the search input
                if !response.has_focus() {
                    response.request_focus();
                }
                // Enter - execute search, Escape - cancel
                if response.lost_focus() {
                    let key_enter = ui.ctx().input(|i| i.key_pressed(Key::Enter));
                    if key_enter {
                        // Compute search matches
                        let query = self.search_query.to_lowercase();
                        self.search_matches = self
                            .messages
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| m.content.to_lowercase().contains(&query))
                            .map(|(i, _)| i)
                            .collect();
                        self.search_match_idx = 0;
                        if !self.search_matches.is_empty() {
                            self.selected_message = Some(self.search_matches[0]);
                            self.scroll_to_selected = true;
                        }
                    }
                    self.search_active = false;
                }
            });
        }

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
                    // Intercept bare Enter to submit (before TextEdit consumes it).
                    // Shift+Enter will insert a newline naturally.
                    // But first check if popup is handling the key
                    let mut enter_to_submit = false;
                    let mut popup_handled_key = false;
                    let input_id = ui.make_persistent_id("agent_panel_input");
                    let has_input_focus = ui.ctx().memory(|mem| mem.has_focus(input_id));

                    // Handle popup keyboard input first (before Enter-to-submit)
                    if has_input_focus
                        && (self.mention_popup.active || self.slash_command_popup.active)
                    {
                        ui.ctx().input_mut(|input| {
                            // Navigate up
                            if input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                                || input.consume_key(egui::Modifiers::CTRL, Key::P)
                                || input.consume_key(egui::Modifiers::CTRL, Key::K)
                            {
                                if self.mention_popup.active {
                                    self.mention_popup.select_prev();
                                } else {
                                    self.slash_command_popup.select_prev();
                                }
                                popup_handled_key = true;
                            }
                            // Navigate down
                            else if input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                                || input.consume_key(egui::Modifiers::CTRL, Key::N)
                                || input.consume_key(egui::Modifiers::CTRL, Key::J)
                            {
                                if self.mention_popup.active {
                                    self.mention_popup.select_next();
                                } else {
                                    self.slash_command_popup.select_next();
                                }
                                popup_handled_key = true;
                            }
                            // Select with Enter or Tab
                            else if input.consume_key(egui::Modifiers::NONE, Key::Enter)
                                || input.consume_key(egui::Modifiers::NONE, Key::Tab)
                            {
                                if self.mention_popup.active {
                                    if let Some(metric) = self.mention_popup.selected() {
                                        let at_pos = self.mention_popup.get_at_position();
                                        let metric_name = metric.to_string();
                                        let prefix = &self.input_text[..at_pos];
                                        self.input_text = format!("{prefix}{metric_name} ");
                                        self.prev_input_text = self.input_text.clone();
                                    }
                                    self.mention_popup.close();
                                } else if let Some(cmd) = self.slash_command_popup.selected() {
                                    let slash_pos = self.slash_command_popup.get_slash_position();
                                    let cmd_name = cmd.name;
                                    let prefix = &self.input_text[..slash_pos];
                                    self.input_text = format!("{prefix}/{cmd_name} ");
                                    self.prev_input_text = self.input_text.clone();
                                    self.slash_command_popup.close();
                                }
                                popup_handled_key = true;
                            }
                            // Cancel with Escape
                            else if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                                self.mention_popup.close();
                                self.slash_command_popup.close();
                                popup_handled_key = true;
                            }
                        });
                    }

                    // Only check for Enter-to-submit if popup didn't handle it
                    if has_input_focus && !popup_handled_key {
                        ui.ctx().input_mut(|input| {
                            if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                                enter_to_submit = true;
                            }
                        });
                    }

                    // Auto-expand height: base 22px, grows with line count, max 120px
                    let line_count = self.input_text.lines().count().max(1);
                    let input_height = (line_count as f32 * 16.0).clamp(22.0, 120.0);

                    let response = ui.add_sized(
                        Vec2::new(ui.available_width() - 50.0, input_height),
                        TextEdit::multiline(&mut self.input_text)
                            .id(input_id)
                            .hint_text(
                                RichText::new(hint_text)
                                    .color(text_tertiary)
                                    .size(typography::MD),
                            )
                            .frame(false)
                            .font(typography::proportional(typography::MD))
                            .desired_rows(1),
                    );

                    // Store rect for popup positioning
                    let input_rect = response.rect;

                    // Input change detection for @ and / triggers
                    let input_len = self.input_text.len();
                    let prev_len = self.prev_input_text.len();

                    if input_len > prev_len {
                        // Characters were added
                        let new_chars = &self.input_text[prev_len..];

                        // Check for "/" (only if mention popup is not active)
                        if !self.mention_popup.active {
                            if new_chars.contains('/') && !self.slash_command_popup.active {
                                if let Some(slash_pos) = self.input_text.rfind('/') {
                                    let is_at_start = slash_pos == 0;
                                    let is_after_space = slash_pos > 0
                                        && self.input_text.chars().nth(slash_pos - 1) == Some(' ');
                                    if is_at_start || is_after_space {
                                        self.slash_command_popup.start(slash_pos);
                                    }
                                }
                            } else if self.slash_command_popup.active {
                                let query = &self.input_text
                                    [self.slash_command_popup.get_slash_position() + 1..];
                                if query.contains(' ') || query.contains('\n') {
                                    self.slash_command_popup.close();
                                } else {
                                    self.slash_command_popup.set_query(query);
                                }
                            }
                        }

                        // Check for "@" (only if slash popup is not active)
                        if !self.slash_command_popup.active {
                            if new_chars.contains('@') {
                                if let Some(at_pos) = self.input_text.rfind('@') {
                                    self.mention_popup.start(at_pos);
                                }
                            } else if self.mention_popup.active {
                                let query =
                                    &self.input_text[self.mention_popup.get_at_position() + 1..];
                                if query.contains(' ') || query.contains('\n') {
                                    self.mention_popup.close();
                                } else {
                                    self.mention_popup.set_query(query);
                                }
                            }
                        }
                    } else if input_len < prev_len {
                        // Characters were deleted
                        if self.slash_command_popup.active {
                            let slash_pos = self.slash_command_popup.get_slash_position();
                            if self.input_text.len() <= slash_pos {
                                self.slash_command_popup.close();
                            } else {
                                let query = &self.input_text[slash_pos + 1..];
                                self.slash_command_popup.set_query(query);
                            }
                        }
                        if self.mention_popup.active {
                            let at_pos = self.mention_popup.get_at_position();
                            if self.input_text.len() <= at_pos {
                                self.mention_popup.close();
                            } else {
                                let query = &self.input_text[at_pos + 1..];
                                self.mention_popup.set_query(query);
                            }
                        }
                    }

                    self.prev_input_text = self.input_text.clone();

                    // Render popups
                    if self.mention_popup.active {
                        let cursor_x = self.calculate_popup_cursor_x(input_rect);
                        self.mention_popup.show(ui, input_rect, cursor_x);
                    }
                    if self.slash_command_popup.active {
                        let cursor_x = self.calculate_popup_cursor_x(input_rect);
                        self.slash_command_popup.show(ui, input_rect, cursor_x);
                    }

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
                    // Skip if popup already handled Escape
                    if response.has_focus() && !popup_handled_key {
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

                    // Handle Enter to send (Shift+Enter inserts newline)
                    if enter_to_submit {
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
                // Apply fade-in opacity for newly streamed text
                let stream_opacity = if message.is_streaming {
                    self.stream_fade_start
                        .map(|start| {
                            let elapsed_ms = start.elapsed().as_millis() as f32;
                            // Fade from 0.6 to 1.0 over 150ms
                            (0.6 + 0.4 * (elapsed_ms / 150.0).min(1.0)).min(1.0)
                        })
                        .unwrap_or(1.0)
                } else {
                    1.0
                };

                let content_response = egui::Frame::new()
                    .fill(msg_bg)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width() - 32.0);
                        if stream_opacity < 1.0 {
                            ui.set_opacity(stream_opacity);
                        }
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

                if message.role == MessageRole::Assistant {
                    // Render assistant messages as markdown
                    super::markdown_renderer::render_markdown(ui, &normalized, self.theme);
                } else {
                    ui.label(
                        RichText::new(normalized)
                            .color(text_primary)
                            .size(typography::MD),
                    );
                }
            }
        }

        // Render inline content blocks (charts, source, search results)
        let mut pending_diff: Option<(String, String)> = None;
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
                InlineContent::Diff(diff) => {
                    if self.render_inline_diff(ui, diff, colors) {
                        // Store commit info for opening diff viewer
                        pending_diff =
                            Some((diff.commit_hash.clone(), diff.commit_message.clone()));
                    }
                }
            }
        }
        // Set pending diff viewer action (if any inline diff was clicked)
        if let Some((hash, message)) = pending_diff {
            self.pending_diff_viewer = Some((hash, message));
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
    fn render_inline_source(
        &mut self,
        ui: &mut egui::Ui,
        source: &InlineSource,
        colors: &ChatColors,
    ) {
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

                    // "Open" dropdown button (native only)
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let btn = ui.add(
                                egui::Button::image_and_text(
                                    egui::Image::new(APP_GHOSTTY.as_image_source())
                                        .fit_to_exact_size(egui::vec2(12.0, 12.0)),
                                    RichText::new(format!(
                                        "Open {}",
                                        egui_nerdfonts::regular::CHEVRON_DOWN
                                    ))
                                    .size(typography::XS)
                                    .color(text_secondary),
                                )
                                .fill(colors.hover_bg())
                                .stroke(egui::Stroke::new(1.0, self.theme.border_subtle()))
                                .corner_radius(4.0),
                            );

                            if btn.clicked() {
                                let popup_pos = btn.rect.left_bottom();
                                self.file_opener.open_with_base(
                                    popup_pos,
                                    std::path::PathBuf::from(&source.file_path),
                                    self.repo_path.clone(),
                                );
                            }
                        });
                    }

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

    /// Render an inline git diff within a message.
    /// Returns true if the user clicked to open the full diff viewer.
    fn render_inline_diff(
        &mut self,
        ui: &mut egui::Ui,
        diff: &InlineDiff,
        colors: &ChatColors,
    ) -> bool {
        use egui_nerdfonts::regular;

        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();

        // GitHub-style diff colors
        let addition_bg = Color32::from_rgba_unmultiplied(46, 160, 67, 25);
        let deletion_bg = Color32::from_rgba_unmultiplied(248, 81, 73, 25);
        let addition_text = Color32::from_rgb(63, 185, 80);
        let deletion_text = Color32::from_rgb(248, 81, 73);
        let hunk_text = Color32::from_rgb(130, 80, 223);

        let mut open_full_diff = false;

        // Diff container with premium styling
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                // Header with commit info
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(regular::SOURCE_COMMIT)
                            .color(self.theme.accent_primary())
                            .size(14.0),
                    );
                    ui.add_space(6.0);

                    // Commit hash - clickable to open full diff
                    if !diff.commit_hash.is_empty() && diff.commit_hash != "working" {
                        let hash_response = ui.add(
                            egui::Label::new(
                                RichText::new(&diff.commit_hash)
                                    .color(self.theme.accent_primary())
                                    .size(typography::SM)
                                    .family(egui::FontFamily::Monospace)
                                    .underline(),
                            )
                            .selectable(false)
                            .sense(egui::Sense::click()),
                        );
                        if hash_response.clicked() {
                            open_full_diff = true;
                        }
                        if hash_response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        hash_response.on_hover_text("Click to open full diff viewer");
                        ui.add_space(8.0);
                    } else if !diff.commit_hash.is_empty() {
                        ui.label(
                            RichText::new(&diff.commit_hash)
                                .color(self.theme.accent_primary())
                                .size(typography::SM)
                                .family(egui::FontFamily::Monospace),
                        );
                        ui.add_space(8.0);
                    }

                    // Commit message (truncated)
                    let message = if diff.commit_message.len() > 50 {
                        format!("{}...", &diff.commit_message[..47])
                    } else {
                        diff.commit_message.clone()
                    };
                    ui.label(
                        RichText::new(message)
                            .color(text_primary)
                            .size(typography::SM),
                    );

                    // Stats and "Open" button on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // "Open" link to open full diff
                        if diff.commit_hash != "working" {
                            let open_response = ui.add(
                                egui::Label::new(
                                    RichText::new(format!("{} Open", regular::LINK_EXTERNAL))
                                        .color(text_tertiary)
                                        .size(typography::XS),
                                )
                                .selectable(false)
                                .sense(egui::Sense::click()),
                            );
                            if open_response.clicked() {
                                open_full_diff = true;
                            }
                            if open_response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            open_response.on_hover_text("Open full diff viewer");
                            ui.add_space(8.0);
                        }

                        ui.label(
                            RichText::new(format!("-{}", diff.deletions))
                                .color(deletion_text)
                                .size(typography::XS),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("+{}", diff.additions))
                                .color(addition_text)
                                .size(typography::XS),
                        );
                    });
                });

                ui.add_space(8.0);

                // Show up to 3 files, with limited lines per file
                let max_files = 3;
                let max_lines_per_file = 12;

                for (file_idx, file_diff) in diff.file_diffs.iter().take(max_files).enumerate() {
                    if file_idx > 0 {
                        ui.add_space(8.0);
                    }

                    // File header
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(regular::FILE_DOCUMENT)
                                .color(text_secondary)
                                .size(12.0),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(&file_diff.path)
                                .color(text_primary)
                                .size(typography::XS)
                                .family(egui::FontFamily::Monospace),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(format!(
                                "+{} -{}",
                                file_diff.additions, file_diff.deletions
                            ))
                            .color(text_tertiary)
                            .size(typography::XS),
                        );
                    });

                    ui.add_space(4.0);

                    // Diff lines in a code-style frame
                    egui::Frame::new()
                        .fill(self.theme.bg_surface())
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            for (line_idx, line) in
                                file_diff.lines.iter().take(max_lines_per_file).enumerate()
                            {
                                let (bg, text_color, prefix) = match line.kind {
                                    InlineDiffLineKind::Addition => {
                                        (addition_bg, addition_text, "+")
                                    }
                                    InlineDiffLineKind::Deletion => {
                                        (deletion_bg, deletion_text, "-")
                                    }
                                    InlineDiffLineKind::Context => {
                                        (Color32::TRANSPARENT, text_tertiary, " ")
                                    }
                                    InlineDiffLineKind::Hunk => {
                                        (Color32::TRANSPARENT, hunk_text, "@")
                                    }
                                };

                                let response = ui.horizontal(|ui| {
                                    // Line number gutter
                                    let line_num = line.new_line.or(line.old_line).unwrap_or(0);
                                    if line.kind != InlineDiffLineKind::Hunk && line_num > 0 {
                                        ui.label(
                                            RichText::new(format!("{line_num:>4}"))
                                                .color(text_tertiary.gamma_multiply(0.5))
                                                .size(typography::XS)
                                                .family(egui::FontFamily::Monospace),
                                        );
                                    } else {
                                        ui.label(
                                            RichText::new("    ")
                                                .size(typography::XS)
                                                .family(egui::FontFamily::Monospace),
                                        );
                                    }

                                    ui.add_space(4.0);

                                    // Prefix (+/-/space)
                                    ui.label(
                                        RichText::new(prefix)
                                            .color(text_color)
                                            .size(typography::XS)
                                            .family(egui::FontFamily::Monospace),
                                    );

                                    // Content
                                    let content = if line.content.len() > 80 {
                                        format!("{}...", &line.content[..77])
                                    } else {
                                        line.content.clone()
                                    };
                                    ui.label(
                                        RichText::new(content)
                                            .color(text_color)
                                            .size(typography::XS)
                                            .family(egui::FontFamily::Monospace),
                                    );
                                });

                                // Background highlight for changed lines
                                if bg != Color32::TRANSPARENT {
                                    let rect = response.response.rect;
                                    ui.painter().rect_filled(
                                        rect.expand2(egui::vec2(4.0, 0.0)),
                                        0.0,
                                        bg,
                                    );
                                }

                                // Show truncation indicator
                                if line_idx == max_lines_per_file - 1
                                    && file_diff.lines.len() > max_lines_per_file
                                {
                                    ui.label(
                                        RichText::new(format!(
                                            "... {} more lines",
                                            file_diff.lines.len() - max_lines_per_file
                                        ))
                                        .color(text_tertiary)
                                        .size(typography::XS)
                                        .italics(),
                                    );
                                }
                            }
                        });
                }

                // "More files" indicator
                if diff.file_diffs.len() > max_files {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "... and {} more files",
                            diff.file_diffs.len() - max_files
                        ))
                        .color(text_tertiary)
                        .size(typography::XS)
                        .italics(),
                    );
                }
            });

        open_full_diff
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

    /// Ensure there's an active conversation thread; create one if needed.
    pub(super) fn ensure_active_thread(&mut self) {
        if self.conversation_store.active_idx.is_none() {
            self.conversation_store.new_thread();
        }
    }

    /// Save current messages to the active conversation thread.
    pub(super) fn sync_messages_to_thread(&mut self) {
        use super::conversation_store::SavedMessage;
        if let Some(thread) = self.conversation_store.active_thread_mut() {
            thread.messages = self
                .messages
                .iter()
                .filter(|m| !m.is_streaming)
                .map(|m| SavedMessage {
                    role: m.role,
                    content: m.content.clone(),
                })
                .collect();
            thread.auto_name_from_messages();
        }
        self.conversation_store.save_active();
    }

    /// Load messages from a thread into the panel.
    fn load_thread_messages(&mut self) {
        self.messages.clear();
        self.current_activities.clear();
        self.response_text.clear();
        self.selected_message = None;
        if let Some(thread) = self.conversation_store.active_thread() {
            self.messages = thread
                .messages
                .iter()
                .map(|m| ChatMessage {
                    role: m.role,
                    content: m.content.clone(),
                    is_streaming: false,
                    inline_blocks: Vec::new(),
                })
                .collect();
        }
        self.scroll_to_bottom = true;
    }

    /// Start a new conversation thread, saving the current one first.
    fn start_new_conversation(&mut self) {
        // Save current thread if it has messages
        if !self.messages.is_empty() {
            self.sync_messages_to_thread();
        }
        self.conversation_store.new_thread();
        self.messages.clear();
        self.current_activities.clear();
        self.response_text.clear();
        self.selected_message = None;
        self.scroll_to_bottom = true;
    }
}
// Streaming methods (send_message, cancel_request, poll_streaming_response)
// are in the `agent_streaming` sibling module.
