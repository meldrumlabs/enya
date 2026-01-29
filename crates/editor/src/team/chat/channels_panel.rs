//! Channels panel component - threads-first sidebar layout with split view.
//!
//! This component renders a premium left-side panel with:
//! 1. Active threads at the top (for quick incident access)
//! 2. Channel tree with collapsible sections
//! 3. Team presence at the bottom
//! 4. Split view for chat messages when a channel/thread is selected
//!
//! Design: Premium glass styling with subtle depth, smooth hover states,
//! and theme-aware colors that adapt to Dark, Nord, Gruvbox, and Light themes.
//!
//! Keyboard: Arrow keys for navigation within the panel, Tab to switch sections.
//! Note: j/k keys are intentionally NOT used here to avoid conflicting with
//! vim-style pane navigation in the main viewport.
//!
//! Layout (Threads-First / Layout E - Slack/Discord style):
//! ```text
//! Split View (when channel/thread selected):
//! ┌─────────────────────────────────────────────────────────┐
//! │ Channels    │  #incidents > P99 latency spike          │
//! ├─────────────┤──────────────────────────────────────────│
//! │ THREADS     │  Alice: P99 latency spike detected...    │
//! │ 🔥 P99 spike│  Bob: Seeing elevated error rates too    │
//! │             │  Claude: Based on the metrics, the...    │
//! ├─────────────┤  You: Scaling up db replicas now         │
//! │ CHANNELS    │                                          │
//! │ # general   │  [Inline chart embed]                    │
//! │ # incidents │                                          │
//! │ # deploys   │                                          │
//! ├─────────────┤──────────────────────────────────────────│
//! │ ONLINE — 3  │  ┌────────────────────────────────────┐  │
//! │ ● Alice     │  │ Type a message... @mention  [Send] │  │
//! │ ● Bob       │  └────────────────────────────────────┘  │
//! └─────────────┴──────────────────────────────────────────┘
//! ```

use egui::{Color32, CornerRadius, RichText, ScrollArea, Stroke, Vec2};
use egui_nerdfonts::regular;
use enya_team_api::UserId;

use super::chat_view::{ChatView, ChatViewAction, ChatViewMode};
use super::thread::ThreadPriority;
use super::{Channel, ChannelId, ChatColors, ChatState, Thread, ThreadId};
use crate::components::pane::PaneInfo;
use crate::team::ui::{MemberPresence, TeamMember};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Actions that can be triggered from the channels panel.
#[derive(Debug, Clone, PartialEq)]
pub enum ChannelsPanelAction {
    /// No action.
    None,
    /// User selected a channel.
    SelectChannel(ChannelId),
    /// User selected a thread.
    SelectThread(ThreadId),
    /// User wants to create a new thread.
    CreateThread(ChannelId),
    /// User wants to create a new channel.
    CreateChannel,
    /// User clicked on a team member.
    SelectMember(enya_team_api::UserId),
    /// User wants to start a DM with a member.
    StartDM(enya_team_api::UserId),
    /// User sent a message (with optional inline chart or visualization).
    SendMessage {
        /// Message text content.
        text: String,
        /// Optional inline chart to attach.
        chart: Option<super::InlineChart>,
        /// Optional inline visualization to attach (stat, table, bar chart).
        visualization: Option<super::InlineVisualization>,
    },
    /// User is searching for commits (# autocomplete in chat).
    SearchCommits(String),
    /// User clicked a commit reference to open diff viewer.
    OpenDiffViewer {
        /// Commit hash.
        hash: String,
        /// Commit message (for title).
        message: String,
        /// Full diff content.
        diff: String,
    },
    /// User wants to return focus to the viewport (vim l key when panel is focused).
    ReturnFocusToViewport,
}

/// Section collapse state.
#[derive(Debug, Clone, Default)]
struct SectionState {
    threads_collapsed: bool,
    channels_collapsed: bool,
    team_collapsed: bool,
}

/// The channels panel component.
pub struct ChannelsPanel {
    /// Current theme.
    theme: AppTheme,
    /// Panel width (sidebar portion in split view).
    sidebar_width: f32,
    /// Total panel width (when in split view, includes chat area).
    total_width: f32,
    /// Selected channel ID.
    selected_channel: Option<ChannelId>,
    /// Selected thread ID.
    selected_thread: Option<ThreadId>,
    /// Section collapse states.
    sections: SectionState,
    /// Keyboard navigation index.
    nav_index: usize,
    /// Which section is focused (0=threads, 1=channels, 2=team).
    focused_section: usize,
    /// Chat view for displaying messages in split view.
    chat_view: ChatView,
    /// Whether split view is active (a channel or thread is selected).
    split_view_active: bool,
    /// Current user ID.
    current_user_id: Option<UserId>,
    /// Whether this panel has keyboard focus (for vim j/k/l navigation).
    has_focus: bool,
    /// Whether an overlay (style picker, command palette, etc.) blocks keyboard input.
    overlay_blocks_input: bool,
}

impl Default for ChannelsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelsPanel {
    /// Create a new channels panel.
    pub fn new() -> Self {
        Self {
            theme: AppTheme::default(),
            sidebar_width: 220.0, // Compact sidebar when split view active
            total_width: 600.0,   // Total width including chat area
            selected_channel: None,
            selected_thread: None,
            sections: SectionState::default(),
            nav_index: 0,
            focused_section: 0,
            chat_view: ChatView::new(),
            split_view_active: false,
            current_user_id: None,
            has_focus: false,
            overlay_blocks_input: false,
        }
    }

    /// Set whether an overlay blocks keyboard input (style picker, command palette, etc.).
    pub fn set_overlay_blocks_input(&mut self, blocks: bool) {
        self.overlay_blocks_input = blocks;
        // Also propagate to the chat view
        self.chat_view.set_overlay_blocks_input(blocks);
    }

    /// Set whether this panel has keyboard focus.
    pub fn set_focus(&mut self, focused: bool) {
        self.has_focus = focused;
    }

    /// Check if this panel has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.chat_view.set_theme(theme);
    }

    /// Set sidebar width (the left portion in split view).
    pub fn set_sidebar_width(&mut self, width: f32) {
        self.sidebar_width = width;
    }

    /// Set total panel width (when in split view).
    pub fn set_total_width(&mut self, width: f32) {
        self.total_width = width;
    }

    /// Set the current user ID for message ownership.
    pub fn set_current_user(&mut self, user_id: Option<UserId>) {
        self.current_user_id = user_id;
        self.chat_view.set_current_user(user_id);
    }

    /// Set available panes for @mention autocomplete in chat.
    pub fn set_available_panes(&mut self, panes: Vec<PaneInfo>) {
        self.chat_view.set_available_panes(panes);
    }

    /// Set available commits for # reference autocomplete in chat.
    pub fn set_available_commits(&mut self, commits: Vec<super::CommitInfo>) {
        self.chat_view.set_available_commits(commits);
    }

    /// Get any pending chart to attach to the next message.
    pub fn take_pending_chart(&mut self) -> Option<super::InlineChart> {
        self.chat_view.take_pending_chart()
    }

    /// Get any pending visualization to attach to the next message.
    pub fn take_pending_visualization(&mut self) -> Option<super::InlineVisualization> {
        self.chat_view.take_pending_visualization()
    }

    /// Check if split view is active.
    pub fn is_split_view_active(&self) -> bool {
        self.split_view_active
    }

    /// Get the recommended panel width (varies based on split view state).
    pub fn recommended_width(&self) -> f32 {
        if self.split_view_active {
            self.total_width
        } else {
            self.sidebar_width + 40.0 // Add some padding when not split
        }
    }

    /// Get the selected channel.
    pub fn selected_channel(&self) -> Option<ChannelId> {
        self.selected_channel
    }

    /// Get the selected thread.
    pub fn selected_thread(&self) -> Option<ThreadId> {
        self.selected_thread
    }

    /// Select a channel and open split view.
    pub fn select_channel(&mut self, id: ChannelId) {
        self.selected_channel = Some(id);
        self.selected_thread = None;
        self.split_view_active = true;
        self.chat_view.set_mode(Some(ChatViewMode::Channel(id)));
    }

    /// Select a thread and open split view.
    pub fn select_thread(&mut self, id: ThreadId) {
        self.selected_thread = Some(id);
        self.split_view_active = true;
        self.chat_view.set_mode(Some(ChatViewMode::Thread(id)));
    }

    /// Close the split view (return to sidebar-only mode).
    pub fn close_split_view(&mut self) {
        self.split_view_active = false;
        self.chat_view.set_mode(None);
        // Keep the selection for highlighting, but close the chat view
    }

    /// Clear selection and close split view.
    pub fn clear_selection(&mut self) {
        self.selected_channel = None;
        self.selected_thread = None;
        self.close_split_view();
    }

    // =========================================================================
    // Premium styling helpers
    // =========================================================================

    /// Get the chat colors helper for the current theme.
    fn colors(&self) -> ChatColors {
        ChatColors::new(self.theme)
    }

    // =========================================================================
    // Main render
    // =========================================================================

    /// Show the channels panel with split view support.
    ///
    /// When a channel or thread is selected, the panel expands to show:
    /// - Left: Compact sidebar with threads, channels, and team
    /// - Right: Chat messages with input
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        threads: &[Thread],
        channels: &[Channel],
        members: &[TeamMember],
        chat_state: &ChatState,
    ) -> ChannelsPanelAction {
        // Get section item counts for navigation bounds
        let section_counts = [threads.len(), channels.len(), members.len()];

        // Handle keyboard navigation
        // Only handle navigation when:
        // - No overlay is blocking input (style picker, command palette, etc.)
        // - AND (Panel has vim focus OR not in split view for arrow keys)
        // When in split view without vim focus, let the chat input handle keys
        let handle_nav_keys =
            !self.overlay_blocks_input && (self.has_focus || !self.split_view_active);

        let mut return_focus = false;
        let mut enter_pressed = false;
        let mut should_close_split_view = false;
        ui.ctx().input(|input| {
            // Section switching with Tab (only when not in split view or has vim focus)
            if handle_nav_keys && input.key_pressed(egui::Key::Tab) {
                self.focused_section = (self.focused_section + 1) % 3;
                self.nav_index = 0;
            }

            // Navigate within section (only when handling nav keys)
            if handle_nav_keys {
                let up_pressed = input.key_pressed(egui::Key::ArrowUp)
                    || (self.has_focus && input.key_pressed(egui::Key::K));
                let down_pressed = input.key_pressed(egui::Key::ArrowDown)
                    || (self.has_focus && input.key_pressed(egui::Key::J));

                let current_section_count = section_counts[self.focused_section];

                if up_pressed {
                    if self.nav_index > 0 {
                        self.nav_index -= 1;
                    } else if self.focused_section > 0 {
                        // Move to previous section
                        self.focused_section -= 1;
                        let prev_count = section_counts[self.focused_section];
                        self.nav_index = prev_count.saturating_sub(1);
                    }
                }
                if down_pressed {
                    if self.nav_index + 1 < current_section_count {
                        self.nav_index += 1;
                    } else if self.focused_section < 2 {
                        // Move to next section
                        self.focused_section += 1;
                        self.nav_index = 0;
                    }
                }

                // Enter key to select current item (only when panel has vim focus)
                if self.has_focus && input.key_pressed(egui::Key::Enter) {
                    enter_pressed = true;
                }
            }

            // l key navigation (vim-style, only when panel has focus and no overlay)
            // In split view: l moves focus to chat input
            // In sidebar-only: l returns focus to viewport
            if self.has_focus && !self.overlay_blocks_input && input.key_pressed(egui::Key::L) {
                if self.split_view_active {
                    // Focus the chat input (to the right)
                    self.chat_view.focus_input();
                    self.has_focus = false; // Release vim focus from sidebar
                } else {
                    return_focus = true;
                }
            }

            // Escape to close split view (only if chat input is NOT focused)
            // When chat input has focus, let chat view handle Escape to return focus to sidebar
            if input.key_pressed(egui::Key::Escape)
                && self.split_view_active
                && !self.chat_view.is_input_focused()
            {
                should_close_split_view = true;
            }
        });

        // Close split view and clear egui focus so vim keys work immediately
        if should_close_split_view {
            self.close_split_view();
            self.has_focus = true; // Restore vim focus to sidebar
            ui.ctx()
                .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
        }

        // Handle Enter key selection
        if enter_pressed {
            match self.focused_section {
                0 => {
                    // Select thread
                    if let Some(thread) = threads.get(self.nav_index) {
                        return ChannelsPanelAction::SelectThread(thread.id);
                    }
                }
                1 => {
                    // Select channel
                    if let Some(channel) = channels.get(self.nav_index) {
                        return ChannelsPanelAction::SelectChannel(channel.id);
                    }
                }
                2 => {
                    // Select team member (start DM)
                    if let Some(member) = members.get(self.nav_index) {
                        return ChannelsPanelAction::StartDM(member.user.id);
                    }
                }
                _ => {}
            }
        }

        // Return focus action if l was pressed
        if return_focus {
            return ChannelsPanelAction::ReturnFocusToViewport;
        }

        if self.split_view_active {
            // Split view layout
            self.render_split_view(ui, threads, channels, members, chat_state)
        } else {
            // Sidebar-only layout
            self.render_sidebar_only(ui, threads, channels, members)
        }
    }

    /// Render the split view (sidebar + chat).
    fn render_split_view(
        &mut self,
        ui: &mut egui::Ui,
        threads: &[Thread],
        channels: &[Channel],
        members: &[TeamMember],
        chat_state: &ChatState,
    ) -> ChannelsPanelAction {
        let mut action = ChannelsPanelAction::None;

        let available_width = ui.available_width();
        let sidebar_width = self.sidebar_width.min(available_width * 0.35);

        // Use SidePanel for the sidebar to properly fill height
        // Add accent border when panel has vim focus
        let sidebar_frame = if self.has_focus {
            egui::Frame::new()
                .fill(self.theme.bg_surface())
                .stroke(Stroke::new(2.0, self.theme.accent_primary()))
                .inner_margin(egui::Margin::ZERO)
        } else {
            egui::Frame::new()
                .fill(self.theme.bg_surface())
                .inner_margin(egui::Margin::ZERO)
        };

        egui::SidePanel::left("chat_split_sidebar")
            .resizable(false)
            .exact_width(sidebar_width)
            .frame(sidebar_frame)
            .show_inside(ui, |ui| {
                action = self.render_sidebar_content(ui, threads, channels, members);
            });

        // Right chat area takes remaining space via CentralPanel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(self.theme.bg_surface())
                    .inner_margin(egui::Margin::ZERO),
            )
            .show_inside(ui, |ui| {
                let chat_action = self.chat_view.show(ui, chat_state);

                // Handle chat view actions
                match chat_action {
                    ChatViewAction::Back | ChatViewAction::Close => {
                        self.close_split_view();
                    }
                    ChatViewAction::SendMessage(text) => {
                        // Return action to workspace to add message to chat state
                        let chart = self.chat_view.take_pending_chart();
                        let visualization = self.chat_view.take_pending_visualization();
                        action = ChannelsPanelAction::SendMessage {
                            text,
                            chart,
                            visualization,
                        };
                    }
                    ChatViewAction::ResolveThread(thread_id) => {
                        action = ChannelsPanelAction::SelectThread(thread_id);
                        // TODO: Mark thread as resolved
                        log::info!("Would resolve thread: {thread_id:?}");
                    }
                    ChatViewAction::EmbedChart => {
                        // TODO: Open chart picker
                        log::info!("Would open chart picker for embedding");
                    }
                    ChatViewAction::NavigateToChart(chart_name) => {
                        // TODO: Navigate to chart in workspace
                        log::info!("Would navigate to chart: {chart_name}");
                    }
                    ChatViewAction::ViewUser(user_id) => {
                        action = ChannelsPanelAction::SelectMember(user_id);
                    }
                    ChatViewAction::SearchCommits(query) => {
                        action = ChannelsPanelAction::SearchCommits(query);
                    }
                    ChatViewAction::OpenDiffViewer {
                        hash,
                        message,
                        diff,
                    } => {
                        action = ChannelsPanelAction::OpenDiffViewer {
                            hash,
                            message,
                            diff,
                        };
                    }
                    ChatViewAction::ReturnFocusToSidebar => {
                        // Escape pressed in chat input - return vim focus to sidebar
                        self.has_focus = true;
                    }
                    _ => {}
                }
            });

        action
    }

    /// Render sidebar-only mode (no chat view).
    fn render_sidebar_only(
        &mut self,
        ui: &mut egui::Ui,
        threads: &[Thread],
        channels: &[Channel],
        members: &[TeamMember],
    ) -> ChannelsPanelAction {
        // Premium panel frame with subtle inner shadow effect
        // Use accent border when panel has vim focus
        let panel_bg = self.theme.bg_surface();
        let (border_color, border_width) = if self.has_focus {
            (self.theme.accent_primary(), 2.0)
        } else {
            (self.theme.border_subtle(), 1.0)
        };

        let frame = egui::Frame::new()
            .fill(panel_bg)
            .stroke(Stroke::new(border_width, border_color))
            .inner_margin(egui::Margin::symmetric(0, 8));

        // Add right border highlight for depth
        let right_border = if self.theme.is_light() {
            self.theme.border_default()
        } else {
            self.theme.bg_elevated()
        };

        let mut action = ChannelsPanelAction::None;

        frame.show(ui, |ui| {
            let sidebar_width = self.sidebar_width + 40.0; // Wider when not split
            ui.set_min_width(sidebar_width);
            ui.set_max_width(sidebar_width);

            // Draw right edge highlight
            let panel_rect = ui.available_rect_before_wrap();
            ui.painter().vline(
                panel_rect.right(),
                panel_rect.y_range(),
                Stroke::new(1.0, right_border),
            );

            action = self.render_sidebar_content(ui, threads, channels, members);
        });

        action
    }

    /// Render the sidebar content (used by both modes).
    fn render_sidebar_content(
        &mut self,
        ui: &mut egui::Ui,
        threads: &[Thread],
        channels: &[Channel],
        members: &[TeamMember],
    ) -> ChannelsPanelAction {
        let mut action = ChannelsPanelAction::None;

        ScrollArea::vertical()
            .id_salt("channels_panel_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Active Threads section
                let active_threads: Vec<_> = threads
                    .iter()
                    .filter(|t| {
                        t.status == super::ThreadStatus::Active
                            && (t.has_unread()
                                || t.is_pinned
                                || t.priority == ThreadPriority::Critical)
                    })
                    .collect();

                if let Some(thread_action) =
                    self.render_threads_section(ui, &active_threads, channels)
                {
                    action = thread_action;
                }

                // Divider
                self.render_divider(ui);

                // Channels section
                if let Some(channel_action) = self.render_channels_section(ui, channels) {
                    action = channel_action;
                }

                // Divider
                self.render_divider(ui);

                // Team section
                if let Some(team_action) = self.render_team_section(ui, members) {
                    action = team_action;
                }

                ui.add_space(8.0);
            });

        action
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

    // =========================================================================
    // Threads section
    // =========================================================================

    /// Render the active threads section.
    fn render_threads_section(
        &mut self,
        ui: &mut egui::Ui,
        threads: &[&Thread],
        channels: &[Channel],
    ) -> Option<ChannelsPanelAction> {
        let mut action = None;

        // Section header with thread count
        let header_label = if threads.is_empty() {
            "THREADS".to_string()
        } else {
            format!("THREADS ({})", threads.len())
        };

        let header_response = self.render_section_header(
            ui,
            &header_label,
            regular::FIRE,
            self.focused_section == 0,
            self.sections.threads_collapsed,
        );

        if header_response.clicked() {
            self.sections.threads_collapsed = !self.sections.threads_collapsed;
        }

        if self.sections.threads_collapsed {
            return None;
        }

        ui.add_space(4.0);

        // Thread list
        if threads.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("No active threads")
                        .size(typography::XS)
                        .color(self.theme.text_tertiary())
                        .italics(),
                );
            });
        } else {
            for (idx, thread) in threads.iter().enumerate() {
                let is_selected = self.selected_thread == Some(thread.id);
                let is_nav_selected = self.focused_section == 0 && self.nav_index == idx;
                let channel_name = channels
                    .iter()
                    .find(|c| c.id == thread.channel_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or("unknown");

                if let Some(thread_action) =
                    self.render_thread_row(ui, thread, channel_name, is_selected, is_nav_selected)
                {
                    action = Some(thread_action);
                }
            }
        }

        action
    }

    /// Render a single thread row with premium styling.
    fn render_thread_row(
        &self,
        ui: &mut egui::Ui,
        thread: &Thread,
        channel_name: &str,
        is_selected: bool,
        is_nav_selected: bool,
    ) -> Option<ChannelsPanelAction> {
        let row_height = 56.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        let is_hovered = response.hovered();
        let highlight = is_selected || is_nav_selected || is_hovered;

        // Background with smooth corners
        let content_rect = rect.shrink2(egui::vec2(6.0, 2.0));
        let colors = self.colors();
        if highlight {
            let bg_color = if is_selected {
                colors.selection_bg()
            } else if is_nav_selected {
                colors.nav_highlight_bg()
            } else {
                colors.hover_bg()
            };
            ui.painter()
                .rect_filled(content_rect, CornerRadius::same(8), bg_color);

            // Add subtle left accent bar for selected items
            if is_selected {
                let accent_rect = egui::Rect::from_min_size(
                    content_rect.left_top() + Vec2::new(0.0, 8.0),
                    egui::vec2(3.0, content_rect.height() - 16.0),
                );
                ui.painter().rect_filled(
                    accent_rect,
                    CornerRadius::same(2),
                    self.theme.accent_primary(),
                );
            }
        }

        // Priority icon with glow effect for critical
        let icon_color = match thread.priority {
            ThreadPriority::Critical => colors.critical_badge(),
            ThreadPriority::High => self.theme.accent_primary(),
            ThreadPriority::Normal => self.theme.text_tertiary(),
        };
        ui.painter().text(
            content_rect.left_top() + Vec2::new(12.0, 14.0),
            egui::Align2::LEFT_TOP,
            thread.priority.icon(),
            typography::proportional(typography::MD),
            icon_color,
        );

        // Thread title
        let title_color = if highlight || thread.has_unread() {
            self.theme.text_primary()
        } else {
            self.theme.text_secondary()
        };
        let title = if thread.title.len() > 22 {
            format!("{}...", &thread.title[..19])
        } else {
            thread.title.clone()
        };
        ui.painter().text(
            content_rect.left_top() + Vec2::new(34.0, 12.0),
            egui::Align2::LEFT_TOP,
            &title,
            typography::proportional(typography::SM),
            title_color,
        );

        // Channel name and reply count
        let subtitle = format!("#{channel_name} · {}", thread.reply_summary());
        ui.painter().text(
            content_rect.left_top() + Vec2::new(34.0, 30.0),
            egui::Align2::LEFT_TOP,
            &subtitle,
            typography::proportional(typography::XS),
            self.theme.text_tertiary(),
        );

        // Unread badge (pill style)
        if thread.has_unread() {
            let badge_text = thread.unread_count.to_string();
            let badge_width = 10.0 + badge_text.len() as f32 * 6.0;
            let badge_rect = egui::Rect::from_center_size(
                content_rect.right_center() + Vec2::new(-badge_width / 2.0 - 8.0, 0.0),
                egui::vec2(badge_width, 18.0),
            );

            let badge_color = if thread.priority == ThreadPriority::Critical {
                colors.critical_badge()
            } else {
                colors.unread_badge()
            };

            ui.painter()
                .rect_filled(badge_rect, CornerRadius::same(9), badge_color);
            ui.painter().text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                badge_text,
                typography::proportional(typography::XS),
                Color32::WHITE,
            );
        } else {
            // Time indicator (only show when no unread badge)
            ui.painter().text(
                content_rect.right_top() + Vec2::new(-8.0, 12.0),
                egui::Align2::RIGHT_TOP,
                thread.relative_activity(),
                typography::proportional(typography::XS),
                self.theme.text_tertiary(),
            );
        }

        if response.clicked() {
            return Some(ChannelsPanelAction::SelectThread(thread.id));
        }

        None
    }

    // =========================================================================
    // Channels section
    // =========================================================================

    /// Render the channels section.
    fn render_channels_section(
        &mut self,
        ui: &mut egui::Ui,
        channels: &[Channel],
    ) -> Option<ChannelsPanelAction> {
        let mut action = None;

        // Section header
        let header_response = self.render_section_header(
            ui,
            "CHANNELS",
            regular::HASH,
            self.focused_section == 1,
            self.sections.channels_collapsed,
        );

        if header_response.clicked() {
            self.sections.channels_collapsed = !self.sections.channels_collapsed;
        }

        if self.sections.channels_collapsed {
            return None;
        }

        ui.add_space(4.0);

        // Channel list
        for (idx, channel) in channels.iter().enumerate() {
            let is_selected = self.selected_channel == Some(channel.id);
            let is_nav_selected = self.focused_section == 1 && self.nav_index == idx;

            if let Some(channel_action) =
                self.render_channel_row(ui, channel, is_selected, is_nav_selected)
            {
                action = Some(channel_action);
            }
        }

        // Add channel button with hover effect
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let btn_text = RichText::new(format!("{} Add channel", regular::PLUS))
                .size(typography::XS)
                .color(self.theme.text_tertiary());

            let add_btn = egui::Button::new(btn_text)
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(4));

            let response = ui.add(add_btn);
            if response.hovered() {
                ui.painter().rect_filled(
                    response.rect,
                    CornerRadius::same(4),
                    self.colors().hover_bg(),
                );
            }
            if response.clicked() {
                action = Some(ChannelsPanelAction::CreateChannel);
            }
        });

        action
    }

    /// Render a single channel row with premium styling.
    fn render_channel_row(
        &self,
        ui: &mut egui::Ui,
        channel: &Channel,
        is_selected: bool,
        is_nav_selected: bool,
    ) -> Option<ChannelsPanelAction> {
        let row_height = 34.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        let is_hovered = response.hovered();
        let highlight = is_selected || is_nav_selected || is_hovered;

        // Background with smooth corners
        let content_rect = rect.shrink2(egui::vec2(6.0, 2.0));
        let colors = self.colors();
        if highlight {
            let bg_color = if is_selected {
                colors.selection_bg()
            } else if is_nav_selected {
                colors.nav_highlight_bg()
            } else {
                colors.hover_bg()
            };
            ui.painter()
                .rect_filled(content_rect, CornerRadius::same(6), bg_color);

            // Left accent bar for selected
            if is_selected {
                let accent_rect = egui::Rect::from_min_size(
                    content_rect.left_top() + Vec2::new(0.0, 6.0),
                    egui::vec2(3.0, content_rect.height() - 12.0),
                );
                ui.painter().rect_filled(
                    accent_rect,
                    CornerRadius::same(2),
                    self.theme.accent_primary(),
                );
            }
        }

        // Channel icon (uses kind color when highlighted)
        let icon_color = if highlight {
            channel.kind.color()
        } else {
            self.theme.text_tertiary()
        };
        ui.painter().text(
            content_rect.left_center() + Vec2::new(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            channel.kind.icon(),
            typography::proportional(typography::SM),
            icon_color,
        );

        // Channel name (bold when has unread)
        let name_color = if channel.has_unread() {
            self.theme.text_primary()
        } else if highlight {
            self.theme.text_secondary()
        } else {
            self.theme.text_tertiary()
        };

        ui.painter().text(
            content_rect.left_center() + Vec2::new(30.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &channel.name,
            typography::proportional(typography::SM),
            name_color,
        );

        // Unread count badge (pill style)
        if channel.has_unread() {
            let badge_text = if channel.unread_count > 99 {
                "99+".to_string()
            } else {
                channel.unread_count.to_string()
            };

            let badge_width = 10.0 + badge_text.len() as f32 * 6.0;
            let badge_rect = egui::Rect::from_center_size(
                content_rect.right_center() + Vec2::new(-badge_width / 2.0 - 8.0, 0.0),
                egui::vec2(badge_width, 18.0),
            );

            ui.painter()
                .rect_filled(badge_rect, CornerRadius::same(9), colors.unread_badge());
            ui.painter().text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                &badge_text,
                typography::proportional(typography::XS),
                Color32::WHITE,
            );
        }

        // Muted indicator
        if channel.is_muted {
            ui.painter().text(
                content_rect.right_center() + Vec2::new(-10.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                regular::BELL_SLASH,
                typography::proportional(typography::XS),
                self.theme.text_tertiary(),
            );
        }

        if response.clicked() {
            return Some(ChannelsPanelAction::SelectChannel(channel.id));
        }

        None
    }

    // =========================================================================
    // Team section
    // =========================================================================

    /// Render the team section (Slack/Discord style with names).
    fn render_team_section(
        &mut self,
        ui: &mut egui::Ui,
        members: &[TeamMember],
    ) -> Option<ChannelsPanelAction> {
        let mut action = None;

        // Sort members by presence: online first, then idle, then offline
        let mut sorted_members: Vec<_> = members.iter().collect();
        sorted_members.sort_by_key(|m| match m.presence {
            MemberPresence::Online => 0,
            MemberPresence::Idle => 1,
            MemberPresence::Offline => 2,
        });

        // Count online members
        let online_count = members
            .iter()
            .filter(|m| matches!(m.presence, MemberPresence::Online | MemberPresence::Idle))
            .count();

        let header_text = format!("ONLINE — {online_count}");
        let header_response = self.render_section_header(
            ui,
            &header_text,
            regular::PEOPLE,
            self.focused_section == 2,
            self.sections.team_collapsed,
        );

        if header_response.clicked() {
            self.sections.team_collapsed = !self.sections.team_collapsed;
        }

        if self.sections.team_collapsed {
            return None;
        }

        ui.add_space(4.0);

        // Vertical member list (Discord/Slack style)
        for (idx, member) in sorted_members.iter().enumerate() {
            let is_nav_selected = self.focused_section == 2 && self.nav_index == idx;

            // Allocate row
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), egui::Sense::click());

            let content_rect = rect.shrink2(egui::vec2(8.0, 2.0));

            // Hover/selection background
            if response.hovered() || is_nav_selected {
                let colors = self.colors();
                let bg_color = if is_nav_selected {
                    colors.nav_highlight_bg()
                } else {
                    colors.hover_bg()
                };
                ui.painter()
                    .rect_filled(content_rect, CornerRadius::same(4), bg_color);
            }

            // Presence dot (left side)
            let dot_color = member.presence.color(self.theme);
            let dot_center = content_rect.left_center() + Vec2::new(8.0, 0.0);
            ui.painter().circle_filled(dot_center, 4.0, dot_color);

            // Member name
            let name_text = if member.is_self {
                format!("{} (you)", member.user.display_name)
            } else {
                member.user.display_name.clone()
            };

            let name_color = match member.presence {
                MemberPresence::Online => self.theme.text_primary(),
                MemberPresence::Idle => self.theme.text_secondary(),
                MemberPresence::Offline => self.theme.text_tertiary(),
            };

            ui.painter().text(
                content_rect.left_center() + Vec2::new(20.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &name_text,
                typography::proportional(typography::SM),
                name_color,
            );

            // "Viewing" indicator on the right (if applicable)
            if let Some(ref viewing) = member.viewing {
                let viewing_text = format!("{} {viewing}", regular::EYE);
                ui.painter().text(
                    content_rect.right_center() + Vec2::new(-8.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    viewing_text,
                    typography::proportional(typography::XS),
                    self.theme.text_tertiary(),
                );
            }

            // Click to select member
            if response.clicked() {
                action = Some(ChannelsPanelAction::SelectMember(member.user.id));
            }

            // Double-click to start DM
            if response.double_clicked() && !member.is_self {
                action = Some(ChannelsPanelAction::StartDM(member.user.id));
            }
        }

        action
    }

    // =========================================================================
    // Section header
    // =========================================================================

    /// Render a section header with premium styling.
    fn render_section_header(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        icon: &str,
        is_focused: bool,
        is_collapsed: bool,
    ) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), egui::Sense::click());

        let content_rect = rect.shrink2(egui::vec2(6.0, 0.0));

        // Hover background
        if response.hovered() {
            ui.painter().rect_filled(
                content_rect,
                CornerRadius::same(4),
                self.colors().hover_bg(),
            );
        }

        // Focus indicator (left accent bar)
        if is_focused {
            let indicator_rect = egui::Rect::from_min_size(
                content_rect.left_top() + Vec2::new(0.0, 6.0),
                egui::vec2(3.0, content_rect.height() - 12.0),
            );
            ui.painter().rect_filled(
                indicator_rect,
                CornerRadius::same(2),
                self.theme.accent_primary(),
            );
        }

        // Collapse chevron
        let chevron = if is_collapsed {
            regular::CHEVRON_RIGHT
        } else {
            regular::CHEVRON_DOWN
        };
        let chevron_color = if response.hovered() {
            self.theme.text_secondary()
        } else {
            self.theme.text_tertiary()
        };
        ui.painter().text(
            content_rect.left_center() + Vec2::new(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            chevron,
            typography::proportional(typography::XS),
            chevron_color,
        );

        // Icon
        let icon_color = if is_focused {
            self.theme.accent_primary()
        } else if response.hovered() {
            self.theme.text_secondary()
        } else {
            self.theme.text_tertiary()
        };
        ui.painter().text(
            content_rect.left_center() + Vec2::new(26.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            typography::proportional(typography::SM),
            icon_color,
        );

        // Label
        let label_color = if is_focused {
            self.theme.text_primary()
        } else if response.hovered() {
            self.theme.text_secondary()
        } else {
            self.theme.text_tertiary()
        };
        ui.painter().text(
            content_rect.left_center() + Vec2::new(46.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            typography::proportional(typography::XS),
            label_color,
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channels_panel_creation() {
        let panel = ChannelsPanel::new();
        assert!(panel.selected_channel.is_none());
        assert!(panel.selected_thread.is_none());
    }

    #[test]
    fn test_channel_selection() {
        let mut panel = ChannelsPanel::new();
        let channel_id = uuid::Uuid::new_v4();

        panel.select_channel(channel_id);
        assert_eq!(panel.selected_channel(), Some(channel_id));
    }
}
