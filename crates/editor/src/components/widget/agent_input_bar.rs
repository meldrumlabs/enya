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
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

#[cfg(not(target_arch = "wasm32"))]
use enya_ai::{AcpClient, AgentEvent};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::overlay::AgentCommand;
#[cfg(not(target_arch = "wasm32"))]
use crate::components::overlay::{parse_commands, strip_command_blocks};
use crate::components::util::ActivityItem;
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::ActivityType;
use crate::components::util::finder_utils::{OverlayColors, OverlayStyle};

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
}

/// Context pane information for display
#[derive(Debug, Clone)]
pub struct ContextPane {
    /// Tile ID for the pane
    pub tile_id: TileId,
    /// Display name
    pub name: String,
}

/// State for the @ mention popup
#[derive(Default)]
struct MentionPopup {
    /// Whether the popup is visible
    active: bool,
    /// The search query (text after @)
    query: String,
    /// Position in input where @ was typed
    at_position: usize,
    /// Available metrics to search
    metrics: Vec<String>,
    /// Filtered results with scores
    results: Vec<(String, i64, Vec<usize>)>,
    /// Selected index in results
    selected_index: usize,
    /// Fuzzy matcher
    matcher: Matcher,
}

impl MentionPopup {
    fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            ..Default::default()
        }
    }

    /// Start the mention popup at the given cursor position
    fn start(&mut self, at_position: usize) {
        self.active = true;
        self.at_position = at_position;
        self.query.clear();
        self.selected_index = 0;
        self.refresh_results();
    }

    /// Close the popup
    fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.selected_index = 0;
        self.results.clear();
    }

    /// Update the search query
    fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.refresh_results();
    }

    /// Set available metrics
    fn set_metrics(&mut self, metrics: Vec<String>) {
        self.metrics = metrics;
        if self.active {
            self.refresh_results();
        }
    }

    /// Refresh filtered results based on query
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.query.is_empty() {
            // Show all metrics when query is empty, sorted alphabetically
            let mut sorted = self.metrics.to_vec();
            sorted.sort();
            for metric in sorted.into_iter().take(10) {
                self.results.push((metric, 0, Vec::new()));
            }
        } else {
            // Fuzzy match
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );

            let mut indices: Vec<u32> = Vec::new();
            let mut buf = Vec::new();
            for metric in &self.metrics {
                indices.clear();
                let haystack = Utf32Str::new(metric, &mut buf);

                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    self.results.push((
                        metric.clone(),
                        i64::from(score),
                        indices.iter().map(|&i| i as usize).collect(),
                    ));
                }
            }
            // Sort by score descending
            self.results.sort_by(|a, b| b.1.cmp(&a.1));
            self.results.truncate(10);
        }

        // Reset selection if out of bounds
        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }
    }

    /// Move selection up
    fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    fn select_next(&mut self) {
        if self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }
    }

    /// Get the currently selected metric
    fn selected(&self) -> Option<&str> {
        self.results
            .get(self.selected_index)
            .map(|(s, _, _)| s.as_str())
    }
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
    /// Current AI provider name (e.g., "Claude", "Codex")
    provider_name: String,
    /// Last response text (for display)
    response_text: String,
    /// Display text (response with command blocks stripped)
    display_text: String,
    /// Whether the response is expanded (for long responses)
    response_expanded: bool,
    /// Processing status message
    processing_status: String,
    /// Processing elapsed time
    processing_start: Option<std::time::Instant>,
    /// Current activities (tool use, thinking, etc.)
    activities: Vec<ActivityItem>,
    /// Last action that can be undone
    can_undo: bool,
    /// Pending commands parsed from AI response
    pending_commands: Vec<AgentCommand>,
    /// @ mention popup state for metric selection
    mention_popup: MentionPopup,
    /// Previous input text (for detecting @ insertion)
    prev_input: String,
    /// Whether to move cursor to end of input on next frame
    cursor_to_end: bool,
    /// Event receiver for streaming AI responses (native only)
    #[cfg(not(target_arch = "wasm32"))]
    event_receiver: Option<Receiver<AgentEvent>>,
    /// Tokio runtime handle for spawning async tasks
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
            provider_name: "Claude".to_string(),
            response_text: String::new(),
            display_text: String::new(),
            response_expanded: false,
            processing_status: String::new(),
            processing_start: None,
            activities: Vec::new(),
            can_undo: false,
            pending_commands: Vec::new(),
            mention_popup: MentionPopup::new(),
            prev_input: String::new(),
            cursor_to_end: false,
            #[cfg(not(target_arch = "wasm32"))]
            event_receiver: None,
            #[cfg(not(target_arch = "wasm32"))]
            runtime_handle: None,
        }
    }

    /// Create a new agent input bar with a tokio runtime handle
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_with_runtime(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            state: AgentInputState::Ready,
            input: String::new(),
            focus_input: true,
            theme: AppTheme::default(),
            context_panes: Vec::new(),
            provider_name: "Claude".to_string(),
            response_text: String::new(),
            display_text: String::new(),
            response_expanded: false,
            processing_status: String::new(),
            processing_start: None,
            activities: Vec::new(),
            can_undo: false,
            pending_commands: Vec::new(),
            mention_popup: MentionPopup::new(),
            prev_input: String::new(),
            cursor_to_end: false,
            event_receiver: None,
            runtime_handle: Some(runtime_handle),
        }
    }

    /// Set the current theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the current AI provider name (e.g., "Claude", "Codex")
    pub fn set_provider_name(&mut self, name: &str) {
        self.provider_name = name.to_string();
    }

    /// Set available metrics for @ mention autocomplete
    pub fn set_available_metrics(&mut self, metrics: Vec<String>) {
        self.mention_popup.set_metrics(metrics);
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

    /// Reset to ready state (for entering agent mode)
    pub fn reset(&mut self) {
        self.state = AgentInputState::Ready;
        self.input.clear();
        self.prev_input.clear();
        self.focus_input = true;
        self.response_text.clear();
        self.display_text.clear();
        self.response_expanded = false;
        self.processing_status.clear();
        self.processing_start = None;
        self.activities.clear();
        self.can_undo = false;
        self.pending_commands.clear();
        self.mention_popup.close();
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
        self.processing_start = Some(std::time::Instant::now());
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

    /// Show the agent input bar
    pub fn show(&mut self, ui: &mut egui::Ui) -> AgentInputBarResult {
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
        let height = match self.state {
            AgentInputState::Ready => base_height,
            AgentInputState::Typing => base_height + 80.0, // Room for suggestions
            AgentInputState::Processing => base_height + 24.0,
            AgentInputState::Response => {
                if self.response_text.len() > 100 || self.response_text.contains('\n') {
                    expanded_height + 60.0
                } else {
                    base_height + 20.0
                }
            }
        };

        // Amber accent for Agent mode
        let accent = match self.theme {
            AppTheme::Light => Color32::from_rgb(245, 158, 11), // Amber
            AppTheme::Dark => Color32::from_rgb(251, 191, 36),  // Bright amber
        };

        // Inner glow color for glass effect
        let inner_glow = match self.theme {
            AppTheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 40),
            AppTheme::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        };

        // Create frame with premium glass styling
        let frame = style
            .frame()
            .inner_margin(egui::Margin::symmetric(16, 10))
            .corner_radius(12.0);

        let frame_response = frame.show(ui, |ui| {
            ui.set_min_height(height);
            ui.set_width(ui.available_width());

            ui.vertical(|ui| {
                // Top row: Provider badge + Context + Input
                ui.horizontal(|ui| {
                    // Provider badge with emerald accent
                    let badge_bg = match self.theme {
                        AppTheme::Light => accent.gamma_multiply(0.15),
                        AppTheme::Dark => accent.gamma_multiply(0.2),
                    };

                    egui::Frame::new()
                        .fill(badge_bg)
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(&self.provider_name)
                                    .color(accent)
                                    .size(typography::SM)
                                    .strong(),
                            );
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
                        let ctx_badge_bg = match self.theme {
                            AppTheme::Light => colors.badge_bg,
                            AppTheme::Dark => colors.badge_bg.gamma_multiply(0.8),
                        };

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
                                    RichText::new("+").color(btn_color).size(typography::SM),
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
                                    RichText::new("−").color(btn_color).size(typography::SM),
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
                                    RichText::new("×").color(btn_color).size(typography::SM),
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

                    // Main content based on state
                    match self.state {
                        AgentInputState::Ready => {
                            self.show_ready_state(ui, &colors, &mut result);
                        }
                        AgentInputState::Typing => {
                            self.show_typing_state(ui, &colors, &mut result);
                        }
                        AgentInputState::Processing => {
                            self.show_processing_state(ui, &colors, accent);
                        }
                        AgentInputState::Response => {
                            self.show_response_state(ui, &colors, &mut result);
                        }
                    }
                });

                // Additional rows for expanded content
                if self.state == AgentInputState::Typing && !self.input.is_empty() {
                    ui.add_space(8.0);
                    self.show_suggestions(ui, &colors);
                }

                if self.state == AgentInputState::Processing && !self.activities.is_empty() {
                    ui.add_space(4.0);
                    self.show_activities(ui, &colors);
                }

                if self.state == AgentInputState::Response && self.response_expanded {
                    ui.add_space(8.0);
                    self.show_expanded_response(ui, &colors);
                }
            });
        });

        // Draw inner highlight for glass effect
        let rect = frame_response.response.rect;
        if style.inner_highlight().is_some() {
            let highlight_rect = egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(1.0, 1.0),
                egui::vec2(rect.width() - 2.0, 1.0),
            );
            ui.painter().rect_filled(highlight_rect, 10.0, inner_glow);
        }

        // Check for @ mention trigger
        self.check_mention_trigger();

        // Show mention popup if active
        if self.mention_popup.active {
            self.show_mention_popup(ui, &colors, rect);
        }

        // Handle keyboard input
        self.handle_keyboard(ui.ctx(), &mut result);

        // Drain any pending commands into the result
        if !self.pending_commands.is_empty() {
            result.commands = std::mem::take(&mut self.pending_commands);
        }

        result
    }

    /// Check if user typed @ to trigger mention popup
    fn check_mention_trigger(&mut self) {
        // Find if a new @ was just typed
        let input_len = self.input.len();
        let prev_len = self.prev_input.len();

        if input_len > prev_len {
            // Character(s) were added
            let new_chars = &self.input[prev_len..];
            if new_chars.contains('@') {
                // Find position of the new @
                if let Some(at_pos) = self.input.rfind('@') {
                    self.mention_popup.start(at_pos);
                }
            } else if self.mention_popup.active {
                // Update query: extract text after @
                let query = &self.input[self.mention_popup.at_position + 1..];
                // Close if there's a space or the @ was deleted
                if query.contains(' ') || query.contains('\n') {
                    self.mention_popup.close();
                } else {
                    self.mention_popup.set_query(query);
                }
            }
        } else if input_len < prev_len && self.mention_popup.active {
            // Character(s) were deleted
            if self.input.len() <= self.mention_popup.at_position {
                // The @ was deleted
                self.mention_popup.close();
            } else {
                // Update query
                let query = &self.input[self.mention_popup.at_position + 1..];
                self.mention_popup.set_query(query);
            }
        }

        self.prev_input = self.input.clone();
    }

    /// Show the mention popup for selecting metrics
    fn show_mention_popup(
        &self,
        ui: &mut egui::Ui,
        _colors: &OverlayColors,
        input_rect: egui::Rect,
    ) {
        use crate::ui::palette;

        if self.mention_popup.results.is_empty() {
            return;
        }

        let text_col = text_color(self.theme);
        let popup_width = 520.0; // Wider to accommodate long metric names
        let row_height = 32.0;
        let header_height = 32.0;
        let footer_height = 28.0;
        let results_height = self.mention_popup.results.len() as f32 * row_height;
        let popup_height = header_height + results_height.min(320.0) + footer_height;

        // Position popup above the input bar, centered horizontally
        let popup_pos = egui::pos2(
            input_rect.center().x - popup_width / 2.0,
            input_rect.top() - popup_height - 8.0,
        );

        // Premium Obsidian Glass styling
        let style = OverlayStyle::frosted_glass(self.theme);

        // Emerald accent colors
        let emerald_accent = match self.theme {
            AppTheme::Light => palette::accent::LIGHT,
            AppTheme::Dark => palette::accent::HOVER,
        };
        let emerald_primary = palette::accent::PRIMARY;

        // Accent color for hover/selection
        let accent_col = match self.theme {
            AppTheme::Light => palette::accent::LIGHT,
            AppTheme::Dark => palette::accent::PRIMARY,
        };

        // Separator color
        let separator_color = match self.theme {
            AppTheme::Light => palette::light_border::SUBTLE,
            AppTheme::Dark => palette::border::SUBTLE,
        };

        // Muted text
        let muted_text = text_col.gamma_multiply(0.6);
        let faint_text = text_col.gamma_multiply(0.4);

        // Use Area to render as a floating overlay (not clipped by parent)
        egui::Area::new(egui::Id::new("mention_popup"))
            .fixed_pos(popup_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                // Create the premium glass frame
                let frame_response = style
                    .frame()
                    .inner_margin(egui::Margin::symmetric(0, 6))
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header with emerald accent
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("@")
                                    .size(typography::MD)
                                    .color(emerald_primary)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Select metric")
                                    .size(typography::SM)
                                    .color(muted_text),
                            );
                        });

                        ui.add_space(6.0);

                        // Premium separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(6.0);

                        // Results
                        for (i, (metric, _score, match_positions)) in
                            self.mention_popup.results.iter().enumerate()
                        {
                            let is_selected = i == self.mention_popup.selected_index;

                            // Allocate row space
                            let (row_rect, response) = ui.allocate_exact_size(
                                egui::vec2(popup_width, row_height),
                                egui::Sense::hover(),
                            );
                            let is_hovered = response.hovered();

                            // Background - use subtle hover style like landing page
                            let bg_color = if is_selected {
                                accent_col.gamma_multiply(0.12)
                            } else if is_hovered {
                                text_col.gamma_multiply(0.05)
                            } else {
                                Color32::TRANSPARENT
                            };

                            if bg_color != Color32::TRANSPARENT {
                                ui.painter().rect_filled(row_rect, 6.0, bg_color);
                            }

                            // Emerald selection indicator bar
                            if is_selected {
                                let indicator_rect = egui::Rect::from_min_size(
                                    row_rect.left_top(),
                                    egui::vec2(3.0, row_height),
                                );
                                ui.painter().rect_filled(indicator_rect, 2.0, accent_col);
                            }

                            // Metric icon - use accent color on hover/select like landing page
                            let icon_pos = row_rect.left_center() + egui::vec2(18.0, 0.0);
                            let icon_color = if is_selected || is_hovered {
                                accent_col
                            } else {
                                text_col.gamma_multiply(0.6)
                            };
                            ui.painter().text(
                                icon_pos,
                                egui::Align2::LEFT_CENTER,
                                semantic_icons::metric_type_icon(metric),
                                typography::proportional(typography::MD),
                                icon_color,
                            );

                            // Metric name - use LayoutJob for highlighted text
                            let text_pos = row_rect.left_center() + egui::vec2(44.0, 0.0);
                            if match_positions.is_empty() {
                                let text_color = if is_selected {
                                    text_col
                                } else {
                                    text_col.gamma_multiply(0.9)
                                };
                                ui.painter().text(
                                    text_pos,
                                    egui::Align2::LEFT_CENTER,
                                    metric,
                                    typography::proportional(typography::MD),
                                    text_color,
                                );
                            } else {
                                // Build a LayoutJob with emerald-highlighted characters
                                let mut job = egui::text::LayoutJob::default();
                                for (idx, c) in metric.chars().enumerate() {
                                    let is_match = match_positions.contains(&idx);
                                    let color = if is_match {
                                        emerald_accent
                                    } else if is_selected {
                                        text_col
                                    } else {
                                        text_col.gamma_multiply(0.9)
                                    };
                                    job.append(
                                        &c.to_string(),
                                        0.0,
                                        egui::TextFormat {
                                            font_id: typography::proportional(typography::MD),
                                            color,
                                            ..Default::default()
                                        },
                                    );
                                }
                                let galley = ui.fonts_mut(|f| f.layout_job(job));
                                ui.painter().galley(
                                    egui::pos2(text_pos.x, text_pos.y - galley.size().y / 2.0),
                                    galley,
                                    text_col,
                                );
                            }
                        }

                        // Footer separator
                        ui.add_space(6.0);
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(6.0);

                        // Footer with keyboard hints
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("↑↓")
                                    .size(typography::XS)
                                    .color(emerald_accent),
                            );
                            ui.label(
                                RichText::new("navigate")
                                    .size(typography::XS)
                                    .color(faint_text),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("⏎")
                                    .size(typography::XS)
                                    .color(emerald_accent),
                            );
                            ui.label(
                                RichText::new("select")
                                    .size(typography::XS)
                                    .color(faint_text),
                            );
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("esc")
                                    .size(typography::XS)
                                    .color(emerald_accent),
                            );
                            ui.label(
                                RichText::new("cancel")
                                    .size(typography::XS)
                                    .color(faint_text),
                            );
                        });
                    });

                // Draw inner highlight for premium glass effect
                let rect = frame_response.response.rect;
                if let Some(highlight_color) = style.inner_highlight() {
                    let highlight_rect = egui::Rect::from_min_size(
                        rect.left_top() + egui::vec2(1.0, 1.0),
                        egui::vec2(rect.width() - 2.0, 1.5),
                    );
                    ui.painter().rect_filled(
                        highlight_rect,
                        style.corner_radius - 1.0,
                        highlight_color,
                    );
                }
            });
    }

    fn show_ready_state(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        _result: &mut AgentInputBarResult,
    ) {
        // Placeholder with quick key hints
        let hint_text = "w: what's wrong  y: why  c: compare  e: explain  ?: help";

        // Text input that looks like placeholder
        let response = ui.add(
            TextEdit::singleline(&mut self.input)
                .hint_text(hint_text)
                .desired_width(ui.available_width() - 20.0)
                .font(typography::proportional(typography::MD))
                .text_color(colors.text)
                .frame(false),
        );

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
        let text_edit_id = ui.make_persistent_id("agent_input_typing");
        let response = ui.add(
            TextEdit::singleline(&mut self.input)
                .id(text_edit_id)
                .desired_width(ui.available_width() - 60.0)
                .font(typography::proportional(typography::MD))
                .text_color(colors.text)
                .frame(false),
        );

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

        // Note: We don't auto-transition back to Ready when input is empty.
        // The user can press Escape to exit or go back.
    }

    fn show_processing_state(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        accent: Color32,
    ) {
        // Spinner
        ui.label(
            RichText::new(semantic_icons::status::LOADING)
                .color(accent)
                .size(typography::MD),
        );

        ui.add_space(8.0);

        // Status text with elapsed time
        let elapsed = self
            .processing_start
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);

        let status_text = if self.processing_status.is_empty() {
            format!("Processing... {elapsed}s")
        } else {
            format!("{} {}s", self.processing_status, elapsed)
        };

        ui.label(
            RichText::new(status_text)
                .color(colors.muted_text)
                .size(typography::MD),
        );

        // Request repaint to update elapsed time
        ui.ctx().request_repaint();
    }

    fn show_response_state(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        result: &mut AgentInputBarResult,
    ) {
        // Use display_text (with command blocks stripped) for display
        let display = &self.display_text;

        // Show response preview (first line or truncated)
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

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        // TODO: Implement real-time AI suggestions based on input
        // For now, show static suggestions based on common patterns

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Suggestions:")
                    .color(colors.muted_text)
                    .size(typography::SM),
            );
        });

        // Placeholder suggestions
        let suggestions = [
            "Analyze selected metrics",
            "Compare to yesterday",
            "Show error rate trend",
        ];

        for suggestion in suggestions.iter().take(3) {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(format!("{}  {suggestion}", semantic_icons::nav::RIGHT))
                        .color(colors.faint_text)
                        .size(typography::SM),
                );
            });
        }
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
        // Handle mention popup keyboard input first
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
                        let at_pos = self.mention_popup.at_position;
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
                    // Escape to cancel (handled by caller)
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                        result.exit_requested = true;
                    }
                }
                AgentInputState::Response => {
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
        // Transition to processing state
        self.state = AgentInputState::Processing;
        self.processing_status = "Sending to agent...".to_string();
        self.processing_start = Some(std::time::Instant::now());
        self.activities.clear();
        self.response_text.clear();
        self.display_text.clear();

        // Get working directory
        let working_dir = std::env::current_dir().ok();

        // Create Claude Code client
        let client = if let Some(handle) = &self.runtime_handle {
            AcpClient::claude_code_with_runtime(handle.clone())
        } else {
            AcpClient::claude_code()
        };

        // Send the query with system context (not concatenated into the query)
        let receiver = client.prompt_with_context(query, working_dir, None, system_context);
        self.event_receiver = Some(receiver);
    }

    /// Send a query (WASM version - not supported)
    #[cfg(target_arch = "wasm32")]
    pub fn send_query(&mut self, _query: &str, _context: Option<&str>) {
        self.state = AgentInputState::Response;
        self.response_text = "Claude Code CLI is not available in the browser.".to_string();
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
