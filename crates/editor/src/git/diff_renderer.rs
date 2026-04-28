//! Stateful diff renderer — provides search, selection, hunk jumping, context expansion,
//! and unified/split diff rendering for any consumer (overlay, PR pane, etc.).

use egui::{Color32, Key, RichText};

use super::diff::{self, DiffLine, DiffLineKind, FileDiff};
use super::diff_widget;
use crate::components::util::syntax_highlight::SyntaxHighlightData;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Callback type for per-line UI injection (e.g., inline comments).
pub type LineCallback<'a> = Option<&'a mut dyn FnMut(&mut egui::Ui, usize, &DiffLine)>;

/// Actions returned from keyboard handling that the caller must process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKeyAction {
    /// No action needed.
    None,
    /// Caller should switch to the next file.
    NextFile,
    /// Caller should switch to the previous file.
    PrevFile,
    /// Caller should copy selected text to clipboard.
    CopySelected,
    /// Caller should open the file in an external app.
    OpenFile,
    /// Caller should open the comment input on the selected line.
    CommentOnSelected,
}

/// Stateful diff renderer with search, selection, hunk navigation, and context expansion.
pub struct DiffRenderer {
    // ── View mode ──
    split_view: bool,

    // ── Scroll ──
    scroll_offset_x: f32,
    scroll_offset_y: f32,

    // ── Scroll animation ──
    scroll_anim_start: Option<Instant>,
    scroll_anim_from: f32,
    scroll_anim_to: f32,

    // ── Hunk navigation ──
    hunk_offsets: Vec<f32>,
    current_hunk_index: usize,
    /// Per-hunk context metadata used by the sticky header overlay:
    /// `(y_offset_in_content, hunk_context_text)`. Populated in parallel with
    /// `hunk_offsets` and invalidated at the same time.
    hunk_contexts: Vec<(f32, Option<String>)>,

    // ── Line selection ──
    selected_lines: Option<(usize, usize)>,
    selection_anchor: Option<usize>,

    // ── Search ──
    search_active: bool,
    search_query: String,
    /// Cached matches: (file_index, line_index, byte_start, byte_end).
    search_matches: Vec<(usize, usize, usize, usize)>,
    current_match_index: usize,

    // ── Deferred actions ──
    /// Hunk line index that was clicked for expansion (caller must process with &mut FileDiff).
    pending_expand_hunk: Option<usize>,
    /// Collapsed block line index that was clicked for expansion.
    pending_expand_block: Option<usize>,
    /// Line that was clicked for commenting: (file_index, line_index).
    pending_comment_line: Option<(usize, usize)>,

    // ── Hunk expansion ──
    /// Whether hunk headers should show a "click to expand" prompt.
    /// Callers that do not preload `FileDiff::old_file_lines` (e.g. the PR
    /// review pane) should set this to `false` so the prompt is not shown.
    allow_hunk_expansion: bool,

    // ── Vim g prefix ──
    /// True when `g` was pressed, waiting for a second key (e.g. `g` again for `gg`).
    g_pending: bool,

    // ── Hunk flash animation ──
    /// The hunk index that is currently flashing (after ]c/[c jump).
    flash_hunk_index: Option<usize>,
    /// When the flash started.
    flash_start: Option<Instant>,

    // ── Content metrics ──
    /// Total line count from last render, used for `G` (jump to bottom).
    last_total_lines: usize,

    // ── Identity ──
    id_salt: String,

    // ── Typography ──
    font_size: f32,

    // ── Floating comments support ──
    /// Line Y positions from the last render pass: `(line_idx, new_line_num, y_top)`.
    /// Populated during `render_unified` / `render_split`, consumed by the caller
    /// to position floating comment cards in the gutter.
    line_y_positions: Vec<(usize, usize, f32)>,

    /// The scroll-area content origin from the last render (top-left of the
    /// scrollable region in screen coords). Callers need this to convert the
    /// relative `line_y_positions` into absolute painter coords.
    last_content_origin: egui::Pos2,
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// Action from rendering a single diff line (internal).
enum LineAction {
    None,
    Click(bool), // shift_held
    ExpandHunk,
    ExpandBlock,
    Comment,
}

// ---------------------------------------------------------------------------
// Construction & accessors
// ---------------------------------------------------------------------------

impl DiffRenderer {
    /// Create a new renderer with a unique ID salt and font size.
    pub fn new(id_salt: &str, font_size: f32) -> Self {
        Self {
            split_view: false,
            scroll_offset_x: 0.0,
            scroll_offset_y: 0.0,
            scroll_anim_start: None,
            scroll_anim_from: 0.0,
            scroll_anim_to: 0.0,
            hunk_offsets: Vec::new(),
            hunk_contexts: Vec::new(),
            current_hunk_index: 0,
            selected_lines: None,
            selection_anchor: None,
            flash_hunk_index: None,
            flash_start: None,
            search_active: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_index: 0,
            pending_expand_hunk: None,
            pending_expand_block: None,
            pending_comment_line: None,
            allow_hunk_expansion: true,
            g_pending: false,
            last_total_lines: 0,
            id_salt: id_salt.to_string(),
            font_size,
            line_y_positions: Vec::new(),
            last_content_origin: egui::Pos2::ZERO,
        }
    }

    /// Take the pending hunk expansion index (if any). Caller processes with `&mut FileDiff`.
    pub fn take_pending_expand(&mut self) -> Option<usize> {
        self.pending_expand_hunk.take()
    }

    /// Take the pending collapsed block expansion index (if any).
    pub fn take_pending_expand_block(&mut self) -> Option<usize> {
        self.pending_expand_block.take()
    }

    /// Take the pending comment line (if any). Returns (file_index, line_index).
    pub fn take_pending_comment(&mut self) -> Option<(usize, usize)> {
        self.pending_comment_line.take()
    }

    pub fn split_view(&self) -> bool {
        self.split_view
    }

    pub fn set_split_view(&mut self, split: bool) {
        self.split_view = split;
    }

    pub fn toggle_split_view(&mut self) {
        self.split_view = !self.split_view;
    }

    pub fn allow_hunk_expansion(&self) -> bool {
        self.allow_hunk_expansion
    }

    pub fn set_allow_hunk_expansion(&mut self, allow: bool) {
        self.allow_hunk_expansion = allow;
    }

    pub fn search_active(&self) -> bool {
        self.search_active
    }

    pub fn selected_lines(&self) -> Option<(usize, usize)> {
        self.selected_lines
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Line height used for unified diff rendering (font_size + vertical padding).
    pub fn line_height(&self) -> f32 {
        self.font_size + 6.0
    }

    pub fn scroll_offset_y(&self) -> f32 {
        self.scroll_offset_y
    }

    /// Line Y positions from the last render: `(line_idx, new_line_num, y_top)`.
    /// The Y values are relative to the scroll-area content origin.
    pub fn line_y_positions(&self) -> &[(usize, usize, f32)] {
        &self.line_y_positions
    }

    /// The scroll-area content origin from the last render.
    pub fn last_content_origin(&self) -> egui::Pos2 {
        self.last_content_origin
    }

    /// Approximate line index at the current scroll position.
    pub fn current_line_approx(&self) -> usize {
        (self.scroll_offset_y / self.line_height()) as usize
    }

    pub fn scroll_down(&mut self, amount: f32) {
        self.scroll_offset_y += amount;
    }

    pub fn scroll_up(&mut self, amount: f32) {
        self.scroll_offset_y = (self.scroll_offset_y - amount).max(0.0);
    }

    /// Start a smooth scroll animation to `target_y`.
    pub fn animate_scroll_to(&mut self, target_y: f32) {
        self.scroll_anim_from = self.scroll_offset_y;
        self.scroll_anim_to = target_y.max(0.0);
        self.scroll_anim_start = Some(Instant::now());
    }

    /// Tick the scroll animation. Returns true if still animating (caller should repaint).
    fn tick_scroll_animation(&mut self) -> bool {
        let Some(start) = self.scroll_anim_start else {
            return false;
        };
        const DURATION: f32 = 0.2; // seconds
        let elapsed = start.elapsed().as_secs_f32();
        let t = (elapsed / DURATION).min(1.0);
        let eased = ease_out_cubic(t);
        self.scroll_offset_y =
            self.scroll_anim_from + (self.scroll_anim_to - self.scroll_anim_from) * eased;
        if t >= 1.0 {
            self.scroll_offset_y = self.scroll_anim_to;
            self.scroll_anim_start = None;
            false
        } else {
            true
        }
    }

    /// Reset state when switching to a different file.
    pub fn reset_for_file_change(&mut self) {
        self.scroll_offset_x = 0.0;
        self.scroll_offset_y = 0.0;
        self.scroll_anim_start = None;
        self.hunk_offsets.clear();
        self.hunk_contexts.clear();
        self.current_hunk_index = 0;
        self.selected_lines = None;
        self.selection_anchor = None;
    }
}

/// Cubic ease-out for smooth deceleration.
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

impl DiffRenderer {
    pub fn open_search(&mut self) {
        self.search_active = true;
    }

    pub fn close_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        self.search_matches.clear();
        self.current_match_index = 0;
    }

    /// Recompute search matches across all given files.
    pub fn recompute_search_matches(&mut self, files: &[FileDiff]) {
        self.search_matches.clear();
        self.current_match_index = 0;
        let query = self.search_query.to_lowercase();
        if query.is_empty() {
            return;
        }
        for (file_idx, file) in files.iter().enumerate() {
            for (line_idx, line) in file.lines.iter().enumerate() {
                if matches!(
                    line.kind,
                    DiffLineKind::HunkHeader | DiffLineKind::FileHeader | DiffLineKind::CollapsedBlock
                ) {
                    continue;
                }
                let lower = line.content.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&query) {
                    let byte_start = start + pos;
                    let byte_end = byte_start + query.len();
                    self.search_matches
                        .push((file_idx, line_idx, byte_start, byte_end));
                    start = byte_end;
                }
            }
        }
    }

    /// Navigate to the next match. Returns the target file index if it changed.
    pub fn next_match(&mut self, current_file_index: usize) -> Option<usize> {
        if self.search_matches.is_empty() {
            return None;
        }
        self.current_match_index = (self.current_match_index + 1) % self.search_matches.len();
        self.scroll_to_current_match(current_file_index)
    }

    /// Navigate to the previous match. Returns the target file index if it changed.
    pub fn prev_match(&mut self, current_file_index: usize) -> Option<usize> {
        if self.search_matches.is_empty() {
            return None;
        }
        self.current_match_index = if self.current_match_index == 0 {
            self.search_matches.len() - 1
        } else {
            self.current_match_index - 1
        };
        self.scroll_to_current_match(current_file_index)
    }

    /// Scroll to the current match. Returns Some(file_index) if the file changed.
    fn scroll_to_current_match(&mut self, current_file_index: usize) -> Option<usize> {
        let &(file_idx, line_idx, _, _) = self.search_matches.get(self.current_match_index)?;

        let file_changed = if file_idx != current_file_index {
            self.scroll_offset_x = 0.0;
            self.hunk_offsets.clear();
            self.hunk_contexts.clear();
            self.current_hunk_index = 0;
            self.selected_lines = None;
            self.selection_anchor = None;
            Some(file_idx)
        } else {
            None
        };

        // Estimate Y position for the matched line
        let line_height = self.line_height();
        // We don't have the file data here, so estimate from line_idx
        let estimated_y = line_idx as f32 * line_height;
        let target = (estimated_y - 100.0).max(0.0);
        if file_changed.is_some() {
            // Instant scroll on file change
            self.scroll_offset_y = target;
            self.scroll_anim_start = None;
        } else {
            self.animate_scroll_to(target);
        }

        file_changed
    }

    /// Render the search bar. Call before `render_diff` when search is active.
    pub fn render_search_bar(
        &mut self,
        ui: &mut egui::Ui,
        theme: AppTheme,
        files: &[FileDiff],
        current_file_index: usize,
    ) -> Option<usize> {
        let mut file_changed = None;

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Search icon
            ui.label(
                RichText::new(egui_nerdfonts::regular::MAGNIFY)
                    .color(theme.accent_primary())
                    .size(14.0),
            );
            ui.add_space(4.0);

            // Text input
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .desired_width(250.0)
                    .font(typography::monospace(typography::SM))
                    .hint_text("Search in diff...")
                    .text_color(theme.text_primary()),
            );
            response.request_focus();

            // Recompute matches when query changes
            if response.changed() {
                self.recompute_search_matches(files);
                if !self.search_matches.is_empty() {
                    let first_in_file = self
                        .search_matches
                        .iter()
                        .position(|m| m.0 == current_file_index);
                    self.current_match_index = first_in_file.unwrap_or(0);
                    file_changed = self.scroll_to_current_match(current_file_index);
                }
            }

            ui.add_space(8.0);

            // Match count indicator
            if !self.search_query.is_empty() {
                let match_text = if self.search_matches.is_empty() {
                    "No matches".to_string()
                } else {
                    format!(
                        "{}/{}",
                        self.current_match_index + 1,
                        self.search_matches.len()
                    )
                };
                ui.label(
                    RichText::new(match_text)
                        .color(if self.search_matches.is_empty() {
                            theme.diff_removed_text()
                        } else {
                            theme.text_secondary()
                        })
                        .font(typography::proportional(typography::SM)),
                );
            }

            // Hint
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Enter next \u{2022} Shift+Enter prev \u{2022} Esc close")
                        .color(theme.text_secondary().gamma_multiply(0.6))
                        .font(typography::proportional(typography::XS)),
                );
            });
        });

        // Separator below search bar
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        file_changed
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

impl DiffRenderer {
    pub fn clear_selection(&mut self) {
        self.selected_lines = None;
        self.selection_anchor = None;
    }

    /// Process a line click for selection.
    fn click_line(&mut self, line_idx: usize, shift_held: bool) {
        if shift_held {
            if let Some(anchor) = self.selection_anchor {
                self.selected_lines = Some((anchor, line_idx));
            }
        } else {
            self.selection_anchor = Some(line_idx);
            self.selected_lines = Some((line_idx, line_idx));
        }
    }

    /// Copy selected lines to a string. Caller should put this on the clipboard.
    pub fn copy_selected(&self, file_diff: &FileDiff) -> Option<String> {
        let (start, end) = self.selected_lines?;
        let min = start.min(end);
        let max = start.max(end);
        let text: String = file_diff
            .lines
            .get(min..=max)
            .unwrap_or_default()
            .iter()
            .filter(|l| {
                !matches!(
                    l.kind,
                    DiffLineKind::HunkHeader | DiffLineKind::FileHeader | DiffLineKind::CollapsedBlock
                )
            })
            .map(|l| l.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() { None } else { Some(text) }
    }
}

// ---------------------------------------------------------------------------
// Context expansion
// ---------------------------------------------------------------------------

impl DiffRenderer {
    /// Expand context around a hunk header by splicing in lines from the full file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn expand_context(&mut self, file_diff: &mut FileDiff, hunk_line_idx: usize) {
        let Some(hunk_line) = file_diff.lines.get(hunk_line_idx) else {
            return;
        };
        if hunk_line.kind != DiffLineKind::HunkHeader {
            return;
        }
        let hidden_count = hunk_line.hidden_lines.unwrap_or(0);
        if hidden_count == 0 {
            return;
        }

        let hunk_old_start = hunk_line.hunk_old_start.unwrap_or(1);
        let hunk_new_start = hunk_line.hunk_new_start.unwrap_or(1);

        let Some(ref old_file_lines) = file_diff.old_file_lines else {
            return;
        };

        let expand_count = hidden_count.min(20);
        let expand_start_old = hunk_old_start.saturating_sub(expand_count + 1);
        let expand_end_old = hunk_old_start.saturating_sub(1);
        let expand_start_new = hunk_new_start.saturating_sub(expand_count + 1);

        let mut new_lines = Vec::new();
        let mut actual_expanded = 0;
        for i in 0..expand_count {
            let old_idx = expand_start_old + i;
            if old_idx >= expand_end_old || old_idx >= old_file_lines.len() {
                break;
            }
            let content = old_file_lines[old_idx].clone();
            new_lines.push(DiffLine {
                content,
                kind: DiffLineKind::Context,
                old_line_num: Some(old_idx + 1),
                new_line_num: Some(expand_start_new + i + 1),
                word_highlights: Vec::new(),
                old_recon_num: None,
                new_recon_num: None,
                hidden_lines: None,
                hunk_context: None,
                hunk_old_start: None,
                hunk_new_start: None,
                collapsed_lines: None,
            });
            actual_expanded += 1;
        }

        if actual_expanded == 0 {
            return;
        }

        // Update hunk header's hidden count
        let remaining = hidden_count.saturating_sub(actual_expanded);
        file_diff.lines[hunk_line_idx].hidden_lines = if remaining > 0 {
            Some(remaining)
        } else {
            Some(0)
        };

        // Insert expanded lines before the hunk header
        for (i, line) in new_lines.into_iter().enumerate() {
            file_diff.lines.insert(hunk_line_idx + i, line);
        }

        // Invalidate caches
        self.hunk_offsets.clear();
        self.hunk_contexts.clear();
        self.search_matches.clear();

        // Recompute syntax highlighting
        file_diff.compute_syntax_highlights();
    }
}

// ---------------------------------------------------------------------------
// Hunk navigation
// ---------------------------------------------------------------------------

impl DiffRenderer {
    pub fn jump_next_hunk(&mut self) {
        if self.current_hunk_index + 1 < self.hunk_offsets.len() {
            self.current_hunk_index += 1;
            let target = self
                .hunk_offsets
                .get(self.current_hunk_index)
                .copied()
                .unwrap_or(0.0);
            self.animate_scroll_to(target);
            self.flash_hunk_index = Some(self.current_hunk_index);
            self.flash_start = Some(Instant::now());
        }
    }

    pub fn jump_prev_hunk(&mut self) {
        if self.current_hunk_index > 0 {
            self.current_hunk_index -= 1;
            let target = self
                .hunk_offsets
                .get(self.current_hunk_index)
                .copied()
                .unwrap_or(0.0);
            self.animate_scroll_to(target);
            self.flash_hunk_index = Some(self.current_hunk_index);
            self.flash_start = Some(Instant::now());
        }
    }

    /// Returns the current flash alpha (0.0 if no flash active). Call each frame
    /// and request repaint if > 0.
    fn tick_hunk_flash(&mut self) -> Option<(usize, f32)> {
        let start = self.flash_start?;
        let hunk_idx = self.flash_hunk_index?;
        const FLASH_DURATION: f32 = 0.4;
        let elapsed = start.elapsed().as_secs_f32();
        if elapsed >= FLASH_DURATION {
            self.flash_hunk_index = None;
            self.flash_start = None;
            return None;
        }
        // Ease out: starts bright, fades to zero
        let alpha = 1.0 - (elapsed / FLASH_DURATION);
        Some((hunk_idx, alpha))
    }
}

// ---------------------------------------------------------------------------
// Keyboard handling
// ---------------------------------------------------------------------------

impl DiffRenderer {
    /// Process standard diff keyboard shortcuts.
    ///
    /// Call inside `ctx.input_mut(|i| renderer.handle_keyboard(i))`.
    /// Does NOT handle Escape — callers have different Escape semantics.
    pub fn handle_keyboard(&mut self, input: &mut egui::InputState) -> DiffKeyAction {
        // Search: open with / or Cmd+F (also consume Text("/") to prevent
        // the workspace viewport filter from stealing the event)
        if !self.search_active {
            let slash_key = input.consume_key(egui::Modifiers::NONE, Key::Slash);
            let cmd_f = input.consume_key(egui::Modifiers::COMMAND, Key::F);
            let slash_text = input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "/"));
            if slash_key || cmd_f || slash_text {
                input
                    .events
                    .retain(|e| !matches!(e, egui::Event::Text(t) if t == "/"));
                self.search_active = true;
            }
        }

        // When search is active, Enter/Shift+Enter navigate matches
        if self.search_active && !self.search_matches.is_empty() {
            if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                self.current_match_index =
                    (self.current_match_index + 1) % self.search_matches.len();
                // Caller should call scroll_to_current_match or we do it via render
                self.apply_match_scroll();
            }
            if input.consume_key(egui::Modifiers::SHIFT, Key::Enter) {
                self.current_match_index = if self.current_match_index == 0 {
                    self.search_matches.len() - 1
                } else {
                    self.current_match_index - 1
                };
                self.apply_match_scroll();
            }
        }

        // Rest of shortcuts only when search is NOT active
        if !self.search_active {
            // Cmd+C — copy
            if input.consume_key(egui::Modifiers::COMMAND, Key::C) {
                return DiffKeyAction::CopySelected;
            }

            // o — open file
            if input.consume_key(egui::Modifiers::NONE, Key::O) {
                return DiffKeyAction::OpenFile;
            }

            // c — comment on selected line
            if input.consume_key(egui::Modifiers::NONE, Key::C) {
                return DiffKeyAction::CommentOnSelected;
            }

            // n / p — file navigation
            if input.consume_key(egui::Modifiers::NONE, Key::N) {
                return DiffKeyAction::NextFile;
            }
            if input.consume_key(egui::Modifiers::NONE, Key::P)
                || input.consume_key(egui::Modifiers::SHIFT, Key::N)
            {
                return DiffKeyAction::PrevFile;
            }

            // s — toggle split view
            if input.consume_key(egui::Modifiers::NONE, Key::S) {
                self.toggle_split_view();
            }

            // { / } — hunk jumping
            if input.consume_key(egui::Modifiers::SHIFT, Key::OpenBracket) {
                self.jump_prev_hunk();
            }
            if input.consume_key(egui::Modifiers::SHIFT, Key::CloseBracket) {
                self.jump_next_hunk();
            }

            // G (Shift+g) — jump to bottom of file
            if input.consume_key(egui::Modifiers::SHIFT, Key::G) {
                self.g_pending = false;
                let target = (self.last_total_lines as f32 * self.line_height()).max(0.0);
                self.animate_scroll_to(target);
            }

            // g — first press sets pending, second press (gg) jumps to top
            if input.consume_key(egui::Modifiers::NONE, Key::G) {
                if self.g_pending {
                    self.g_pending = false;
                    self.animate_scroll_to(0.0);
                } else {
                    self.g_pending = true;
                }
            } else if self.g_pending
                && input.events.iter().any(
                    |e| matches!(e, egui::Event::Key { pressed: true, key, .. } if *key != Key::G),
                )
            {
                // Any other key cancels the g prefix
                self.g_pending = false;
            }

            // j/k/h/l — vim scroll
            let scroll_step = 40.0;
            let h_scroll_step = 50.0;
            if input.consume_key(egui::Modifiers::NONE, Key::J) {
                self.g_pending = false;
                self.scroll_offset_y += scroll_step;
            }
            if input.consume_key(egui::Modifiers::NONE, Key::K) {
                self.g_pending = false;
                self.scroll_offset_y = (self.scroll_offset_y - scroll_step).max(0.0);
            }
            if input.consume_key(egui::Modifiers::NONE, Key::H) {
                self.g_pending = false;
                self.scroll_offset_x = (self.scroll_offset_x - h_scroll_step).max(0.0);
            }
            if input.consume_key(egui::Modifiers::NONE, Key::L) {
                self.g_pending = false;
                self.scroll_offset_x += h_scroll_step;
            }
        }

        DiffKeyAction::None
    }

    /// Apply scroll position to reach the current search match.
    fn apply_match_scroll(&mut self) {
        if let Some(&(_, line_idx, _, _)) = self.search_matches.get(self.current_match_index) {
            let estimated_y = line_idx as f32 * self.line_height();
            self.animate_scroll_to((estimated_y - 100.0).max(0.0));
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering — main entry point
// ---------------------------------------------------------------------------

impl DiffRenderer {
    /// Render the diff content for one file.
    ///
    /// `line_callback` is called after each content line, letting callers inject
    /// extra UI (e.g., inline comments). Receives `(ui, line_idx, &DiffLine)`.
    pub fn render_diff(
        &mut self,
        ui: &mut egui::Ui,
        file_diff: &FileDiff,
        current_file_index: usize,
        theme: AppTheme,
        line_callback: LineCallback<'_>,
    ) {
        // Tick scroll animation
        if self.tick_scroll_animation() {
            ui.ctx().request_repaint();
        }

        // Clear line positions from previous frame (repopulated by render_unified)
        self.line_y_positions.clear();

        // Track total lines for gg/G navigation
        self.last_total_lines = file_diff.lines.len();

        if file_diff.lines.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No changes")
                        .color(theme.text_secondary())
                        .font(typography::proportional(self.font_size)),
                );
            });
            return;
        }

        if self.split_view {
            self.render_split(ui, file_diff, current_file_index, theme, line_callback);
        } else {
            self.render_unified(ui, file_diff, current_file_index, theme, line_callback);
        }
    }

    // ── Unified view ──

    fn render_unified(
        &mut self,
        ui: &mut egui::Ui,
        file_diff: &FileDiff,
        current_file_index: usize,
        theme: AppTheme,
        mut line_callback: LineCallback<'_>,
    ) {
        let line_num_width = diff_widget::max_line_num_width(file_diff);
        let line_height = self.line_height();
        let hunk_header_height = typography::SM + 12.0;

        // Pre-compute hunk offsets (skip FileHeader lines since they're not rendered)
        if self.hunk_offsets.is_empty() {
            let mut y = 4.0;
            for line in &file_diff.lines {
                if line.kind == DiffLineKind::FileHeader {
                    continue;
                }
                if line.kind == DiffLineKind::HunkHeader {
                    self.hunk_offsets.push(y);
                    self.hunk_contexts.push((y, line.hunk_context.clone()));
                }
                y += if matches!(line.kind, DiffLineKind::HunkHeader | DiffLineKind::CollapsedBlock)
                {
                    hunk_header_height
                } else {
                    line_height
                };
            }
        }

        // Tick hunk flash animation
        let flash_state = self.tick_hunk_flash();
        if flash_state.is_some() {
            ui.ctx().request_repaint();
        }

        let selected_lines = self.selected_lines;
        let accent = theme.accent_primary();
        let font_size = self.font_size;

        // Build search state refs for the closure
        let search_matches = &self.search_matches;
        let search_query = &self.search_query;
        let current_match_index = self.current_match_index;

        let mut clicked_line: Option<(usize, bool)> = None;
        let mut expand_hunk_idx: Option<usize> = None;
        let mut expand_block_idx: Option<usize> = None;
        let mut comment_line_idx: Option<usize> = None;

        let mut collected_y_positions: Vec<(usize, usize, f32)> = Vec::new();

        let scroll_output = egui::ScrollArea::both()
            .id_salt(format!("{}_unified", self.id_salt))
            .scroll_offset(egui::vec2(self.scroll_offset_x, self.scroll_offset_y))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.style_mut().spacing.item_spacing.y = 0.0;

                let mut hunk_counter: usize = 0;

                for (line_idx, line) in file_diff.lines.iter().enumerate() {
                    // Skip file headers — the caller renders the file path in its own header
                    if line.kind == DiffLineKind::FileHeader {
                        continue;
                    }

                    // Track hunk index for flash animation
                    let hunk_flash_alpha = if line.kind == DiffLineKind::HunkHeader {
                        let idx = hunk_counter;
                        hunk_counter += 1;
                        flash_state.filter(|(fi, _)| *fi == idx).map(|(_, a)| a)
                    } else {
                        None
                    };

                    // Search highlights for this line
                    let line_search_highlights: Vec<(usize, usize, bool)> =
                        if search_query.is_empty() {
                            Vec::new()
                        } else {
                            search_matches
                                .iter()
                                .enumerate()
                                .filter(|(_, m)| m.0 == current_file_index && m.1 == line_idx)
                                .map(|(i, m)| (m.2, m.3, i == current_match_index))
                                .collect()
                        };

                    let action = render_unified_line(
                        ui,
                        line,
                        line_idx,
                        line_num_width,
                        theme,
                        file_diff.old_highlight.as_ref(),
                        file_diff.new_highlight.as_ref(),
                        selected_lines,
                        accent,
                        &line_search_highlights,
                        font_size,
                        hunk_flash_alpha,
                        self.allow_hunk_expansion,
                    );

                    match action {
                        LineAction::Click(shift) => clicked_line = Some((line_idx, shift)),
                        LineAction::ExpandHunk => expand_hunk_idx = Some(line_idx),
                        LineAction::ExpandBlock => expand_block_idx = Some(line_idx),
                        LineAction::Comment => comment_line_idx = Some(line_idx),
                        LineAction::None => {}
                    }

                    // Capture Y position for floating comment anchoring
                    if let Some(new_ln) = line.new_line_num {
                        // cursor().top() is the Y *after* this line was laid out;
                        // subtract line_height to get the top of this line.
                        let y_top = ui.cursor().top() - line_height;
                        collected_y_positions.push((line_idx, new_ln, y_top));
                    }

                    // Per-line callback (inline comments, etc.)
                    if let Some(ref mut cb) = line_callback {
                        cb(ui, line_idx, line);
                    }
                }

                ui.add_space(8.0);
            });

        // Sync scroll offset back from egui so mouse-wheel scrolling works
        self.scroll_offset_x = scroll_output.state.offset.x;
        self.scroll_offset_y = scroll_output.state.offset.y;

        // Store line positions for floating comments
        self.line_y_positions = collected_y_positions;
        self.last_content_origin = scroll_output.inner_rect.left_top();

        // Sticky hunk-context header: paint the current hunk's function-context
        // pinned to the top of the viewport whenever its real header has scrolled
        // out of view. Overlays the scroll area so it stays visible during scroll.
        self.paint_sticky_hunk_header(ui, scroll_output.inner_rect, theme);

        // Process clicks
        if let Some((line_idx, shift)) = clicked_line {
            self.click_line(line_idx, shift);
        }

        // Store hunk expansion for caller to process with &mut FileDiff
        if expand_hunk_idx.is_some() {
            self.pending_expand_hunk = expand_hunk_idx;
        }

        // Store collapsed block expansion for caller to process
        if expand_block_idx.is_some() {
            self.pending_expand_block = expand_block_idx;
        }

        // Store comment request for caller to process
        if let Some(line_idx) = comment_line_idx {
            self.pending_comment_line = Some((current_file_index, line_idx));
        }

        // Show copy hint when lines are selected
        if let Some((start, end)) = self.selected_lines {
            let count = start.abs_diff(end) + 1;
            let hint_text = if count == 1 {
                "\u{2318}C copy line".to_string()
            } else {
                format!("\u{2318}C copy {count} lines")
            };
            // Floating hint in the bottom-right corner
            let avail = ui.available_rect_before_wrap();
            let hint_pos = egui::pos2(avail.right() - 16.0, avail.bottom() + 4.0);
            let hint_galley = ui.painter().layout_no_wrap(
                hint_text,
                typography::proportional(typography::XS),
                theme.text_secondary().gamma_multiply(0.7),
            );
            let hint_rect = egui::Rect::from_min_size(
                egui::pos2(
                    hint_pos.x - hint_galley.size().x - 8.0,
                    hint_pos.y - hint_galley.size().y - 6.0,
                ),
                hint_galley.size() + egui::vec2(12.0, 8.0),
            );
            ui.painter()
                .rect_filled(hint_rect, 4.0, theme.bg_elevated());
            ui.painter().rect_stroke(
                hint_rect,
                4.0,
                egui::Stroke::new(1.0, theme.border_subtle()),
                egui::StrokeKind::Outside,
            );
            ui.painter().galley(
                egui::pos2(hint_rect.left() + 6.0, hint_rect.top() + 4.0),
                hint_galley,
                theme.text_secondary(),
            );
        }
    }

    // ── Split view ──

    fn render_split(
        &mut self,
        ui: &mut egui::Ui,
        file_diff: &FileDiff,
        _current_file_index: usize,
        theme: AppTheme,
        mut line_callback: LineCallback<'_>,
    ) {
        let available_width = ui.available_width();
        let side_width = ((available_width - 8.0) / 2.0).max(1.0);
        let line_num_width = diff_widget::max_line_num_width(file_diff);
        let split_line_height = self.font_size + 6.0;
        let hunk_header_height = typography::SM + 12.0;
        let font_size = self.font_size;

        let paired_lines = diff::build_split_view_lines_ref(&file_diff.lines);

        // Pre-compute pointer → index map to avoid O(n) scans per row
        let line_index_map: rustc_hash::FxHashMap<*const DiffLine, usize> = file_diff
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| (l as *const DiffLine, i))
            .collect();

        // Pre-compute hunk offsets (skip FileHeader lines since they're not rendered)
        if self.hunk_offsets.is_empty() {
            let header_row_height = typography::SM + 4.0;
            let mut y = header_row_height + 4.0;
            for (left, _) in &paired_lines {
                let is_file_header = left
                    .as_ref()
                    .is_some_and(|l| l.kind == DiffLineKind::FileHeader);
                if is_file_header {
                    continue;
                }
                let is_hunk = left
                    .as_ref()
                    .is_some_and(|l| l.kind == DiffLineKind::HunkHeader);
                let is_collapsed = left
                    .as_ref()
                    .is_some_and(|l| l.kind == DiffLineKind::CollapsedBlock);
                if is_hunk {
                    self.hunk_offsets.push(y);
                    if let Some(l) = left.as_ref() {
                        self.hunk_contexts.push((y, l.hunk_context.clone()));
                    }
                }
                y += if is_hunk || is_collapsed {
                    hunk_header_height
                } else {
                    split_line_height
                };
            }
        }

        // Column headers
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, typography::SM + 4.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Old")
                            .color(theme.diff_removed_text().gamma_multiply(0.7))
                            .font(typography::proportional(typography::XS))
                            .strong(),
                    );
                },
            );
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, typography::SM + 4.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("New")
                            .color(theme.diff_added_text().gamma_multiply(0.7))
                            .font(typography::proportional(typography::XS))
                            .strong(),
                    );
                },
            );
        });

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        let scroll_output = egui::ScrollArea::vertical()
            .id_salt(format!("{}_split", self.id_salt))
            .scroll_offset(egui::vec2(0.0, self.scroll_offset_y))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(available_width);
                ui.add_space(4.0);
                ui.style_mut().spacing.item_spacing.y = 0.0;

                for (left, right) in &paired_lines {
                    // Skip file headers — the caller renders the file path in its own header
                    let is_file_header = left
                        .as_ref()
                        .is_some_and(|l| l.kind == DiffLineKind::FileHeader);
                    if is_file_header {
                        continue;
                    }

                    let is_header = left
                        .as_ref()
                        .is_some_and(|l| l.kind == DiffLineKind::HunkHeader);
                    let is_collapsed = left
                        .as_ref()
                        .is_some_and(|l| l.kind == DiffLineKind::CollapsedBlock);

                    if is_header {
                        if let Some(line) = left.as_ref() {
                            render_split_header(ui, line, available_width, theme);
                        }
                    } else if is_collapsed {
                        if let Some(line) = left.as_ref() {
                            let row_height = font_size + 12.0;
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(available_width, row_height),
                                egui::Sense::click(),
                            );
                            let bg = if response.hovered() {
                                theme.bg_elevated().gamma_multiply(0.7)
                            } else {
                                theme.bg_elevated().gamma_multiply(0.5)
                            };
                            ui.painter().rect_filled(rect, 4.0, bg);
                            ui.painter().rect_stroke(
                                rect.shrink(2.0),
                                4.0,
                                egui::Stroke::new(
                                    1.0,
                                    theme.border_subtle().gamma_multiply(0.5),
                                ),
                                egui::StrokeKind::Inside,
                            );
                            let icon = if response.hovered() {
                                egui_nerdfonts::regular::CHEVRON_DOWN
                            } else {
                                egui_nerdfonts::regular::CHEVRON_RIGHT
                            };
                            let text = format!("{icon} {}", line.content);
                            let text_color = if response.hovered() {
                                theme.text_primary()
                            } else {
                                theme.text_secondary()
                            };
                            let galley = ui.painter().layout_no_wrap(
                                text,
                                typography::proportional(typography::XS),
                                text_color,
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    rect.left() + 16.0,
                                    rect.center().y - galley.size().y / 2.0,
                                ),
                                galley,
                                text_color,
                            );
                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if response.clicked() {
                                if let Some(l) = left {
                                    if let Some(&idx) =
                                        line_index_map.get(&(*l as *const DiffLine))
                                    {
                                        self.pending_expand_block = Some(idx);
                                    }
                                }
                            }
                        }
                    } else {
                        let row_response = ui.horizontal(|ui| {
                            ui.set_max_width(available_width);

                            // Left side
                            ui.allocate_ui_with_layout(
                                egui::vec2(side_width, split_line_height),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(side_width);
                                    render_split_line(
                                        ui,
                                        *left,
                                        line_num_width,
                                        true,
                                        side_width,
                                        theme,
                                        file_diff.old_highlight.as_ref(),
                                        file_diff.new_highlight.as_ref(),
                                        font_size,
                                    );
                                },
                            );

                            // Separator
                            let sep = ui.available_rect_before_wrap();
                            ui.painter().vline(
                                sep.left(),
                                sep.y_range(),
                                egui::Stroke::new(1.0, theme.border_subtle()),
                            );
                            ui.add_space(4.0);

                            // Right side
                            ui.allocate_ui_with_layout(
                                egui::vec2(side_width, split_line_height),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(side_width);
                                    render_split_line(
                                        ui,
                                        *right,
                                        line_num_width,
                                        false,
                                        side_width,
                                        theme,
                                        file_diff.old_highlight.as_ref(),
                                        file_diff.new_highlight.as_ref(),
                                        font_size,
                                    );
                                },
                            );
                        });

                        // "+" comment button on hover for split view rows
                        if let Some(line) = right.or(*left) {
                            if line.kind != DiffLineKind::FileHeader
                                && line.kind != DiffLineKind::HunkHeader
                            {
                                let row_rect = row_response.response.rect;
                                if ui.rect_contains_pointer(row_rect) {
                                    // Show "+" icon at the left edge of the right pane gutter
                                    let icon_size = 14.0;
                                    let icon_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            row_rect.left() + 2.0,
                                            row_rect.center().y - icon_size / 2.0,
                                        ),
                                        egui::vec2(icon_size, icon_size),
                                    );
                                    ui.painter().text(
                                        icon_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "+",
                                        typography::proportional(typography::XS),
                                        theme.accent_primary(),
                                    );

                                    // Detect click on the "+" area
                                    if ui.input(|i| i.pointer.primary_clicked()) {
                                        if let Some(click_pos) =
                                            ui.input(|i| i.pointer.interact_pos())
                                        {
                                            if icon_rect.expand(4.0).contains(click_pos) {
                                                if let Some(&idx) =
                                                    line_index_map.get(&(line as *const DiffLine))
                                                {
                                                    self.pending_comment_line =
                                                        Some((_current_file_index, idx));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Per-line callback — use the right-side line (or left if right is None)
                        if let Some(ref mut cb) = line_callback {
                            if let Some(line) = right.or(*left) {
                                if let Some(&idx) = line_index_map.get(&(line as *const DiffLine)) {
                                    cb(ui, idx, line);
                                }
                            }
                        }
                    }
                }

                ui.add_space(8.0);
            });

        // Sync scroll offset back from egui so mouse-wheel scrolling works
        self.scroll_offset_y = scroll_output.state.offset.y;

        // Sticky hunk-context header overlay (same behavior as unified view).
        self.paint_sticky_hunk_header(ui, scroll_output.inner_rect, theme);
    }

    /// Paint the current hunk's function-context pinned to the top of the diff
    /// viewport. No-op when there is no active hunk context, or when the real
    /// header is still visible on-screen.
    fn paint_sticky_hunk_header(&self, ui: &egui::Ui, viewport: egui::Rect, theme: AppTheme) {
        if self.hunk_contexts.is_empty() {
            return;
        }

        // Find the last hunk whose y_offset is at or above the current scroll
        // position. That's the hunk currently containing the top of the viewport.
        let scroll_y = self.scroll_offset_y;
        let active = self
            .hunk_contexts
            .iter()
            .rev()
            .find(|(y, _)| *y <= scroll_y + 2.0);
        let Some((active_y, Some(context))) = active else {
            return;
        };
        if context.trim().is_empty() {
            return;
        }

        // Only paint when the real header is scrolled out of view — otherwise
        // it would stack on top of itself.
        let hunk_header_height = typography::SM + 12.0;
        if active_y + hunk_header_height > scroll_y {
            return;
        }

        // Don't paint if the *next* hunk header is currently at or above the
        // viewport top (the new hunk's real header is about to take over).
        if let Some((next_y, _)) = self.hunk_contexts.iter().find(|(y, _)| *y > *active_y) {
            if *next_y <= scroll_y + hunk_header_height {
                return;
            }
        }

        let header_rect = egui::Rect::from_min_size(
            viewport.left_top(),
            egui::vec2(viewport.width(), hunk_header_height),
        );
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new(("sticky_hunk", &self.id_salt)),
        ));
        painter.rect_filled(header_rect, 0.0, theme.diff_hunk_bg().gamma_multiply(0.92));
        painter.hline(
            header_rect.x_range(),
            header_rect.bottom(),
            egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.7)),
        );
        // Subtle bottom drop shadow so the sticky header visually lifts off
        // the content beneath it.
        let shadow_rect = egui::Rect::from_min_size(
            egui::pos2(header_rect.left(), header_rect.bottom()),
            egui::vec2(header_rect.width(), 4.0),
        );
        painter.rect_filled(shadow_rect, 0.0, egui::Color32::from_black_alpha(18));

        let text_color = theme.diff_hunk_text();
        let galley = painter.layout_no_wrap(
            context.clone(),
            typography::monospace(self.font_size),
            text_color,
        );
        painter.galley(
            egui::pos2(
                header_rect.left() + 16.0,
                header_rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            text_color,
        );
    }
}

// ---------------------------------------------------------------------------
// Free rendering functions (internal to this module)
// ---------------------------------------------------------------------------

/// Render a single line in unified view with all features.
#[allow(clippy::too_many_arguments)]
fn render_unified_line(
    ui: &mut egui::Ui,
    line: &DiffLine,
    line_idx: usize,
    line_num_width: usize,
    theme: AppTheme,
    old_highlight: Option<&SyntaxHighlightData>,
    new_highlight: Option<&SyntaxHighlightData>,
    selected_lines: Option<(usize, usize)>,
    accent: Color32,
    search_highlights: &[(usize, usize, bool)],
    font_size: f32,
    hunk_flash_alpha: Option<f32>,
    allow_hunk_expansion: bool,
) -> LineAction {
    let mut clicked_shift: Option<bool> = None;

    // Hunk headers — styled separator with expand-on-click
    if line.kind == DiffLineKind::HunkHeader {
        let available_width = ui.available_width();
        let header_height = typography::SM + 12.0;
        let has_hidden = line.hidden_lines.is_some_and(|n| n > 0);
        let can_expand = allow_hunk_expansion && has_hidden;

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(available_width, header_height),
            if can_expand {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );

        let bg = if can_expand && response.hovered() {
            theme.diff_hunk_bg().gamma_multiply(1.4)
        } else {
            theme.diff_hunk_bg()
        };
        ui.painter().rect_filled(rect, 0.0, bg);

        // Flash overlay after hunk jump
        if let Some(alpha) = hunk_flash_alpha {
            let flash_color = theme.accent_primary().gamma_multiply(0.25 * alpha);
            ui.painter().rect_filled(rect, 0.0, flash_color);
        }

        // Separator lines
        let sep_color = theme.diff_hunk_text().gamma_multiply(0.2);
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            egui::Stroke::new(1.0, sep_color),
        );
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, sep_color),
        );

        // Display text
        let hidden_text = if has_hidden {
            let n = line.hidden_lines.unwrap_or(0);
            if can_expand {
                format!(
                    "{} \u{00B7}\u{00B7}\u{00B7} {n} lines hidden \u{00B7}\u{00B7}\u{00B7} click to expand",
                    egui_nerdfonts::regular::UNFOLD_MORE_HORIZONTAL
                )
            } else {
                format!(
                    "{} \u{00B7}\u{00B7}\u{00B7} {n} lines hidden \u{00B7}\u{00B7}\u{00B7}",
                    egui_nerdfonts::regular::UNFOLD_MORE_HORIZONTAL
                )
            }
        } else {
            "\u{00B7}\u{00B7}\u{00B7}".to_string()
        };
        let context_text = line.hunk_context.as_deref().unwrap_or("");
        let center_y = rect.center().y;

        let text_alpha = if can_expand && response.hovered() {
            1.0
        } else {
            0.7
        };
        let hidden_galley = ui.painter().layout_no_wrap(
            hidden_text,
            typography::proportional(typography::XS),
            theme.diff_hunk_text().gamma_multiply(text_alpha),
        );
        let text_x = rect.left() + 16.0;
        ui.painter().galley(
            egui::pos2(text_x, center_y - hidden_galley.size().y / 2.0),
            hidden_galley.clone(),
            theme.diff_hunk_text().gamma_multiply(text_alpha),
        );

        if !context_text.is_empty() {
            let ctx_x = text_x + hidden_galley.size().x + 12.0;
            ui.painter().text(
                egui::pos2(ctx_x, center_y),
                egui::Align2::LEFT_CENTER,
                context_text,
                typography::monospace(typography::XS),
                theme.syntax_function().gamma_multiply(0.8),
            );
        }

        if can_expand && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if can_expand && response.clicked() {
            return LineAction::ExpandHunk;
        }
        return LineAction::None;
    }

    // File headers
    if line.kind == DiffLineKind::FileHeader {
        let available_width = ui.available_width();
        let line_height = font_size + 6.0;
        let (line_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, line_height),
            egui::Sense::hover(),
        );
        ui.painter()
            .rect_filled(line_rect, 0.0, theme.diff_file_header_bg());
        ui.painter().text(
            line_rect.left_center() + egui::vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &line.content,
            typography::monospace(font_size),
            theme.diff_file_header(),
        );
        return LineAction::None;
    }

    // Collapsed block — disclosure row with expand-on-click
    if line.kind == DiffLineKind::CollapsedBlock {
        let available_width = ui.available_width();
        let row_height = font_size + 12.0;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(available_width, row_height), egui::Sense::click());

        let bg = if response.hovered() {
            theme.bg_elevated().gamma_multiply(0.7)
        } else {
            theme.bg_elevated().gamma_multiply(0.5)
        };
        ui.painter().rect_filled(rect, 4.0, bg);

        // Dashed border to indicate "folded" content
        ui.painter().rect_stroke(
            rect.shrink(2.0),
            4.0,
            egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.5)),
            egui::StrokeKind::Inside,
        );

        let icon = if response.hovered() {
            egui_nerdfonts::regular::CHEVRON_DOWN
        } else {
            egui_nerdfonts::regular::CHEVRON_RIGHT
        };
        let text = format!("{icon} {}", line.content);
        let text_color = if response.hovered() {
            theme.text_primary()
        } else {
            theme.text_secondary()
        };
        let galley = ui.painter().layout_no_wrap(
            text,
            typography::proportional(typography::XS),
            text_color,
        );
        ui.painter().galley(
            egui::pos2(rect.left() + 16.0, rect.center().y - galley.size().y / 2.0),
            galley,
            text_color,
        );

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            return LineAction::ExpandBlock;
        }
        return LineAction::None;
    }

    // Regular content lines — painter-based approach for consistent line heights
    let (base_text_color, bg_color, gutter_color) = diff_widget::diff_line_colors(line.kind, theme);
    let available_width = ui.available_width();
    let line_height = font_size + 6.0;

    let is_selected = selected_lines.is_some_and(|(start, end)| {
        let min = start.min(end);
        let max = start.max(end);
        line_idx >= min && line_idx <= max
    });

    let gutter_width = 4.0;
    let line_num_area_width = (line_num_width * 2 + 3) as f32 * 8.0;

    // Allocate a fixed-size rect for the entire line (clickable for line selection)
    let (line_rect, line_response) = ui.allocate_exact_size(
        egui::vec2(available_width, line_height),
        egui::Sense::click(),
    );

    // Background fill — expand rect by 0.5px vertically to cover subpixel gaps
    // between adjacent lines that cause thin dark lines to appear.
    if let Some(bg) = bg_color {
        let bg_rect = line_rect.expand2(egui::vec2(0.0, 0.5));
        ui.painter().rect_filled(bg_rect, 0.0, bg);
    }
    // Selection overlay
    if is_selected {
        ui.painter()
            .rect_filled(line_rect, 0.0, accent.gamma_multiply(0.12));
    }

    let mut cursor_x = line_rect.left();

    // Gutter stripe — shows "+" comment icon on hover
    let gutter_rect = egui::Rect::from_min_size(
        egui::pos2(cursor_x, line_rect.top()),
        egui::vec2(gutter_width, line_height),
    );
    if line_response.hovered() && line.kind != DiffLineKind::FileHeader {
        // Show "+" icon for commenting
        let plus_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x - 1.0, line_rect.top()),
            egui::vec2(gutter_width + 6.0, line_height),
        );
        ui.painter()
            .rect_filled(plus_rect, 2.0, accent.gamma_multiply(0.2));
        ui.painter().text(
            plus_rect.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            typography::monospace(typography::XS),
            accent,
        );
    } else if let Some(gc) = gutter_color {
        ui.painter().rect_filled(gutter_rect, 0.0, gc);
    }
    cursor_x += gutter_width + 4.0;

    // Line numbers background
    let line_num_rect = egui::Rect::from_min_size(
        egui::pos2(cursor_x, line_rect.top()),
        egui::vec2(line_num_area_width, line_height),
    );
    ui.painter()
        .rect_filled(line_num_rect, 0.0, theme.diff_line_number_bg());

    // Line numbers text
    let old_num_str = line
        .old_line_num
        .map(|n| format!("{n:>line_num_width$}"))
        .unwrap_or_else(|| " ".repeat(line_num_width));
    let new_num_str = line
        .new_line_num
        .map(|n| format!("{n:>line_num_width$}"))
        .unwrap_or_else(|| " ".repeat(line_num_width));

    ui.painter().text(
        line_num_rect.left_center() + egui::vec2(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{old_num_str} {new_num_str}"),
        typography::monospace(typography::SM),
        theme.diff_line_number(),
    );
    cursor_x += line_num_area_width + 8.0;

    // Structural cues for Context lines: zebra banding + indent guides.
    // Painted before the galley so leading whitespace shows the guides and
    // actual code paints on top (hiding guides under character glyphs).
    paint_context_rails(
        ui.painter(),
        line_rect,
        cursor_x,
        line,
        theme,
        font_size,
        is_selected,
    );

    // Content with syntax highlighting
    let syntax_spans =
        diff_widget::get_syntax_spans_for_line(line, old_highlight, new_highlight, theme);
    let content = if line.content.is_empty() {
        " "
    } else {
        &line.content
    };
    let word_bg = diff::diff_word_bg(line.kind, theme);

    let layout_job = diff_widget::build_diff_line_layout_job(
        content,
        &line.word_highlights,
        base_text_color,
        word_bg,
        &syntax_spans,
        search_highlights,
        font_size,
        theme,
    );

    let galley = ui.painter().layout_job(layout_job);
    ui.painter().galley(
        egui::pos2(cursor_x, line_rect.center().y - galley.size().y / 2.0),
        galley,
        base_text_color,
    );

    // Handle click — gutter area triggers comment, line number area triggers selection
    if line_response.clicked() {
        if let Some(pos) = line_response.interact_pointer_pos() {
            let click_x = pos.x - line_rect.left();
            if click_x < gutter_width + 4.0 {
                // Clicked on gutter area → comment
                return LineAction::Comment;
            }
        }
        let shift = ui.input(|i| i.modifiers.shift);
        clicked_shift = Some(shift);
    }

    match clicked_shift {
        Some(shift) => LineAction::Click(shift),
        None => LineAction::None,
    }
}

/// Paint structural rails — a subtle zebra band every 5 Context lines plus
/// thin indent guides under leading whitespace — so the eye reads structure
/// quickly on long unchanged blocks. No-op on non-Context lines so changed
/// lines keep their own visual weight.
fn paint_context_rails(
    painter: &egui::Painter,
    line_rect: egui::Rect,
    content_start_x: f32,
    line: &DiffLine,
    theme: AppTheme,
    font_size: f32,
    is_selected: bool,
) {
    if line.kind != DiffLineKind::Context {
        return;
    }

    // Zebra: tint every 5-line band using the file's line number so the
    // rhythm is tied to actual source lines, not paint order. Skip when the
    // row is selected — the selection overlay already provides contrast.
    if !is_selected {
        if let Some(n) = line.new_line_num {
            let band = (n.saturating_sub(1) / 5) % 2;
            if band == 1 {
                let content_rect = egui::Rect::from_min_max(
                    egui::pos2(content_start_x, line_rect.top()),
                    line_rect.right_bottom(),
                );
                painter.rect_filled(
                    content_rect,
                    0.0,
                    theme.text_primary().gamma_multiply(0.025),
                );
            }
        }
    }

    // Indent guides: one thin vertical rule per 4-space indent level, drawn
    // only under the leading whitespace so code never paints over a stripe
    // at a weird offset.
    let leading_spaces = line.content.chars().take_while(|&c| c == ' ').count();
    let levels = leading_spaces / 4;
    if levels == 0 {
        return;
    }
    // Approx glyph width for the monospace diff font. Fine to keep as a constant
    // ratio — indent guides are visual cues, not alignment rulers.
    let char_w = font_size * 0.6;
    let guide_color = theme.text_secondary().gamma_multiply(0.18);
    for lvl in 1..=levels {
        let x = content_start_x + (lvl * 4) as f32 * char_w - char_w / 2.0;
        painter.vline(
            x,
            line_rect.top()..=line_rect.bottom(),
            egui::Stroke::new(1.0, guide_color),
        );
    }
}

/// Render a header line spanning full width in split view.
fn render_split_header(ui: &mut egui::Ui, line: &DiffLine, available_width: f32, theme: AppTheme) {
    if line.kind == DiffLineKind::HunkHeader {
        let header_height = typography::SM + 12.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, header_height),
            egui::Sense::hover(),
        );

        ui.painter().rect_filled(rect, 0.0, theme.diff_hunk_bg());

        let sep_color = theme.diff_hunk_text().gamma_multiply(0.2);
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            egui::Stroke::new(1.0, sep_color),
        );
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, sep_color),
        );

        let hidden_text = line
            .hidden_lines
            .map(|n| format!("\u{00B7}\u{00B7}\u{00B7} {n} lines hidden \u{00B7}\u{00B7}\u{00B7}"))
            .unwrap_or_else(|| "\u{00B7}\u{00B7}\u{00B7}".to_string());
        let context_text = line.hunk_context.as_deref().unwrap_or("");
        let center_y = rect.center().y;

        let hidden_galley = ui.painter().layout_no_wrap(
            hidden_text,
            typography::proportional(typography::XS),
            theme.diff_hunk_text().gamma_multiply(0.7),
        );
        let text_x = rect.left() + 16.0;
        ui.painter().galley(
            egui::pos2(text_x, center_y - hidden_galley.size().y / 2.0),
            hidden_galley.clone(),
            theme.diff_hunk_text().gamma_multiply(0.7),
        );

        if !context_text.is_empty() {
            let ctx_x = text_x + hidden_galley.size().x + 12.0;
            ui.painter().text(
                egui::pos2(ctx_x, center_y),
                egui::Align2::LEFT_CENTER,
                context_text,
                typography::monospace(typography::XS),
                theme.syntax_function().gamma_multiply(0.8),
            );
        }
    } else {
        // File header
        let line_height = typography::SM + 6.0;
        let (line_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, line_height),
            egui::Sense::hover(),
        );
        ui.painter()
            .rect_filled(line_rect, 0.0, theme.diff_file_header_bg());
        ui.painter().text(
            line_rect.left_center() + egui::vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &line.content,
            typography::monospace(typography::SM),
            theme.diff_file_header(),
        );
    }
}

/// Render a single line in split view with syntax highlighting.
#[allow(clippy::too_many_arguments)]
fn render_split_line(
    ui: &mut egui::Ui,
    line: Option<&DiffLine>,
    line_num_width: usize,
    is_left: bool,
    side_width: f32,
    theme: AppTheme,
    old_highlight: Option<&SyntaxHighlightData>,
    new_highlight: Option<&SyntaxHighlightData>,
    font_size: f32,
) {
    ui.set_max_width(side_width);
    let line_height = font_size + 6.0;

    let Some(line) = line else {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());
        let bg_rect = rect.expand2(egui::vec2(0.0, 0.5));
        ui.painter().rect_filled(
            bg_rect,
            0.0,
            theme.diff_line_number_bg().gamma_multiply(0.5),
        );
        return;
    };

    let (text_color, bg_color, gutter_color) = diff_widget::diff_line_colors(line.kind, theme);

    let gutter_width = 3.0;
    let line_num_area_width = (line_num_width + 1) as f32 * 8.0;
    let content_max_width = (side_width - gutter_width - line_num_area_width - 12.0).max(50.0);

    let (line_rect, _) =
        ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());

    // Expand rect by 0.5px vertically to cover subpixel gaps between lines
    if let Some(bg) = bg_color {
        let bg_rect = line_rect.expand2(egui::vec2(0.0, 0.5));
        ui.painter().rect_filled(bg_rect, 0.0, bg);
    }

    let mut cursor_x = line_rect.left();

    // Gutter
    if let Some(gc) = gutter_color {
        let gutter_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x, line_rect.top()),
            egui::vec2(gutter_width, line_height),
        );
        ui.painter().rect_filled(gutter_rect, 0.0, gc);
    }
    cursor_x += gutter_width + 2.0;

    // Line number
    let line_num_rect = egui::Rect::from_min_size(
        egui::pos2(cursor_x, line_rect.top()),
        egui::vec2(line_num_area_width, line_height),
    );
    ui.painter()
        .rect_filled(line_num_rect, 0.0, theme.diff_line_number_bg());

    let line_num = if is_left {
        line.old_line_num
    } else {
        line.new_line_num
    };
    let line_num_str = line_num
        .map(|n| format!("{n:>line_num_width$}"))
        .unwrap_or_else(|| " ".repeat(line_num_width));

    ui.painter().text(
        line_num_rect.left_center() + egui::vec2(2.0, 0.0),
        egui::Align2::LEFT_CENTER,
        line_num_str,
        typography::monospace(typography::SM),
        theme.diff_line_number(),
    );
    cursor_x += line_num_area_width + 4.0;

    // Content with syntax highlighting and truncation
    let content = if line.content.is_empty() {
        " ".to_string()
    } else {
        let char_width = 7.0;
        let max_chars = (content_max_width / char_width) as usize;
        let char_count = line.content.chars().count();
        if char_count > max_chars && max_chars > 3 {
            let truncate_at = max_chars.saturating_sub(1);
            let truncated: String = line.content.chars().take(truncate_at).collect();
            format!("{truncated}\u{2026}")
        } else {
            line.content.clone()
        }
    };

    let syntax_spans =
        diff_widget::get_syntax_spans_for_line(line, old_highlight, new_highlight, theme);

    let word_bg = diff::diff_word_bg(line.kind, theme);
    let job = diff_widget::build_diff_line_layout_job(
        &content,
        &line.word_highlights,
        text_color,
        word_bg,
        &syntax_spans,
        &[],
        font_size,
        theme,
    );
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    ui.painter().galley(
        egui::pos2(cursor_x, line_rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
}
