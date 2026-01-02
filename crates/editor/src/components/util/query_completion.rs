//! Inline completion popup for PromQL queries.
//!
//! Provides context-aware suggestions while typing queries.

use egui::{Color32, Key};
use enya_promql::completion as promql_completion;

use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// A suggestion item for the completion popup
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The text to insert
    pub text: String,
    /// Display label (may differ from text)
    pub label: String,
    /// Icon to show
    pub icon: &'static str,
    /// Category/type description
    pub kind: CompletionKind,
}

/// The kind of completion item
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// Keyword like AND, OR, by, without
    Keyword,
    /// Operator like !, (, ), {, }, [, ]
    Operator,
    /// Tag key like "env", "service"
    TagKey,
    /// Tag value like "prod", "staging"
    TagValue,
    /// Function like sum, avg, min, max, count, rate, etc.
    Function,
    /// Duration like 1m, 5m, 1h, 1d
    Duration,
    /// Metric name like node_cpu_seconds_total
    Metric,
}

impl CompletionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Operator => "operator",
            Self::TagKey => "key",
            Self::TagValue => "value",
            Self::Function => "function",
            Self::Duration => "duration",
            Self::Metric => "metric",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Keyword => semantic_icons::completion::KEYWORD,
            Self::Operator => semantic_icons::completion::OPERATOR,
            Self::TagKey => semantic_icons::completion::TAG_KEY,
            Self::TagValue => semantic_icons::completion::TAG_VALUE,
            Self::Function => semantic_icons::completion::FUNCTION,
            Self::Duration => semantic_icons::completion::DURATION,
            Self::Metric => semantic_icons::completion::METRIC,
        }
    }
}

/// Result of showing the completion popup
#[derive(Debug, Clone)]
pub enum CompletionResult {
    /// No action taken
    None,
    /// User selected a completion item
    Selected(CompletionItem),
    /// User dismissed the popup
    Dismissed,
}

/// Inline completion popup for query input
pub struct QueryCompletion {
    /// Whether the popup is visible
    is_open: bool,
    /// Current completion items
    items: Vec<CompletionItem>,
    /// Selected item index
    selected_index: usize,
    /// Current theme
    theme: AppTheme,
    /// Known tag keys / label names (from metrics store)
    known_tag_keys: Vec<String>,
    /// Known tag values / label values per key
    known_tag_values: rustc_hash::FxHashMap<String, Vec<String>>,
    /// Known metric names (for suggesting inside functions)
    known_metrics: Vec<String>,
    /// Current completion context
    current_context: Option<promql_completion::Context>,
}

impl Default for QueryCompletion {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryCompletion {
    pub fn new() -> Self {
        Self {
            is_open: false,
            items: Vec::new(),
            selected_index: 0,
            theme: AppTheme::default(),
            known_tag_keys: Vec::new(),
            known_tag_values: rustc_hash::FxHashMap::default(),
            known_metrics: Vec::new(),
            current_context: None,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set known tag keys for completion
    pub fn set_tag_keys(&mut self, keys: Vec<String>) {
        self.known_tag_keys = keys;
    }

    /// Set known values for a specific tag key
    pub fn set_tag_values(&mut self, key: &str, values: Vec<String>) {
        self.known_tag_values.insert(key.to_string(), values);
    }

    /// Set known metric names for completion
    pub fn set_metric_names(&mut self, metrics: Vec<String>) {
        self.known_metrics = metrics;
    }

    /// Clear all known tag keys, values, and metrics
    pub fn clear(&mut self) {
        self.known_tag_keys.clear();
        self.known_tag_values.clear();
        self.known_metrics.clear();
    }

    /// Check if the popup is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Close the popup
    pub fn close(&mut self) {
        self.is_open = false;
        self.items.clear();
        self.selected_index = 0;
        self.current_context = None;
    }

    /// Update completion suggestions based on current input and cursor position
    #[profiling::function]
    pub fn update(&mut self, input: &str, cursor: usize) {
        self.items.clear();
        self.selected_index = 0;

        self.update_promql(input, cursor);

        // Show popup if we have items
        self.is_open = !self.items.is_empty();
    }

    /// Update completion for PromQL mode
    fn update_promql(&mut self, input: &str, cursor: usize) {
        use promql_completion::Context;

        let ctx = promql_completion::analyze(input, cursor);
        self.current_context = Some(ctx.clone());

        // Get syntax suggestions from promql
        let suggestions = promql_completion::syntax_suggestions(&ctx);

        match &ctx {
            Context::Empty | Context::ExpectExpr => {
                // Suggest functions/aggregations
                for s in suggestions {
                    let kind = if enya_promql::is_callable(s) {
                        CompletionKind::Function
                    } else {
                        CompletionKind::Keyword
                    };
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: kind.icon(),
                        kind,
                    });
                }
                // Also suggest metric names
                for metric in &self.known_metrics {
                    self.items.push(CompletionItem {
                        text: metric.clone(),
                        label: metric.clone(),
                        icon: CompletionKind::Metric.icon(),
                        kind: CompletionKind::Metric,
                    });
                }
            }

            Context::InName(partial) => {
                // Filter functions/keywords by partial match
                for s in suggestions {
                    let kind = if enya_promql::is_callable(s) {
                        CompletionKind::Function
                    } else if enya_promql::is_keyword(s) {
                        CompletionKind::Keyword
                    } else {
                        CompletionKind::Operator
                    };
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: kind.icon(),
                        kind,
                    });
                }
                // Also suggest metric names filtered by partial
                let partial_lower = partial.to_lowercase();
                for metric in &self.known_metrics {
                    if metric.to_lowercase().starts_with(&partial_lower) {
                        self.items.push(CompletionItem {
                            text: metric.clone(),
                            label: metric.clone(),
                            icon: CompletionKind::Metric.icon(),
                            kind: CompletionKind::Metric,
                        });
                    }
                }
            }

            Context::InSelector => {
                // Suggest label names (no partial typed yet)
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::TagKey.icon(),
                        kind: CompletionKind::TagKey,
                    });
                }
                // Add all known label names
                for key in &self.known_tag_keys {
                    self.items.push(CompletionItem {
                        text: key.clone(),
                        label: key.clone(),
                        icon: CompletionKind::TagKey.icon(),
                        kind: CompletionKind::TagKey,
                    });
                }
            }

            Context::InLabelName(partial) => {
                // Suggest label names filtered by partial match
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::TagKey.icon(),
                        kind: CompletionKind::TagKey,
                    });
                }
                // Add known label names filtered by partial
                let partial_lower = partial.to_lowercase();
                for key in &self.known_tag_keys {
                    if key.to_lowercase().starts_with(&partial_lower) {
                        self.items.push(CompletionItem {
                            text: key.clone(),
                            label: key.clone(),
                            icon: CompletionKind::TagKey.icon(),
                            kind: CompletionKind::TagKey,
                        });
                    }
                }
            }

            Context::ExpectLabelOp => {
                // Suggest label operators
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::Operator.icon(),
                        kind: CompletionKind::Operator,
                    });
                }
            }

            Context::InLabelValue { key, partial } => {
                // Suggest label values
                if let Some(values) = self.known_tag_values.get(key) {
                    let partial_lower = partial.to_lowercase();
                    for value in values {
                        if partial.is_empty() || value.to_lowercase().starts_with(&partial_lower) {
                            self.items.push(CompletionItem {
                                text: value.clone(),
                                label: value.clone(),
                                icon: CompletionKind::TagValue.icon(),
                                kind: CompletionKind::TagValue,
                            });
                        }
                    }
                }
            }

            Context::ExpectLabelCommaOrClose => {
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::Operator.icon(),
                        kind: CompletionKind::Operator,
                    });
                }
            }

            Context::InDuration(_) => {
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::Duration.icon(),
                        kind: CompletionKind::Duration,
                    });
                }
            }

            Context::ExpectModifier | Context::ExpectBinaryOp => {
                for s in suggestions {
                    let kind = if enya_promql::is_keyword(s) {
                        CompletionKind::Keyword
                    } else {
                        CompletionKind::Operator
                    };
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: kind.icon(),
                        kind,
                    });
                }
            }

            Context::ExpectGroupingOpen => {
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::Operator.icon(),
                        kind: CompletionKind::Operator,
                    });
                }
            }

            Context::InGroupingLabels | Context::InGroupingLabelName(_) => {
                // Suggest closing paren and label names
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::Operator.icon(),
                        kind: CompletionKind::Operator,
                    });
                }
                // Add known label names
                let partial = match &ctx {
                    Context::InGroupingLabelName(p) => p.to_lowercase(),
                    _ => String::new(),
                };
                for key in &self.known_tag_keys {
                    if partial.is_empty() || key.to_lowercase().starts_with(&partial) {
                        self.items.push(CompletionItem {
                            text: key.clone(),
                            label: key.clone(),
                            icon: CompletionKind::TagKey.icon(),
                            kind: CompletionKind::TagKey,
                        });
                    }
                }
            }

            Context::ExpectAtModifier => {
                for s in suggestions {
                    self.items.push(CompletionItem {
                        text: s.to_string(),
                        label: s.to_string(),
                        icon: CompletionKind::Function.icon(),
                        kind: CompletionKind::Function,
                    });
                }
            }
        }
    }

    /// Navigate selection up
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Navigate selection down
    pub fn select_next(&mut self) {
        if self.selected_index + 1 < self.items.len() {
            self.selected_index += 1;
        }
    }

    /// Get the currently selected item
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected_index)
    }

    /// Apply the selected completion to the input string.
    /// Returns the new input string and new cursor position.
    pub fn apply_completion(&self, input: &str, cursor: usize) -> Option<(String, usize)> {
        let item = self.selected_item()?;
        let ctx = self.current_context.as_ref()?;

        self.apply_promql_completion(input, cursor, item, ctx)
    }

    /// Apply completion for PromQL context
    fn apply_promql_completion(
        &self,
        input: &str,
        cursor: usize,
        item: &CompletionItem,
        ctx: &promql_completion::Context,
    ) -> Option<(String, usize)> {
        use promql_completion::Context;

        let (new_input, new_cursor) = match ctx {
            Context::Empty
            | Context::ExpectExpr
            | Context::ExpectLabelOp
            | Context::ExpectLabelCommaOrClose
            | Context::ExpectModifier
            | Context::ExpectGroupingOpen
            | Context::ExpectBinaryOp
            | Context::ExpectAtModifier => {
                // Insert at cursor, possibly with space before
                let before = &input[..cursor];
                let after = &input[cursor..];

                let needs_space = !before.is_empty()
                    && !before.ends_with(char::is_whitespace)
                    && !before.ends_with('(')
                    && !before.ends_with('{')
                    && !before.ends_with('[');
                let prefix = if needs_space { " " } else { "" };

                // Add trailing space after keywords and functions
                let needs_suffix = matches!(
                    item.kind,
                    CompletionKind::Keyword | CompletionKind::Function
                ) && !item.text.ends_with('(')
                    && !item.text.ends_with(')')
                    && !after.starts_with(char::is_whitespace)
                    && !after.starts_with('(');
                let suffix = if needs_suffix { " " } else { "" };

                let new_input = format!("{before}{prefix}{}{suffix}{after}", item.text);
                let new_cursor = cursor + prefix.len() + item.text.len() + suffix.len();
                (new_input, new_cursor)
            }

            Context::InName(partial)
            | Context::InLabelName(partial)
            | Context::InGroupingLabelName(partial) => {
                // Replace the partial with the full text
                let word_start = find_word_start(input, cursor, partial);
                let before = &input[..word_start];
                let after = &input[cursor..];
                let new_input = format!("{before}{}{after}", item.text);
                let new_cursor = word_start + item.text.len();
                (new_input, new_cursor)
            }

            Context::InSelector | Context::InGroupingLabels => {
                // Insert at cursor
                let before = &input[..cursor];
                let after = &input[cursor..];
                let new_input = format!("{before}{}{after}", item.text);
                let new_cursor = cursor + item.text.len();
                (new_input, new_cursor)
            }

            Context::InLabelValue { partial, .. } => {
                // Find where the value starts (after the quote or = operator)
                let before_cursor = &input[..cursor];

                // Look for the quote or the = operator
                let value_start = before_cursor
                    .rfind('"')
                    .or_else(|| before_cursor.rfind('\''))
                    .or_else(|| before_cursor.rfind('='))
                    .map(|i| i + 1)
                    .unwrap_or(cursor.saturating_sub(partial.len()));

                let before = &input[..value_start];
                let after = &input[cursor..];
                let new_input = format!("{before}{}{after}", item.text);
                let new_cursor = value_start + item.text.len();
                (new_input, new_cursor)
            }

            Context::InDuration(partial) => {
                // Find where the duration starts (after the opening bracket)
                let before_cursor = &input[..cursor];
                let duration_start = before_cursor
                    .rfind('[')
                    .map(|i| i + 1)
                    .unwrap_or(cursor.saturating_sub(partial.len()));

                let before = &input[..duration_start];
                let after = &input[cursor..];
                let new_input = format!("{before}{}{after}", item.text);
                let new_cursor = duration_start + item.text.len();
                (new_input, new_cursor)
            }
        };

        Some((new_input, new_cursor))
    }

    /// Show the completion popup near the text input.
    /// Returns the result of user interaction.
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui, text_edit_rect: egui::Rect) -> CompletionResult {
        if !self.is_open || self.items.is_empty() {
            return CompletionResult::None;
        }

        let mut result = CompletionResult::None;

        // Position popup below the text input
        let popup_pos = egui::pos2(text_edit_rect.left(), text_edit_rect.bottom() + 4.0);
        let popup_width = text_edit_rect.width().clamp(500.0, 600.0);
        let item_height = 32.0;
        let max_visible_items = 8;
        let visible_items = self.items.len().min(max_visible_items);
        let popup_height = visible_items as f32 * item_height + 8.0;

        // Premium Obsidian Glass theme colors - darker, more distinct popup
        let bg_color = match self.theme {
            AppTheme::Light => palette::light_bg::SURFACE,
            AppTheme::Dark => Color32::from_rgb(16, 16, 20), // Darker obsidian for distinction
        };
        let border_color = match self.theme {
            AppTheme::Light => palette::light_border::DEFAULT,
            AppTheme::Dark => Color32::from_rgb(50, 55, 52), // Subtle emerald-tinted border
        };
        let text_col = palette::text_primary(self.theme);
        let text_tertiary = palette::text_tertiary(self.theme);
        let accent_color = match self.theme {
            AppTheme::Light => palette::accent::LIGHT,
            AppTheme::Dark => palette::accent::HOVER, // Luminous emerald
        };

        let popup_rect =
            egui::Rect::from_min_size(popup_pos, egui::vec2(popup_width, popup_height));

        // Premium layered shadows for depth perception - deeper shadows
        // Outer ambient shadow
        let shadow_rect = popup_rect.translate(egui::vec2(0.0, 10.0)).expand(12.0);
        ui.painter()
            .rect_filled(shadow_rect, 16.0, Color32::from_black_alpha(80));
        // Mid shadow
        let shadow_rect2 = popup_rect.translate(egui::vec2(0.0, 5.0)).expand(6.0);
        ui.painter()
            .rect_filled(shadow_rect2, 14.0, Color32::from_black_alpha(60));
        // Inner contact shadow
        let shadow_rect3 = popup_rect.translate(egui::vec2(0.0, 2.0)).expand(2.0);
        ui.painter()
            .rect_filled(shadow_rect3, 12.0, Color32::from_black_alpha(40));

        // Draw popup background with premium rounded corners
        ui.painter().rect(
            popup_rect,
            12.0, // More rounded for premium feel
            bg_color,
            egui::Stroke::new(1.0, border_color),
            egui::StrokeKind::Inside,
        );

        // Inner top highlight for glass effect - more visible
        let highlight_rect = egui::Rect::from_min_size(
            popup_rect.left_top() + egui::vec2(1.0, 1.0),
            egui::vec2(popup_rect.width() - 2.0, 1.5),
        );
        let highlight_color = match self.theme {
            AppTheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 80),
            AppTheme::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 20), // Stronger highlight
        };
        ui.painter()
            .rect_filled(highlight_rect, 10.0, highlight_color);

        // Subtle inner glow at top for depth
        let glow_rect = egui::Rect::from_min_size(
            popup_rect.left_top() + egui::vec2(2.0, 3.0),
            egui::vec2(popup_rect.width() - 4.0, 8.0),
        );
        ui.painter().rect_filled(
            glow_rect,
            8.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 4),
        );

        // Draw items
        let content_rect = popup_rect.shrink(4.0);
        let mut y = content_rect.top();

        for (i, item) in self.items.iter().enumerate().take(max_visible_items) {
            let item_rect = egui::Rect::from_min_size(
                egui::pos2(content_rect.left(), y),
                egui::vec2(content_rect.width(), item_height),
            );
            let is_selected = i == self.selected_index;

            // Handle hover
            let response = ui.allocate_rect(item_rect, egui::Sense::click());
            let is_hovered = response.hovered();

            // Item background - use subtle hover style like landing page
            let item_bg = if is_selected {
                accent_color.gamma_multiply(0.12)
            } else if is_hovered {
                text_col.gamma_multiply(0.05)
            } else {
                Color32::TRANSPARENT
            };

            if item_bg != Color32::TRANSPARENT {
                ui.painter().rect_filled(item_rect, 6.0, item_bg);
            }

            // Selection indicator - elegant emerald accent bar with glow
            if is_selected {
                // Glow behind indicator - more visible
                let glow_rect = egui::Rect::from_min_size(
                    item_rect.min - egui::vec2(2.0, 0.0),
                    egui::vec2(7.0, item_height),
                );
                ui.painter()
                    .rect_filled(glow_rect, 4.0, accent_color.gamma_multiply(0.4));
                // Main indicator - slightly thicker
                let indicator_rect =
                    egui::Rect::from_min_size(item_rect.min, egui::vec2(3.5, item_height));
                ui.painter().rect_filled(indicator_rect, 3.0, accent_color);
            }

            // Icon - use accent color for selected/hover like landing page
            let icon_color = if is_selected || is_hovered {
                accent_color
            } else {
                text_col.gamma_multiply(0.6)
            };
            let icon_pos = egui::pos2(item_rect.left() + 12.0, item_rect.center().y);
            ui.painter().text(
                icon_pos,
                egui::Align2::LEFT_CENTER,
                item.icon,
                typography::proportional(typography::XL),
                icon_color,
            );

            // Kind badge with subtle background pill
            let kind_label = item.kind.label();
            let kind_pos = egui::pos2(item_rect.right() - 12.0, item_rect.center().y);

            // Measure kind text for background pill
            let kind_galley = ui.painter().layout_no_wrap(
                kind_label.to_string(),
                typography::proportional(typography::XS),
                text_tertiary,
            );
            let badge_rect = egui::Rect::from_center_size(
                kind_pos - egui::vec2(kind_galley.size().x / 2.0, 0.0),
                kind_galley.size() + egui::vec2(10.0, 6.0),
            );
            let badge_bg = match self.theme {
                AppTheme::Light => palette::light_bg::ELEVATED,
                AppTheme::Dark => palette::bg::HOVER.gamma_multiply(0.6),
            };
            ui.painter().rect_filled(badge_rect, 4.0, badge_bg);

            ui.painter().text(
                kind_pos,
                egui::Align2::RIGHT_CENTER,
                kind_label,
                typography::proportional(typography::XS),
                text_tertiary,
            );

            // Label (truncate if too long to avoid overlapping kind badge)
            let label_start = item_rect.left() + 34.0;
            let label_pos = egui::pos2(label_start, item_rect.center().y);

            // Truncate label if needed
            let display_label = if item.label.len() > 50 {
                format!("{}…", &item.label[..49])
            } else {
                item.label.clone()
            };

            // Brighter text for selected item
            let label_color = if is_selected {
                palette::text::PRIMARY
            } else {
                text_col
            };

            ui.painter().text(
                label_pos,
                egui::Align2::LEFT_CENTER,
                &display_label,
                typography::monospace(typography::LG),
                label_color,
            );

            // Handle click
            if response.clicked() {
                result = CompletionResult::Selected(item.clone());
            }

            y += item_height;
        }

        // Show scroll indicator if there are more items
        if self.items.len() > max_visible_items {
            let more_count = self.items.len() - max_visible_items;

            // Draw indicator with subtle background
            let indicator_text = format!("↓ +{more_count} more");
            let indicator_pos = egui::pos2(content_rect.center().x, content_rect.bottom() - 2.0);

            let indicator_galley = ui.painter().layout_no_wrap(
                indicator_text.clone(),
                typography::proportional(typography::XS),
                text_tertiary,
            );
            let indicator_bg_rect = egui::Rect::from_center_size(
                indicator_pos - egui::vec2(0.0, indicator_galley.size().y / 2.0),
                indicator_galley.size() + egui::vec2(12.0, 4.0),
            );
            ui.painter()
                .rect_filled(indicator_bg_rect, 4.0, bg_color.gamma_multiply(0.9));

            ui.painter().text(
                indicator_pos,
                egui::Align2::CENTER_BOTTOM,
                indicator_text,
                typography::proportional(typography::XS),
                text_tertiary,
            );
        }

        result
    }

    /// Handle keyboard input for the completion popup using the context.
    /// Uses key_pressed for navigation (arrow keys) but consume_key for
    /// action keys (Enter/Tab/Escape) to prevent them from reaching TextEdit.
    /// Returns the result of keyboard interaction.
    pub fn handle_keyboard_ctx(&mut self, ctx: &egui::Context) -> Option<CompletionResult> {
        if !self.is_open {
            return None;
        }

        // Use key_pressed for navigation - let TextEdit also see arrow keys
        // (cursor might move but that's acceptable)
        let (up_pressed, down_pressed, ctrl_p, ctrl_n) = ctx.input(|input| {
            (
                input.key_pressed(Key::ArrowUp),
                input.key_pressed(Key::ArrowDown),
                input.key_pressed(Key::P) && input.modifiers.ctrl,
                input.key_pressed(Key::N) && input.modifiers.ctrl,
            )
        });

        // Handle navigation
        if up_pressed || ctrl_p {
            self.select_prev();
        }
        if down_pressed || ctrl_n {
            self.select_next();
        }

        // Use consume_key for action keys to prevent TextEdit from seeing them
        // This prevents Enter from inserting a newline when selecting a completion
        let tab_pressed = ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Tab));
        let enter_pressed =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Enter));
        let escape_pressed =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape));

        // Check for selection keys
        if tab_pressed || enter_pressed {
            if let Some(item) = self.selected_item() {
                return Some(CompletionResult::Selected(item.clone()));
            }
        }

        // Check for dismiss
        if escape_pressed {
            return Some(CompletionResult::Dismissed);
        }

        None
    }
}

/// Find the start of a word for PromQL completion (partial replacement)
fn find_word_start(input: &str, cursor: usize, partial: &str) -> usize {
    let before = &input[..cursor];
    before
        .rfind(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '(' | ')'
                        | '{'
                        | '}'
                        | '['
                        | ']'
                        | ','
                        | '='
                        | '!'
                        | '~'
                        | '<'
                        | '>'
                        | '+'
                        | '-'
                        | '*'
                        | '/'
                        | '%'
                        | '^'
                        | '@'
                        | ':'
                )
        })
        .map(|i| i + 1)
        .unwrap_or(cursor.saturating_sub(partial.len()))
}

/// Collect unique tag keys from a list of tag strings (format: "key:value")
pub fn extract_tag_keys(tags: &[String]) -> Vec<String> {
    let mut keys: Vec<String> = tags
        .iter()
        .filter_map(|t| t.split(':').next())
        .map(String::from)
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Collect unique values for a specific tag key from a list of tag strings
pub fn extract_tag_values(tags: &[String], key: &str) -> Vec<String> {
    let prefix = format!("{key}:");
    let mut values: Vec<String> = tags
        .iter()
        .filter(|t| t.starts_with(&prefix))
        .filter_map(|t| t.strip_prefix(&prefix))
        .map(String::from)
        .collect();
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tag_keys() {
        let tags = vec![
            "env:prod".to_string(),
            "env:staging".to_string(),
            "service:api".to_string(),
            "service:db".to_string(),
        ];
        let keys = extract_tag_keys(&tags);
        assert_eq!(keys, vec!["env", "service"]);
    }

    #[test]
    fn test_extract_tag_values() {
        let tags = vec![
            "env:prod".to_string(),
            "env:staging".to_string(),
            "env:dev".to_string(),
            "service:api".to_string(),
        ];
        let values = extract_tag_values(&tags, "env");
        assert_eq!(values, vec!["dev", "prod", "staging"]);
    }

    // Tests for PromQL mode
    #[test]
    fn test_completion_update_promql_empty() {
        let mut completion = QueryCompletion::new();
        completion.update("", 0);
        assert!(completion.is_open());

        // Should have functions/aggregations
        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"sum"));
        assert!(labels.contains(&"rate"));
    }

    #[test]
    fn test_completion_update_promql_typing_function() {
        let mut completion = QueryCompletion::new();
        completion.update("rat", 3);
        assert!(completion.is_open());

        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"rate"));
    }

    #[test]
    fn test_completion_update_promql_selector() {
        let mut completion = QueryCompletion::new();
        completion.set_tag_keys(vec!["method".to_string(), "status".to_string()]);
        completion.update("http_requests{", 14);
        assert!(completion.is_open());

        // Should have label names
        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"method"));
        assert!(labels.contains(&"status"));
    }

    #[test]
    fn test_completion_update_promql_label_value() {
        let mut completion = QueryCompletion::new();
        completion.set_tag_keys(vec!["method".to_string()]);
        completion.set_tag_values("method", vec!["GET".to_string(), "POST".to_string()]);
        completion.update("http_requests{method=\"", 22);
        assert!(completion.is_open());

        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"GET"));
        assert!(labels.contains(&"POST"));
    }

    #[test]
    fn test_completion_update_promql_metric_after_function() {
        let mut completion = QueryCompletion::new();
        completion.set_metric_names(vec!["node_cpu_seconds_total".to_string()]);
        completion.update("rate(", 5);
        assert!(completion.is_open());

        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        // Should suggest metrics in ExpectExpr context
        assert!(
            labels.contains(&"node_cpu_seconds_total"),
            "Expected node_cpu_seconds_total in {labels:?}"
        );
    }
}
