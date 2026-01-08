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
use crate::ui::theme::AppTheme;
use crate::ui::typography;

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
    /// Whether the input is focused.
    input_focused: bool,
    /// Scroll to bottom flag.
    scroll_to_bottom: bool,
    /// Current user ID (for highlighting own messages).
    current_user_id: Option<UserId>,
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
            input_focused: false,
            scroll_to_bottom: true,
            current_user_id: None,
        }
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
        let action = None;

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
            return action;
        }

        // Regular message layout - use Frame for hover background
        let padding = 12.0;

        // Use a frame-based approach that auto-sizes instead of fixed height
        let frame = egui::Frame::new()
            .fill(Color32::TRANSPARENT)
            .inner_margin(egui::Margin::symmetric(0, 4));

        let response = frame
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
                    ui.vertical(|ui| {
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

                        egui::Frame::new()
                            .fill(bg_color)
                            .corner_radius(CornerRadius::same(12)) // Slightly more rounded
                            .inner_margin(egui::Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.set_max_width(content_width - 24.0);
                                ui.label(
                                    RichText::new(&message.content)
                                        .size(typography::SM)
                                        .color(self.theme.text_primary()),
                                );
                            });

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
                    });

                    ui.add_space(padding);
                });
            })
            .response;

        // Draw hover background behind content (paint on lower layer)
        if response.hovered() {
            ui.painter().rect_filled(
                response.rect.expand(2.0),
                CornerRadius::same(4),
                self.colors().message_hover_bg(),
            );
        }

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

        // Text input
        let text_rect = input_rect.shrink(8.0);
        let text_edit = egui::TextEdit::singleline(&mut self.input_text)
            .font(typography::proportional(typography::SM))
            .text_color(self.theme.text_primary())
            .frame(false)
            .hint_text(
                RichText::new("Type a message... @mention to tag")
                    .color(self.theme.text_tertiary()),
            );

        let response = ui.put(text_rect, text_edit);
        self.input_focused = response.has_focus();

        // Handle Enter to send
        if response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !self.input_text.trim().is_empty()
        {
            action = Some(ChatViewAction::SendMessage(self.input_text.clone()));
            self.input_text.clear();
            self.scroll_to_bottom = true;
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

        let can_send = !self.input_text.trim().is_empty();
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
            self.scroll_to_bottom = true;
        }

        action
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
        let channel_id = super::super::ChannelId::new();

        view.set_mode(Some(ChatViewMode::Channel(channel_id)));
        assert!(view.is_open());

        view.set_mode(None);
        assert!(!view.is_open());
    }
}
