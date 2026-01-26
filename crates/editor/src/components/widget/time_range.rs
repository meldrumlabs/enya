use std::time::Duration;

use egui::RichText;

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

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
        // Use web_time on WASM, std::time on native
        #[cfg(target_arch = "wasm32")]
        {
            use web_time::SystemTime;
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0)
        }
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
    /// Whether the custom button was clicked this frame
    custom_clicked: bool,
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
            custom_clicked: false,
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

    /// Get the time range as nanoseconds (for query execution)
    /// Returns (start_ns, end_ns) for the current time range.
    ///
    /// For relative presets (Last 5m, Last 1h, etc.), this always calculates
    /// relative to the current time, not the time when the preset was set.
    pub fn get_range_ns(&self) -> (u128, u128) {
        // For relative presets, recalculate based on current time
        let (start_secs, end_secs) = if self.time_range.preset != TimeRangePreset::Custom {
            let now = TimeRange::now();
            let duration = self.time_range.preset.duration().unwrap_or_default();
            (now - duration.as_secs_f64(), now)
        } else {
            // Custom time range - use stored values
            (self.time_range.start, self.time_range.end)
        };

        // Convert seconds to nanoseconds
        let start_ns = (start_secs * 1_000_000_000.0) as u128;
        let end_ns = (end_secs * 1_000_000_000.0) as u128;
        (start_ns, end_ns)
    }

    /// Set the time range preset
    pub fn set_preset(&mut self, preset: TimeRangePreset) {
        self.time_range.set_preset(preset);
        self.changed = true;
    }

    /// Set a custom absolute time range.
    ///
    /// # Arguments
    ///
    /// * `start_secs` - Start timestamp in seconds (Unix epoch)
    /// * `end_secs` - End timestamp in seconds (Unix epoch)
    pub fn set_custom_range(&mut self, start_secs: f64, end_secs: f64) {
        self.time_range = TimeRange::custom(start_secs, end_secs);
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

    /// Check if the custom button was clicked this frame
    pub fn custom_clicked(&self) -> bool {
        self.custom_clicked
    }

    /// Render the toolbar (without countdown)
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.show_with_countdown(ui, None);
    }

    /// Render the toolbar with an optional refresh countdown
    #[profiling::function]
    pub fn show_with_countdown(&mut self, ui: &mut egui::Ui, countdown_secs: Option<u64>) {
        self.changed = false;
        self.custom_clicked = false;
        let text_color = self.theme.text_primary();

        // Get accent colors based on theme for better visibility
        let accent_color = self.theme.accent_primary();
        // Use subtle selection background like landing page
        let selected_bg = accent_color.gamma_multiply(0.12);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            // Time range presets
            for preset in TimeRangePreset::all_presets() {
                let is_selected = self.time_range.preset == *preset;
                let label = preset.label();

                let button = if is_selected {
                    egui::Button::new(RichText::new(label).color(accent_color).strong())
                        .fill(selected_bg)
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
                    RichText::new(format!("{} Custom", semantic_icons::time::CALENDAR))
                        .color(accent_color)
                        .strong(),
                )
                .fill(selected_bg)
            } else {
                egui::Button::new(
                    RichText::new(format!("{} Custom", semantic_icons::time::CALENDAR))
                        .color(text_color.gamma_multiply(0.7)),
                )
            };

            if ui
                .add(custom_button)
                .on_hover_text("Custom time range (Space+t)")
                .clicked()
            {
                log::debug!("Custom time range clicked");
                self.custom_clicked = true;
            }

            ui.separator();

            // Auto-refresh indicator with countdown
            if let Some(secs) = countdown_secs {
                let refresh_icon = semantic_icons::action::REFRESH;
                let countdown_label = if secs < 60 {
                    format!("{refresh_icon} {secs}s")
                } else {
                    format!("{refresh_icon} {}m", secs / 60)
                };

                ui.label(RichText::new(countdown_label).color(accent_color).strong())
                    .on_hover_text("Auto-refresh countdown (use :refresh to change)");
            }

            // Manual refresh button
            if ui
                .button(RichText::new(semantic_icons::action::RELOAD).color(text_color))
                .on_hover_text("Refresh now (use :refresh <interval> to enable auto-refresh)")
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== TimeRangePreset Tests ====================

    #[test]
    fn test_preset_duration_5_minutes() {
        let preset = TimeRangePreset::Last5Minutes;
        let duration = preset.duration().expect("should have duration");
        assert_eq!(duration, Duration::from_secs(5 * 60));
    }

    #[test]
    fn test_preset_duration_15_minutes() {
        let preset = TimeRangePreset::Last15Minutes;
        let duration = preset.duration().expect("should have duration");
        assert_eq!(duration, Duration::from_secs(15 * 60));
    }

    #[test]
    fn test_preset_duration_30_minutes() {
        let preset = TimeRangePreset::Last30Minutes;
        let duration = preset.duration().expect("should have duration");
        assert_eq!(duration, Duration::from_secs(30 * 60));
    }

    #[test]
    fn test_preset_duration_1_hour() {
        let preset = TimeRangePreset::Last1Hour;
        let duration = preset.duration().expect("should have duration");
        assert_eq!(duration, Duration::from_secs(60 * 60));
    }

    #[test]
    fn test_preset_duration_6_hours() {
        let preset = TimeRangePreset::Last6Hours;
        let duration = preset.duration().expect("should have duration");
        assert_eq!(duration, Duration::from_secs(6 * 60 * 60));
    }

    #[test]
    fn test_preset_duration_24_hours() {
        let preset = TimeRangePreset::Last24Hours;
        let duration = preset.duration().expect("should have duration");
        assert_eq!(duration, Duration::from_secs(24 * 60 * 60));
    }

    #[test]
    fn test_preset_duration_7_days() {
        let preset = TimeRangePreset::Last7Days;
        let duration = preset.duration().expect("should have duration");
        assert_eq!(duration, Duration::from_secs(7 * 24 * 60 * 60));
    }

    #[test]
    fn test_preset_duration_custom_returns_none() {
        let preset = TimeRangePreset::Custom;
        assert!(preset.duration().is_none());
    }

    #[test]
    fn test_preset_labels() {
        assert_eq!(TimeRangePreset::Last5Minutes.label(), "5m");
        assert_eq!(TimeRangePreset::Last15Minutes.label(), "15m");
        assert_eq!(TimeRangePreset::Last30Minutes.label(), "30m");
        assert_eq!(TimeRangePreset::Last1Hour.label(), "1h");
        assert_eq!(TimeRangePreset::Last6Hours.label(), "6h");
        assert_eq!(TimeRangePreset::Last24Hours.label(), "24h");
        assert_eq!(TimeRangePreset::Last7Days.label(), "7d");
        assert_eq!(TimeRangePreset::Custom.label(), "Custom");
    }

    #[test]
    fn test_all_presets_excludes_custom() {
        let presets = TimeRangePreset::all_presets();
        assert_eq!(presets.len(), 7);
        assert!(!presets.contains(&TimeRangePreset::Custom));
    }

    #[test]
    fn test_all_presets_order() {
        let presets = TimeRangePreset::all_presets();
        assert_eq!(presets[0], TimeRangePreset::Last5Minutes);
        assert_eq!(presets[1], TimeRangePreset::Last15Minutes);
        assert_eq!(presets[2], TimeRangePreset::Last30Minutes);
        assert_eq!(presets[3], TimeRangePreset::Last1Hour);
        assert_eq!(presets[4], TimeRangePreset::Last6Hours);
        assert_eq!(presets[5], TimeRangePreset::Last24Hours);
        assert_eq!(presets[6], TimeRangePreset::Last7Days);
    }

    #[test]
    fn test_default_preset_is_15_minutes() {
        assert_eq!(TimeRangePreset::default(), TimeRangePreset::Last15Minutes);
    }

    // ==================== TimeRange Tests ====================

    #[test]
    fn test_time_range_from_preset_sets_correct_preset() {
        let range = TimeRange::from_preset(TimeRangePreset::Last1Hour);
        assert_eq!(range.preset, TimeRangePreset::Last1Hour);
    }

    #[test]
    fn test_time_range_from_preset_calculates_duration() {
        let range = TimeRange::from_preset(TimeRangePreset::Last1Hour);
        // end - start should be ~1 hour (3600 seconds)
        let duration_secs = range.end - range.start;
        assert!((duration_secs - 3600.0).abs() < 1.0);
    }

    #[test]
    fn test_time_range_from_preset_end_is_now() {
        let before = TimeRange::now();
        let range = TimeRange::from_preset(TimeRangePreset::Last5Minutes);
        let after = TimeRange::now();

        // end should be between before and after
        assert!(range.end >= before);
        assert!(range.end <= after);
    }

    #[test]
    fn test_time_range_custom() {
        let start = 1000.0;
        let end = 2000.0;
        let range = TimeRange::custom(start, end);

        assert_eq!(range.preset, TimeRangePreset::Custom);
        assert_eq!(range.start, start);
        assert_eq!(range.end, end);
    }

    #[test]
    fn test_time_range_duration() {
        let range = TimeRange::custom(1000.0, 2000.0);
        let duration = range.duration();
        assert_eq!(duration, Duration::from_secs(1000));
    }

    #[test]
    fn test_time_range_duration_handles_negative() {
        // start > end should give 0 duration (max(0.0))
        let range = TimeRange::custom(2000.0, 1000.0);
        let duration = range.duration();
        assert_eq!(duration, Duration::ZERO);
    }

    #[test]
    fn test_time_range_set_preset() {
        let mut range = TimeRange::from_preset(TimeRangePreset::Last5Minutes);
        assert_eq!(range.preset, TimeRangePreset::Last5Minutes);

        range.set_preset(TimeRangePreset::Last1Hour);
        assert_eq!(range.preset, TimeRangePreset::Last1Hour);

        // Duration should now be ~1 hour
        let duration_secs = range.end - range.start;
        assert!((duration_secs - 3600.0).abs() < 1.0);
    }

    #[test]
    fn test_time_range_refresh_preset() {
        let mut range = TimeRange::from_preset(TimeRangePreset::Last5Minutes);
        let original_end = range.end;

        // Wait a tiny bit (simulate time passing)
        std::thread::sleep(std::time::Duration::from_millis(10));

        range.refresh();

        // End should be updated (newer)
        assert!(range.end >= original_end);
        // Preset should remain the same
        assert_eq!(range.preset, TimeRangePreset::Last5Minutes);
        // Duration should still be ~5 minutes
        let duration_secs = range.end - range.start;
        assert!((duration_secs - 300.0).abs() < 1.0);
    }

    #[test]
    fn test_time_range_refresh_custom_preserves_duration() {
        let mut range = TimeRange::custom(1000.0, 2000.0);
        let original_duration = range.duration();

        range.refresh();

        // Duration should be preserved
        let new_duration = range.duration();
        assert_eq!(original_duration, new_duration);
        // Preset should remain Custom
        assert_eq!(range.preset, TimeRangePreset::Custom);
        // End should now be close to "now"
        let now = TimeRange::now();
        assert!((range.end - now).abs() < 1.0);
    }

    #[test]
    fn test_time_range_default_is_15_minutes() {
        let range = TimeRange::default();
        assert_eq!(range.preset, TimeRangePreset::Last15Minutes);
        let duration_secs = range.end - range.start;
        assert!((duration_secs - 900.0).abs() < 1.0);
    }

    #[test]
    fn test_time_range_now_returns_reasonable_value() {
        let now = TimeRange::now();
        // Should be a large positive number (seconds since Unix epoch)
        // As of 2024, this should be around 1.7 billion seconds
        assert!(now > 1_700_000_000.0);
        // And not absurdly large (less than year 2100)
        assert!(now < 4_100_000_000.0);
    }

    #[test]
    fn test_format_range_minutes() {
        let now = TimeRange::now();
        let range = TimeRange {
            preset: TimeRangePreset::Last15Minutes,
            start: now - 15.0 * 60.0,
            end: now,
        };
        let formatted = range.format_range();
        assert!(formatted.contains("15 minutes") || formatted.contains("14 minutes"));
    }

    #[test]
    fn test_format_range_hours() {
        let now = TimeRange::now();
        let range = TimeRange {
            preset: TimeRangePreset::Last6Hours,
            start: now - 6.0 * 60.0 * 60.0,
            end: now,
        };
        let formatted = range.format_range();
        assert!(formatted.contains("6 hours") || formatted.contains("5 hours"));
    }

    #[test]
    fn test_format_range_days() {
        let now = TimeRange::now();
        let range = TimeRange {
            preset: TimeRangePreset::Last7Days,
            start: now - 7.0 * 24.0 * 60.0 * 60.0,
            end: now,
        };
        let formatted = range.format_range();
        assert!(formatted.contains("7 days") || formatted.contains("6 days"));
    }

    #[test]
    fn test_format_range_historical() {
        let now = TimeRange::now();
        // A range that ended 30 minutes ago
        let range = TimeRange {
            preset: TimeRangePreset::Custom,
            start: now - 60.0 * 60.0,
            end: now - 30.0 * 60.0,
        };
        let formatted = range.format_range();
        // Should show "X min ago - Y min ago" format
        assert!(formatted.contains("min ago"));
    }

    // ==================== TimeRangeToolbar Tests ====================

    #[test]
    fn test_toolbar_new() {
        let toolbar = TimeRangeToolbar::new();
        assert_eq!(toolbar.time_range().preset, TimeRangePreset::Last15Minutes);
        assert!(!toolbar.auto_refresh());
        assert!(!toolbar.changed());
    }

    #[test]
    fn test_toolbar_default_equals_new() {
        let new = TimeRangeToolbar::new();
        let default = TimeRangeToolbar::default();

        assert_eq!(new.time_range().preset, default.time_range().preset);
        assert_eq!(new.auto_refresh(), default.auto_refresh());
        assert_eq!(new.changed(), default.changed());
    }

    #[test]
    fn test_toolbar_set_preset() {
        let mut toolbar = TimeRangeToolbar::new();
        assert!(!toolbar.changed());

        toolbar.set_preset(TimeRangePreset::Last1Hour);

        assert_eq!(toolbar.time_range().preset, TimeRangePreset::Last1Hour);
        assert!(toolbar.changed());
    }

    #[test]
    fn test_toolbar_set_theme() {
        let mut toolbar = TimeRangeToolbar::new();
        toolbar.set_theme(AppTheme::Light);
        // Theme is private, but we can verify it doesn't panic
        toolbar.set_theme(AppTheme::Dark);
    }

    #[test]
    fn test_toolbar_get_range_ns_conversion() {
        let mut toolbar = TimeRangeToolbar::new();
        toolbar.set_preset(TimeRangePreset::Last5Minutes);

        let (start_ns, end_ns) = toolbar.get_range_ns();

        // end_ns should be greater than start_ns
        assert!(end_ns > start_ns);

        // The difference should be ~5 minutes in nanoseconds
        let diff_ns = end_ns - start_ns;
        let expected_ns: u128 = 5 * 60 * 1_000_000_000;

        // Allow some tolerance (1 second)
        let tolerance: u128 = 1_000_000_000;
        assert!(
            (diff_ns as i128 - expected_ns as i128).unsigned_abs() < tolerance,
            "Expected ~5 min in ns, got diff_ns={diff_ns}"
        );
    }

    #[test]
    fn test_toolbar_get_range_ns_custom_uses_stored_values() {
        let mut toolbar = TimeRangeToolbar::new();

        // Set custom range through the underlying time_range
        toolbar.time_range = TimeRange::custom(1000.0, 2000.0);

        let (start_ns, end_ns) = toolbar.get_range_ns();

        // Should use stored values, not recalculate
        let expected_start_ns = (1000.0 * 1_000_000_000.0) as u128;
        let expected_end_ns = (2000.0 * 1_000_000_000.0) as u128;

        assert_eq!(start_ns, expected_start_ns);
        assert_eq!(end_ns, expected_end_ns);
    }

    #[test]
    fn test_toolbar_get_range_ns_preset_recalculates() {
        let mut toolbar = TimeRangeToolbar::new();
        toolbar.set_preset(TimeRangePreset::Last5Minutes);

        let (_, end_ns_1) = toolbar.get_range_ns();

        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        let (_, end_ns_2) = toolbar.get_range_ns();

        // For presets, end_ns should increase (recalculated from "now")
        assert!(
            end_ns_2 >= end_ns_1,
            "end_ns should increase for presets, got {end_ns_2} >= {end_ns_1}"
        );
    }

    #[test]
    fn test_toolbar_time_range_returns_reference() {
        let toolbar = TimeRangeToolbar::new();
        let range = toolbar.time_range();
        assert_eq!(range.preset, TimeRangePreset::Last15Minutes);
    }

    #[test]
    fn test_preset_equality() {
        assert_eq!(TimeRangePreset::Last5Minutes, TimeRangePreset::Last5Minutes);
        assert_ne!(TimeRangePreset::Last5Minutes, TimeRangePreset::Last1Hour);
    }

    #[test]
    fn test_preset_clone() {
        let preset = TimeRangePreset::Last6Hours;
        let cloned = preset;
        assert_eq!(preset, cloned);
    }

    #[test]
    fn test_time_range_clone() {
        let range = TimeRange::custom(100.0, 200.0);
        let cloned = range.clone();
        assert_eq!(range.start, cloned.start);
        assert_eq!(range.end, cloned.end);
        assert_eq!(range.preset, cloned.preset);
    }

    #[test]
    fn test_time_range_equality() {
        let range1 = TimeRange::custom(100.0, 200.0);
        let range2 = TimeRange::custom(100.0, 200.0);
        let range3 = TimeRange::custom(100.0, 300.0);

        assert_eq!(range1, range2);
        assert_ne!(range1, range3);
    }
}
