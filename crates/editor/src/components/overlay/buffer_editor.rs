use egui::text::LayoutJob;
use egui::{Color32, FontId, Key, RichText, TextFormat};

use crate::components::overlay::diagnostics::{Diagnostic, DiagnosticLevel};
use crate::components::util::query_completion::{CompletionResult, QueryCompletion, QueryLanguage};
use crate::components::util::query_state::QueryState;
use crate::components::util::query_validation::{ValidationResult, validate_query};
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop};

/// Truncate a message to a maximum length, adding ellipsis if needed
fn truncate_message(msg: &str, max_len: usize) -> String {
    if msg.chars().count() <= max_len {
        msg.to_string()
    } else {
        let truncated: String = msg.chars().take(max_len.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Syntax highlighting colors for query language
struct QuerySyntaxColors {
    keyword: Color32,   // AND, OR, NOT
    tag_key: Color32,   // env, service, etc.
    colon: Color32,     // :
    tag_value: Color32, // prod, db, etc.
    wildcard: Color32,  // *
    paren: Color32,     // ( )
    negation: Color32,  // !
    default: Color32,   // fallback
}

impl QuerySyntaxColors {
    fn for_theme(theme: AppTheme) -> Self {
        Self {
            keyword: theme.syntax_keyword(),
            tag_key: theme.syntax_key(),
            colon: theme.syntax_punctuation(),
            tag_value: theme.syntax_value(),
            wildcard: theme.syntax_type(), // Use type color for wildcards/special
            paren: theme.syntax_punctuation(),
            negation: theme.semantic_error(),
            default: theme.text_primary(),
        }
    }
}

/// Create a syntax-highlighted layout for PromQL text
fn highlight_promql(text: &str, theme: AppTheme, font_id: FontId) -> LayoutJob {
    let colors = QuerySyntaxColors::for_theme(theme);
    let mut job = LayoutJob::default();

    let mut i = 0;
    let bytes = text.as_bytes();

    while i < text.len() {
        let c = text[i..].chars().next().unwrap();
        let c_len = c.len_utf8();

        match c {
            // Delimiters and operators
            '(' | ')' | '[' | ']' | '{' | '}' => {
                job.append(
                    &text[i..i + c_len],
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: colors.paren,
                        ..Default::default()
                    },
                );
                i += c_len;
            }
            ',' | ';' => {
                job.append(
                    &text[i..i + c_len],
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: colors.colon,
                        ..Default::default()
                    },
                );
                i += c_len;
            }
            // Operators
            '+' | '-' | '*' | '/' | '%' | '^' | '=' | '!' | '<' | '>' | '~' => {
                // Check for multi-char operators: ==, !=, <=, >=, =~, !~
                let op_end = i + c_len;
                let next_char = if op_end < text.len() {
                    text[op_end..].chars().next()
                } else {
                    None
                };
                let op_len = match (c, next_char) {
                    ('=', Some('=')) | ('=', Some('~')) => 2,
                    ('!', Some('=')) | ('!', Some('~')) => 2,
                    ('<', Some('=')) | ('>', Some('=')) => 2,
                    _ => c_len,
                };
                job.append(
                    &text[i..i + op_len],
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: colors.negation, // Use negation color for operators
                        ..Default::default()
                    },
                );
                i += op_len;
            }
            // Strings (label values)
            '"' | '\'' | '`' => {
                let quote = c;
                let start = i;
                i += c_len;
                // Find end of string
                while i < text.len() {
                    let sc = text[i..].chars().next().unwrap();
                    let sc_len = sc.len_utf8();
                    if sc == '\\' && i + sc_len < text.len() {
                        // Skip escaped char
                        i += sc_len;
                        i += text[i..].chars().next().map_or(0, |c| c.len_utf8());
                    } else if sc == quote {
                        i += sc_len;
                        break;
                    } else {
                        i += sc_len;
                    }
                }
                job.append(
                    &text[start..i],
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: colors.tag_value,
                        ..Default::default()
                    },
                );
            }
            // Whitespace
            ' ' | '\t' | '\n' | '\r' => {
                job.append(
                    &text[i..i + c_len],
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: colors.default,
                        ..Default::default()
                    },
                );
                i += c_len;
            }
            // Numbers and durations
            '0'..='9' | '.' => {
                let start = i;
                // Consume digits and decimal point
                while i < text.len() {
                    let nc = bytes.get(i).copied().unwrap_or(0);
                    if nc.is_ascii_digit()
                        || nc == b'.'
                        || nc == b'e'
                        || nc == b'E'
                        || nc == b'+'
                        || nc == b'-'
                    {
                        i += 1;
                    } else {
                        break;
                    }
                }
                // Check for duration suffix (s, m, h, d, w, y, ms)
                let has_duration_suffix = if i < text.len() {
                    let nc = bytes.get(i).copied().unwrap_or(0);
                    if nc == b'm' && bytes.get(i + 1).copied() == Some(b's') {
                        i += 2;
                        true
                    } else if matches!(nc, b's' | b'm' | b'h' | b'd' | b'w' | b'y') {
                        i += 1;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                let color = if has_duration_suffix {
                    colors.wildcard // Use wildcard color for durations
                } else {
                    colors.default
                };
                job.append(
                    &text[start..i],
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color,
                        ..Default::default()
                    },
                );
            }
            // Identifiers (metric names, label names, functions, keywords)
            _ if c.is_alphabetic() || c == '_' || c == ':' => {
                let start = i;
                while i < text.len() {
                    let nc = text[i..].chars().next().unwrap();
                    if nc.is_alphanumeric() || nc == '_' || nc == ':' {
                        i += nc.len_utf8();
                    } else {
                        break;
                    }
                }
                let word = &text[start..i];
                let lower = word.to_lowercase();

                // Classify the word
                let color = if enya_promql::is_callable(&lower) || enya_promql::is_keyword(&lower) {
                    colors.keyword // Functions/aggregations/keywords
                } else {
                    // Check if it looks like a label name (inside {})
                    // by looking back for { without seeing }
                    let before = &text[..start];
                    let in_selector = before
                        .rfind('{')
                        .is_some_and(|brace_pos| before[brace_pos..].rfind('}').is_none());
                    if in_selector {
                        colors.tag_key // Label name
                    } else {
                        colors.default // Metric name or other identifier
                    }
                };

                job.append(
                    word,
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color,
                        ..Default::default()
                    },
                );
            }
            // Anything else
            _ => {
                job.append(
                    &text[i..i + c_len],
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: colors.default,
                        ..Default::default()
                    },
                );
                i += c_len;
            }
        }
    }

    // Ensure job has content for empty strings
    if job.is_empty() {
        job.append(
            "",
            0.0,
            TextFormat {
                font_id,
                color: colors.default,
                ..Default::default()
            },
        );
    }

    job
}

/// Result of the buffer editor modal
#[derive(Debug, Clone)]
pub enum BufferEditorResult {
    /// No action (modal still open or was cancelled)
    None,
    /// Query was saved - contains the query string and query state
    Saved(String, QueryState),
    /// Editor was cancelled (Escape pressed)
    Cancelled,
}

/// A modal overlay for editing buffer queries, styled like the fuzzy finder.
/// Opens as a transparent overlay so the chart remains visible underneath.
/// Includes keybindings for granularity.
pub struct BufferEditor {
    /// Whether the editor is currently open
    is_open: bool,
    /// The query being edited
    query: String,
    /// The original query (for revert on cancel)
    original_query: String,
    /// Name/title of the buffer being edited
    buffer_name: String,
    /// Current theme
    theme: AppTheme,
    /// Whether the text input should request focus
    needs_focus: bool,
    /// Query state (granularity, time range)
    query_state: QueryState,
    /// Original query state (for revert on cancel)
    original_query_state: QueryState,
    /// Query completion popup
    completion: QueryCompletion,
    /// Cursor position in the text edit (byte offset)
    cursor_position: usize,
    /// Last known text edit rect for positioning completion popup
    text_edit_rect: Option<egui::Rect>,
    /// Pending cursor position to set after completion (if Some, will be applied next frame)
    pending_cursor: Option<usize>,
    /// Cached validation result
    validation_result: Option<ValidationResult>,
    /// Whether inline diagnostics are shown
    show_inline_diagnostics: bool,
    /// The original metric name extracted when opening (for label fetching)
    original_metric_name: Option<String>,
}

impl Default for BufferEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferEditor {
    pub fn new() -> Self {
        let mut editor = Self {
            is_open: false,
            query: String::new(),
            original_query: String::new(),
            buffer_name: String::new(),
            theme: AppTheme::default(),
            needs_focus: false,
            query_state: QueryState::default(),
            original_query_state: QueryState::default(),
            completion: QueryCompletion::new(),
            cursor_position: 0,
            text_edit_rect: None,
            pending_cursor: None,
            validation_result: None,
            show_inline_diagnostics: true,
            original_metric_name: None,
        };

        // TODO: Load tag keys and values from metrics store instead of these defaults.
        // The dashboard should call set_tag_keys() and set_tag_values() with actual
        // data from the metrics store when it loads or when new metrics are ingested.
        editor.init_default_completions();

        editor
    }

    /// Initialize default completion suggestions.
    /// This provides a reasonable starting set of tag keys and values.
    fn init_default_completions(&mut self) {
        // Common tag keys used in metrics
        let default_keys = vec![
            "env".to_string(),
            "service".to_string(),
            "region".to_string(),
            "host".to_string(),
            "instance".to_string(),
            "status".to_string(),
            "method".to_string(),
            "endpoint".to_string(),
        ];
        self.completion.set_tag_keys(default_keys);

        // Common values for each key
        self.completion.set_tag_values(
            "env",
            vec![
                "prod".to_string(),
                "staging".to_string(),
                "dev".to_string(),
                "test".to_string(),
            ],
        );
        self.completion.set_tag_values(
            "service",
            vec![
                "api".to_string(),
                "web".to_string(),
                "db".to_string(),
                "cache".to_string(),
                "worker".to_string(),
            ],
        );
        self.completion.set_tag_values(
            "region",
            vec![
                "us-east-1".to_string(),
                "us-west-2".to_string(),
                "eu-west-1".to_string(),
                "ap-southeast-1".to_string(),
            ],
        );
        self.completion.set_tag_values(
            "status",
            vec![
                "2xx".to_string(),
                "4xx".to_string(),
                "5xx".to_string(),
                "ok".to_string(),
                "error".to_string(),
            ],
        );
        self.completion.set_tag_values(
            "method",
            vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "PATCH".to_string(),
            ],
        );
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.completion.set_theme(theme);
    }

    /// Set known tag keys for completion
    pub fn set_tag_keys(&mut self, keys: Vec<String>) {
        self.completion.set_tag_keys(keys);
    }

    /// Set known tag values for a specific key
    pub fn set_tag_values(&mut self, key: &str, values: Vec<String>) {
        self.completion.set_tag_values(key, values);
    }

    /// Set known metric names for completion
    pub fn set_metric_names(&mut self, metrics: Vec<String>) {
        log::debug!(
            "BufferEditor::set_metric_names called with {} metrics, is_open={}",
            metrics.len(),
            self.is_open
        );
        self.completion.set_metric_names(metrics);
        // Refresh completion if editor is open (so newly fetched metrics appear immediately)
        if self.is_open {
            log::debug!(
                "Refreshing completion with query='{}', cursor={}",
                self.query,
                self.cursor_position
            );
            self.completion.update(&self.query, self.cursor_position);
        }
    }

    /// Clear all completions (use before setting new completions from backend)
    pub fn clear_completions(&mut self) {
        self.completion.clear();
    }

    /// Set completions from MetricLabels data fetched from a backend.
    /// This replaces any existing completion data with the labels from the metric.
    pub fn set_completions_from_labels(
        &mut self,
        labels: &rustc_hash::FxHashMap<String, Vec<String>>,
    ) {
        // Clear existing completions
        self.completion.clear();

        // Set tag keys (label names)
        let keys: Vec<String> = labels.keys().cloned().collect();
        self.completion.set_tag_keys(keys);

        // Set tag values for each key
        for (key, values) in labels {
            self.completion.set_tag_values(key, values.clone());
        }
    }

    /// Get a reference to the completion component
    pub fn completion(&self) -> &QueryCompletion {
        &self.completion
    }

    /// Get a mutable reference to the completion component
    pub fn completion_mut(&mut self) -> &mut QueryCompletion {
        &mut self.completion
    }

    /// Check if the editor is currently open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Get the buffer name currently being edited (if open)
    pub fn editing_buffer_name(&self) -> Option<&str> {
        if self.is_open {
            Some(&self.buffer_name)
        } else {
            None
        }
    }

    /// Get the original metric name that was extracted when the editor was opened.
    /// This is used for label fetching - we want to fetch labels for the original metric,
    /// not for partial text as the user types (e.g., "r", "ra", "rat", "rate").
    pub fn editing_metric_name(&self) -> Option<&str> {
        if self.is_open {
            self.original_metric_name.as_deref()
        } else {
            None
        }
    }

    /// Extract a metric name from a query string.
    /// For PromQL, this is the first identifier before any `{`, `(`, or whitespace.
    fn extract_metric_name(query: &str) -> Option<String> {
        if query.is_empty() {
            return None;
        }

        // Find the first "word" in the query - this is typically the metric name
        // Stop at any delimiter like {, (, [, whitespace, or operators
        let metric: String = query
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
            .collect();

        if metric.is_empty() {
            None
        } else {
            Some(metric)
        }
    }

    /// Open the editor with the given query, buffer name, and query state
    pub fn open(&mut self, query: &str, buffer_name: &str) {
        self.open_with_state(query, buffer_name, QueryState::default());
    }

    /// Open the editor with a specific query state
    pub fn open_with_state(&mut self, query: &str, buffer_name: &str, state: QueryState) {
        self.is_open = true;
        self.query = query.to_string();
        self.original_query = query.to_string();
        self.buffer_name = buffer_name.to_string();
        self.needs_focus = true;
        self.query_state = state.clone();
        self.original_query_state = state;
        // Initialize cursor position to end of query so completion context is correct
        self.cursor_position = query.len();
        // Extract and store the original metric name for label fetching
        self.original_metric_name = Self::extract_metric_name(query);
    }

    /// Open the editor for a specific query language (PromQL or LogQL)
    pub fn open_with_language(&mut self, query: &str, buffer_name: &str, language: QueryLanguage) {
        self.completion.set_language(language);
        self.open(query, buffer_name);
    }

    /// Set the query language for completion (PromQL or LogQL)
    pub fn set_language(&mut self, language: QueryLanguage) {
        self.completion.set_language(language);
    }

    /// Get the current query language
    pub fn language(&self) -> QueryLanguage {
        self.completion.language()
    }

    /// Close the editor without saving
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.original_query.clear();
        self.buffer_name.clear();
        self.query_state = QueryState::default();
        self.original_query_state = QueryState::default();
        self.completion.close();
        self.cursor_position = 0;
        self.text_edit_rect = None;
        self.pending_cursor = None;
        self.validation_result = None;
        self.original_metric_name = None;
    }

    /// Validate the current query and cache the result
    fn run_validation(&mut self) {
        self.validation_result = Some(validate_query(&self.query));
    }

    /// Get the current validation result
    pub fn validation_result(&self) -> Option<&ValidationResult> {
        self.validation_result.as_ref()
    }

    /// Get the diagnostics from the last validation
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.validation_result
            .as_ref()
            .map(|r| r.diagnostics.clone())
            .unwrap_or_default()
    }

    /// Check if the query is valid (no errors)
    pub fn is_valid(&self) -> bool {
        self.validation_result
            .as_ref()
            .map(|r| r.is_valid)
            .unwrap_or(true)
    }

    /// Toggle inline diagnostics display
    pub fn toggle_inline_diagnostics(&mut self) {
        self.show_inline_diagnostics = !self.show_inline_diagnostics;
    }

    /// Get the current query content
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Get the current query state
    pub fn query_state(&self) -> &QueryState {
        &self.query_state
    }

    /// Check if the query has been modified
    pub fn is_modified(&self) -> bool {
        self.query != self.original_query || self.query_state != self.original_query_state
    }

    /// Show the buffer editor modal. Returns the result of the interaction.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> BufferEditorResult {
        if !self.is_open {
            return BufferEditorResult::None;
        }

        let mut result = BufferEditorResult::None;
        let mut should_close = false;
        let mut should_save = false;

        // Handle keyboard shortcuts (when completion is not open)
        if !self.completion.is_open() {
            ctx.input(|input| {
                // Escape - cancel and close
                if input.key_pressed(Key::Escape) {
                    should_close = true;
                }
                // Ctrl+Enter or Cmd+Enter - save and close
                if input.key_pressed(Key::Enter) && input.modifiers.command {
                    should_save = true;
                }
            });
        }

        // Semi-transparent backdrop
        draw_backdrop(ctx, self.theme, "buffer_editor");

        // Main editor popup
        #[allow(deprecated)]
        let screen_rect = ctx.screen_rect();
        let popup_width = (screen_rect.width() * 0.7).clamp(500.0, 900.0);

        egui::Area::new(egui::Id::new("buffer_editor_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                // Muted accent for badges/buttons
                let accent_color = self.theme.accent_muted();

                let frame_response = overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header with buffer name and edit icon
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        // Edit icon with subtle accent tint
                        ui.label(
                            RichText::new(semantic_icons::action::EDIT)
                                .color(accent_color)
                                .size(typography::LG),
                        );

                        ui.add_space(10.0);

                        // Buffer name
                        ui.label(
                            RichText::new(&self.buffer_name)
                                .color(text_color(self.theme))
                                .size(typography::XL),
                        );

                        // Modified indicator
                        if self.is_modified() {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("[+]")
                                    .color(palette::semantic::WARNING)
                                    .size(typography::MD),
                            );
                        }

                        // Validation indicator
                        if let Some(ref validation) = self.validation_result {
                            ui.add_space(8.0);
                            if validation.is_valid {
                                if validation.diagnostics.is_empty() {
                                    ui.label(
                                        RichText::new(semantic_icons::status::SUCCESS)
                                            .color(palette::semantic::SUCCESS)
                                            .size(typography::MD),
                                    )
                                    .on_hover_text("Query is valid");
                                } else {
                                    // Valid but has warnings/hints
                                    let warn_count = validation
                                        .diagnostics
                                        .iter()
                                        .filter(|d| d.level == DiagnosticLevel::Warning)
                                        .count();
                                    if warn_count > 0 {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} {}",
                                                semantic_icons::diagnostic::WARNING,
                                                warn_count
                                            ))
                                            .color(palette::semantic::WARNING)
                                            .size(typography::SM),
                                        )
                                        .on_hover_text(format!("{warn_count} warning(s)"));
                                    }
                                }
                            } else {
                                // Has errors
                                let error_count = validation
                                    .diagnostics
                                    .iter()
                                    .filter(|d| d.level == DiagnosticLevel::Error)
                                    .count();
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        semantic_icons::diagnostic::ERROR,
                                        error_count
                                    ))
                                    .color(Color32::from_rgb(220, 60, 60))
                                    .size(typography::SM),
                                )
                                .on_hover_text(format!("{error_count} error(s)"));
                            }
                        }

                        // Spacer and close hint
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("Esc to cancel")
                                    .color(text_color(self.theme).gamma_multiply(0.4))
                                    .size(typography::SM),
                            );
                        });
                    });

                    ui.add_space(12.0);

                    // Separator
                    let separator_color = self.theme.border_subtle();
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );

                    ui.add_space(8.0);

                    // Query input label
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::file::CODE)
                                .color(text_color(self.theme).gamma_multiply(0.6))
                                .size(typography::XL),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Query")
                                .color(text_color(self.theme).gamma_multiply(0.7))
                                .size(typography::MD),
                        );
                    });

                    ui.add_space(8.0);

                    // Query text editor
                    let text_edit_id = egui::Id::new("buffer_editor_query_input");

                    // Apply pending cursor position if set
                    if let Some(cursor_pos) = self.pending_cursor.take() {
                        if let Some(mut state) =
                            egui::text_edit::TextEditState::load(ui.ctx(), text_edit_id)
                        {
                            let ccursor = egui::text::CCursor::new(cursor_pos);
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                            state.store(ui.ctx(), text_edit_id);
                        }
                    }

                    // Track if completion is open for focus management
                    let completion_is_open = self.completion.is_open();

                    // Handle completion keyboard before rendering
                    // Using key_pressed (not consume_key) similar to FuzzyFinder
                    let completion_result = if completion_is_open {
                        self.completion.handle_keyboard_ctx(ui.ctx())
                    } else {
                        None
                    };

                    let text_edit_output = ui.horizontal(|ui| {
                        ui.add_space(16.0);

                        let editor_width = popup_width - 32.0;

                        // Premium editor styling
                        let editor_bg = self.theme.bg_inset();
                        let editor_border = self.theme.border_subtle();

                        // Create layouter closure for syntax highlighting
                        // Use larger font for better readability
                        let theme = self.theme;
                        let editor_font = typography::code_lg();
                        let mut layouter =
                            move |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                                let text_str = text.as_str();
                                let mut job =
                                    highlight_promql(text_str, theme, editor_font.clone());
                                job.wrap.max_width = wrap_width;
                                ui.fonts_mut(|f| f.layout_job(job))
                            };

                        // Premium editor frame with subtle inset shadow effect
                        let output = egui::Frame::new()
                            .fill(editor_bg)
                            .corner_radius(8.0) // More rounded
                            .inner_margin(egui::vec2(14.0, 12.0))
                            .stroke(egui::Stroke::new(1.0, editor_border))
                            .show(ui, |ui| {
                                // Draw subtle inner shadow at top for inset effect
                                let inner_rect = ui.available_rect_before_wrap();
                                let inset_shadow = egui::Rect::from_min_size(
                                    inner_rect.left_top(),
                                    egui::vec2(inner_rect.width(), 2.0),
                                );
                                ui.painter().rect_filled(
                                    inset_shadow,
                                    0.0,
                                    Color32::from_rgba_unmultiplied(0, 0, 0, 15),
                                );

                                // Language-aware hint text
                                let hint = match self.language() {
                                    QueryLanguage::PromQL => "e.g., rate(http_requests_total[5m])",
                                    QueryLanguage::LogQL => {
                                        r#"e.g., {app="nginx"} |= "error" | json"#
                                    }
                                };

                                egui::TextEdit::multiline(&mut self.query)
                                    .id(text_edit_id)
                                    .font(typography::code_lg())
                                    .hint_text(
                                        RichText::new(hint)
                                            .font(typography::code_lg())
                                            .color(text_color(self.theme).gamma_multiply(0.35)),
                                    )
                                    .desired_width(editor_width - 28.0)
                                    .desired_rows(6)
                                    .frame(false) // Remove default frame
                                    .layouter(&mut layouter)
                                    .lock_focus(true)
                                    .show(ui)
                            })
                            .inner;

                        // Request focus on first show
                        if self.needs_focus {
                            output.response.request_focus();
                            self.needs_focus = false;
                        }

                        ui.add_space(16.0);

                        output
                    });

                    // Update cursor position from text edit state
                    let output = text_edit_output.inner;
                    if let Some(cursor_range) = output.cursor_range {
                        // Use the primary cursor position (character index)
                        self.cursor_position = cursor_range.primary.index;
                    }

                    // Store text edit rect for completion popup positioning
                    self.text_edit_rect = Some(output.response.rect);

                    // Handle completion result after UI rendering
                    match completion_result {
                        Some(CompletionResult::Selected(_)) => {
                            if let Some((new_query, new_cursor)) = self
                                .completion
                                .apply_completion(&self.query, self.cursor_position)
                            {
                                self.query = new_query;
                                self.cursor_position = new_cursor;
                                self.pending_cursor = Some(new_cursor);
                            }
                            self.completion.close();
                        }
                        Some(CompletionResult::Dismissed) => {
                            self.completion.close();
                        }
                        Some(CompletionResult::None) | None => {}
                    }

                    // Update completion when text changes or cursor moves
                    if output.response.changed() || output.response.has_focus() {
                        self.completion.update(&self.query, self.cursor_position);
                    }

                    // Validate query when text changes
                    if output.response.changed() {
                        self.run_validation();
                    }

                    // Show tiny-inline-diagnostic style virtual text
                    if self.show_inline_diagnostics {
                        if let Some(ref validation) = self.validation_result {
                            if !validation.diagnostics.is_empty() {
                                // Get the text edit rect and calculate line metrics
                                let text_rect = output.response.rect;
                                // Use approximate metrics for monospace 14pt font
                                let line_height = 18.0; // ~14pt + line spacing
                                let char_width = 8.4; // ~0.6 * font size for monospace

                                // Calculate the end of the query text on each line
                                let lines: Vec<&str> = self.query.lines().collect();
                                let num_lines = lines.len().max(1);

                                // Padding inside the text edit
                                let inner_margin = 4.0;

                                // Get the first diagnostic to show inline
                                // (tiny-inline-diagnostic typically shows one per line)
                                if let Some(diag) = validation.diagnostics.first() {
                                    let (icon, color, bg_color) = match diag.level {
                                        DiagnosticLevel::Error => (
                                            semantic_icons::diagnostic::ERROR,
                                            palette::semantic::ERROR,
                                            palette::semantic::ERROR.gamma_multiply(0.15),
                                        ),
                                        DiagnosticLevel::Warning => (
                                            semantic_icons::diagnostic::WARNING,
                                            palette::semantic::WARNING,
                                            palette::semantic::WARNING.gamma_multiply(0.15),
                                        ),
                                        DiagnosticLevel::Info => (
                                            semantic_icons::diagnostic::INFO,
                                            palette::semantic::INFO,
                                            palette::semantic::INFO.gamma_multiply(0.15),
                                        ),
                                        DiagnosticLevel::Hint => (
                                            semantic_icons::diagnostic::HINT,
                                            palette::semantic::SUCCESS,
                                            palette::semantic::SUCCESS.gamma_multiply(0.15),
                                        ),
                                    };

                                    // Determine which line to show the diagnostic on
                                    let diag_line = diag.line.unwrap_or(1).saturating_sub(1);
                                    let target_line = diag_line.min(num_lines.saturating_sub(1));

                                    // Calculate the x position after the line text
                                    let line_text = lines.get(target_line).unwrap_or(&"");
                                    let text_end_x = text_rect.left()
                                        + inner_margin
                                        + (line_text.chars().count() as f32 * char_width);

                                    // Calculate y position for this line
                                    let line_y = text_rect.top()
                                        + inner_margin
                                        + (target_line as f32 * line_height);

                                    // Build the diagnostic text with count indicator
                                    let diag_count = validation.diagnostics.len();
                                    let diag_text = if diag_count > 1 {
                                        format!(
                                            " {} {} (+{} more)",
                                            icon,
                                            truncate_message(&diag.message, 40),
                                            diag_count - 1
                                        )
                                    } else {
                                        format!(" {} {}", icon, truncate_message(&diag.message, 50))
                                    };

                                    // Position the virtual text with some spacing
                                    let virtual_text_x = text_end_x + 16.0;
                                    let virtual_text_pos = egui::pos2(virtual_text_x, line_y + 1.0);

                                    // Only paint if within bounds
                                    if virtual_text_x < text_rect.right() - 20.0 {
                                        let painter = ui.painter();
                                        let small_font = typography::body();

                                        // Measure text for background
                                        let galley = painter.layout_no_wrap(
                                            diag_text.clone(),
                                            small_font.clone(),
                                            color,
                                        );

                                        // Draw subtle background pill
                                        let bg_rect = egui::Rect::from_min_size(
                                            virtual_text_pos - egui::vec2(4.0, 2.0),
                                            galley.size() + egui::vec2(8.0, 4.0),
                                        );
                                        painter.rect_filled(bg_rect, 4.0, bg_color);

                                        // Draw the text
                                        painter.text(
                                            virtual_text_pos,
                                            egui::Align2::LEFT_TOP,
                                            diag_text,
                                            small_font,
                                            color,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );

                    ui.add_space(8.0);

                    // Footer with keyboard hints and save button
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        let hint_color = text_color(self.theme).gamma_multiply(0.35);
                        let key_bg = self.theme.bg_elevated();

                        // Premium keyboard hints with key badges
                        crate::components::util::finder_utils::render_key_badge(
                            ui, "⌘↵", key_bg, hint_color,
                        );
                        ui.add_space(4.0);
                        ui.label(RichText::new("save").color(hint_color).size(typography::SM));
                        ui.add_space(16.0);
                        crate::components::util::finder_utils::render_key_badge(
                            ui, "Esc", key_bg, hint_color,
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("cancel")
                                .color(hint_color)
                                .size(typography::SM),
                        );

                        // Right side - save and cancel buttons
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(20.0);

                            // Premium save button with hover glow
                            let save_btn = egui::Button::new(
                                RichText::new(format!("{} Save", semantic_icons::action::SAVE))
                                    .size(typography::MD)
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(accent_color)
                            .corner_radius(6.0)
                            .min_size(egui::vec2(80.0, 32.0));

                            let save_response = ui.add(save_btn);

                            // Draw glow behind save button on hover
                            if save_response.hovered() {
                                let glow_rect = save_response.rect.expand(3.0);
                                ui.painter().rect_filled(
                                    glow_rect,
                                    8.0,
                                    accent_color.gamma_multiply(0.25),
                                );
                                // Redraw button fill on top
                                ui.painter().rect_filled(
                                    save_response.rect,
                                    6.0,
                                    palette::accent::HOVER,
                                );
                            }

                            if save_response.clicked() {
                                should_save = true;
                            }

                            ui.add_space(10.0);

                            // Cancel button with refined ghost styling
                            let cancel_bg = self.theme.bg_elevated();
                            let cancel_border = self.theme.border_subtle();
                            let cancel_btn = egui::Button::new(
                                RichText::new("Cancel")
                                    .size(typography::MD)
                                    .color(text_color(self.theme).gamma_multiply(0.7)),
                            )
                            .fill(cancel_bg)
                            .stroke(egui::Stroke::new(1.0, cancel_border))
                            .corner_radius(6.0)
                            .min_size(egui::vec2(72.0, 32.0));

                            let cancel_response = ui.add(cancel_btn);
                            if cancel_response.clicked() {
                                should_close = true;
                            }
                        });
                    });

                    ui.add_space(14.0);
                });

                // Draw inner highlight on the frame for glass effect
                overlay_style.draw_inner_highlight(ui, frame_response.response.rect);
            });

        // Show completion popup if open (rendered in a separate area on top)
        if self.completion.is_open() {
            if let Some(text_rect) = self.text_edit_rect {
                egui::Area::new(egui::Id::new("buffer_editor_completion"))
                    .fixed_pos(egui::pos2(text_rect.left(), text_rect.bottom() + 4.0))
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        match self.completion.show(ui, text_rect) {
                            CompletionResult::Selected(_) => {
                                // Apply the selected completion
                                if let Some((new_query, new_cursor)) = self
                                    .completion
                                    .apply_completion(&self.query, self.cursor_position)
                                {
                                    self.query = new_query;
                                    self.cursor_position = new_cursor;
                                    // Set pending cursor to move the TextEdit cursor next frame
                                    self.pending_cursor = Some(new_cursor);
                                }
                                self.completion.close();
                            }
                            CompletionResult::Dismissed => {
                                self.completion.close();
                            }
                            CompletionResult::None => {}
                        }
                    });
            }
        }

        // Handle save/close
        if should_save {
            let saved_query = self.query.clone();
            let saved_state = self.query_state.clone();
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
            result = BufferEditorResult::Saved(saved_query, saved_state);
        } else if should_close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
            result = BufferEditorResult::Cancelled;
        }

        result
    }
}
