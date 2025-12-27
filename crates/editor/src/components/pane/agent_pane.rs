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
use crate::components::util::{
    ActivityItem, ActivityType, AiModel, AiProvider, MessageRole, ResponseStatus, normalize_unicode,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{truncate_first_line, truncate_path_suffix};
use crate::theme::AppTheme;
use crate::ui::palette;
use crate::ui::typography;

/// Inline content block that can be embedded in chat messages.
///
/// These are rendered inline within the message, allowing the agent to
/// show visualizations and source code directly in the conversation.
#[derive(Debug, Clone)]
pub enum InlineContent {
    /// An inline time series chart with data
    Chart(InlineChart),
    /// An inline source code preview
    Source(InlineSource),
}

/// Inline time series chart data.
///
/// Contains the data needed to render a compact chart within a message.
#[derive(Debug, Clone)]
pub struct InlineChart {
    /// Chart title (e.g., metric name)
    pub title: String,
    /// Data series to plot
    pub series: Vec<super::time_series_chart::Series>,
    /// Optional height override (default: 120px)
    pub height: Option<f32>,
}

/// Inline source code preview.
///
/// Contains the data needed to render a syntax-highlighted code snippet.
#[derive(Debug, Clone)]
pub struct InlineSource {
    /// File path (relative)
    pub file_path: String,
    /// Target line number (1-indexed)
    pub line: usize,
    /// Source lines to display
    pub lines: Vec<String>,
    /// Start line number (1-indexed)
    pub start_line: usize,
    /// Language for syntax highlighting (e.g., "rust", "go")
    pub language: String,
    /// Pre-computed tree-sitter syntax highlighting data
    pub highlight_data: crate::components::util::SyntaxHighlightData,
}

/// A message in the chat history.
///
/// Note: This struct differs from `agent_panel::ChatMessage` by including
/// `inline_blocks` for inline visualizations and source previews.
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

/// Actions that can result from agent pane interaction
#[derive(Debug, Clone)]
pub enum AgentPaneAction {
    /// No action
    None,
    /// Commands parsed from agent response
    Commands(Vec<AgentCommand>),
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

    /// Add inline content to the last assistant message.
    ///
    /// This is used by the workspace to inject chart data or source previews
    /// into the agent's response after parsing commands.
    pub fn add_inline_content(&mut self, content: InlineContent) {
        // Find the last assistant message and add the inline content
        if let Some(msg) = self
            .messages
            .iter_mut()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
        {
            msg.inline_blocks.push(content);
            log::debug!("Added inline content to assistant message");
        } else {
            log::warn!("No assistant message found to add inline content");
        }
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

            // Clone messages for iteration to allow mutable access during rendering
            let messages = self.messages.to_vec();
            let activities = self.current_activities.to_vec();

            for (i, message) in messages.iter().enumerate() {
                self.render_message(ui, message, colors);
                ui.add_space(4.0);

                // Show activities after the last user message
                if Some(i) == last_user_idx && !activities.is_empty() {
                    for activity in &activities {
                        self.render_activity(ui, activity, colors);
                        ui.add_space(2.0);
                    }
                }
            }
        }
    }

    fn render_message(&mut self, ui: &mut egui::Ui, message: &ChatMessage, colors: &OverlayColors) {
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
        &mut self,
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
                let normalized = normalize_unicode(&display_content);
                ui.label(
                    RichText::new(normalized)
                        .color(colors.text)
                        .size(typography::MD),
                );
            }
        }

        // Render inline content blocks
        for block in &message.inline_blocks {
            ui.add_space(8.0);
            match block {
                InlineContent::Chart(chart) => {
                    self.render_inline_chart(ui, chart, colors);
                }
                InlineContent::Source(source) => {
                    self.render_inline_source(ui, source, colors);
                }
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

    /// Render an inline time series chart within a message.
    ///
    /// Uses the TimeSeriesChart component for consistent styling with dashboard charts.
    fn render_inline_chart(
        &mut self,
        ui: &mut egui::Ui,
        chart: &InlineChart,
        colors: &OverlayColors,
    ) {
        use super::time_series_chart::TimeSeriesChart;

        let chart_height = chart.height.unwrap_or(150.0);

        // Chart container with border
        egui::Frame::new()
            .fill(colors.elevated_bg)
            .corner_radius(6.0)
            .stroke(egui::Stroke::new(1.0, colors.separator))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                // Title header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(egui_nerdfonts::regular::CHART_LINE)
                            .color(colors.accent)
                            .size(12.0),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&chart.title)
                            .color(colors.text)
                            .size(typography::SM)
                            .strong(),
                    );
                });

                ui.add_space(4.0);

                if chart.series.is_empty() {
                    ui.label(
                        RichText::new("No data")
                            .color(colors.faint_text)
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
    ///
    /// Shows syntax-highlighted source with line numbers using tree-sitter.
    fn render_inline_source(
        &self,
        ui: &mut egui::Ui,
        source: &InlineSource,
        colors: &OverlayColors,
    ) {
        // Source container with border
        egui::Frame::new()
            .fill(colors.elevated_bg)
            .corner_radius(6.0)
            .stroke(egui::Stroke::new(1.0, colors.separator))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                // Header with file path and line number
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(egui_nerdfonts::regular::FILE_CODE)
                            .color(colors.accent)
                            .size(12.0),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("{}:{}", source.file_path, source.line))
                            .color(colors.accent)
                            .size(typography::SM)
                            .strong(),
                    );

                    // Language badge
                    if !source.language.is_empty() {
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(colors.badge_bg)
                            .corner_radius(3.0)
                            .inner_margin(egui::Margin::symmetric(4, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(&source.language)
                                        .color(colors.muted_text)
                                        .size(typography::XS),
                                );
                            });
                    }
                });

                ui.add_space(6.0);

                // Line number width
                let max_line = source.start_line + source.lines.len();
                let line_num_width = format!("{max_line}").len();

                // Source lines with line numbers and tree-sitter syntax highlighting
                for (i, line) in source.lines.iter().enumerate() {
                    let line_num = source.start_line + i;
                    let is_target = line_num == source.line;

                    let (line_color, bg_color) = if is_target {
                        (
                            palette::semantic::WARNING,
                            match self.theme {
                                AppTheme::Light => Color32::from_rgba_unmultiplied(255, 220, 0, 40),
                                AppTheme::Dark => Color32::from_rgba_unmultiplied(255, 220, 0, 25),
                            },
                        )
                    } else {
                        (colors.faint_text, Color32::TRANSPARENT)
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
                        ui.painter().rect_filled(rect, 2.0, bg_color);
                    }
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
            inline_blocks: Vec::new(),
        });

        self.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "AI agents are not available in the browser.".to_string(),
            is_streaming: false,
            inline_blocks: Vec::new(),
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
                                return Some(truncate_path_suffix(path, 50));
                            }
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
