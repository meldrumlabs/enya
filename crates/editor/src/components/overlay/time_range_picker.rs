//! Time Range Picker - A smart overlay for selecting custom time ranges.
//!
//! This module provides a natural language time range picker with fuzzy
//! autocomplete suggestions. Users can type expressions like:
//! - "last 2h", "2h ago", "last 2 hours"
//! - "yesterday", "today", "this week"
//! - "jan 15 to jan 20"
//! - "2024-01-15 09:00 to 2024-01-15 18:00"

use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, NaiveDateTime, NaiveTime,
    TimeZone,
};
use egui::{Key, RichText};

use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;
use crate::util::now_unix_secs_f64;

/// Category for grouping suggestions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionCategory {
    /// Quick duration presets (last 1h, last 24h, etc.)
    Quick,
    /// Named date ranges (today, yesterday, this week, etc.)
    Named,
    /// Custom date ranges (jan 15 to jan 20)
    Custom,
}

impl SuggestionCategory {
    /// Get the display label for this category
    fn label(&self) -> &'static str {
        match self {
            Self::Quick => "Quick",
            Self::Named => "Named Dates",
            Self::Custom => "Custom Range",
        }
    }

    /// Get the icon for this category
    fn icon(&self) -> &'static str {
        match self {
            Self::Quick => semantic_icons::time::TIMER,
            Self::Named => semantic_icons::time::CALENDAR,
            Self::Custom => semantic_icons::time::CLOCK,
        }
    }
}

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
    /// Category for grouping
    pub category: SuggestionCategory,
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

        // Collect suggestions in separate categories for proper grouping
        let mut custom_suggestions = Vec::new();
        let mut quick_suggestions = Vec::new();
        let mut named_suggestions = Vec::new();

        // Try to parse date range first (highest priority if matched)
        if let Some(suggestion) = parse_date_range_query(&self.query) {
            custom_suggestions.push(suggestion);
        }

        // Try to parse custom duration from query (e.g., "2h", "30m", "1d")
        if let Some((duration_secs, label)) = parse_duration_query(&self.query) {
            let desc = format_duration_desc(duration_secs);
            if !custom_suggestions
                .iter()
                .any(|s: &TimeSuggestion| s.label == label)
            {
                custom_suggestions.push(TimeSuggestion {
                    label,
                    description: desc,
                    start_secs: now - duration_secs,
                    end_secs: now,
                    category: SuggestionCategory::Quick,
                });
            }
        }

        // Get all available presets
        let named_presets = build_named_date_presets();
        let duration_presets = build_duration_presets(now);

        if query_lower.is_empty() {
            // Show curated suggestions when empty, organized by category
            // Quick durations
            let quick_labels = [
                "last 1 hour",
                "last 6 hours",
                "last 24 hours",
                "last 7 days",
            ];
            for label in quick_labels {
                if let Some(preset) = duration_presets.iter().find(|p| p.label == label) {
                    quick_suggestions.push(preset.clone());
                }
            }

            // Named dates
            let named_labels = ["today", "yesterday", "this week", "last week", "this month"];
            for label in named_labels {
                if let Some(preset) = named_presets.iter().find(|p| p.label == label) {
                    named_suggestions.push(preset.clone());
                }
            }
        } else {
            // When user is typing, filter all options by fuzzy match
            for preset in duration_presets {
                if fuzzy_match(&preset.label, &query_lower) {
                    quick_suggestions.push(preset);
                }
            }
            for preset in named_presets {
                if fuzzy_match(&preset.label, &query_lower) {
                    named_suggestions.push(preset);
                }
            }
        }

        // Combine in category order: Custom first (user's parsed input), then Quick, then Named
        let mut suggestions = Vec::new();
        suggestions.extend(custom_suggestions);
        suggestions.extend(quick_suggestions);
        suggestions.extend(named_suggestions);

        // Remove duplicates by label
        let mut seen = rustc_hash::FxHashSet::default();
        suggestions.retain(|s| seen.insert(s.label.clone()));

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
                // Number keys 1-9 for quick selection (Cmd/Ctrl + number)
                let number_keys = [
                    (Key::Num1, 0),
                    (Key::Num2, 1),
                    (Key::Num3, 2),
                    (Key::Num4, 3),
                    (Key::Num5, 4),
                    (Key::Num6, 5),
                    (Key::Num7, 6),
                    (Key::Num8, 7),
                    (Key::Num9, 8),
                ];
                for (key, idx) in number_keys {
                    if i.consume_key(egui::Modifiers::COMMAND, key) {
                        if let Some(suggestion) = self.suggestions.get(idx) {
                            result = TimeRangePickerResult::Selected {
                                start_secs: suggestion.start_secs,
                                end_secs: suggestion.end_secs,
                            };
                            should_close = true;
                        }
                    }
                }
            });
        } else {
            self.just_opened = false;
        }

        // Check if current query parses to a valid time range
        let has_valid_parse = !self.query.is_empty()
            && (parse_date_range_query(&self.query).is_some()
                || parse_duration_query(&self.query).is_some());

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.45).clamp(420.0, 520.0);

        egui::Area::new(egui::Id::new("time_range_picker_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -50.0])
            .constrain_to(ctx.available_rect())
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = self.theme.border_subtle();
                let muted_text = self.theme.text_primary().gamma_multiply(0.5);
                let subtle_text = self.theme.text_primary().gamma_multiply(0.35);
                let accent_color = self.theme.accent_primary();
                let hover_bg = self.theme.text_primary().gamma_multiply(0.06);

                egui::Frame::new()
                    .fill(overlay_style.bg)
                    .corner_radius(overlay_style.corner_radius)
                    .stroke(egui::Stroke::new(1.0, overlay_style.border))
                    .shadow(egui::epaint::Shadow {
                        spread: 12,
                        blur: 32,
                        color: egui::Color32::from_black_alpha(80),
                        offset: [0, 8],
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);
                        ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                        ui.vertical(|ui| {
                            // Input section with search icon
                            ui.add_space(16.0);
                            let input_row_response = ui
                                .horizontal(|ui| {
                                    ui.add_space(16.0);

                                    // Search icon (changes to checkmark when valid)
                                    let icon = if has_valid_parse {
                                        semantic_icons::status::SUCCESS
                                    } else {
                                        semantic_icons::action::SEARCH
                                    };
                                    let icon_color = if has_valid_parse {
                                        palette::semantic::SUCCESS
                                    } else {
                                        muted_text
                                    };
                                    ui.label(
                                        RichText::new(icon).color(icon_color).size(typography::MD),
                                    );

                                    ui.add_space(8.0);

                                    // Input field with premium styling
                                    let response = ui.add(
                                        egui::TextEdit::singleline(&mut self.query)
                                            .hint_text("Type a time range...")
                                            .desired_width(popup_width - 100.0)
                                            .font(egui::FontId::new(
                                                typography::MD,
                                                egui::FontFamily::Proportional,
                                            ))
                                            .frame(false),
                                    );

                                    if self.request_focus {
                                        response.request_focus();
                                        self.request_focus = false;
                                    }

                                    if response.changed() {
                                        query_changed = true;
                                    }
                                })
                                .response;

                            // Focus underline - subtle accent line under input area
                            let underline_rect = input_row_response.rect;
                            let underline_color = if has_valid_parse {
                                palette::semantic::SUCCESS.gamma_multiply(0.6)
                            } else {
                                accent_color.gamma_multiply(0.4)
                            };
                            ui.painter().line_segment(
                                [
                                    egui::pos2(
                                        underline_rect.left() + 40.0,
                                        underline_rect.bottom(),
                                    ),
                                    egui::pos2(
                                        underline_rect.right() - 16.0,
                                        underline_rect.bottom(),
                                    ),
                                ],
                                egui::Stroke::new(1.5, underline_color),
                            );

                            ui.add_space(12.0);

                            // Subtle separator
                            let separator_rect = ui.available_rect_before_wrap();
                            ui.painter().line_segment(
                                [
                                    egui::pos2(separator_rect.left() + 16.0, separator_rect.top()),
                                    egui::pos2(
                                        separator_rect.left() + popup_width - 16.0,
                                        separator_rect.top(),
                                    ),
                                ],
                                egui::Stroke::new(1.0, separator_color),
                            );

                            ui.add_space(8.0);

                            // Suggestions list with category grouping
                            if !self.suggestions.is_empty() {
                                let mut current_category: Option<SuggestionCategory> = None;

                                for (idx, suggestion) in self.suggestions.iter().enumerate() {
                                    // Category header (only show when category changes)
                                    if current_category != Some(suggestion.category) {
                                        if current_category.is_some() {
                                            ui.add_space(8.0);
                                        }
                                        current_category = Some(suggestion.category);

                                        ui.horizontal(|ui| {
                                            ui.add_space(16.0);
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} {}",
                                                    suggestion.category.icon(),
                                                    suggestion.category.label()
                                                ))
                                                .color(subtle_text)
                                                .size(typography::XS),
                                            );
                                        });
                                        ui.add_space(4.0);
                                    }

                                    let is_selected = idx == self.selected_index;
                                    let is_hovered = ui
                                        .ctx()
                                        .pointer_hover_pos()
                                        .map(|_| false)
                                        .unwrap_or(false);

                                    // Calculate colors based on state
                                    let bg_color = if is_selected {
                                        accent_color.gamma_multiply(0.12)
                                    } else if is_hovered {
                                        hover_bg
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    let label_color = if is_selected {
                                        accent_color
                                    } else {
                                        self.theme.text_primary()
                                    };

                                    let desc_color = if is_selected {
                                        accent_color.gamma_multiply(0.7)
                                    } else {
                                        muted_text
                                    };

                                    // Suggestion row - inset from edges for sleek look
                                    let row_margin = 12.0;
                                    let row_response = ui
                                        .horizontal(|ui| {
                                            ui.add_space(row_margin);
                                            egui::Frame::new()
                                                .fill(bg_color)
                                                .corner_radius(6.0)
                                                .inner_margin(egui::Margin {
                                                    left: 10,
                                                    right: 10,
                                                    top: 6,
                                                    bottom: 6,
                                                })
                                                .show(ui, |ui| {
                                                    // Constrain width: total - margins - inner padding
                                                    let content_width =
                                                        popup_width - (row_margin * 2.0) - 20.0;
                                                    ui.set_max_width(content_width);
                                                    ui.set_min_width(content_width);
                                                    ui.horizontal(|ui| {
                                                        // Number badge for quick selection (1-9)
                                                        if idx < 9 {
                                                            let badge_bg = self
                                                                .theme
                                                                .text_primary()
                                                                .gamma_multiply(0.08);
                                                            let badge_color = if is_selected {
                                                                accent_color.gamma_multiply(0.8)
                                                            } else {
                                                                subtle_text
                                                            };
                                                            egui::Frame::new()
                                                                .fill(badge_bg)
                                                                .corner_radius(3.0)
                                                                .inner_margin(
                                                                    egui::Margin::symmetric(4, 1),
                                                                )
                                                                .show(ui, |ui| {
                                                                    ui.label(
                                                                        RichText::new(format!(
                                                                            "{}",
                                                                            idx + 1
                                                                        ))
                                                                        .color(badge_color)
                                                                        .size(typography::XS)
                                                                        .strong(),
                                                                    );
                                                                });
                                                            ui.add_space(6.0);
                                                        }

                                                        // Label
                                                        ui.label(
                                                            RichText::new(&suggestion.label)
                                                                .color(label_color)
                                                                .size(typography::MD),
                                                        );

                                                        // Right-aligned description and enter hint
                                                        ui.with_layout(
                                                            egui::Layout::right_to_left(
                                                                egui::Align::Center,
                                                            ),
                                                            |ui| {
                                                                // Show enter hint on selected
                                                                if is_selected {
                                                                    ui.add_space(4.0);
                                                                    ui.label(
                                                                        RichText::new("↵")
                                                                            .color(
                                                                                accent_color
                                                                                    .gamma_multiply(
                                                                                        0.6,
                                                                                    ),
                                                                            )
                                                                            .size(typography::SM),
                                                                    );
                                                                }

                                                                // Description
                                                                ui.label(
                                                                    RichText::new(
                                                                        &suggestion.description,
                                                                    )
                                                                    .color(desc_color)
                                                                    .size(typography::SM),
                                                                );
                                                            },
                                                        );
                                                    });
                                                });
                                        })
                                        .response;

                                    // Handle hover state for selection
                                    if row_response.hovered() {
                                        self.selected_index = idx;
                                    }

                                    // Click to select
                                    if row_response.interact(egui::Sense::click()).clicked() {
                                        result = TimeRangePickerResult::Selected {
                                            start_secs: suggestion.start_secs,
                                            end_secs: suggestion.end_secs,
                                        };
                                        should_close = true;
                                    }
                                }
                            } else if !self.query.is_empty() {
                                // No results found - show help
                                ui.add_space(16.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("No matches found")
                                            .color(muted_text)
                                            .size(typography::SM),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(
                                            "Try: 2h, yesterday, last week, jan 15 to jan 20",
                                        )
                                        .color(subtle_text)
                                        .size(typography::XS),
                                    );
                                });
                                ui.add_space(16.0);
                            }

                            ui.add_space(12.0);

                            // Footer separator
                            let footer_rect = ui.available_rect_before_wrap();
                            ui.painter().line_segment(
                                [
                                    egui::pos2(footer_rect.left() + 16.0, footer_rect.top()),
                                    egui::pos2(
                                        footer_rect.left() + popup_width - 16.0,
                                        footer_rect.top(),
                                    ),
                                ],
                                egui::Stroke::new(1.0, separator_color),
                            );

                            // Footer with keyboard hints
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);

                                // Keyboard hints as pills
                                let hint_bg = self.theme.text_primary().gamma_multiply(0.05);
                                let hints = [
                                    ("↑↓", "navigate"),
                                    ("↵", "select"),
                                    ("⌘1-9", "quick"),
                                    ("esc", "close"),
                                ];

                                for (i, (key, action)) in hints.iter().enumerate() {
                                    if i > 0 {
                                        ui.add_space(12.0);
                                    }

                                    egui::Frame::new()
                                        .fill(hint_bg)
                                        .corner_radius(3.0)
                                        .inner_margin(egui::Margin::symmetric(4, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(*key)
                                                    .color(muted_text)
                                                    .size(typography::XS)
                                                    .strong(),
                                            );
                                        });

                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(*action)
                                            .color(subtle_text)
                                            .size(typography::XS),
                                    );
                                }
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

/// Build named date presets (today, yesterday, this week, etc.)
fn build_named_date_presets() -> Vec<TimeSuggestion> {
    let now = Local::now();
    let today = now.date_naive();
    let mut presets = Vec::new();

    // Today: start of today to now
    let today_start = today
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0);
    let now_secs = now.timestamp() as f64;
    presets.push(TimeSuggestion {
        label: "today".to_string(),
        description: format_date_range_desc(today_start, now_secs),
        start_secs: today_start,
        end_secs: now_secs,
        category: SuggestionCategory::Named,
    });

    // Yesterday: full day
    let yesterday = today - ChronoDuration::days(1);
    let yesterday_start = yesterday
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0);
    let yesterday_end = yesterday
        .and_hms_opt(23, 59, 59)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0);
    presets.push(TimeSuggestion {
        label: "yesterday".to_string(),
        description: format_date_range_desc(yesterday_start, yesterday_end),
        start_secs: yesterday_start,
        end_secs: yesterday_end,
        category: SuggestionCategory::Named,
    });

    // This week: Monday to now
    let days_since_monday = now.weekday().num_days_from_monday();
    let monday = today - ChronoDuration::days(days_since_monday as i64);
    let week_start = monday
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0);
    presets.push(TimeSuggestion {
        label: "this week".to_string(),
        description: format_date_range_desc(week_start, now_secs),
        start_secs: week_start,
        end_secs: now_secs,
        category: SuggestionCategory::Named,
    });

    // Last week: Previous Monday to Sunday
    let last_monday = monday - ChronoDuration::days(7);
    let last_sunday = monday - ChronoDuration::days(1);
    let last_week_start = last_monday
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0);
    let last_week_end = last_sunday
        .and_hms_opt(23, 59, 59)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0);
    presets.push(TimeSuggestion {
        label: "last week".to_string(),
        description: format_date_range_desc(last_week_start, last_week_end),
        start_secs: last_week_start,
        end_secs: last_week_end,
        category: SuggestionCategory::Named,
    });

    // This month: 1st of month to now
    let month_start_date = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let month_start = month_start_date
        .and_hms_opt(0, 0, 0)
        .and_then(|dt| Local.from_local_datetime(&dt).single())
        .map(|dt| dt.timestamp() as f64)
        .unwrap_or(0.0);
    presets.push(TimeSuggestion {
        label: "this month".to_string(),
        description: format_date_range_desc(month_start, now_secs),
        start_secs: month_start,
        end_secs: now_secs,
        category: SuggestionCategory::Named,
    });

    presets
}

/// Build duration-based presets (last X hours/days)
fn build_duration_presets(now: f64) -> Vec<TimeSuggestion> {
    let duration_data = [
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

    duration_data
        .into_iter()
        .map(|(label, desc, duration_secs)| TimeSuggestion {
            label: label.to_string(),
            description: desc.to_string(),
            start_secs: now - duration_secs,
            end_secs: now,
            category: SuggestionCategory::Quick,
        })
        .collect()
}

/// Parse a date range query like "jan 15 to jan 20" or "2024-01-15 to 2024-01-20"
fn parse_date_range_query(query: &str) -> Option<TimeSuggestion> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }

    // Look for range separators
    let separators = [" to ", " - ", " → ", ".."];
    for sep in separators {
        if let Some(idx) = query.find(sep) {
            let start_str = query[..idx].trim();
            let end_str = query[idx + sep.len()..].trim();

            if let (Some(start_secs), Some(end_secs)) = (
                parse_single_date(start_str, true),
                parse_single_date(end_str, false),
            ) {
                let label = format!("{start_str} to {end_str}");
                let desc = format_date_range_desc(start_secs, end_secs);
                return Some(TimeSuggestion {
                    label,
                    description: desc,
                    start_secs,
                    end_secs,
                    category: SuggestionCategory::Custom,
                });
            }
        }
    }

    // Try parsing as a single date (implies full day range)
    if let Some((start_secs, end_secs, label)) = parse_single_date_as_range(&query) {
        let desc = format_date_range_desc(start_secs, end_secs);
        return Some(TimeSuggestion {
            label,
            description: desc,
            start_secs,
            end_secs,
            category: SuggestionCategory::Custom,
        });
    }

    None
}

/// Parse a single date string into a timestamp
/// `is_start` determines whether to use start of day (true) or end of day (false)
fn parse_single_date(input: &str, is_start: bool) -> Option<f64> {
    let input = input.trim().to_lowercase();
    let now = Local::now();
    let today = now.date_naive();

    // Named dates
    match input.as_str() {
        "today" => {
            let dt = if is_start {
                today.and_hms_opt(0, 0, 0)?
            } else {
                today.and_hms_opt(23, 59, 59)?
            };
            return Local
                .from_local_datetime(&dt)
                .single()
                .map(|d| d.timestamp() as f64);
        }
        "yesterday" => {
            let date = today - ChronoDuration::days(1);
            let dt = if is_start {
                date.and_hms_opt(0, 0, 0)?
            } else {
                date.and_hms_opt(23, 59, 59)?
            };
            return Local
                .from_local_datetime(&dt)
                .single()
                .map(|d| d.timestamp() as f64);
        }
        "tomorrow" => {
            let date = today + ChronoDuration::days(1);
            let dt = if is_start {
                date.and_hms_opt(0, 0, 0)?
            } else {
                date.and_hms_opt(23, 59, 59)?
            };
            return Local
                .from_local_datetime(&dt)
                .single()
                .map(|d| d.timestamp() as f64);
        }
        _ => {}
    }

    // Try ISO date format: 2024-01-15 or 2024-01-15 09:00
    if let Some(dt) = parse_iso_datetime(&input, is_start) {
        return Some(dt);
    }

    // Try month day format: jan 15, january 15
    if let Some(dt) = parse_month_day(&input, is_start) {
        return Some(dt);
    }

    None
}

/// Parse a single date as a full day range (start to end)
fn parse_single_date_as_range(input: &str) -> Option<(f64, f64, String)> {
    let input = input.trim().to_lowercase();
    let now = Local::now();
    let today = now.date_naive();

    // Named dates that represent a range
    match input.as_str() {
        "today" | "yesterday" | "tomorrow" => {
            // Already handled in named presets
            return None;
        }
        _ => {}
    }

    // Try ISO date (just date, not datetime) -> full day
    if let Ok(date) = NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
        let start = date
            .and_hms_opt(0, 0, 0)
            .and_then(|dt| Local.from_local_datetime(&dt).single())
            .map(|d| d.timestamp() as f64)?;
        let end = date
            .and_hms_opt(23, 59, 59)
            .and_then(|dt| Local.from_local_datetime(&dt).single())
            .map(|d| d.timestamp() as f64)?;
        return Some((start, end, input.to_string()));
    }

    // Try month day -> full day
    if let Some(date) = parse_month_day_to_date(&input, today.year()) {
        let start = date
            .and_hms_opt(0, 0, 0)
            .and_then(|dt| Local.from_local_datetime(&dt).single())
            .map(|d| d.timestamp() as f64)?;
        let end = date
            .and_hms_opt(23, 59, 59)
            .and_then(|dt| Local.from_local_datetime(&dt).single())
            .map(|d| d.timestamp() as f64)?;
        return Some((start, end, input.to_string()));
    }

    None
}

/// Parse ISO date/datetime format
fn parse_iso_datetime(input: &str, is_start: bool) -> Option<f64> {
    // Try with time: 2024-01-15 09:00 or 2024-01-15T09:00
    let input_normalized = input.replace(['t', ' '], "T");
    if input_normalized.contains('T') {
        // Has time component
        let formats = ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"];
        for fmt in formats {
            if let Ok(dt) = NaiveDateTime::parse_from_str(&input_normalized, fmt) {
                return Local
                    .from_local_datetime(&dt)
                    .single()
                    .map(|d| d.timestamp() as f64);
            }
        }
    }

    // Just date: 2024-01-15
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let time = if is_start {
            NaiveTime::from_hms_opt(0, 0, 0)?
        } else {
            NaiveTime::from_hms_opt(23, 59, 59)?
        };
        let dt = date.and_time(time);
        return Local
            .from_local_datetime(&dt)
            .single()
            .map(|d| d.timestamp() as f64);
    }

    None
}

/// Parse month day format like "jan 15", "january 15", "15 jan"
fn parse_month_day(input: &str, is_start: bool) -> Option<f64> {
    let now = Local::now();
    let year = now.year();

    let date = parse_month_day_to_date(input, year)?;
    let time = if is_start {
        NaiveTime::from_hms_opt(0, 0, 0)?
    } else {
        NaiveTime::from_hms_opt(23, 59, 59)?
    };
    let dt = date.and_time(time);
    Local
        .from_local_datetime(&dt)
        .single()
        .map(|d| d.timestamp() as f64)
}

/// Parse month day to a NaiveDate
fn parse_month_day_to_date(input: &str, year: i32) -> Option<NaiveDate> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    // Try "jan 15" or "15 jan"
    let (month_str, day_str) = if parts[0].chars().next()?.is_ascii_digit() {
        (parts[1], parts[0])
    } else {
        (parts[0], parts[1])
    };

    let month = parse_month_name(month_str)?;
    let day: u32 = day_str.parse().ok()?;

    NaiveDate::from_ymd_opt(year, month, day)
}

/// Parse month name to month number (1-12)
fn parse_month_name(name: &str) -> Option<u32> {
    match name.to_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

/// Format a date range description with readable dates
fn format_date_range_desc(start_secs: f64, end_secs: f64) -> String {
    let start = DateTime::from_timestamp(start_secs as i64, 0).map(|dt| dt.with_timezone(&Local));
    let end = DateTime::from_timestamp(end_secs as i64, 0).map(|dt| dt.with_timezone(&Local));

    match (start, end) {
        (Some(s), Some(e)) => {
            let start_fmt = s.format("%b %d %H:%M");
            let end_fmt = e.format("%b %d %H:%M");
            format!("{start_fmt} → {end_fmt}")
        }
        _ => "invalid range".to_string(),
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
