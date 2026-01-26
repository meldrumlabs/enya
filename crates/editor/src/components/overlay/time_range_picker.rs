//! Time Range Picker - A smart overlay for selecting custom time ranges.
//!
//! This module provides a natural language time range picker with fuzzy
//! autocomplete suggestions. Users can type expressions like:
//! - "last 2h", "2h ago", "last 2 hours"
//! - "yesterday", "today"
//! - "jan 15 to jan 20"
//! - "2024-01-15 09:00 to 2024-01-15 18:00"

use egui::{Key, RichText};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;
use crate::util::now_unix_secs_f64;

/// A time suggestion with display info and parsed values
#[derive(Debug, Clone)]
pub struct TimeSuggestion {
    /// Display label (e.g., "last 2 hours")
    pub label: String,
    /// Description showing the range (e.g., "2h ago → now")
    pub description: String,
    /// Start timestamp in seconds (Unix epoch)
    pub start_secs: f64,
    /// End timestamp in seconds (Unix epoch)
    pub end_secs: f64,
}

/// Result from the time range picker overlay
#[derive(Debug, Clone, PartialEq)]
pub enum TimeRangePickerResult {
    /// No action taken
    None,
    /// User cancelled
    Cancelled,
    /// User selected a time range
    Selected { start_secs: f64, end_secs: f64 },
}

/// Smart time range picker overlay with natural language input
pub struct TimeRangePicker {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip input on the first frame after opening
    just_opened: bool,
    /// Current theme
    theme: AppTheme,
    /// User input text
    query: String,
    /// Generated suggestions based on query
    suggestions: Vec<TimeSuggestion>,
    /// Currently selected suggestion index
    selected_index: usize,
    /// Request focus on text input
    request_focus: bool,
}

impl Default for TimeRangePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeRangePicker {
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            query: String::new(),
            suggestions: Vec::new(),
            selected_index: 0,
            request_focus: false,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay
    pub fn open(&mut self) {
        self.is_open = true;
        self.just_opened = true;
        self.query.clear();
        self.selected_index = 0;
        self.request_focus = true;
        self.refresh_suggestions();
    }

    /// Close the overlay
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if the overlay is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Refresh suggestions based on current query
    fn refresh_suggestions(&mut self) {
        let now = now_unix_secs_f64();
        let query_lower = self.query.to_lowercase();

        // Build suggestions based on query
        let mut suggestions = Vec::new();

        // Default suggestions when empty or matching
        let presets = [
            ("last 5 minutes", "5m ago → now", 5.0 * 60.0),
            ("last 15 minutes", "15m ago → now", 15.0 * 60.0),
            ("last 30 minutes", "30m ago → now", 30.0 * 60.0),
            ("last 1 hour", "1h ago → now", 60.0 * 60.0),
            ("last 2 hours", "2h ago → now", 2.0 * 60.0 * 60.0),
            ("last 6 hours", "6h ago → now", 6.0 * 60.0 * 60.0),
            ("last 12 hours", "12h ago → now", 12.0 * 60.0 * 60.0),
            ("last 24 hours", "24h ago → now", 24.0 * 60.0 * 60.0),
            ("last 2 days", "2d ago → now", 2.0 * 24.0 * 60.0 * 60.0),
            ("last 7 days", "7d ago → now", 7.0 * 24.0 * 60.0 * 60.0),
            ("last 30 days", "30d ago → now", 30.0 * 24.0 * 60.0 * 60.0),
        ];

        for (label, desc, duration_secs) in presets {
            if query_lower.is_empty() || fuzzy_match(label, &query_lower) {
                suggestions.push(TimeSuggestion {
                    label: label.to_string(),
                    description: desc.to_string(),
                    start_secs: now - duration_secs,
                    end_secs: now,
                });
            }
        }

        // Try to parse custom duration from query (e.g., "2h", "30m", "1d")
        if let Some((duration_secs, label)) = parse_duration_query(&self.query) {
            let desc = format_duration_desc(duration_secs);
            // Only add if not already in suggestions
            if !suggestions.iter().any(|s| s.label == label) {
                suggestions.insert(
                    0,
                    TimeSuggestion {
                        label,
                        description: desc,
                        start_secs: now - duration_secs,
                        end_secs: now,
                    },
                );
            }
        }

        // Limit suggestions
        suggestions.truncate(10);

        self.suggestions = suggestions;

        // Reset selection if out of bounds
        if self.selected_index >= self.suggestions.len() {
            self.selected_index = 0;
        }
    }

    /// Show the overlay. Returns the result of the interaction.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> TimeRangePickerResult {
        if !self.is_open {
            return TimeRangePickerResult::None;
        }

        let mut result = TimeRangePickerResult::None;
        let mut should_close = false;
        let mut query_changed = false;

        // Handle keyboard input (not in text field)
        if !self.just_opened {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    should_close = true;
                    result = TimeRangePickerResult::Cancelled;
                }
                // Arrow keys for navigation
                if (i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(egui::Modifiers::CTRL, Key::N))
                    && !self.suggestions.is_empty()
                {
                    self.selected_index = (self.selected_index + 1) % self.suggestions.len();
                }
                if (i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                    || i.consume_key(egui::Modifiers::CTRL, Key::P))
                    && !self.suggestions.is_empty()
                {
                    self.selected_index = if self.selected_index == 0 {
                        self.suggestions.len() - 1
                    } else {
                        self.selected_index - 1
                    };
                }
                // Enter to select
                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    if let Some(suggestion) = self.suggestions.get(self.selected_index) {
                        result = TimeRangePickerResult::Selected {
                            start_secs: suggestion.start_secs,
                            end_secs: suggestion.end_secs,
                        };
                        should_close = true;
                    }
                }
            });
        } else {
            self.just_opened = false;
        }

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.45).clamp(400.0, 550.0);

        egui::Area::new(egui::Id::new("time_range_picker_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -50.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = self.theme.border_subtle();
                let muted_text = self.theme.text_primary().gamma_multiply(0.6);
                let accent_color = self.theme.accent_primary();

                egui::Frame::new()
                    .fill(overlay_style.bg)
                    .corner_radius(overlay_style.corner_radius)
                    .stroke(egui::Stroke::new(1.0, overlay_style.border))
                    .shadow(egui::epaint::Shadow {
                        spread: 8,
                        blur: 24,
                        color: egui::Color32::from_black_alpha(60),
                        offset: [0, 4],
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);
                        ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 8.0);

                        ui.vertical(|ui| {
                            ui.add_space(16.0);

                            // Header
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{} Custom Time Range",
                                        semantic_icons::time::CALENDAR
                                    ))
                                    .color(self.theme.text_primary())
                                    .size(typography::LG)
                                    .strong(),
                                );
                            });

                            ui.add_space(8.0);

                            // Separator
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                let rect = ui.available_rect_before_wrap();
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(rect.left(), rect.top()),
                                        egui::pos2(rect.left() + popup_width - 32.0, rect.top()),
                                    ],
                                    egui::Stroke::new(1.0, separator_color),
                                );
                            });

                            ui.add_space(12.0);

                            // Input field
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.vertical(|ui| {
                                    ui.set_width(popup_width - 32.0);

                                    let response = ui.add(
                                        egui::TextEdit::singleline(&mut self.query)
                                            .hint_text("e.g., last 2 hours, 30m, 1d")
                                            .desired_width(popup_width - 48.0)
                                            .font(egui::FontId::new(
                                                typography::MD,
                                                egui::FontFamily::Monospace,
                                            )),
                                    );

                                    if self.request_focus {
                                        response.request_focus();
                                        self.request_focus = false;
                                    }

                                    if response.changed() {
                                        query_changed = true;
                                    }
                                });
                            });

                            ui.add_space(8.0);

                            // Suggestions list
                            if !self.suggestions.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.add_space(16.0);
                                    ui.vertical(|ui| {
                                        for (idx, suggestion) in self.suggestions.iter().enumerate()
                                        {
                                            let is_selected = idx == self.selected_index;
                                            let bg_color = if is_selected {
                                                accent_color.gamma_multiply(0.15)
                                            } else {
                                                egui::Color32::TRANSPARENT
                                            };
                                            let text_color = if is_selected {
                                                accent_color
                                            } else {
                                                self.theme.text_primary()
                                            };

                                            let response = ui
                                                .horizontal(|ui| {
                                                    egui::Frame::new()
                                                        .fill(bg_color)
                                                        .corner_radius(4.0)
                                                        .inner_margin(egui::Margin::symmetric(8, 4))
                                                        .show(ui, |ui| {
                                                            ui.set_width(popup_width - 48.0);
                                                            ui.horizontal(|ui| {
                                                                ui.label(
                                                                    RichText::new(
                                                                        &suggestion.label,
                                                                    )
                                                                    .color(text_color)
                                                                    .size(typography::MD),
                                                                );
                                                                ui.with_layout(
                                                                    egui::Layout::right_to_left(
                                                                        egui::Align::Center,
                                                                    ),
                                                                    |ui| {
                                                                        ui.label(
                                                                            RichText::new(
                                                                                &suggestion
                                                                                    .description,
                                                                            )
                                                                            .color(muted_text)
                                                                            .size(typography::SM),
                                                                        );
                                                                    },
                                                                );
                                                            });
                                                        });
                                                })
                                                .response;

                                            // Click to select
                                            if response.interact(egui::Sense::click()).clicked() {
                                                result = TimeRangePickerResult::Selected {
                                                    start_secs: suggestion.start_secs,
                                                    end_secs: suggestion.end_secs,
                                                };
                                                should_close = true;
                                            }
                                        }
                                    });
                                });
                            }

                            ui.add_space(8.0);

                            // Footer with hints
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new("↑↓ navigate  Enter select  Esc cancel")
                                        .color(muted_text)
                                        .size(typography::XS),
                                );
                            });

                            ui.add_space(12.0);
                        });
                    });
            });

        if query_changed {
            self.refresh_suggestions();
        }

        if should_close {
            self.close();
        }

        result
    }
}

/// Simple fuzzy match - checks if all chars in query appear in target in order
fn fuzzy_match(target: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let target_lower = target.to_lowercase();
    let mut target_chars = target_lower.chars();

    for query_char in query.chars() {
        loop {
            match target_chars.next() {
                Some(target_char) if target_char == query_char => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// Parse duration from query like "2h", "30m", "1d", "last 2h"
fn parse_duration_query(query: &str) -> Option<(f64, String)> {
    let query = query.trim().to_lowercase();

    // Strip "last " prefix if present
    let query = query.strip_prefix("last ").unwrap_or(&query);

    // Try to parse number + unit
    let mut num_str = String::new();
    let mut unit_str = String::new();

    for c in query.chars() {
        if c.is_ascii_digit() || c == '.' {
            if unit_str.is_empty() {
                num_str.push(c);
            }
        } else if c.is_alphabetic() {
            unit_str.push(c);
        }
    }

    if num_str.is_empty() {
        return None;
    }

    let num: f64 = num_str.parse().ok()?;
    if num <= 0.0 {
        return None;
    }

    let (multiplier, unit_label) = match unit_str.as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => (1.0, "seconds"),
        "m" | "min" | "mins" | "minute" | "minutes" => (60.0, "minutes"),
        "h" | "hr" | "hrs" | "hour" | "hours" => (60.0 * 60.0, "hours"),
        "d" | "day" | "days" => (24.0 * 60.0 * 60.0, "days"),
        "w" | "wk" | "wks" | "week" | "weeks" => (7.0 * 24.0 * 60.0 * 60.0, "weeks"),
        _ => return None,
    };

    let duration_secs = num * multiplier;
    let label = if num == 1.0 {
        format!("last 1 {}", unit_label.trim_end_matches('s'))
    } else {
        format!("last {num} {unit_label}")
    };

    Some((duration_secs, label))
}

/// Format duration description like "2h ago → now"
fn format_duration_desc(duration_secs: f64) -> String {
    if duration_secs < 60.0 {
        format!("{}s ago → now", duration_secs as i64)
    } else if duration_secs < 60.0 * 60.0 {
        format!("{}m ago → now", (duration_secs / 60.0) as i64)
    } else if duration_secs < 24.0 * 60.0 * 60.0 {
        format!("{}h ago → now", (duration_secs / 3600.0) as i64)
    } else if duration_secs < 7.0 * 24.0 * 60.0 * 60.0 {
        format!("{}d ago → now", (duration_secs / 86400.0) as i64)
    } else {
        format!("{}w ago → now", (duration_secs / 604800.0) as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("last 2 hours", "l2h"));
        assert!(fuzzy_match("last 2 hours", "2h"));
        assert!(fuzzy_match("last 2 hours", "hours"));
        assert!(fuzzy_match("last 2 hours", ""));
        assert!(!fuzzy_match("last 2 hours", "xyz"));
    }

    #[test]
    fn test_parse_duration_query() {
        let (secs, label) = parse_duration_query("2h").unwrap();
        assert_eq!(secs, 2.0 * 60.0 * 60.0);
        assert_eq!(label, "last 2 hours");

        let (secs, label) = parse_duration_query("30m").unwrap();
        assert_eq!(secs, 30.0 * 60.0);
        assert_eq!(label, "last 30 minutes");

        let (secs, label) = parse_duration_query("1d").unwrap();
        assert_eq!(secs, 24.0 * 60.0 * 60.0);
        assert_eq!(label, "last 1 day");

        let (secs, label) = parse_duration_query("last 6h").unwrap();
        assert_eq!(secs, 6.0 * 60.0 * 60.0);
        assert_eq!(label, "last 6 hours");

        assert!(parse_duration_query("invalid").is_none());
        assert!(parse_duration_query("").is_none());
    }

    #[test]
    fn test_format_duration_desc() {
        assert_eq!(format_duration_desc(30.0), "30s ago → now");
        assert_eq!(format_duration_desc(300.0), "5m ago → now");
        assert_eq!(format_duration_desc(7200.0), "2h ago → now");
        assert_eq!(format_duration_desc(172800.0), "2d ago → now");
        assert_eq!(format_duration_desc(604800.0), "1w ago → now");
    }
}
