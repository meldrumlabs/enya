use std::time::Duration;

use egui::RichText;

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;

/// Predefined time range presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeRangePreset {
    /// Last 5 minutes
    Last5Minutes,
    /// Last 15 minutes
    #[default]
    Last15Minutes,
    /// Last 30 minutes
    Last30Minutes,
    /// Last 1 hour
    Last1Hour,
    /// Last 6 hours
    Last6Hours,
    /// Last 24 hours
    Last24Hours,
    /// Last 7 days
    Last7Days,
    /// Custom time range
    Custom,
}

impl TimeRangePreset {
    /// Get the duration for this preset
    pub fn duration(&self) -> Option<Duration> {
        match self {
            Self::Last5Minutes => Some(Duration::from_secs(5 * 60)),
            Self::Last15Minutes => Some(Duration::from_secs(15 * 60)),
            Self::Last30Minutes => Some(Duration::from_secs(30 * 60)),
            Self::Last1Hour => Some(Duration::from_secs(60 * 60)),
            Self::Last6Hours => Some(Duration::from_secs(6 * 60 * 60)),
            Self::Last24Hours => Some(Duration::from_secs(24 * 60 * 60)),
            Self::Last7Days => Some(Duration::from_secs(7 * 24 * 60 * 60)),
            Self::Custom => None,
        }
    }

    /// Get the display label for this preset
    pub fn label(&self) -> &'static str {
        match self {
            Self::Last5Minutes => "5m",
            Self::Last15Minutes => "15m",
            Self::Last30Minutes => "30m",
            Self::Last1Hour => "1h",
            Self::Last6Hours => "6h",
            Self::Last24Hours => "24h",
            Self::Last7Days => "7d",
            Self::Custom => "Custom",
        }
    }

    /// Get all presets (excluding Custom)
    pub fn all_presets() -> &'static [TimeRangePreset] {
        &[
            Self::Last5Minutes,
            Self::Last15Minutes,
            Self::Last30Minutes,
            Self::Last1Hour,
            Self::Last6Hours,
            Self::Last24Hours,
            Self::Last7Days,
        ]
    }
}

/// Represents a time range for querying metrics
#[derive(Debug, Clone, PartialEq)]
pub struct TimeRange {
    /// The selected preset (or Custom if using custom range)
    pub preset: TimeRangePreset,
    /// Start timestamp in seconds (Unix epoch)
    pub start: f64,
    /// End timestamp in seconds (Unix epoch)
    pub end: f64,
}

impl Default for TimeRange {
    fn default() -> Self {
        Self::from_preset(TimeRangePreset::default())
    }
}

impl TimeRange {
    /// Create a time range from a preset (relative to "now")
    pub fn from_preset(preset: TimeRangePreset) -> Self {
        let now = Self::now();
        let duration = preset.duration().unwrap_or(Duration::from_secs(15 * 60));
        Self {
            preset,
            start: now - duration.as_secs_f64(),
            end: now,
        }
    }

    /// Create a custom time range
    pub fn custom(start: f64, end: f64) -> Self {
        Self {
            preset: TimeRangePreset::Custom,
            start,
            end,
        }
    }

    /// Get the current timestamp in seconds
    pub fn now() -> f64 {
        // In a real app, this would use actual system time
        // For demo purposes, we use a fixed timestamp
        1700000000.0 + 3600.0 // Demo: 1 hour after the base timestamp
    }

    /// Get the duration of this time range
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64((self.end - self.start).max(0.0))
    }

    /// Update to a new preset (recalculates start/end based on "now")
    pub fn set_preset(&mut self, preset: TimeRangePreset) {
        *self = Self::from_preset(preset);
    }

    /// Refresh the time range (update end to "now" and recalculate start)
    pub fn refresh(&mut self) {
        if self.preset != TimeRangePreset::Custom {
            *self = Self::from_preset(self.preset);
        } else {
            // For custom, just update end to now and keep the same duration
            let duration = self.duration();
            self.end = Self::now();
            self.start = self.end - duration.as_secs_f64();
        }
    }

    /// Format the time range for display
    pub fn format_range(&self) -> String {
        // Simple formatting - in a real app, use proper datetime formatting
        let start_mins = ((Self::now() - self.start) / 60.0) as i64;
        let end_mins = ((Self::now() - self.end) / 60.0) as i64;

        if end_mins == 0 {
            if start_mins < 60 {
                format!("Last {start_mins} minutes")
            } else if start_mins < 60 * 24 {
                let hours = start_mins / 60;
                format!("Last {hours} hours")
            } else {
                let days = start_mins / (60 * 24);
                format!("Last {days} days")
            }
        } else {
            format!("{start_mins} min ago - {end_mins} min ago")
        }
    }
}

/// Time range toolbar component
pub struct TimeRangeToolbar {
    /// Current time range
    time_range: TimeRange,
    /// Whether auto-refresh is enabled
    auto_refresh: bool,
    /// Current theme
    theme: AppTheme,
    /// Whether the time range changed this frame
    changed: bool,
}

impl Default for TimeRangeToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeRangeToolbar {
    pub fn new() -> Self {
        Self {
            time_range: TimeRange::default(),
            auto_refresh: false,
            theme: AppTheme::default(),
            changed: false,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Get the current time range
    pub fn time_range(&self) -> &TimeRange {
        &self.time_range
    }

    /// Set the time range preset
    pub fn set_preset(&mut self, preset: TimeRangePreset) {
        self.time_range.set_preset(preset);
        self.changed = true;
    }

    /// Check if the time range changed this frame
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Check if auto-refresh is enabled
    pub fn auto_refresh(&self) -> bool {
        self.auto_refresh
    }

    /// Render the toolbar
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.changed = false;
        let text_color = text_color(self.theme);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            // Time range presets
            for preset in TimeRangePreset::all_presets() {
                let is_selected = self.time_range.preset == *preset;
                let label = preset.label();

                let button = if is_selected {
                    egui::Button::new(RichText::new(label).strong())
                        .fill(ui.visuals().selection.bg_fill)
                } else {
                    egui::Button::new(RichText::new(label).color(text_color))
                };

                if ui.add(button).clicked() {
                    self.time_range.set_preset(*preset);
                    self.changed = true;
                }
            }

            ui.separator();

            // Custom range button (future: opens a date picker)
            let custom_button = if self.time_range.preset == TimeRangePreset::Custom {
                egui::Button::new(
                    RichText::new(format!("{} Custom", semantic_icons::time::CALENDAR)).strong(),
                )
                .fill(ui.visuals().selection.bg_fill)
            } else {
                egui::Button::new(
                    RichText::new(format!("{} Custom", semantic_icons::time::CALENDAR))
                        .color(text_color.gamma_multiply(0.7)),
                )
            };

            if ui
                .add(custom_button)
                .on_hover_text("Custom time range")
                .clicked()
            {
                // Future: open a date picker modal
                log::debug!("Custom time range clicked");
            }

            ui.separator();

            // Auto-refresh toggle
            let refresh_icon = semantic_icons::action::REFRESH;

            let auto_button = if self.auto_refresh {
                egui::Button::new(RichText::new(refresh_icon).strong())
                    .fill(egui::Color32::from_rgb(34, 197, 94).gamma_multiply(0.3))
            } else {
                egui::Button::new(RichText::new(refresh_icon).color(text_color.gamma_multiply(0.7)))
            };

            if ui
                .add(auto_button)
                .on_hover_text(if self.auto_refresh {
                    "Auto-refresh ON"
                } else {
                    "Auto-refresh OFF"
                })
                .clicked()
            {
                self.auto_refresh = !self.auto_refresh;
            }

            // Manual refresh button
            if ui
                .button(RichText::new(semantic_icons::action::RELOAD).color(text_color))
                .on_hover_text("Refresh now")
                .clicked()
            {
                self.time_range.refresh();
                self.changed = true;
            }

            ui.separator();

            // Show current range description
            ui.label(
                RichText::new(self.time_range.format_range())
                    .color(text_color.gamma_multiply(0.6))
                    .small(),
            );
        });
    }
}
