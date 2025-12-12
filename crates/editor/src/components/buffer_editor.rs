use egui::text::LayoutJob;
use egui::{Color32, FontId, Key, RichText, TextFormat};

use crate::components::diagnostics_pane::{Diagnostic, DiagnosticLevel};
use crate::components::query_completion::{CompletionResult, QueryCompletion};
use crate::components::query_state::QueryState;
use crate::components::query_validation::{QueryValidator, ValidationResult};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

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
        match theme {
            AppTheme::Light => Self {
                keyword: Color32::from_rgb(166, 38, 164), // purple - keywords
                tag_key: Color32::from_rgb(0, 92, 197),   // blue - keys
                colon: palette::light_text::TERTIARY,
                tag_value: palette::accent::LIGHT, // emerald - values
                wildcard: Color32::from_rgb(200, 120, 0), // orange
                paren: palette::light_text::TERTIARY,
                negation: palette::semantic::ERROR,
                default: palette::light_text::PRIMARY,
            },
            AppTheme::Dark => Self {
                keyword: palette::syntax::KEYWORD,
                tag_key: palette::syntax::KEY,
                colon: palette::syntax::PUNCTUATION,
                tag_value: palette::syntax::VALUE,
                wildcard: palette::syntax::SPECIAL,
                paren: palette::syntax::PUNCTUATION,
                negation: palette::syntax::NEGATION,
                default: palette::text::PRIMARY,
            },
        }
    }
}

/// Create a syntax-highlighted layout for query text
fn highlight_query_detailed(text: &str, theme: AppTheme, font_id: FontId) -> LayoutJob {
    let colors = QuerySyntaxColors::for_theme(theme);
    let mut job = LayoutJob::default();

    let mut i = 0;

    while i < text.len() {
        let c = text[i..].chars().next().unwrap();
        let c_len = c.len_utf8();

        match c {
            '(' | ')' => {
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
            '!' => {
                job.append(
                    "!",
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: colors.negation,
                        ..Default::default()
                    },
                );
                i += c_len;
            }
            ' ' | '\t' | '\n' => {
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
            _ => {
                // Read a word (until whitespace or special char)
                let word_start = i;
                while i < text.len() {
                    let next_c = text[i..].chars().next().unwrap();
                    if matches!(next_c, ' ' | '\t' | '\n' | '(' | ')' | '!') {
                        break;
                    }
                    i += next_c.len_utf8();
                }
                let word = &text[word_start..i];

                // Classify and color the word
                let upper = word.to_uppercase();
                if matches!(upper.as_str(), "AND" | "OR" | "NOT") {
                    // Keyword
                    job.append(
                        word,
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            color: colors.keyword,
                            ..Default::default()
                        },
                    );
                } else if word == "*" {
                    // Standalone wildcard
                    job.append(
                        word,
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            color: colors.wildcard,
                            ..Default::default()
                        },
                    );
                } else if let Some(colon_pos) = word.find(':') {
                    // Tag expression: key:value or key:value*
                    let key = &word[..colon_pos];
                    let colon = &word[colon_pos..colon_pos + 1];
                    let value = &word[colon_pos + 1..];

                    // Key
                    job.append(
                        key,
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            color: colors.tag_key,
                            ..Default::default()
                        },
                    );
                    // Colon
                    job.append(
                        colon,
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            color: colors.colon,
                            ..Default::default()
                        },
                    );
                    // Value (check for wildcard suffix)
                    if let Some(value_part) = value.strip_suffix('*') {
                        job.append(
                            value_part,
                            0.0,
                            TextFormat {
                                font_id: font_id.clone(),
                                color: colors.tag_value,
                                ..Default::default()
                            },
                        );
                        job.append(
                            "*",
                            0.0,
                            TextFormat {
                                font_id: font_id.clone(),
                                color: colors.wildcard,
                                ..Default::default()
                            },
                        );
                    } else {
                        job.append(
                            value,
                            0.0,
                            TextFormat {
                                font_id: font_id.clone(),
                                color: colors.tag_value,
                                ..Default::default()
                            },
                        );
                    }
                } else {
                    // Unknown/default
                    job.append(
                        word,
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            color: colors.default,
                            ..Default::default()
                        },
                    );
                }
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
    /// Query validator for inline validation
    validator: QueryValidator,
    /// Cached validation result
    validation_result: Option<ValidationResult>,
    /// Whether inline diagnostics are shown
    show_inline_diagnostics: bool,
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
            validator: QueryValidator::new(),
            validation_result: None,
            show_inline_diagnostics: true,
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
    }

    /// Validate the current query and cache the result
    fn validate_query(&mut self) {
        self.validation_result = Some(self.validator.validate(&self.query));
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
        #[allow(deprecated)]
        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new("buffer_editor_backdrop"))
            .fixed_pos(screen_rect.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let backdrop_color = match self.theme {
                    AppTheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 120),
                    AppTheme::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                };
                ui.painter().rect_filled(screen_rect, 0.0, backdrop_color);
            });

        // Main editor popup
        let popup_width = (screen_rect.width() * 0.7).clamp(500.0, 900.0);

        egui::Area::new(egui::Id::new("buffer_editor_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let bg_color = match self.theme {
                    AppTheme::Light => palette::light_bg::SURFACE,
                    AppTheme::Dark => palette::bg::SURFACE,
                };
                // Subtle border matching command palette style
                let border_color = match self.theme {
                    AppTheme::Light => palette::light_border::DEFAULT,
                    AppTheme::Dark => palette::border::SUBTLE,
                };
                // Muted accent for badges/buttons (darker, less saturated for dark mode)
                let accent_color = match self.theme {
                    AppTheme::Light => palette::accent::LIGHT,
                    AppTheme::Dark => Color32::from_rgb(13, 148, 103), // Darker emerald (~0.8x PRIMARY)
                };

                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .corner_radius(8.0)
                    .inner_margin(0.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header with mode indicator and buffer name
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);

                            // INSERT mode badge (muted emerald)
                            egui::Frame::new()
                                .fill(accent_color)
                                .corner_radius(3.0)
                                .inner_margin(egui::vec2(8.0, 3.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("INSERT")
                                            .color(Color32::WHITE)
                                            .size(11.0)
                                            .strong(),
                                    );
                                });

                            ui.add_space(12.0);

                            // Buffer name
                            ui.label(
                                RichText::new(&self.buffer_name)
                                    .color(text_color(self.theme))
                                    .size(14.0),
                            );

                            // Modified indicator
                            if self.is_modified() {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("[+]")
                                        .color(palette::semantic::WARNING)
                                        .size(12.0),
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
                                                .size(12.0),
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
                                                .size(11.0),
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
                                        .size(11.0),
                                    )
                                    .on_hover_text(format!("{error_count} error(s)"));
                                }
                            }

                            // Spacer and close hint
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(16.0);
                                    ui.label(
                                        RichText::new("Esc to cancel")
                                            .color(text_color(self.theme).gamma_multiply(0.4))
                                            .size(11.0),
                                    );
                                },
                            );
                        });

                        ui.add_space(12.0);

                        // Separator
                        let separator_color = match self.theme {
                            AppTheme::Light => palette::light_border::SUBTLE,
                            AppTheme::Dark => palette::border::SUBTLE,
                        };
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
                                    .size(14.0),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Query")
                                    .color(text_color(self.theme).gamma_multiply(0.7))
                                    .size(12.0),
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

                            // Editor background color based on theme
                            let editor_bg = match self.theme {
                                AppTheme::Light => palette::light_bg::ELEVATED,
                                AppTheme::Dark => palette::bg::ELEVATED,
                            };

                            // Create layouter closure for syntax highlighting
                            let theme = self.theme;
                            let mut layouter =
                                move |ui: &egui::Ui,
                                      text: &dyn egui::TextBuffer,
                                      wrap_width: f32| {
                                    let text_str = text.as_str();
                                    let mut job = highlight_query_detailed(
                                        text_str,
                                        theme,
                                        FontId::monospace(14.0),
                                    );
                                    job.wrap.max_width = wrap_width;
                                    ui.fonts_mut(|f| f.layout_job(job))
                                };

                            // Use a Frame for the editor background
                            let output = egui::Frame::new()
                                .fill(editor_bg)
                                .corner_radius(4.0)
                                .inner_margin(egui::vec2(8.0, 6.0))
                                .show(ui, |ui| {
                                    egui::TextEdit::multiline(&mut self.query)
                                        .id(text_edit_id)
                                        .font(FontId::monospace(14.0))
                                        .hint_text(
                                            RichText::new(
                                                "Enter query (e.g., env:prod AND service:db)",
                                            )
                                            .color(text_color(self.theme).gamma_multiply(0.4)),
                                        )
                                        .desired_width(editor_width - 16.0)
                                        .desired_rows(4)
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
                            self.validate_query();
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
                                        let target_line =
                                            diag_line.min(num_lines.saturating_sub(1));

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
                                            format!(
                                                " {} {}",
                                                icon,
                                                truncate_message(&diag.message, 50)
                                            )
                                        };

                                        // Position the virtual text with some spacing
                                        let virtual_text_x = text_end_x + 16.0;
                                        let virtual_text_pos =
                                            egui::pos2(virtual_text_x, line_y + 1.0);

                                        // Only paint if within bounds
                                        if virtual_text_x < text_rect.right() - 20.0 {
                                            let painter = ui.painter();
                                            let small_font = FontId::proportional(11.0);

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
                            ui.add_space(16.0);

                            let hint_color = text_color(self.theme).gamma_multiply(0.4);

                            // Keyboard hints
                            ui.label(RichText::new("⌘↵").color(hint_color).size(11.0));
                            ui.label(RichText::new("save").color(hint_color).size(11.0));
                            ui.add_space(16.0);
                            ui.label(RichText::new("esc").color(hint_color).size(11.0));
                            ui.label(RichText::new("cancel").color(hint_color).size(11.0));

                            // Right side - save button
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(16.0);

                                    let save_btn = egui::Button::new(
                                        RichText::new(format!(
                                            "{} Save",
                                            semantic_icons::action::SAVE
                                        ))
                                        .size(12.0),
                                    )
                                    .fill(accent_color);

                                    if ui.add(save_btn).clicked() {
                                        should_save = true;
                                    }

                                    // Cancel button
                                    let cancel_btn = egui::Button::new(
                                        RichText::new("Cancel")
                                            .size(12.0)
                                            .color(text_color(self.theme)),
                                    );

                                    if ui.add(cancel_btn).clicked() {
                                        should_close = true;
                                    }
                                },
                            );
                        });

                        ui.add_space(12.0);
                    });
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
            self.close();
            result = BufferEditorResult::Saved(saved_query, saved_state);
        } else if should_close {
            self.close();
            result = BufferEditorResult::Cancelled;
        }

        result
    }
}
