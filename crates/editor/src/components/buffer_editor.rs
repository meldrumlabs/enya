use egui::{Color32, FontId, Key, RichText};

use crate::components::query_completion::{CompletionResult, QueryCompletion};
use crate::components::query_state::QueryState;
use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;

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
/// Includes keybindings for aggregation mode and granularity.
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
    /// Query state (aggregation, granularity, time range)
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

        // Handle aggregation keybindings (Ctrl+key)
        ctx.input(|input| {
            if input.modifiers.ctrl {
                // Ctrl+S - toggle sum
                if input.key_pressed(Key::S) {
                    self.query_state.set_sum();
                }
                // Ctrl+A - toggle avg
                if input.key_pressed(Key::A) {
                    self.query_state.set_avg();
                }
                // Ctrl+P - cycle percentiles (p50 -> p95 -> p99 -> off)
                if input.key_pressed(Key::P) {
                    self.query_state.cycle_percentiles();
                }
                // Ctrl+M - toggle min
                if input.key_pressed(Key::M) {
                    self.query_state.set_min();
                }
                // Ctrl+X - toggle max
                if input.key_pressed(Key::X) {
                    self.query_state.set_max();
                }
                // Ctrl+G - cycle granularity
                if input.key_pressed(Key::G) {
                    self.query_state.cycle_granularity();
                }
            }
            // < and > to adjust granularity (no modifier needed)
            if input.key_pressed(Key::Period) && input.modifiers.shift {
                // > key (shift+period) - increase granularity
                self.query_state.cycle_granularity();
            }
            if input.key_pressed(Key::Comma) && input.modifiers.shift {
                // < key (shift+comma) - decrease granularity
                self.query_state.cycle_granularity_back();
            }
        });

        // Semi-transparent backdrop
        #[allow(deprecated)]
        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new("buffer_editor_backdrop"))
            .fixed_pos(screen_rect.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let backdrop_color = match self.theme {
                    AppTheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 120),
                    AppTheme::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, 150),
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
                    AppTheme::Light => Color32::from_rgb(250, 250, 250),
                    AppTheme::Dark => Color32::from_rgb(30, 30, 35),
                };
                let accent_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(80, 120, 200),
                    AppTheme::Dark => Color32::from_rgb(130, 180, 255),
                };

                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(2.0, accent_color))
                    .corner_radius(8.0)
                    .inner_margin(0.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header with mode indicator and buffer name
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);

                            // INSERT mode badge
                            egui::Frame::new()
                                .fill(Color32::from_rgb(100, 160, 80))
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
                                        .color(Color32::from_rgb(220, 160, 50))
                                        .size(12.0),
                                );
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
                            AppTheme::Light => Color32::from_rgb(220, 220, 220),
                            AppTheme::Dark => Color32::from_rgb(50, 50, 55),
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
                            let output = egui::TextEdit::multiline(&mut self.query)
                                .id(text_edit_id)
                                .font(FontId::monospace(14.0))
                                .hint_text(
                                    RichText::new("Enter query (e.g., env:prod AND service:db)")
                                        .color(text_color(self.theme).gamma_multiply(0.4)),
                                )
                                .desired_width(editor_width)
                                .desired_rows(4)
                                .lock_focus(true)
                                .show(ui);

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

                        ui.add_space(12.0);

                        // Separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );

                        ui.add_space(8.0);

                        // Status line showing aggregation state
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);

                            let status_bg = match self.theme {
                                AppTheme::Light => Color32::from_rgb(235, 240, 250),
                                AppTheme::Dark => Color32::from_rgb(40, 45, 55),
                            };

                            egui::Frame::new()
                                .fill(status_bg)
                                .corner_radius(4.0)
                                .inner_margin(egui::vec2(8.0, 4.0))
                                .show(ui, |ui| {
                                    let agg_color = if self.query_state.aggregation
                                        == crate::components::query_state::AggregationMode::None
                                    {
                                        text_color(self.theme).gamma_multiply(0.5)
                                    } else {
                                        Color32::from_rgb(100, 180, 100)
                                    };

                                    // Aggregation badge
                                    ui.label(
                                        RichText::new(format!(
                                            "[{}]",
                                            self.query_state.aggregation.label()
                                        ))
                                        .color(agg_color)
                                        .size(12.0)
                                        .strong(),
                                    );

                                    ui.add_space(8.0);

                                    // Time range
                                    ui.label(
                                        RichText::new(&self.query_state.time_range_label)
                                            .color(text_color(self.theme).gamma_multiply(0.7))
                                            .size(12.0),
                                    );

                                    ui.add_space(8.0);

                                    // Granularity
                                    ui.label(
                                        RichText::new(self.query_state.granularity.label())
                                            .color(text_color(self.theme).gamma_multiply(0.7))
                                            .size(12.0),
                                    );
                                });

                            ui.add_space(16.0);

                            // Aggregation keyboard hints
                            let hint_color = text_color(self.theme).gamma_multiply(0.35);
                            ui.label(RichText::new("^s").color(hint_color).size(10.0));
                            ui.label(RichText::new("sum").color(hint_color).size(10.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new("^a").color(hint_color).size(10.0));
                            ui.label(RichText::new("avg").color(hint_color).size(10.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new("^p").color(hint_color).size(10.0));
                            ui.label(RichText::new("p95").color(hint_color).size(10.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new("</>").color(hint_color).size(10.0));
                            ui.label(RichText::new("granularity").color(hint_color).size(10.0));
                        });

                        ui.add_space(8.0);

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
