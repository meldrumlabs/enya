//! Chat view component for displaying messages in a channel or thread.
//!
//! This component renders the right side of the split view when a channel
//! or thread is selected. It displays:
//! - Thread/channel header with title and actions
//! - Message list with avatars, content, and reactions
//! - Inline embedded plots/charts when referenced
//! - Message input with @mention autocomplete
//!
//! Design: Premium styling with message bubbles, hover states,
//! and seamless integration with the workspace for chart embeds.

use egui::{Color32, CornerRadius, RichText, ScrollArea, Stroke, StrokeKind, Vec2};
use egui_nerdfonts::regular;
use enya_team_api::UserId;

use super::{
    Channel, ChannelId, ChatColors, ChatMessage, ChatMessageAuthor, ChatState, Thread, ThreadId,
};
use crate::components::pane::time_series_chart::{Series, TimeSeriesChart};
// Re-import from pane module (these types are shared with non-chat code)
use crate::components::pane::{CommitInfo, PaneInfo, PaneVisualization};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Part of a message content (text or commit reference).
enum ContentPart {
    Text(String),
    CommitRef(String),
}

/// Reference to an embedded chart in a message.
#[derive(Debug, Clone)]
pub struct EmbeddedChart {
    /// The chart/pane name or ID.
    pub chart_name: String,
    /// Optional snapshot image data (for async loading).
    pub snapshot: Option<Vec<u8>>,
    /// Width of the embedded chart.
    pub width: f32,
    /// Height of the embedded chart.
    pub height: f32,
}

/// Inline time series chart data embedded in a chat message (snapshot).
///
/// When a user shares a chart in chat, the data is captured at share time
/// and embedded directly in the message. This ensures all team members
/// see the exact same data regardless of their Prometheus connection.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineChart {
    /// Chart title (e.g., metric name)
    pub title: String,
    /// Data series to plot (snapshot of the data at share time)
    pub series: Vec<Series>,
    /// Optional height override (default: 150px)
    pub height: Option<f32>,
}

impl InlineChart {
    /// Create a new inline chart with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            series: Vec::new(),
            height: None,
        }
    }

    /// Add a series to the chart.
    pub fn with_series(mut self, series: Series) -> Self {
        self.series.push(series);
        self
    }

    /// Set a custom height for the chart.
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

/// A single stat card for displaying a key metric value.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineStat {
    /// Stat label (e.g., "P99 Latency")
    pub label: String,
    /// Current value (e.g., "450ms")
    pub value: String,
    /// Optional previous value for comparison
    pub previous_value: Option<String>,
    /// Change direction: positive, negative, or neutral
    pub trend: StatTrend,
    /// Optional subtitle (e.g., "Last 5 minutes")
    pub subtitle: Option<String>,
}

/// Trend direction for stat cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatTrend {
    /// Value increased (shown in red for metrics where up is bad)
    Up,
    /// Value decreased (shown in green for metrics where down is good)
    Down,
    /// No significant change
    #[default]
    Neutral,
}

impl InlineStat {
    /// Create a new stat with label and value.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            previous_value: None,
            trend: StatTrend::Neutral,
            subtitle: None,
        }
    }

    /// Set the previous value for comparison.
    pub fn with_previous(mut self, prev: impl Into<String>) -> Self {
        self.previous_value = Some(prev.into());
        self
    }

    /// Set the trend direction.
    pub fn with_trend(mut self, trend: StatTrend) -> Self {
        self.trend = trend;
        self
    }

    /// Set the subtitle.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }
}

/// A table for displaying structured data.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineTable {
    /// Table title
    pub title: String,
    /// Column headers
    pub headers: Vec<String>,
    /// Row data (each row is a vector of cell values)
    pub rows: Vec<Vec<String>>,
    /// Maximum rows to display (default: 5)
    pub max_rows: usize,
}

impl InlineTable {
    /// Create a new table with the given title and headers.
    pub fn new(title: impl Into<String>, headers: Vec<String>) -> Self {
        Self {
            title: title.into(),
            headers,
            rows: Vec::new(),
            max_rows: 5,
        }
    }

    /// Add a row to the table.
    pub fn with_row(mut self, row: Vec<String>) -> Self {
        self.rows.push(row);
        self
    }

    /// Set maximum visible rows.
    pub fn with_max_rows(mut self, max: usize) -> Self {
        self.max_rows = max;
        self
    }
}

/// A bar chart for comparing categorical values.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineBarChart {
    /// Chart title
    pub title: String,
    /// Bars to display (label, value, optional color)
    pub bars: Vec<BarData>,
    /// Whether to show horizontal bars (default: true for inline)
    pub horizontal: bool,
    /// Optional height override
    pub height: Option<f32>,
}

/// Data for a single bar in a bar chart.
#[derive(Debug, Clone, PartialEq)]
pub struct BarData {
    /// Bar label
    pub label: String,
    /// Bar value
    pub value: f64,
    /// Optional custom color
    pub color: Option<Color32>,
}

impl BarData {
    /// Create a new bar with label and value.
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            color: None,
        }
    }

    /// Set a custom color for this bar.
    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

impl InlineBarChart {
    /// Create a new bar chart with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            bars: Vec::new(),
            horizontal: true,
            height: None,
        }
    }

    /// Add a bar to the chart.
    pub fn with_bar(mut self, bar: BarData) -> Self {
        self.bars.push(bar);
        self
    }

    /// Set whether bars are horizontal.
    pub fn horizontal(mut self, h: bool) -> Self {
        self.horizontal = h;
        self
    }

    /// Set a custom height.
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

/// A gauge visualization for showing a value on a range.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineGauge {
    /// Gauge title/label
    pub title: String,
    /// Current value
    pub value: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Unit suffix (e.g., "%", "MB")
    pub unit: String,
    /// Optional height override
    pub height: Option<f32>,
}

impl InlineGauge {
    /// Create a new gauge with title, value, and range.
    pub fn new(title: impl Into<String>, value: f64, min: f64, max: f64) -> Self {
        Self {
            title: title.into(),
            value,
            min,
            max,
            unit: String::new(),
            height: None,
        }
    }

    /// Set the unit suffix.
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Set a custom height.
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }
}

/// A visualization that can be embedded inline in a chat message.
#[derive(Debug, Clone, PartialEq)]
pub enum InlineVisualization {
    /// Time series line chart
    Chart(InlineChart),
    /// Single stat card (e.g., "P99: 450ms")
    Stat(InlineStat),
    /// Tabular data
    Table(InlineTable),
    /// Bar chart for categorical comparisons
    BarChart(InlineBarChart),
    /// Gauge for showing a value on a range
    Gauge(InlineGauge),
}

/// Actions that can be triggered from the chat view.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatViewAction {
    /// No action.
    None,
    /// User clicked the back button.
    Back,
    /// User wants to close the chat view.
    Close,
    /// User wants to resolve/close a thread.
    ResolveThread(ThreadId),
    /// User sent a message.
    SendMessage(String),
    /// User wants to @mention someone.
    OpenMentionPicker,
    /// User wants to embed a chart.
    EmbedChart,
    /// User clicked on an embedded chart (to navigate to it).
    NavigateToChart(String),
    /// User clicked on a user mention.
    ViewUser(UserId),
    /// User reacted to a message.
    AddReaction(super::MessageId, String),
    /// User clicked on a commit reference (to open diff viewer).
    OpenDiffViewer {
        /// Commit hash.
        hash: String,
        /// Commit message (for title).
        message: String,
        /// Full diff content.
        diff: String,
    },
    /// User is typing a commit search query (workspace should provide results).
    SearchCommits(String),
    /// User pressed Escape in input to return focus to sidebar (vim navigation).
    ReturnFocusToSidebar,
}

/// The view mode for the chat.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatViewMode {
    /// Viewing a channel's messages.
    Channel(ChannelId),
    /// Viewing a thread.
    Thread(ThreadId),
}

/// Chat view component for displaying messages.
pub struct ChatView {
    /// Current theme.
    theme: AppTheme,
    /// Current view mode.
    mode: Option<ChatViewMode>,
    /// Message input text.
    input_text: String,
    /// Previous input text (for detecting @ and # insertion).
    prev_input: String,
    /// Whether the input is focused.
    input_focused: bool,
    /// Scroll to bottom flag.
    scroll_to_bottom: bool,
    /// Current user ID (for highlighting own messages).
    current_user_id: Option<UserId>,
    /// Available panes for @mention autocomplete.
    available_panes: Vec<PaneInfo>,
    /// Autocomplete state: whether the popup is visible (@ pane mentions).
    autocomplete_visible: bool,
    /// Autocomplete state: selected index in the popup.
    autocomplete_index: usize,
    /// Autocomplete state: the query being typed after @.
    autocomplete_query: String,
    /// Autocomplete state: position of the @ in the input.
    autocomplete_at_position: usize,
    /// Pending inline chart to attach to the next message.
    pending_chart: Option<InlineChart>,
    /// Pending visualization to attach to the next message.
    pending_visualization: Option<InlineVisualization>,
    // --- Commit autocomplete state (# references) ---
    /// Commit autocomplete: available commit results from search.
    available_commits: Vec<CommitInfo>,
    /// Commit autocomplete: whether the popup is visible.
    commit_autocomplete_visible: bool,
    /// Commit autocomplete: selected index in the popup.
    commit_autocomplete_index: usize,
    /// Commit autocomplete: the query being typed after #.
    commit_autocomplete_query: String,
    /// Commit autocomplete: position of the # in the input.
    commit_autocomplete_hash_position: usize,
    /// Whether an overlay (style picker, command palette, etc.) blocks keyboard input.
    overlay_blocks_input: bool,
    /// Request focus on the input field next frame.
    request_input_focus: bool,
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatView {
    /// Create a new chat view.
    pub fn new() -> Self {
        Self {
            theme: AppTheme::default(),
            mode: None,
            input_text: String::new(),
            prev_input: String::new(),
            input_focused: false,
            scroll_to_bottom: true,
            current_user_id: None,
            available_panes: Vec::new(),
            autocomplete_visible: false,
            autocomplete_index: 0,
            autocomplete_query: String::new(),
            autocomplete_at_position: 0,
            pending_chart: None,
            pending_visualization: None,
            // Commit autocomplete
            available_commits: Vec::new(),
            commit_autocomplete_visible: false,
            commit_autocomplete_index: 0,
            commit_autocomplete_query: String::new(),
            commit_autocomplete_hash_position: 0,
            overlay_blocks_input: false,
            request_input_focus: false,
        }
    }

    /// Set whether an overlay blocks keyboard input (style picker, command palette, etc.).
    pub fn set_overlay_blocks_input(&mut self, blocks: bool) {
        self.overlay_blocks_input = blocks;
    }

    /// Request focus on the chat input field.
    pub fn focus_input(&mut self) {
        self.request_input_focus = true;
    }

    /// Check if the input field currently has focus.
    pub fn is_input_focused(&self) -> bool {
        self.input_focused
    }

    /// Set available commit results for # autocomplete.
    pub fn set_available_commits(&mut self, commits: Vec<CommitInfo>) {
        self.available_commits = commits;
        // Reset index if out of bounds
        let filtered_count = self.filtered_commits().len();
        if self.commit_autocomplete_index >= filtered_count {
            self.commit_autocomplete_index = 0;
        }
    }

    /// Set the available panes for @mention autocomplete.
    pub fn set_available_panes(&mut self, panes: Vec<PaneInfo>) {
        self.available_panes = panes;
    }

    /// Get any pending inline chart (consumes it).
    pub fn take_pending_chart(&mut self) -> Option<InlineChart> {
        self.pending_chart.take()
    }

    /// Get any pending visualization (consumes it).
    pub fn take_pending_visualization(&mut self) -> Option<InlineVisualization> {
        self.pending_visualization.take()
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the current user ID.
    pub fn set_current_user(&mut self, user_id: Option<UserId>) {
        self.current_user_id = user_id;
    }

    /// Set the view mode (channel or thread).
    pub fn set_mode(&mut self, mode: Option<ChatViewMode>) {
        if self.mode != mode {
            self.mode = mode;
            self.scroll_to_bottom = true;
        }
    }

    /// Get the current view mode.
    pub fn mode(&self) -> Option<&ChatViewMode> {
        self.mode.as_ref()
    }

    /// Check if the chat view is open.
    pub fn is_open(&self) -> bool {
        self.mode.is_some()
    }

    /// Clear the input text.
    pub fn clear_input(&mut self) {
        self.input_text.clear();
        self.prev_input.clear();
        self.close_autocomplete();
        self.close_commit_autocomplete();
    }

    // =========================================================================
    // Premium styling helpers (theme-aware)
    // =========================================================================

    /// Get the chat colors helper for the current theme.
    fn colors(&self) -> ChatColors {
        ChatColors::new(self.theme)
    }

    // =========================================================================
    // Main render
    // =========================================================================

    /// Show the chat view.
    pub fn show(&mut self, ui: &mut egui::Ui, chat_state: &ChatState) -> ChatViewAction {
        let Some(ref mode) = self.mode else {
            return ChatViewAction::None;
        };

        let mut action = ChatViewAction::None;

        // Get the relevant data based on mode
        let (header_info, messages) = match mode {
            ChatViewMode::Channel(channel_id) => {
                let channel = chat_state.get_channel(*channel_id);
                let msgs: Vec<_> = chat_state
                    .messages()
                    .iter()
                    .filter(|m| m.thread_id.is_none())
                    .collect();
                (channel.map(|c| HeaderInfo::Channel(c.clone())), msgs)
            }
            ChatViewMode::Thread(thread_id) => {
                let thread = chat_state.get_thread(*thread_id);
                let channel = thread.and_then(|t| chat_state.get_channel(t.channel_id));
                let msgs: Vec<_> = chat_state.thread_messages(*thread_id);
                (
                    thread.map(|t| HeaderInfo::Thread {
                        thread: t.clone(),
                        channel_name: channel.map(|c| c.name.clone()).unwrap_or_default(),
                    }),
                    msgs,
                )
            }
        };

        // Use a simpler layout approach that respects panel boundaries
        // Reserve input area at bottom using TopBottomPanel equivalent layout
        let total_rect = ui.available_rect_before_wrap();
        let input_height = 64.0;

        // Header section (fixed height)
        let header_height = 48.0;
        let header_rect =
            egui::Rect::from_min_size(total_rect.min, Vec2::new(total_rect.width(), header_height));

        ui.scope_builder(egui::UiBuilder::new().max_rect(header_rect), |ui| {
            if let Some(ref info) = header_info {
                if let Some(header_action) = self.render_header(ui, info) {
                    action = header_action;
                }
            }
        });

        // Divider below header
        let divider_y = total_rect.top() + header_height;
        ui.painter().hline(
            total_rect.x_range(),
            divider_y,
            Stroke::new(1.0, self.theme.border_subtle()),
        );

        // Input area at bottom
        let input_rect = egui::Rect::from_min_max(
            egui::pos2(total_rect.left(), total_rect.bottom() - input_height),
            total_rect.max,
        );

        // Messages area fills the middle
        let messages_rect = egui::Rect::from_min_max(
            egui::pos2(total_rect.left(), divider_y + 1.0),
            egui::pos2(total_rect.right(), input_rect.top()),
        );

        // Render messages in the middle area
        ui.scope_builder(egui::UiBuilder::new().max_rect(messages_rect), |ui| {
            if let Some(msg_action) = self.render_messages(ui, &messages) {
                action = msg_action;
            }
        });

        // Render input at the bottom
        ui.scope_builder(egui::UiBuilder::new().max_rect(input_rect), |ui| {
            if let Some(input_action) = self.render_input(ui) {
                action = input_action;
            }
        });

        action
    }

    // =========================================================================
    // Header
    // =========================================================================

    /// Render the chat header.
    fn render_header(&mut self, ui: &mut egui::Ui, info: &HeaderInfo) -> Option<ChatViewAction> {
        let mut action = None;

        let header_height = 48.0;
        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), header_height),
            egui::Sense::hover(),
        );

        // Background
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, self.theme.bg_elevated());

        let content_rect = rect.shrink2(Vec2::new(12.0, 0.0));

        // Back button
        let back_btn_rect =
            egui::Rect::from_min_size(content_rect.left_top(), Vec2::new(28.0, header_height));
        let back_response = ui.allocate_rect(back_btn_rect, egui::Sense::click());

        let back_color = if back_response.hovered() {
            self.theme.text_primary()
        } else {
            self.theme.text_tertiary()
        };
        ui.painter().text(
            back_btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            regular::ARROW_LEFT,
            typography::proportional(typography::MD),
            back_color,
        );

        if back_response.clicked() {
            action = Some(ChatViewAction::Back);
        }

        // Title and subtitle
        match info {
            HeaderInfo::Channel(channel) => {
                // Channel icon and name
                ui.painter().text(
                    content_rect.left_center() + Vec2::new(36.0, -8.0),
                    egui::Align2::LEFT_CENTER,
                    channel.kind.icon(),
                    typography::proportional(typography::SM),
                    channel.kind.color(),
                );
                ui.painter().text(
                    content_rect.left_center() + Vec2::new(54.0, -8.0),
                    egui::Align2::LEFT_CENTER,
                    &channel.name,
                    typography::proportional(typography::MD),
                    self.theme.text_primary(),
                );
                // Description
                if let Some(ref desc) = channel.description {
                    ui.painter().text(
                        content_rect.left_center() + Vec2::new(36.0, 8.0),
                        egui::Align2::LEFT_CENTER,
                        desc,
                        typography::proportional(typography::XS),
                        self.theme.text_tertiary(),
                    );
                }
            }
            HeaderInfo::Thread {
                thread,
                channel_name,
            } => {
                // Priority icon
                let priority_color = thread.priority.color();
                ui.painter().text(
                    content_rect.left_center() + Vec2::new(36.0, -8.0),
                    egui::Align2::LEFT_CENTER,
                    thread.priority.icon(),
                    typography::proportional(typography::SM),
                    priority_color,
                );
                // Thread title
                ui.painter().text(
                    content_rect.left_center() + Vec2::new(54.0, -8.0),
                    egui::Align2::LEFT_CENTER,
                    &thread.title,
                    typography::proportional(typography::MD),
                    self.theme.text_primary(),
                );
                // Channel name and reply count
                let subtitle = format!("#{channel_name} · {}", thread.reply_summary());
                ui.painter().text(
                    content_rect.left_center() + Vec2::new(36.0, 8.0),
                    egui::Align2::LEFT_CENTER,
                    subtitle,
                    typography::proportional(typography::XS),
                    self.theme.text_tertiary(),
                );

                // Resolve button (for active threads)
                if thread.status == super::ThreadStatus::Active {
                    let resolve_rect = egui::Rect::from_center_size(
                        content_rect.right_center() + Vec2::new(-40.0, 0.0),
                        Vec2::new(60.0, 28.0),
                    );
                    let resolve_response = ui.allocate_rect(resolve_rect, egui::Sense::click());

                    let (bg, fg) = if resolve_response.hovered() {
                        (
                            self.theme.accent_primary().gamma_multiply(0.2),
                            self.theme.accent_primary(),
                        )
                    } else {
                        (Color32::TRANSPARENT, self.theme.text_secondary())
                    };

                    ui.painter()
                        .rect_filled(resolve_rect, CornerRadius::same(4), bg);
                    ui.painter().text(
                        resolve_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{} Done", regular::CHECK),
                        typography::proportional(typography::XS),
                        fg,
                    );

                    if resolve_response.clicked() {
                        action = Some(ChatViewAction::ResolveThread(thread.id));
                    }
                }
            }
        }

        // Close button
        let close_rect = egui::Rect::from_center_size(
            content_rect.right_center() + Vec2::new(-8.0, 0.0),
            Vec2::new(24.0, 24.0),
        );
        let close_response = ui.allocate_rect(close_rect, egui::Sense::click());

        let close_color = if close_response.hovered() {
            self.theme.text_primary()
        } else {
            self.theme.text_tertiary()
        };
        ui.painter().text(
            close_rect.center(),
            egui::Align2::CENTER_CENTER,
            regular::X,
            typography::proportional(typography::SM),
            close_color,
        );

        if close_response.clicked() {
            action = Some(ChatViewAction::Close);
        }

        action
    }

    // =========================================================================
    // Messages
    // =========================================================================

    /// Render the messages list.
    fn render_messages(
        &mut self,
        ui: &mut egui::Ui,
        messages: &[&ChatMessage],
    ) -> Option<ChatViewAction> {
        let mut action = None;
        let scroll_id = egui::Id::new("chat_messages_scroll");

        ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, false])
            .stick_to_bottom(self.scroll_to_bottom)
            .show(ui, |ui| {
                ui.add_space(12.0);

                if messages.is_empty() {
                    // Empty state
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(regular::COMMENT)
                                .size(32.0)
                                .color(self.theme.text_tertiary()),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("No messages yet")
                                .size(typography::SM)
                                .color(self.theme.text_tertiary()),
                        );
                        ui.label(
                            RichText::new("Start the conversation!")
                                .size(typography::XS)
                                .color(self.theme.text_tertiary()),
                        );
                    });
                } else {
                    for message in messages {
                        if let Some(msg_action) = self.render_message(ui, message) {
                            action = Some(msg_action);
                        }
                        ui.add_space(8.0);
                    }
                }

                ui.add_space(12.0);
            });

        // Reset scroll flag after first render
        self.scroll_to_bottom = false;

        action
    }

    /// Render a single message.
    fn render_message(&self, ui: &mut egui::Ui, message: &ChatMessage) -> Option<ChatViewAction> {
        let is_own = match &message.author {
            ChatMessageAuthor::User { user_id, .. } => self.current_user_id == Some(*user_id),
            _ => false,
        };
        let is_agent = message.author.is_agent();
        let is_system = message.author.is_system();

        // System messages are centered and minimal
        if is_system {
            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() * 0.2);
                ui.label(
                    RichText::new(&message.content)
                        .size(typography::XS)
                        .color(self.colors().system_message())
                        .italics(),
                );
            });
            return None;
        }

        // Regular message layout - use Frame for hover background
        let padding = 12.0;

        // Use a frame-based approach that auto-sizes instead of fixed height
        let frame = egui::Frame::new()
            .fill(Color32::TRANSPARENT)
            .inner_margin(egui::Margin::symmetric(0, 4));

        frame
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(padding);

                    // Avatar circle with premium styling
                    let avatar_size = 32.0;
                    let (avatar_rect, _) =
                        ui.allocate_exact_size(Vec2::splat(avatar_size), egui::Sense::hover());

                    let avatar_color = if is_agent {
                        self.theme.accent_primary()
                    } else if is_own {
                        self.theme.accent_primary().gamma_multiply(0.7)
                    } else {
                        self.theme.text_tertiary()
                    };

                    // Premium glow ring for agent avatars
                    if is_agent {
                        ui.painter().circle_filled(
                            avatar_rect.center(),
                            avatar_size / 2.0 + 2.0,
                            self.colors().agent_avatar_ring(),
                        );
                    }

                    ui.painter().circle_filled(
                        avatar_rect.center(),
                        avatar_size / 2.0,
                        avatar_color,
                    );

                    // Avatar initial
                    let initial = message
                        .author
                        .display_name()
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string();
                    ui.painter().text(
                        avatar_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        initial,
                        typography::proportional(typography::SM),
                        Color32::WHITE,
                    );

                    ui.add_space(8.0);

                    // Message content area
                    let content_width = (ui.available_width() - padding - 60.0).max(100.0);
                    let vertical_action = ui
                        .vertical(|ui| {
                            ui.set_max_width(content_width);

                            // Author name and time
                            ui.horizontal(|ui| {
                                let name_color = if is_agent {
                                    self.theme.accent_primary()
                                } else {
                                    self.theme.text_primary()
                                };
                                ui.label(
                                    RichText::new(message.author.display_name())
                                        .size(typography::SM)
                                        .color(name_color)
                                        .strong(),
                                );

                                if is_agent {
                                    // Agent badge (premium pill style)
                                    ui.add_space(4.0);
                                    let badge_text = RichText::new(" AI ")
                                        .size(typography::XS)
                                        .color(Color32::WHITE)
                                        .background_color(self.theme.accent_primary());
                                    ui.label(badge_text);
                                }

                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(message.relative_time())
                                        .size(typography::XS)
                                        .color(self.theme.text_tertiary()),
                                );
                            });

                            // Message bubble with premium corner radius
                            let colors = self.colors();
                            let bg_color = if is_agent {
                                colors.agent_message_bg()
                            } else if is_own {
                                colors.own_message_bg()
                            } else {
                                colors.other_message_bg()
                            };

                            let content_action = egui::Frame::new()
                                .fill(bg_color)
                                .corner_radius(CornerRadius::same(12)) // Slightly more rounded
                                .inner_margin(egui::Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.set_max_width(content_width - 24.0);
                                    // Render message content with clickable commit references
                                    self.render_message_content(ui, &message.content)
                                })
                                .inner;

                            // Reactions (if any) with premium pill styling
                            if !message.reactions.is_empty() {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    for (emoji, count) in &message.reactions {
                                        let reaction_text = format!("{emoji} {count}");
                                        let btn = egui::Button::new(
                                            RichText::new(reaction_text)
                                                .size(typography::XS)
                                                .color(self.theme.text_secondary()),
                                        )
                                        .fill(self.theme.bg_surface())
                                        .stroke(Stroke::new(1.0, self.theme.border_subtle()))
                                        .corner_radius(CornerRadius::same(12)); // Pill shape
                                        ui.add(btn);
                                        ui.add_space(4.0);
                                    }
                                });
                            }

                            // Inline charts (snapshot data embedded in message)
                            for chart in &message.inline_charts {
                                ui.add_space(8.0);
                                self.render_inline_chart(ui, chart);
                            }

                            // Other inline visualizations (stats, tables, bar charts)
                            for viz in &message.visualizations {
                                ui.add_space(8.0);
                                self.render_inline_visualization(ui, viz);
                            }

                            content_action
                        })
                        .inner;

                    ui.add_space(padding);

                    vertical_action
                })
                .inner
            })
            .inner
    }

    /// Render message content with clickable commit references.
    /// Commit references are in the format `[#hash]` and become clickable links.
    fn render_message_content(&self, ui: &mut egui::Ui, content: &str) -> Option<ChatViewAction> {
        let mut action = None;

        // Pattern to match commit references: [#abc1234]
        // Split content into parts: text and commit references
        let mut parts: Vec<ContentPart> = Vec::new();
        let mut remaining = content;

        while let Some(start) = remaining.find("[#") {
            // Add text before the reference
            if start > 0 {
                parts.push(ContentPart::Text(remaining[..start].to_string()));
            }

            // Find the closing bracket
            if let Some(end) = remaining[start..].find(']') {
                let ref_text = &remaining[start + 2..start + end]; // Extract hash without [# and ]
                parts.push(ContentPart::CommitRef(ref_text.to_string()));
                remaining = &remaining[start + end + 1..];
            } else {
                // No closing bracket, treat as text
                parts.push(ContentPart::Text(remaining.to_string()));
                remaining = "";
            }
        }

        // Add any remaining text
        if !remaining.is_empty() {
            parts.push(ContentPart::Text(remaining.to_string()));
        }

        // If no commit references, render as plain text
        if parts.is_empty() || (parts.len() == 1 && matches!(&parts[0], ContentPart::Text(_))) {
            ui.label(
                RichText::new(content)
                    .size(typography::SM)
                    .color(self.theme.text_primary()),
            );
            return None;
        }

        // Render mixed content with clickable commit references
        ui.horizontal_wrapped(|ui| {
            for part in &parts {
                match part {
                    ContentPart::Text(text) => {
                        ui.label(
                            RichText::new(text)
                                .size(typography::SM)
                                .color(self.theme.text_primary()),
                        );
                    }
                    ContentPart::CommitRef(hash) => {
                        // Render as a clickable button styled like a code link
                        let btn_text = format!("#{hash}");
                        let btn = egui::Button::new(
                            RichText::new(btn_text)
                                .size(typography::SM)
                                .color(self.theme.accent_primary())
                                .family(egui::FontFamily::Monospace),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .frame(false);

                        if ui.add(btn).on_hover_text("Click to view diff").clicked() {
                            // Look up the full commit info to get the diff
                            if let Some(commit) = self
                                .available_commits
                                .iter()
                                .find(|c| c.short_hash == *hash || c.full_hash.starts_with(hash))
                            {
                                action = Some(ChatViewAction::OpenDiffViewer {
                                    hash: commit.full_hash.clone(),
                                    message: commit.message.clone(),
                                    diff: commit.diff.clone(),
                                });
                            } else {
                                // Commit not in current list, just use the hash as fallback
                                action = Some(ChatViewAction::OpenDiffViewer {
                                    hash: hash.clone(),
                                    message: format!("Commit {}", &hash[..7.min(hash.len())]),
                                    diff: String::new(),
                                });
                            }
                        }
                    }
                }
            }
        });

        action
    }

    // =========================================================================
    // Input
    // =========================================================================

    /// Render the message input area.
    fn render_input(&mut self, ui: &mut egui::Ui) -> Option<ChatViewAction> {
        let mut action = None;

        // Use the full available rect
        let rect = ui.available_rect_before_wrap();

        // Background fill for input area
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, self.theme.bg_elevated());

        // Top border
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            Stroke::new(1.0, self.theme.border_subtle()),
        );

        // Center the input field vertically with fixed height
        let input_field_height = 40.0;
        let vertical_padding = (rect.height() - input_field_height) / 2.0;
        let content_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 12.0, rect.top() + vertical_padding),
            egui::pos2(
                rect.right() - 12.0,
                rect.top() + vertical_padding + input_field_height,
            ),
        );

        // Input area (text field takes most of the width, leaving space for buttons)
        let input_rect = egui::Rect::from_min_max(
            content_rect.left_top(),
            egui::pos2(content_rect.right() - 80.0, content_rect.bottom()),
        );

        // Draw input background
        ui.painter()
            .rect_filled(input_rect, CornerRadius::same(8), self.theme.bg_surface());
        ui.painter().rect_stroke(
            input_rect,
            CornerRadius::same(8),
            Stroke::new(1.0, self.theme.border_subtle()),
            StrokeKind::Inside,
        );

        // Handle keyboard input for autocomplete BEFORE text edit (to consume keys)
        // Skip keyboard handling when an overlay is blocking input (style picker, etc.)
        let mut send_message = false;
        let mut escape_pressed = false;
        let mut select_pressed = false;
        let mut commit_escape_pressed = false;
        let mut commit_select_pressed = false;

        // Handle @ autocomplete (panes) - skip when overlay is blocking
        if self.autocomplete_visible && !self.overlay_blocks_input {
            let filtered_count = self.filtered_panes().len();
            let autocomplete_index = &mut self.autocomplete_index;
            ui.ctx().input_mut(|input| {
                // Escape to close
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    escape_pressed = true;
                }
                // Arrow up
                else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    if *autocomplete_index > 0 {
                        *autocomplete_index -= 1;
                    }
                }
                // Arrow down
                else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    if *autocomplete_index < filtered_count.saturating_sub(1) {
                        *autocomplete_index += 1;
                    }
                }
                // Tab or Enter to select
                else if input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                {
                    select_pressed = true;
                }
            });

            // Handle escape
            if escape_pressed {
                self.close_autocomplete();
            }
            // Complete selection if Tab/Enter was pressed
            else if select_pressed {
                let filtered = self.filtered_panes();
                if !filtered.is_empty() && self.autocomplete_index < filtered.len() {
                    let selected_pane = filtered[self.autocomplete_index].clone();
                    self.complete_pane_mention(&selected_pane);
                }
            }
        }
        // Handle commit autocomplete (#) - skip when overlay is blocking
        else if self.commit_autocomplete_visible && !self.overlay_blocks_input {
            let filtered_count = self.filtered_commits().len();
            let commit_index = &mut self.commit_autocomplete_index;
            ui.ctx().input_mut(|input| {
                // Escape to close
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    commit_escape_pressed = true;
                }
                // Arrow up
                else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    if *commit_index > 0 {
                        *commit_index -= 1;
                    }
                }
                // Arrow down
                else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    if *commit_index < filtered_count.saturating_sub(1) {
                        *commit_index += 1;
                    }
                }
                // Tab or Enter to select
                else if input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                {
                    commit_select_pressed = true;
                }
            });

            // Handle escape
            if commit_escape_pressed {
                self.close_commit_autocomplete();
            }
            // Complete selection if Tab/Enter was pressed
            else if commit_select_pressed {
                let filtered = self.filtered_commits();
                if !filtered.is_empty() && self.commit_autocomplete_index < filtered.len() {
                    let selected_commit = filtered[self.commit_autocomplete_index].clone();
                    self.complete_commit_reference(&selected_commit);
                }
            }
        } else if !self.overlay_blocks_input {
            // When autocomplete is not visible, check for Enter to send
            // Skip when overlay is blocking input
            let input_not_empty = !self.input_text.trim().is_empty();
            ui.ctx().input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) && input_not_empty {
                    send_message = true;
                }
            });
        }

        // Text input
        let text_rect = input_rect.shrink(8.0);
        let text_edit = egui::TextEdit::singleline(&mut self.input_text)
            .font(typography::proportional(typography::SM))
            .text_color(self.theme.text_primary())
            .frame(false)
            .hint_text(
                RichText::new("Type a message... @ to embed, # for commits")
                    .color(self.theme.text_tertiary()),
            );

        let response = ui.put(text_rect, text_edit);
        self.input_focused = response.has_focus();

        // Request focus on input if flagged (vim l key from sidebar)
        if self.request_input_focus {
            response.request_focus();
            self.request_input_focus = false;
        }

        // Handle Escape when input is focused (and no autocomplete visible)
        // to return focus to sidebar for vim navigation
        if self.input_focused
            && !self.autocomplete_visible
            && !self.commit_autocomplete_visible
            && !self.overlay_blocks_input
        {
            let mut should_surrender = false;
            ui.ctx().input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    action = Some(ChatViewAction::ReturnFocusToSidebar);
                    should_surrender = true;
                }
            });
            // Surrender egui focus from text input so vim keys work in sidebar
            if should_surrender {
                response.surrender_focus();
                // Also clear global egui focus so keyboard handler doesn't skip vim keys
                ui.ctx()
                    .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            }
        }

        // Detect @ and # for autocomplete (compare current vs previous input)
        if let Some(trigger_action) = self.check_input_triggers() {
            action = Some(trigger_action);
        }

        // Handle send message
        if send_message {
            action = Some(ChatViewAction::SendMessage(self.input_text.clone()));
            self.input_text.clear();
            self.prev_input.clear();
            // Note: pending_chart is cleared by take_pending_chart() in channels_panel
            self.scroll_to_bottom = true;
        }

        // Render pane autocomplete popup above the input
        if self.autocomplete_visible && self.input_focused {
            self.render_autocomplete_popup(ui, input_rect);
        }

        // Render commit autocomplete popup above the input
        if self.commit_autocomplete_visible && self.input_focused {
            self.render_commit_autocomplete_popup(ui, input_rect);
        }

        // Show pending chart indicator
        if self.pending_chart.is_some() {
            let indicator_rect = egui::Rect::from_min_size(
                input_rect.left_top() + Vec2::new(0.0, -24.0),
                Vec2::new(input_rect.width(), 20.0),
            );
            ui.painter().rect_filled(
                indicator_rect,
                CornerRadius::same(4),
                self.theme.accent_primary().gamma_multiply(0.15),
            );
            ui.painter().text(
                indicator_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} Chart attached", regular::CHART_LINE),
                typography::proportional(typography::XS),
                self.theme.accent_primary(),
            );
        }

        // Show pending visualization indicator
        if let Some(ref viz) = self.pending_visualization {
            let viz_type = match viz {
                InlineVisualization::Stat(_) => ("Stat", regular::COUNTER),
                InlineVisualization::Table(_) => ("Table", regular::TABLE),
                InlineVisualization::BarChart(_) => ("Bar Chart", regular::CHART_BAR),
                InlineVisualization::Chart(_) => ("Chart", regular::CHART_LINE),
                InlineVisualization::Gauge(_) => ("Gauge", regular::GAUGE),
            };
            let indicator_rect = egui::Rect::from_min_size(
                input_rect.left_top()
                    + Vec2::new(
                        0.0,
                        if self.pending_chart.is_some() {
                            -48.0
                        } else {
                            -24.0
                        },
                    ),
                Vec2::new(input_rect.width(), 20.0),
            );
            ui.painter().rect_filled(
                indicator_rect,
                CornerRadius::same(4),
                self.theme.accent_primary().gamma_multiply(0.15),
            );
            ui.painter().text(
                indicator_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} {} attached", viz_type.1, viz_type.0),
                typography::proportional(typography::XS),
                self.theme.accent_primary(),
            );
        }

        // Action buttons
        let buttons_rect = egui::Rect::from_min_max(
            content_rect.right_top() + Vec2::new(-76.0, 0.0),
            content_rect.right_bottom(),
        );

        // Embed chart button
        let chart_btn_rect = egui::Rect::from_center_size(
            buttons_rect.left_center() + Vec2::new(18.0, 0.0),
            Vec2::new(32.0, 32.0),
        );
        let chart_response = ui.allocate_rect(chart_btn_rect, egui::Sense::click());

        let chart_color = if chart_response.hovered() {
            self.theme.accent_primary()
        } else {
            self.theme.text_tertiary()
        };
        ui.painter().text(
            chart_btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            regular::CHART_LINE,
            typography::proportional(typography::MD),
            chart_color,
        );

        if chart_response.clicked() {
            action = Some(ChatViewAction::EmbedChart);
        }

        // Send button
        let send_btn_rect = egui::Rect::from_center_size(
            buttons_rect.right_center() + Vec2::new(-18.0, 0.0),
            Vec2::new(32.0, 32.0),
        );
        let send_response = ui.allocate_rect(send_btn_rect, egui::Sense::click());

        let can_send = !self.input_text.trim().is_empty() || self.pending_chart.is_some();
        let send_color = if can_send {
            if send_response.hovered() {
                self.theme.accent_primary()
            } else {
                self.theme.accent_primary().gamma_multiply(0.8)
            }
        } else {
            self.theme.text_tertiary()
        };

        ui.painter()
            .circle_filled(send_btn_rect.center(), 14.0, send_color);
        ui.painter().text(
            send_btn_rect.center(),
            egui::Align2::CENTER_CENTER,
            regular::ARROW_UP,
            typography::proportional(typography::SM),
            Color32::WHITE,
        );

        if send_response.clicked() && can_send {
            action = Some(ChatViewAction::SendMessage(self.input_text.clone()));
            self.input_text.clear();
            self.prev_input.clear();
            // Note: pending_chart is cleared by take_pending_chart() in channels_panel
            self.scroll_to_bottom = true;
        }

        action
    }

    /// Check for @ and # triggers in the input text (similar to agent_input_bar.rs pattern).
    fn check_input_triggers(&mut self) -> Option<ChatViewAction> {
        let input_len = self.input_text.len();
        let prev_len = self.prev_input.len();
        let mut action = None;

        if input_len > prev_len {
            // Character(s) were added
            let new_chars = &self.input_text[prev_len..];

            // Check for @ mention trigger (embed visualizations or panes)
            if new_chars.contains('@') {
                // Find position of the new @
                if let Some(at_pos) = self.input_text.rfind('@') {
                    self.autocomplete_at_position = at_pos;
                    self.autocomplete_visible = true;
                    self.autocomplete_query.clear();
                    self.autocomplete_index = 0;
                    // Close other autocompletes if open
                    self.close_commit_autocomplete();
                }
            }
            // Check for # commit trigger
            else if new_chars.contains('#') {
                // Find position of the new #
                if let Some(hash_pos) = self.input_text.rfind('#') {
                    self.commit_autocomplete_hash_position = hash_pos;
                    self.commit_autocomplete_visible = true;
                    self.commit_autocomplete_query.clear();
                    self.commit_autocomplete_index = 0;
                    // Close pane autocomplete if open
                    self.close_autocomplete();
                    // Request commit search
                    action = Some(ChatViewAction::SearchCommits(String::new()));
                }
            }
            // Update @ autocomplete query if visible
            else if self.autocomplete_visible {
                if self.autocomplete_at_position < self.input_text.len() {
                    let query = &self.input_text[self.autocomplete_at_position + 1..];
                    // Close if there's a space (mention was completed) or newline
                    if query.contains(' ') || query.contains('\n') {
                        self.close_autocomplete();
                    } else {
                        self.autocomplete_query = query.to_string();
                        // Reset index if out of bounds
                        let filtered_count = self.filtered_panes().len();
                        if self.autocomplete_index >= filtered_count {
                            self.autocomplete_index = 0;
                        }
                    }
                }
            }
            // Update # commit autocomplete query if visible
            else if self.commit_autocomplete_visible
                && self.commit_autocomplete_hash_position < self.input_text.len()
            {
                let query = &self.input_text[self.commit_autocomplete_hash_position + 1..];
                // Close if there's a space (reference was completed) or newline
                if query.contains(' ') || query.contains('\n') {
                    self.close_commit_autocomplete();
                } else {
                    self.commit_autocomplete_query = query.to_string();
                    // Reset index if out of bounds
                    let filtered_count = self.filtered_commits().len();
                    if self.commit_autocomplete_index >= filtered_count {
                        self.commit_autocomplete_index = 0;
                    }
                    // Request updated commit search
                    action = Some(ChatViewAction::SearchCommits(query.to_string()));
                }
            }
        } else if input_len < prev_len {
            // Character(s) were deleted
            if self.autocomplete_visible {
                if self.input_text.len() <= self.autocomplete_at_position {
                    // The @ was deleted
                    self.close_autocomplete();
                } else {
                    // Update query
                    let query = &self.input_text[self.autocomplete_at_position + 1..];
                    self.autocomplete_query = query.to_string();
                    // Reset index if out of bounds
                    let filtered_count = self.filtered_panes().len();
                    if self.autocomplete_index >= filtered_count {
                        self.autocomplete_index = 0;
                    }
                }
            }
            if self.commit_autocomplete_visible {
                if self.input_text.len() <= self.commit_autocomplete_hash_position {
                    // The # was deleted
                    self.close_commit_autocomplete();
                } else {
                    // Update query
                    let query = &self.input_text[self.commit_autocomplete_hash_position + 1..];
                    self.commit_autocomplete_query = query.to_string();
                    // Reset index if out of bounds
                    let filtered_count = self.filtered_commits().len();
                    if self.commit_autocomplete_index >= filtered_count {
                        self.commit_autocomplete_index = 0;
                    }
                    // Request updated commit search
                    action = Some(ChatViewAction::SearchCommits(query.to_string()));
                }
            }
        }

        // Update prev_input AFTER the check
        self.prev_input = self.input_text.clone();
        action
    }

    /// Close the autocomplete popup.
    fn close_autocomplete(&mut self) {
        self.autocomplete_visible = false;
        self.autocomplete_query.clear();
        self.autocomplete_index = 0;
    }

    /// Close the commit autocomplete popup.
    fn close_commit_autocomplete(&mut self) {
        self.commit_autocomplete_visible = false;
        self.commit_autocomplete_query.clear();
        self.commit_autocomplete_index = 0;
    }

    /// Get filtered panes based on autocomplete query.
    fn filtered_panes(&self) -> Vec<PaneInfo> {
        let query = self.autocomplete_query.to_lowercase();
        self.available_panes
            .iter()
            .filter(|p| query.is_empty() || p.name.to_lowercase().contains(&query))
            .cloned()
            .collect()
    }

    /// Complete a pane mention and create the inline chart.
    fn complete_pane_mention(&mut self, pane: &PaneInfo) {
        // Replace @query with @pane-name and add a space
        let at_pos = self.autocomplete_at_position;
        let prefix = &self.input_text[..at_pos];
        self.input_text = format!("{prefix}@{} ", pane.name);
        // Sync prev_input to prevent re-triggering
        self.prev_input = self.input_text.clone();

        // Create the appropriate inline visualization based on pane type
        match &pane.visualization {
            PaneVisualization::TimeSeries { series } => {
                let chart = InlineChart {
                    title: pane.name.clone(),
                    series: series.clone(),
                    height: Some(150.0),
                };
                self.pending_chart = Some(chart);
            }
            PaneVisualization::Stat {
                value,
                unit,
                sparkline,
            } => {
                let mut stat = InlineStat::new(&pane.name, format!("{value:.1}{unit}"));
                // Determine trend from sparkline if available
                if sparkline.len() >= 2 {
                    let last = sparkline[sparkline.len() - 1];
                    let prev = sparkline[sparkline.len() - 2];
                    if last > prev * 1.05 {
                        stat = stat.with_trend(StatTrend::Up);
                    } else if last < prev * 0.95 {
                        stat = stat.with_trend(StatTrend::Down);
                    }
                }
                self.pending_visualization = Some(InlineVisualization::Stat(stat));
            }
            PaneVisualization::Gauge {
                value,
                min,
                max,
                unit,
            } => {
                // Create an actual gauge visualization
                let gauge = InlineGauge::new(&pane.name, *value, *min, *max)
                    .with_unit(unit)
                    .with_height(120.0);
                self.pending_visualization = Some(InlineVisualization::Gauge(gauge));
            }
            PaneVisualization::BarChart { bars } => {
                let mut bar_chart = InlineBarChart::new(&pane.name);
                for (label, value) in bars {
                    bar_chart = bar_chart.with_bar(BarData::new(label, *value));
                }
                self.pending_visualization = Some(InlineVisualization::BarChart(bar_chart));
            }
            PaneVisualization::Sparkline { data } => {
                // Show sparkline as a simple stat with the latest value
                if let Some(&last) = data.last() {
                    let stat = InlineStat::new(&pane.name, format!("{last:.1}"));
                    self.pending_visualization = Some(InlineVisualization::Stat(stat));
                }
            }
            PaneVisualization::Heatmap => {
                // Heatmaps are complex; just reference the pane name
                let stat = InlineStat::new(&pane.name, "Heatmap")
                    .with_subtitle("View in workspace for details");
                self.pending_visualization = Some(InlineVisualization::Stat(stat));
            }
        }

        // Close autocomplete
        self.close_autocomplete();
    }

    /// Get filtered commits based on autocomplete query.
    fn filtered_commits(&self) -> Vec<CommitInfo> {
        let query = self.commit_autocomplete_query.to_lowercase();
        self.available_commits
            .iter()
            .filter(|c| {
                query.is_empty()
                    || c.short_hash.to_lowercase().contains(&query)
                    || c.message.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    /// Complete a commit reference and insert a clickable link marker.
    fn complete_commit_reference(&mut self, commit: &CommitInfo) {
        // Replace #query with #short_hash and add a space
        // The format [#hash] is used to mark clickable commit references
        let hash_pos = self.commit_autocomplete_hash_position;
        let prefix = &self.input_text[..hash_pos];
        self.input_text = format!("{prefix}[#{}] ", commit.short_hash);
        // Sync prev_input to prevent re-triggering
        self.prev_input = self.input_text.clone();

        // Close commit autocomplete
        self.close_commit_autocomplete();
    }

    /// Render the autocomplete popup using egui::Area for proper layering.
    fn render_autocomplete_popup(&mut self, ui: &mut egui::Ui, input_rect: egui::Rect) {
        let filtered = self.filtered_panes();
        if filtered.is_empty() {
            return;
        }

        let colors = self.colors();
        let row_height = 32.0;
        let max_visible = 5.min(filtered.len());
        // Account for header (~24px) + items + footer (~24px) + margins
        let popup_height = row_height * max_visible as f32 + 56.0;
        let popup_width = input_rect.width().min(300.0);

        // Position popup above the input (with sufficient gap to not cover input)
        let popup_pos = egui::pos2(input_rect.left(), input_rect.top() - popup_height - 12.0);

        // Use Area to render as a floating overlay (not clipped by parent, renders on top)
        egui::Area::new(egui::Id::new("chat_pane_autocomplete"))
            .fixed_pos(popup_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(self.theme.bg_elevated())
                    .stroke(Stroke::new(1.0, self.theme.border_default()))
                    .corner_radius(CornerRadius::same(8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 12,
                        spread: 0,
                        color: Color32::from_black_alpha(40),
                    })
                    .inner_margin(egui::Margin::symmetric(4, 4))
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("@")
                                    .color(self.theme.accent_primary())
                                    .size(typography::SM)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Embed visualization")
                                    .color(self.theme.text_tertiary())
                                    .size(typography::XS),
                            );
                        });

                        ui.add_space(4.0);

                        // Render items
                        for (idx, pane) in filtered.iter().take(max_visible).enumerate() {
                            let is_selected = idx == self.autocomplete_index;

                            let response = ui.allocate_response(
                                Vec2::new(popup_width - 8.0, row_height),
                                egui::Sense::click(),
                            );
                            let row_rect = response.rect;

                            // Hover/selection background
                            if is_selected || response.hovered() {
                                let bg_color = if is_selected {
                                    colors.selection_bg()
                                } else {
                                    colors.hover_bg()
                                };
                                ui.painter()
                                    .rect_filled(row_rect, CornerRadius::same(4), bg_color);
                            }

                            // Selection indicator bar
                            if is_selected {
                                let indicator_rect = egui::Rect::from_min_size(
                                    row_rect.left_top(),
                                    Vec2::new(3.0, row_height),
                                );
                                ui.painter().rect_filled(
                                    indicator_rect,
                                    CornerRadius::same(2),
                                    self.theme.accent_primary(),
                                );
                            }

                            // Viz type icon (use the icon from VisualizationType)
                            let icon = pane.viz_type.icon();
                            ui.painter().text(
                                row_rect.left_center() + Vec2::new(12.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                icon,
                                typography::proportional(typography::SM),
                                if is_selected || response.hovered() {
                                    self.theme.accent_primary()
                                } else {
                                    self.theme.text_secondary()
                                },
                            );

                            // Pane name
                            ui.painter().text(
                                row_rect.left_center() + Vec2::new(32.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                &pane.name,
                                typography::proportional(typography::SM),
                                self.theme.text_primary(),
                            );

                            // Viz type label
                            let type_label = pane.viz_type.label();
                            ui.painter().text(
                                row_rect.right_center() + Vec2::new(-8.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                type_label,
                                typography::proportional(typography::XS),
                                self.theme.text_tertiary(),
                            );

                            // Click to select
                            if response.clicked() {
                                let pane_clone = pane.clone();
                                self.complete_pane_mention(&pane_clone);
                            }
                        }

                        // Footer with hints
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("↑↓")
                                    .color(self.theme.accent_primary())
                                    .size(typography::XS),
                            );
                            ui.label(
                                RichText::new("navigate")
                                    .color(self.theme.text_tertiary())
                                    .size(typography::XS),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("⏎")
                                    .color(self.theme.accent_primary())
                                    .size(typography::XS),
                            );
                            ui.label(
                                RichText::new("select")
                                    .color(self.theme.text_tertiary())
                                    .size(typography::XS),
                            );
                        });
                    });
            });
    }

    /// Render the commit autocomplete popup using egui::Area for proper layering.
    fn render_commit_autocomplete_popup(&mut self, ui: &mut egui::Ui, input_rect: egui::Rect) {
        let filtered = self.filtered_commits();
        if filtered.is_empty() {
            return;
        }

        let colors = self.colors();
        let row_height = 40.0; // Taller rows for commit info
        let max_visible = 5.min(filtered.len());
        // Account for header (~24px) + items + footer (~24px) + margins
        let popup_height = row_height * max_visible as f32 + 56.0;
        let popup_width = input_rect.width().min(400.0); // Wider for commit messages

        // Position popup above the input (with sufficient gap to not cover input)
        let popup_pos = egui::pos2(input_rect.left(), input_rect.top() - popup_height - 12.0);

        // Use Area to render as a floating overlay (not clipped by parent, renders on top)
        egui::Area::new(egui::Id::new("chat_commit_autocomplete"))
            .fixed_pos(popup_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(self.theme.bg_elevated())
                    .stroke(Stroke::new(1.0, self.theme.border_default()))
                    .corner_radius(CornerRadius::same(8))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 12,
                        spread: 0,
                        color: Color32::from_black_alpha(40),
                    })
                    .inner_margin(egui::Margin::symmetric(4, 4))
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("#")
                                    .color(self.theme.accent_primary())
                                    .size(typography::SM)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Reference a commit")
                                    .color(self.theme.text_tertiary())
                                    .size(typography::XS),
                            );
                        });

                        ui.add_space(4.0);

                        // Render items
                        for (idx, commit) in filtered.iter().take(max_visible).enumerate() {
                            let is_selected = idx == self.commit_autocomplete_index;

                            let response = ui.allocate_response(
                                Vec2::new(popup_width - 8.0, row_height),
                                egui::Sense::click(),
                            );
                            let row_rect = response.rect;

                            // Hover/selection background
                            if is_selected || response.hovered() {
                                let bg_color = if is_selected {
                                    colors.selection_bg()
                                } else {
                                    colors.hover_bg()
                                };
                                ui.painter()
                                    .rect_filled(row_rect, CornerRadius::same(4), bg_color);
                            }

                            // Selection indicator bar
                            if is_selected {
                                let indicator_rect = egui::Rect::from_min_size(
                                    row_rect.left_top(),
                                    Vec2::new(3.0, row_height),
                                );
                                ui.painter().rect_filled(
                                    indicator_rect,
                                    CornerRadius::same(2),
                                    self.theme.accent_primary(),
                                );
                            }

                            // Git commit icon
                            ui.painter().text(
                                row_rect.left_center() + Vec2::new(12.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                regular::GIT_COMMIT,
                                typography::proportional(typography::SM),
                                if is_selected || response.hovered() {
                                    self.theme.accent_primary()
                                } else {
                                    self.theme.text_secondary()
                                },
                            );

                            // Commit hash (styled as code)
                            ui.painter().text(
                                row_rect.left_center() + Vec2::new(32.0, -8.0),
                                egui::Align2::LEFT_CENTER,
                                &commit.short_hash,
                                typography::monospace(typography::SM),
                                self.theme.accent_primary(),
                            );

                            // Commit message (truncated)
                            let msg = if commit.message.len() > 50 {
                                format!("{}...", &commit.message[..47])
                            } else {
                                commit.message.clone()
                            };
                            ui.painter().text(
                                row_rect.left_center() + Vec2::new(32.0, 8.0),
                                egui::Align2::LEFT_CENTER,
                                msg,
                                typography::proportional(typography::XS),
                                self.theme.text_secondary(),
                            );

                            // Relative time
                            let relative_time = Self::format_commit_time(commit.timestamp);
                            ui.painter().text(
                                row_rect.right_center() + Vec2::new(-8.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                relative_time,
                                typography::proportional(typography::XS),
                                self.theme.text_tertiary(),
                            );

                            // Click to select
                            if response.clicked() {
                                let commit_clone = commit.clone();
                                self.complete_commit_reference(&commit_clone);
                            }
                        }

                        // Footer with hints
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("↑↓")
                                    .color(self.theme.accent_primary())
                                    .size(typography::XS),
                            );
                            ui.label(
                                RichText::new("navigate")
                                    .color(self.theme.text_tertiary())
                                    .size(typography::XS),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("⏎")
                                    .color(self.theme.accent_primary())
                                    .size(typography::XS),
                            );
                            ui.label(
                                RichText::new("insert ref")
                                    .color(self.theme.text_tertiary())
                                    .size(typography::XS),
                            );
                        });
                    });
            });
    }

    /// Format a commit timestamp as relative time.
    fn format_commit_time(timestamp: i64) -> String {
        let now = crate::util::now_unix_secs();
        let diff = now - timestamp;

        if diff < 60 {
            "just now".to_string()
        } else if diff < 3600 {
            format!("{}m ago", diff / 60)
        } else if diff < 86400 {
            format!("{}h ago", diff / 3600)
        } else if diff < 604800 {
            format!("{}d ago", diff / 86400)
        } else {
            format!("{}w ago", diff / 604800)
        }
    }

    // =========================================================================
    // Inline chart embed
    // =========================================================================

    /// Render an embedded chart placeholder.
    /// In a full implementation, this would render an actual chart snapshot.
    pub fn render_chart_embed(
        &self,
        ui: &mut egui::Ui,
        chart_name: &str,
    ) -> Option<ChatViewAction> {
        let embed_height = 180.0;
        let embed_width = ui.available_width().min(400.0);

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(embed_width, embed_height), egui::Sense::click());

        // Border with accent color
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(2.0, self.colors().chart_embed_border()),
            StrokeKind::Inside,
        );

        // Background
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), self.theme.bg_surface());

        // Chart icon and name
        ui.painter().text(
            rect.center() + Vec2::new(0.0, -16.0),
            egui::Align2::CENTER_CENTER,
            regular::CHART_LINE,
            typography::proportional(24.0),
            self.theme.accent_primary(),
        );

        ui.painter().text(
            rect.center() + Vec2::new(0.0, 16.0),
            egui::Align2::CENTER_CENTER,
            chart_name,
            typography::proportional(typography::SM),
            self.theme.text_secondary(),
        );

        // Click hint
        ui.painter().text(
            rect.center() + Vec2::new(0.0, 40.0),
            egui::Align2::CENTER_CENTER,
            "Click to view",
            typography::proportional(typography::XS),
            self.theme.text_tertiary(),
        );

        if response.clicked() {
            return Some(ChatViewAction::NavigateToChart(chart_name.to_string()));
        }

        None
    }

    /// Render an inline time series chart (snapshot embedded in message).
    fn render_inline_chart(
        &self,
        ui: &mut egui::Ui,
        chart: &InlineChart,
    ) -> Option<ChatViewAction> {
        let chart_height = chart.height.unwrap_or(150.0);
        let colors = self.colors();

        // Use asymmetric margins: more on left for y-axis labels
        let frame = egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 8,
                bottom: 8,
            });

        frame.show(ui, |ui| {
            // Title header with chart icon
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(regular::CHART_LINE)
                        .color(self.theme.accent_primary())
                        .size(12.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(&chart.title)
                        .color(self.theme.text_primary())
                        .size(typography::SM)
                        .strong(),
                );
            });

            ui.add_space(4.0);

            if chart.series.is_empty() {
                ui.label(
                    RichText::new("No data")
                        .color(self.theme.text_tertiary())
                        .size(typography::SM),
                );
            } else {
                // Render the time series chart in compact mode
                let mut ts_chart = TimeSeriesChart::new(&chart.title);
                ts_chart.set_theme(self.theme);
                ts_chart.set_show_legend(false);
                ts_chart.set_compact(true); // No background, no interaction

                for series in &chart.series {
                    ts_chart.add_series(series.clone());
                }

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), chart_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ts_chart.show(ui);
                    },
                );
            }
        });

        None
    }

    /// Render an inline stat card (single metric value).
    fn render_inline_stat(&self, ui: &mut egui::Ui, stat: &InlineStat) -> Option<ChatViewAction> {
        let colors = self.colors();

        let frame = egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(16, 12));

        frame.show(ui, |ui| {
            ui.set_min_width(120.0);

            // Label
            ui.label(
                RichText::new(&stat.label)
                    .color(self.theme.text_secondary())
                    .size(typography::XS),
            );

            ui.add_space(4.0);

            // Value with trend indicator
            ui.horizontal(|ui| {
                // Main value (large)
                ui.label(
                    RichText::new(&stat.value)
                        .color(self.theme.text_primary())
                        .size(24.0)
                        .strong(),
                );

                // Trend indicator
                let (trend_icon, trend_color) = match stat.trend {
                    StatTrend::Up => (regular::ARROW_UP, colors.trend_up()),
                    StatTrend::Down => (regular::ARROW_DOWN, colors.trend_down()),
                    StatTrend::Neutral => (regular::MINUS, self.theme.text_tertiary()),
                };

                if stat.trend != StatTrend::Neutral {
                    ui.add_space(4.0);
                    ui.label(RichText::new(trend_icon).color(trend_color).size(14.0));
                }

                // Previous value comparison
                if let Some(ref prev) = stat.previous_value {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("(was {prev})"))
                            .color(self.theme.text_tertiary())
                            .size(typography::XS),
                    );
                }
            });

            // Subtitle
            if let Some(ref subtitle) = stat.subtitle {
                ui.add_space(2.0);
                ui.label(
                    RichText::new(subtitle)
                        .color(self.theme.text_tertiary())
                        .size(typography::XS),
                );
            }
        });

        None
    }

    /// Render an inline table.
    fn render_inline_table(
        &self,
        ui: &mut egui::Ui,
        table: &InlineTable,
    ) -> Option<ChatViewAction> {
        let colors = self.colors();

        let frame = egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(12, 8));

        frame.show(ui, |ui| {
            // Title header with table icon
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(regular::TABLE)
                        .color(self.theme.accent_primary())
                        .size(12.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(&table.title)
                        .color(self.theme.text_primary())
                        .size(typography::SM)
                        .strong(),
                );
            });

            ui.add_space(6.0);

            if table.headers.is_empty() {
                ui.label(
                    RichText::new("No data")
                        .color(self.theme.text_tertiary())
                        .size(typography::SM),
                );
                return;
            }

            // Calculate column widths (simple even distribution)
            let available_width = ui.available_width() - 8.0;
            let col_count = table.headers.len();
            let col_width = available_width / col_count as f32;

            // Header row
            ui.horizontal(|ui| {
                for header in &table.headers {
                    ui.allocate_ui_with_layout(
                        egui::vec2(col_width, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new(header)
                                    .color(self.theme.text_secondary())
                                    .size(typography::XS)
                                    .strong(),
                            );
                        },
                    );
                }
            });

            // Separator line
            ui.add_space(2.0);
            let separator_rect = ui.available_rect_before_wrap();
            ui.painter().hline(
                separator_rect.x_range(),
                separator_rect.top(),
                Stroke::new(1.0, self.theme.border_subtle()),
            );
            ui.add_space(4.0);

            // Data rows (limited by max_rows)
            let visible_rows = table.rows.iter().take(table.max_rows);
            for row in visible_rows {
                ui.horizontal(|ui| {
                    for (i, cell) in row.iter().enumerate() {
                        if i < col_count {
                            ui.allocate_ui_with_layout(
                                egui::vec2(col_width, 18.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    // Truncate long cell values
                                    let display_text = if cell.len() > 20 {
                                        format!("{}...", &cell[..17])
                                    } else {
                                        cell.clone()
                                    };
                                    ui.label(
                                        RichText::new(display_text)
                                            .color(self.theme.text_primary())
                                            .size(typography::XS),
                                    );
                                },
                            );
                        }
                    }
                });
            }

            // Show "more rows" indicator
            if table.rows.len() > table.max_rows {
                ui.add_space(4.0);
                let remaining = table.rows.len() - table.max_rows;
                ui.label(
                    RichText::new(format!("... and {remaining} more rows"))
                        .color(self.theme.text_tertiary())
                        .size(typography::XS)
                        .italics(),
                );
            }
        });

        None
    }

    /// Render an inline horizontal bar chart.
    fn render_inline_bar_chart(
        &self,
        ui: &mut egui::Ui,
        chart: &InlineBarChart,
    ) -> Option<ChatViewAction> {
        let colors = self.colors();
        let bar_height = 20.0;
        let bar_spacing = 4.0;

        let frame = egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(12, 8));

        frame.show(ui, |ui| {
            // Title header with bar chart icon
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(regular::CHART_BAR)
                        .color(self.theme.accent_primary())
                        .size(12.0),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(&chart.title)
                        .color(self.theme.text_primary())
                        .size(typography::SM)
                        .strong(),
                );
            });

            ui.add_space(6.0);

            if chart.bars.is_empty() {
                ui.label(
                    RichText::new("No data")
                        .color(self.theme.text_tertiary())
                        .size(typography::SM),
                );
                return;
            }

            // Find max value for scaling
            let max_value = chart
                .bars
                .iter()
                .map(|b| b.value)
                .fold(0.0_f64, f64::max)
                .max(1.0);

            // Calculate label width (use longest label)
            let label_width =
                chart.bars.iter().map(|b| b.label.len()).max().unwrap_or(5) as f32 * 7.0;
            let label_width = label_width.clamp(60.0, 100.0);

            let available_bar_width = ui.available_width() - label_width - 60.0; // 60 for value text

            // Default bar colors (cycle through theme-appropriate colors)
            let default_colors = [
                self.theme.accent_primary(),
                colors.chart_color_1(),
                colors.chart_color_2(),
                colors.chart_color_3(),
            ];

            for (i, bar) in chart.bars.iter().enumerate() {
                ui.horizontal(|ui| {
                    // Label
                    ui.allocate_ui_with_layout(
                        egui::vec2(label_width, bar_height),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new(&bar.label)
                                    .color(self.theme.text_secondary())
                                    .size(typography::XS),
                            );
                        },
                    );

                    ui.add_space(8.0);

                    // Bar
                    let bar_width = (bar.value / max_value * available_bar_width as f64) as f32;
                    let bar_color = bar
                        .color
                        .unwrap_or(default_colors[i % default_colors.len()]);

                    let (bar_rect, _) = ui.allocate_exact_size(
                        Vec2::new(available_bar_width, bar_height),
                        egui::Sense::hover(),
                    );

                    // Background track
                    ui.painter().rect_filled(
                        bar_rect,
                        CornerRadius::same(3),
                        self.theme.bg_surface(),
                    );

                    // Filled bar
                    let filled_rect = egui::Rect::from_min_size(
                        bar_rect.min,
                        Vec2::new(bar_width.max(4.0), bar_height),
                    );
                    ui.painter()
                        .rect_filled(filled_rect, CornerRadius::same(3), bar_color);

                    ui.add_space(8.0);

                    // Value text
                    ui.label(
                        RichText::new(format!("{:.1}", bar.value))
                            .color(self.theme.text_primary())
                            .size(typography::XS)
                            .strong(),
                    );
                });

                if i < chart.bars.len() - 1 {
                    ui.add_space(bar_spacing);
                }
            }
        });

        None
    }

    /// Render any inline visualization.
    fn render_inline_visualization(
        &self,
        ui: &mut egui::Ui,
        viz: &InlineVisualization,
    ) -> Option<ChatViewAction> {
        match viz {
            InlineVisualization::Chart(chart) => self.render_inline_chart(ui, chart),
            InlineVisualization::Stat(stat) => self.render_inline_stat(ui, stat),
            InlineVisualization::Table(table) => self.render_inline_table(ui, table),
            InlineVisualization::BarChart(bar_chart) => self.render_inline_bar_chart(ui, bar_chart),
            InlineVisualization::Gauge(gauge) => self.render_inline_gauge(ui, gauge),
        }
    }

    /// Render an inline gauge visualization using the actual GaugeChart component.
    fn render_inline_gauge(
        &self,
        ui: &mut egui::Ui,
        gauge: &InlineGauge,
    ) -> Option<ChatViewAction> {
        use crate::components::pane::visualization::GaugeChart;

        let gauge_height = gauge.height.unwrap_or(120.0);
        let colors = self.colors();

        let frame = egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, colors.chart_embed_border()))
            .inner_margin(egui::Margin::symmetric(12, 8));

        frame.show(ui, |ui| {
            // Create and configure a GaugeChart
            let mut gauge_chart = GaugeChart::new(&gauge.title);
            gauge_chart.set_theme(self.theme);
            gauge_chart.set_value(gauge.value);
            gauge_chart.set_range(gauge.min, gauge.max);
            if !gauge.unit.is_empty() {
                gauge_chart.set_unit(&gauge.unit);
            }

            // Render the gauge in a constrained area
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), gauge_height),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    gauge_chart.show(ui);
                },
            );
        });

        None
    }
}

/// Header information for the chat view.
enum HeaderInfo {
    Channel(Channel),
    Thread {
        thread: Thread,
        channel_name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_view_creation() {
        let view = ChatView::new();
        assert!(view.mode.is_none());
        assert!(view.input_text.is_empty());
    }

    #[test]
    fn test_chat_view_mode() {
        let mut view = ChatView::new();
        let channel_id = uuid::Uuid::new_v4();

        view.set_mode(Some(ChatViewMode::Channel(channel_id)));
        assert!(view.is_open());

        view.set_mode(None);
        assert!(!view.is_open());
    }
}
