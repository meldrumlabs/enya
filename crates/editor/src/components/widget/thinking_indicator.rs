//! Thinking Indicator - Terminal-style animated indicator for AI agent activity.
//!
//! Displays a terminal-native animated indicator when the AI agent is working,
//! using braille spinner characters (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏), stage-based messages, and elapsed time.

use egui::{Color32, RichText, Ui};

use crate::components::util::{ActivityItem, ActivityType, ResponseStatus};
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;

/// Stage of agent thinking for display purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingStage {
    /// Initial waiting for response
    Connecting,
    /// Agent is reading/analyzing context
    ReadingContext,
    /// Agent is thinking/reasoning
    Thinking,
    /// Agent is using tools
    UsingTools,
    /// Agent is generating response
    Generating,
}

impl ThinkingStage {
    /// Get the display message for this stage.
    pub fn message(&self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::ReadingContext => "Reading context",
            Self::Thinking => "Thinking",
            Self::UsingTools => "Working",
            Self::Generating => "Generating",
        }
    }

    /// Determine stage from response status and activities.
    pub fn from_status(status: ResponseStatus, activities: &[ActivityItem]) -> Self {
        // Check for active tool use first
        let has_active_tool = activities
            .iter()
            .any(|a| a.in_progress && matches!(a.activity_type, ActivityType::ToolUse { .. }));

        if has_active_tool {
            return Self::UsingTools;
        }

        // Check for active thinking
        let has_active_thinking = activities
            .iter()
            .any(|a| a.in_progress && matches!(a.activity_type, ActivityType::Thinking(_)));

        if has_active_thinking {
            return Self::Thinking;
        }

        // Fall back to response status
        match status {
            ResponseStatus::Waiting => Self::Connecting,
            ResponseStatus::Thinking => Self::Thinking,
            ResponseStatus::Responding => Self::Generating,
            ResponseStatus::Complete => Self::Generating, // Shouldn't happen when active
        }
    }
}

/// Configuration for the thinking indicator.
pub struct ThinkingIndicatorConfig {
    /// Show elapsed time
    pub show_elapsed: bool,
    /// Show the stage message
    pub show_message: bool,
    /// Compact mode (smaller, inline)
    pub compact: bool,
}

impl Default for ThinkingIndicatorConfig {
    fn default() -> Self {
        Self {
            show_elapsed: true,
            show_message: true,
            compact: false,
        }
    }
}

/// Renders an Amp-style thinking indicator with animated pulsing dots.
pub struct ThinkingIndicator {
    /// Current theme
    theme: AppTheme,
    /// When the indicator started (for elapsed time)
    start_time: Option<Instant>,
    /// Current thinking stage
    stage: ThinkingStage,
    /// Configuration
    config: ThinkingIndicatorConfig,
}

impl ThinkingIndicator {
    /// Create a new thinking indicator.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            theme,
            start_time: None,
            stage: ThinkingStage::Connecting,
            config: ThinkingIndicatorConfig::default(),
        }
    }

    /// Set the start time for elapsed tracking.
    pub fn with_start_time(mut self, start: Option<Instant>) -> Self {
        self.start_time = start;
        self
    }

    /// Set the current stage.
    pub fn with_stage(mut self, stage: ThinkingStage) -> Self {
        self.stage = stage;
        self
    }

    /// Set compact mode.
    pub fn compact(mut self, compact: bool) -> Self {
        self.config.compact = compact;
        self
    }

    /// Determine stage from status and activities.
    pub fn with_status_and_activities(
        mut self,
        status: ResponseStatus,
        activities: &[ActivityItem],
    ) -> Self {
        self.stage = ThinkingStage::from_status(status, activities);
        self
    }

    /// Render the thinking indicator.
    pub fn show(&self, ui: &mut Ui) {
        let accent = self.theme.accent_primary();
        let text_secondary = self.theme.text_secondary();

        // Request continuous repaint for smooth animation
        ui.ctx().request_repaint();

        let dot_size = if self.config.compact { 4.0 } else { 5.0 };
        let dot_spacing = if self.config.compact { 4.0 } else { 5.0 };
        let font_size = if self.config.compact {
            typography::SM
        } else {
            typography::MD
        };

        ui.horizontal(|ui| {
            // Animated pulsing dots
            self.render_pulsing_dots(ui, accent, dot_size, dot_spacing);

            ui.add_space(if self.config.compact { 6.0 } else { 10.0 });

            // Stage message
            if self.config.show_message {
                ui.label(
                    RichText::new(self.stage.message())
                        .color(text_secondary)
                        .size(font_size),
                );
            }

            // Elapsed time
            if self.config.show_elapsed {
                if let Some(start) = self.start_time {
                    let elapsed = start.elapsed().as_secs_f32();
                    let time_str = format_elapsed(elapsed);

                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(time_str)
                            .color(accent.gamma_multiply(0.8))
                            .size(font_size),
                    );
                }
            }
        });
    }

    /// Render braille spinner.
    /// Uses the classic terminal spinner pattern: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
    fn render_pulsing_dots(&self, ui: &mut Ui, color: Color32, _dot_size: f32, _spacing: f32) {
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
}

/// Format elapsed time in a human-friendly way.
fn format_elapsed(seconds: f32) -> String {
    if seconds < 1.0 {
        "<1s".to_string()
    } else if seconds < 10.0 {
        format!("{seconds:.1}s")
    } else if seconds < 60.0 {
        format!("{seconds:.0}s")
    } else {
        let mins = (seconds / 60.0).floor() as u32;
        let secs = (seconds % 60.0).floor() as u32;
        format!("{mins}:{secs:02}")
    }
}

/// A more prominent thinking indicator for the activity area.
/// Shows the current stage with animated dots and tool activity summary.
pub struct ThinkingBanner {
    theme: AppTheme,
    start_time: Option<Instant>,
    stage: ThinkingStage,
    /// Current tool being used (if any)
    current_tool: Option<String>,
    /// Current tool summary (if any)
    current_summary: Option<String>,
}

impl ThinkingBanner {
    /// Create a new thinking banner.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            theme,
            start_time: None,
            stage: ThinkingStage::Connecting,
            current_tool: None,
            current_summary: None,
        }
    }

    /// Set the start time.
    pub fn with_start_time(mut self, start: Option<Instant>) -> Self {
        self.start_time = start;
        self
    }

    /// Set stage and extract current tool from activities.
    pub fn with_status_and_activities(
        mut self,
        status: ResponseStatus,
        activities: &[ActivityItem],
    ) -> Self {
        self.stage = ThinkingStage::from_status(status, activities);

        // Find the current active tool
        for activity in activities.iter().rev() {
            if activity.in_progress {
                if let ActivityType::ToolUse { tool, summary } = &activity.activity_type {
                    self.current_tool = Some(tool.clone());
                    self.current_summary = Some(summary.clone());
                    break;
                }
            }
        }

        self
    }

    /// Render the thinking banner.
    pub fn show(&self, ui: &mut Ui) {
        let accent = self.theme.accent_primary();
        let bg = self.theme.bg_elevated();
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();

        // Request continuous repaint for smooth animation
        ui.ctx().request_repaint();

        // Premium container with subtle background
        egui::Frame::new()
            .fill(bg.gamma_multiply(0.6))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(12, 10))
            .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Animated pulsing dots
                    self.render_pulsing_dots(ui, accent);

                    ui.add_space(12.0);

                    // Main message area
                    ui.vertical(|ui| {
                        // Primary stage message
                        ui.horizontal(|ui| {
                            let message = if let Some(tool) = &self.current_tool {
                                format!("Using {tool}")
                            } else {
                                self.stage.message().to_string()
                            };

                            ui.label(
                                RichText::new(message)
                                    .color(text_primary)
                                    .size(typography::MD),
                            );

                            // Animated ellipsis
                            let time = ui.ctx().input(|i| i.time);
                            let dot_count = ((time * 2.0) as usize % 4).max(1);
                            let dots: String = ".".repeat(dot_count);
                            ui.label(
                                RichText::new(dots)
                                    .color(text_secondary)
                                    .size(typography::MD),
                            );
                        });

                        // Tool summary if available
                        if let Some(summary) = &self.current_summary {
                            if !summary.is_empty() {
                                ui.label(
                                    RichText::new(summary)
                                        .color(text_tertiary)
                                        .size(typography::SM),
                                );
                            }
                        }
                    });

                    // Elapsed time on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(start) = self.start_time {
                            let elapsed = start.elapsed().as_secs_f32();
                            let time_str = format_elapsed(elapsed);

                            ui.label(
                                RichText::new(time_str)
                                    .color(accent)
                                    .size(typography::SM)
                                    .strong(),
                            );
                        }
                    });
                });
            });
    }

    /// Render braille spinner.
    /// Uses the classic terminal spinner pattern: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
    fn render_pulsing_dots(&self, ui: &mut Ui, color: Color32) {
        const BRAILLE_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

        let time = ui.ctx().input(|i| i.time);
        // 10 frames per second for smooth but not too fast spinning
        let frame_index = ((time * 10.0) as usize) % BRAILLE_FRAMES.len();
        let spinner_char = BRAILLE_FRAMES[frame_index];

        ui.label(
            RichText::new(spinner_char.to_string())
                .color(color)
                .size(typography::LG),
        );
    }
}
