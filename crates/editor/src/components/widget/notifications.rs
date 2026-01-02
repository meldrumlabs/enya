//! Notifications component - An nvim-notify inspired notification system for the Enya UI.
//!
//! Displays toast-style notifications in the corner of the screen with different
//! severity levels, icons, and auto-dismiss functionality.
//! Styled to match the obsidian glass emerald theme.

use std::time::Duration;

use egui::{Color32, RichText, Ui};

use crate::components::util::id_generator::next_id;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::util::Instant;

/// Notification severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Informational message
    Info,
    /// Success message
    Success,
    /// Warning message
    Warn,
    /// Error message
    Error,
}

impl NotificationLevel {
    /// Get the icon for this notification level
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => semantic_icons::status::INFO,
            Self::Success => semantic_icons::status::SUCCESS,
            Self::Warn => semantic_icons::status::WARNING,
            Self::Error => semantic_icons::status::ERROR,
        }
    }

    /// Get the accent color for this notification level
    /// Uses the obsidian glass emerald theme semantic colors
    pub fn color(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::Info => match theme {
                AppTheme::Light => Color32::from_rgb(37, 99, 235), // Blue 600
                AppTheme::Dark => palette::semantic::INFO,
            },
            Self::Success => match theme {
                AppTheme::Light => palette::accent::LIGHT, // Emerald (brand color)
                AppTheme::Dark => palette::accent::PRIMARY, // Emerald (brand color)
            },
            Self::Warn => match theme {
                AppTheme::Light => Color32::from_rgb(217, 119, 6), // Amber 600
                AppTheme::Dark => palette::semantic::WARNING,
            },
            Self::Error => match theme {
                AppTheme::Light => Color32::from_rgb(220, 38, 38), // Red 600
                AppTheme::Dark => palette::semantic::ERROR,
            },
        }
    }
}

/// A single notification
#[derive(Debug, Clone)]
pub struct Notification {
    /// Unique identifier
    pub id: u64,
    /// The notification title
    pub title: String,
    /// Optional message body
    pub message: Option<String>,
    /// Severity level
    pub level: NotificationLevel,
    /// When the notification was created
    pub created_at: Instant,
    /// How long to show the notification (None = until dismissed)
    pub duration: Option<Duration>,
    /// Whether the notification has been dismissed
    pub dismissed: bool,
}

impl Notification {
    /// Create a new notification
    pub fn new(title: impl Into<String>, level: NotificationLevel) -> Self {
        Self {
            id: next_id(),
            title: title.into(),
            message: None,
            level,
            created_at: Instant::now(),
            duration: Some(Duration::from_secs(4)),
            dismissed: false,
        }
    }

    /// Add a message body
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set custom duration (None = persistent until dismissed)
    pub fn with_duration(mut self, duration: Option<Duration>) -> Self {
        self.duration = duration;
        self
    }

    /// Check if the notification should be removed
    pub fn should_remove(&self) -> bool {
        if self.dismissed {
            return true;
        }
        if let Some(duration) = self.duration {
            self.created_at.elapsed() > duration
        } else {
            false
        }
    }

    /// Get the progress (0.0 to 1.0) for the auto-dismiss timer
    pub fn progress(&self) -> Option<f32> {
        self.duration.map(|d| {
            let elapsed = self.created_at.elapsed().as_secs_f32();
            let total = d.as_secs_f32();
            (elapsed / total).clamp(0.0, 1.0)
        })
    }
}

/// Notification manager - handles displaying and managing notifications
pub struct NotificationManager {
    /// Active notifications
    notifications: Vec<Notification>,
    /// Current theme
    theme: AppTheme,
    /// Maximum number of visible notifications
    max_visible: usize,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationManager {
    /// Create a new notification manager
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            theme: AppTheme::Dark,
            max_visible: 5,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Add a notification
    pub fn notify(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    /// Add an info notification
    pub fn info(&mut self, title: impl Into<String>) {
        self.notify(Notification::new(title, NotificationLevel::Info));
    }

    /// Add a success notification
    pub fn success(&mut self, title: impl Into<String>) {
        self.notify(Notification::new(title, NotificationLevel::Success));
    }

    /// Add a warning notification
    pub fn warn(&mut self, title: impl Into<String>) {
        self.notify(Notification::new(title, NotificationLevel::Warn));
    }

    /// Add an error notification
    pub fn error(&mut self, title: impl Into<String>) {
        self.notify(Notification::new(title, NotificationLevel::Error));
    }

    /// Dismiss a notification by id
    pub fn dismiss(&mut self, id: u64) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.dismissed = true;
        }
    }

    /// Clear all notifications
    pub fn clear(&mut self) {
        self.notifications.clear();
    }

    /// Remove expired notifications
    fn cleanup(&mut self) {
        self.notifications.retain(|n| !n.should_remove());
    }

    /// Render notifications in the top-right corner
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) {
        self.cleanup();

        if self.notifications.is_empty() {
            return;
        }

        // Request repaint for animations
        ctx.request_repaint();

        let margin = 16.0;
        let top_padding = 48.0; // Extra padding from top to avoid being cut off
        let notification_width = 320.0;
        let spacing = 8.0;
        let theme = self.theme;

        // Clone notifications for rendering to avoid borrow issues
        let visible_notifications: Vec<_> = self
            .notifications
            .iter()
            .rev()
            .take(self.max_visible)
            .cloned()
            .collect();

        let mut y_offset = margin + top_padding;

        for notification in &visible_notifications {
            let notification_id = notification.id;

            egui::Area::new(egui::Id::new(format!("notification_{notification_id}")))
                .anchor(egui::Align2::RIGHT_TOP, [-margin, y_offset])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let height =
                        Self::render_notification(ui, notification, notification_width, theme);
                    y_offset += height + spacing;
                });
        }
    }

    /// Render a single notification, returns the height used
    /// Styled with obsidian glass theme - frosted glass background with emerald accents
    fn render_notification(
        ui: &mut Ui,
        notification: &Notification,
        width: f32,
        theme: AppTheme,
    ) -> f32 {
        let accent_color = notification.level.color(theme);

        // Obsidian glass theme colors
        let (bg_color, border_color, text_color, muted_color) = match theme {
            AppTheme::Dark => (
                // Frosted glass background - semi-transparent surface
                palette::bg::SURFACE.gamma_multiply(0.95),
                palette::border::SUBTLE,
                palette::text::PRIMARY,
                palette::text::SECONDARY,
            ),
            AppTheme::Light => (
                palette::light_bg::SURFACE.gamma_multiply(0.98),
                palette::light_border::SUBTLE,
                palette::light_text::PRIMARY,
                palette::light_text::SECONDARY,
            ),
        };

        // Calculate opacity based on progress (fade out near the end)
        let opacity = if let Some(progress) = notification.progress() {
            if progress > 0.8 {
                // Fade out in the last 20%
                1.0 - ((progress - 0.8) / 0.2)
            } else {
                1.0
            }
        } else {
            1.0
        };

        let mut height = 0.0;

        egui::Frame::new()
            .fill(bg_color.gamma_multiply(opacity))
            .stroke(egui::Stroke::new(1.0, border_color.gamma_multiply(opacity)))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .shadow(egui::epaint::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: Color32::from_black_alpha((60.0 * opacity) as u8),
            })
            .show(ui, |ui| {
                ui.set_width(width);

                // Header row: icon + title + close button
                ui.horizontal(|ui| {
                    // Accent bar on the left (thicker, rounded)
                    let bar_rect = ui.allocate_space(egui::vec2(3.0, 22.0)).1;
                    ui.painter()
                        .rect_filled(bar_rect, 2.0, accent_color.gamma_multiply(opacity));

                    ui.add_space(10.0);

                    // Icon with glow effect in dark mode
                    ui.label(
                        RichText::new(notification.level.icon())
                            .color(accent_color.gamma_multiply(opacity))
                            .size(18.0),
                    );

                    ui.add_space(10.0);

                    // Title
                    ui.label(
                        RichText::new(&notification.title)
                            .color(text_color.gamma_multiply(opacity))
                            .strong()
                            .size(14.0),
                    );

                    // Close button (right aligned)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close_btn = ui.add(
                            egui::Button::new(
                                RichText::new(semantic_icons::action::CLOSE)
                                    .color(muted_color.gamma_multiply(opacity))
                                    .size(14.0),
                            )
                            .frame(false),
                        );
                        if close_btn.clicked() {
                            // Mark for dismissal - we can't mutate here directly
                            // The caller will handle this
                        }
                    });
                });

                // Message body (if present)
                if let Some(ref message) = notification.message {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(19.0); // Align with title
                        ui.label(
                            RichText::new(message)
                                .color(muted_color.gamma_multiply(opacity))
                                .size(12.0),
                        );
                    });
                }

                // Progress bar (if has duration)
                if let Some(progress) = notification.progress() {
                    ui.add_space(8.0);
                    let bar_width = width - 24.0;
                    let bar_height = 2.0;
                    let (bar_rect, _) = ui.allocate_exact_size(
                        egui::vec2(bar_width, bar_height),
                        egui::Sense::hover(),
                    );

                    // Background
                    ui.painter().rect_filled(
                        bar_rect,
                        1.0,
                        accent_color.gamma_multiply(0.2 * opacity),
                    );

                    // Progress
                    let progress_width = bar_width * (1.0 - progress);
                    let progress_rect = egui::Rect::from_min_size(
                        bar_rect.min,
                        egui::vec2(progress_width, bar_height),
                    );
                    ui.painter().rect_filled(
                        progress_rect,
                        1.0,
                        accent_color.gamma_multiply(opacity),
                    );
                }

                height = ui.min_rect().height();
            });

        height + 24.0 // Add padding
    }
}
