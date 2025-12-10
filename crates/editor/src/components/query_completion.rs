//! Inline completion popup for enya-lang queries.
//!
//! Provides context-aware suggestions while typing filter queries.

use egui::{Color32, FontId, Key};
use enya_lang::completion::{Context, analyze, syntax_suggestions};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;

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
    /// Keyword like AND, OR
    Keyword,
    /// Operator like !, (, )
    Operator,
    /// Tag key like "env", "service"
    TagKey,
    /// Tag value like "prod", "staging"
    TagValue,
}

impl CompletionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Operator => "operator",
            Self::TagKey => "key",
            Self::TagValue => "value",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Keyword => semantic_icons::completion::KEYWORD,
            Self::Operator => semantic_icons::completion::OPERATOR,
            Self::TagKey => semantic_icons::completion::TAG_KEY,
            Self::TagValue => semantic_icons::completion::TAG_VALUE,
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
    /// Known tag keys (from metrics store)
    known_tag_keys: Vec<String>,
    /// Known tag values per key
    known_tag_values: std::collections::HashMap<String, Vec<String>>,
    /// Current completion context
    current_context: Option<Context>,
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
            known_tag_values: std::collections::HashMap::new(),
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
    pub fn update(&mut self, input: &str, cursor: usize) {
        let ctx = analyze(input, cursor);
        self.current_context = Some(ctx.clone());
        self.items.clear();
        self.selected_index = 0;

        match &ctx {
            Context::ExpectExpr | Context::ExpectOperator => {
                // Add syntax suggestions
                let suggestions = syntax_suggestions(&ctx);
                for s in suggestions {
                    let kind = if matches!(s, "AND" | "OR") {
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

                // For ExpectExpr, also suggest known tag keys
                if matches!(ctx, Context::ExpectExpr) {
                    for key in &self.known_tag_keys {
                        self.items.push(CompletionItem {
                            text: format!("{key}:"),
                            label: key.clone(),
                            icon: CompletionKind::TagKey.icon(),
                            kind: CompletionKind::TagKey,
                        });
                    }
                }
            }
            Context::InTagKey(partial) => {
                // Filter tag keys by partial match
                let partial_lower = partial.to_lowercase();
                for key in &self.known_tag_keys {
                    if key.to_lowercase().starts_with(&partial_lower) {
                        self.items.push(CompletionItem {
                            text: format!("{key}:"),
                            label: key.clone(),
                            icon: CompletionKind::TagKey.icon(),
                            kind: CompletionKind::TagKey,
                        });
                    }
                }
            }
            Context::InTagValue { key, partial_value } => {
                // Filter tag values by partial match
                if let Some(values) = self.known_tag_values.get(key) {
                    let partial_lower = partial_value.to_lowercase();
                    for value in values {
                        if partial_value.is_empty()
                            || value.to_lowercase().starts_with(&partial_lower)
                        {
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
        }

        // Show popup if we have items
        self.is_open = !self.items.is_empty();
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

        let (new_input, new_cursor) = match ctx {
            Context::ExpectExpr | Context::ExpectOperator => {
                // Insert at cursor, possibly with space before
                let before = &input[..cursor];
                let after = &input[cursor..];

                let needs_space = !before.is_empty()
                    && !before.ends_with(char::is_whitespace)
                    && !before.ends_with('(');
                let prefix = if needs_space { " " } else { "" };

                // Add trailing space after keywords (AND, OR) and operators (!, ()
                // but not after tag keys (which end with :)
                let needs_suffix = matches!(
                    item.kind,
                    CompletionKind::Keyword | CompletionKind::Operator
                ) && !item.text.ends_with(':')
                    && !item.text.ends_with(')')
                    && !after.starts_with(char::is_whitespace);
                let suffix = if needs_suffix { " " } else { "" };

                let new_input = format!("{before}{prefix}{}{suffix}{after}", item.text);
                let new_cursor = cursor + prefix.len() + item.text.len() + suffix.len();
                (new_input, new_cursor)
            }
            Context::InTagKey(partial) => {
                // Replace the partial key with the full key
                let trimmed_before = input[..cursor].trim_start();
                let word_start = trimmed_before
                    .rfind(|c: char| c.is_whitespace() || c == '(' || c == ')')
                    .map_or(0, |i| {
                        // Find the actual byte position in the original input
                        let prefix_len = input.len() - input.trim_start().len();
                        prefix_len + i + 1
                    });

                // Handle case where we're at the start
                let actual_start = if word_start == 0 && !input.starts_with(partial) {
                    input[..cursor]
                        .rfind(|c: char| c.is_whitespace() || c == '(' || c == ')')
                        .map_or(0, |i| i + 1)
                } else {
                    word_start
                };

                let before = &input[..actual_start];
                let after = &input[cursor..];
                let new_input = format!("{before}{}{after}", item.text);
                let new_cursor = actual_start + item.text.len();
                (new_input, new_cursor)
            }
            Context::InTagValue { key, .. } => {
                // Find where the value starts (after the colon)
                let before_cursor = &input[..cursor];
                if let Some(colon_pos) = before_cursor.rfind(':') {
                    let before_colon = &input[..=colon_pos];
                    let after = &input[cursor..];
                    let new_input = format!("{before_colon}{}{after}", item.text);
                    let new_cursor = colon_pos + 1 + item.text.len();
                    (new_input, new_cursor)
                } else {
                    // Fallback: just insert the value at cursor
                    let new_input = format!(
                        "{}{}:{}{}",
                        &input[..cursor],
                        key,
                        item.text,
                        &input[cursor..]
                    );
                    let new_cursor = cursor + key.len() + 1 + item.text.len();
                    (new_input, new_cursor)
                }
            }
        };

        Some((new_input, new_cursor))
    }

    /// Show the completion popup near the text input.
    /// Returns the result of user interaction.
    pub fn show(&mut self, ui: &mut egui::Ui, text_edit_rect: egui::Rect) -> CompletionResult {
        if !self.is_open || self.items.is_empty() {
            return CompletionResult::None;
        }

        let mut result = CompletionResult::None;

        // Position popup below the text input
        let popup_pos = egui::pos2(text_edit_rect.left(), text_edit_rect.bottom() + 4.0);
        let popup_width = text_edit_rect.width().min(400.0);
        let item_height = 28.0;
        let max_visible_items = 8;
        let visible_items = self.items.len().min(max_visible_items);
        let popup_height = visible_items as f32 * item_height + 8.0;

        let bg_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(252, 252, 252),
            AppTheme::Dark => Color32::from_rgb(35, 35, 40),
        };
        let border_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(200, 200, 200),
            AppTheme::Dark => Color32::from_rgb(60, 60, 70),
        };
        let selected_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(230, 240, 255),
            AppTheme::Dark => Color32::from_rgb(50, 55, 75),
        };
        let text_col = text_color(self.theme);

        let popup_rect =
            egui::Rect::from_min_size(popup_pos, egui::vec2(popup_width, popup_height));

        // Draw shadow first
        let shadow_rect = popup_rect.translate(egui::vec2(2.0, 2.0));
        ui.painter()
            .rect_filled(shadow_rect, 4.0, Color32::from_black_alpha(30));

        // Draw popup background on top of shadow
        ui.painter().rect(
            popup_rect,
            4.0,
            bg_color,
            egui::Stroke::new(1.0, border_color),
            egui::StrokeKind::Inside,
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

            // Item background
            if is_selected {
                ui.painter().rect_filled(item_rect, 2.0, selected_bg);
            }

            // Icon
            let icon_pos = egui::pos2(item_rect.left() + 8.0, item_rect.center().y - 7.0);
            ui.painter().text(
                icon_pos,
                egui::Align2::LEFT_CENTER,
                item.icon,
                FontId::proportional(14.0),
                text_col.gamma_multiply(0.6),
            );

            // Label
            let label_pos = egui::pos2(item_rect.left() + 30.0, item_rect.center().y);
            ui.painter().text(
                label_pos,
                egui::Align2::LEFT_CENTER,
                &item.label,
                FontId::monospace(13.0),
                text_col,
            );

            // Kind badge
            let kind_label = item.kind.label();
            let kind_pos = egui::pos2(item_rect.right() - 8.0, item_rect.center().y);
            ui.painter().text(
                kind_pos,
                egui::Align2::RIGHT_CENTER,
                kind_label,
                FontId::proportional(10.0),
                text_col.gamma_multiply(0.4),
            );

            // Handle click
            let response = ui.allocate_rect(item_rect, egui::Sense::click());
            if response.clicked() {
                result = CompletionResult::Selected(item.clone());
            }

            y += item_height;
        }

        // Show scroll indicator if there are more items
        if self.items.len() > max_visible_items {
            let more_count = self.items.len() - max_visible_items;
            let indicator_pos = egui::pos2(content_rect.center().x, content_rect.bottom() - 4.0);
            ui.painter().text(
                indicator_pos,
                egui::Align2::CENTER_BOTTOM,
                format!("... +{more_count} more"),
                FontId::proportional(10.0),
                text_col.gamma_multiply(0.4),
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

    #[test]
    fn test_completion_update_expect_expr() {
        let mut completion = QueryCompletion::new();
        completion.set_tag_keys(vec!["env".to_string(), "service".to_string()]);

        completion.update("", 0);
        assert!(completion.is_open());
        assert!(!completion.items.is_empty());

        // Should have operators and tag keys
        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"!"));
        assert!(labels.contains(&"("));
        assert!(labels.contains(&"env"));
        assert!(labels.contains(&"service"));
    }

    #[test]
    fn test_completion_update_expect_operator() {
        let mut completion = QueryCompletion::new();
        completion.update("env:prod", 8);
        assert!(completion.is_open());

        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"AND"));
        assert!(labels.contains(&"OR"));
    }

    #[test]
    fn test_completion_update_in_tag_key() {
        let mut completion = QueryCompletion::new();
        completion.set_tag_keys(vec![
            "env".to_string(),
            "environment".to_string(),
            "service".to_string(),
        ]);

        completion.update("en", 2);
        assert!(completion.is_open());

        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"env"));
        assert!(labels.contains(&"environment"));
        assert!(!labels.contains(&"service"));
    }

    #[test]
    fn test_completion_update_in_tag_value() {
        let mut completion = QueryCompletion::new();
        completion.set_tag_keys(vec!["env".to_string()]);
        completion.set_tag_values(
            "env",
            vec!["prod".to_string(), "staging".to_string(), "dev".to_string()],
        );

        completion.update("env:", 4);
        assert!(completion.is_open());

        let labels: Vec<_> = completion.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"prod"));
        assert!(labels.contains(&"staging"));
        assert!(labels.contains(&"dev"));
    }

    #[test]
    fn test_completion_apply_expect_expr() {
        let mut completion = QueryCompletion::new();
        completion.set_tag_keys(vec!["env".to_string()]);
        completion.update("", 0);

        // Select "env:" item
        for (i, item) in completion.items.iter().enumerate() {
            if item.label == "env" {
                completion.selected_index = i;
                break;
            }
        }

        let result = completion.apply_completion("", 0);
        assert!(result.is_some());
        let (new_input, new_cursor) = result.unwrap();
        assert_eq!(new_input, "env:");
        assert_eq!(new_cursor, 4);
    }

    #[test]
    fn test_completion_apply_after_and() {
        let mut completion = QueryCompletion::new();
        completion.set_tag_keys(vec!["service".to_string()]);
        completion.update("env:prod AND ", 13);

        // Select "service:" item
        for (i, item) in completion.items.iter().enumerate() {
            if item.label == "service" {
                completion.selected_index = i;
                break;
            }
        }

        let result = completion.apply_completion("env:prod AND ", 13);
        assert!(result.is_some());
        let (new_input, new_cursor) = result.unwrap();
        assert_eq!(new_input, "env:prod AND service:");
        assert_eq!(new_cursor, 21);
    }
}
