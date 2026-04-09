//! Diff view for the PR review pane — renders per-file diffs with floating or inline comments.
//!
//! When the pane is wide enough (≥700px), comments float in a gutter to the right of the diff,
//! anchored to their source line but not interrupting the diff flow. For narrow panes, comments
//! render inline below the relevant diff line (the classic GitHub-style layout).

use egui::RichText;

use crate::git::api::{self, CommentThread, DraftComment, PrComment};
use crate::git::diff::DiffLine;
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::icons::APP_GHOSTTY;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::PrReviewPane;

/// Actions returned from a floating comment card.
struct FloatingCardAction {
    /// Submit a comment: (file_path, line_num, body).
    submit: Option<(String, usize, String)>,
    /// Cancel the compose input.
    cancel: bool,
    /// Start replying to a thread: (file_idx, line_idx).
    start_reply: Option<(usize, usize)>,
}

/// Minimum pane width to enable floating comment gutter.
const FLOATING_MIN_WIDTH: f32 = 700.0;

/// Width of the floating comment gutter.
const GUTTER_WIDTH: f32 = 300.0;

/// Gap between diff area and gutter.
const GUTTER_GAP: f32 = 8.0;

/// Minimum vertical gap between stacked floating cards.
const CARD_GAP: f32 = 6.0;

impl PrReviewPane {
    /// Render the diff view for the currently selected file.
    pub(super) fn show_diff_view(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;

        if self.file_diffs.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No file selected")
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::MD)),
                );
            });
            return;
        }

        let Some(file_diff) = self.file_diffs.get(self.selected_file_index) else {
            return;
        };

        // ── File path header toolbar ──────────────────────────────────────
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(&file_diff.path)
                    .color(theme.text_primary())
                    .font(typography::monospace(typography::SM)),
            );

            ui.add_space(8.0);

            let file_count = self.file_diffs.len();
            let file_index = self.selected_file_index + 1;
            ui.label(
                RichText::new(format!("{file_index}/{file_count}"))
                    .color(theme.text_secondary())
                    .font(typography::proportional(typography::XS)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let open_btn = ui.add(
                        egui::Button::image_and_text(
                            egui::Image::new(APP_GHOSTTY.as_image_source())
                                .fit_to_exact_size(egui::vec2(14.0, 14.0)),
                            RichText::new(format!(
                                "Open {}",
                                egui_nerdfonts::regular::CHEVRON_DOWN
                            ))
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                        )
                        .fill(theme.bg_elevated())
                        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                        .corner_radius(4.0),
                    );

                    if open_btn.clicked() || self.pending_open_file_opener {
                        self.pending_open_file_opener = false;
                        let popup_pos = open_btn.rect.left_bottom();
                        let file_path = if let Some(root) = &self.repo_root {
                            root.join(&file_diff.path)
                        } else {
                            std::path::PathBuf::from(&file_diff.path)
                        };
                        self.file_opener.open_with_base(
                            popup_pos,
                            file_path,
                            self.repo_root.clone(),
                        );
                    }
                }

                ui.add_space(4.0);

                let copy_btn = ui.add(
                    egui::Button::new(
                        RichText::new(egui_nerdfonts::regular::COPY)
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(theme.bg_elevated())
                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                    .corner_radius(4.0),
                );
                if copy_btn.clicked() {
                    let contents: String = file_diff
                        .lines
                        .iter()
                        .filter(|l| {
                            matches!(
                                l.kind,
                                crate::git::diff::DiffLineKind::Context
                                    | crate::git::diff::DiffLineKind::Addition
                            )
                        })
                        .map(|l| l.content.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(contents);
                }
                copy_btn.on_hover_text("Copy file contents");

                ui.add_space(4.0);

                let split_label = if self.diff_renderer.split_view() {
                    "Stacked"
                } else {
                    "Split"
                };
                let split_btn = ui.add(
                    egui::Button::new(
                        RichText::new(split_label)
                            .size(typography::SM)
                            .color(theme.text_secondary()),
                    )
                    .fill(theme.bg_elevated())
                    .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
                    .corner_radius(4.0),
                );
                if split_btn.clicked() {
                    self.diff_renderer.toggle_split_view();
                }
            });
        });
        ui.add_space(4.0);

        // Separator
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, theme.border_subtle()),
        );

        // Search bar (when active)
        if self.diff_renderer.search_active() {
            if let Some(new_file) = self.diff_renderer.render_search_bar(
                ui,
                theme,
                &self.file_diffs,
                self.selected_file_index,
            ) {
                self.selected_file_index = new_file;
                self.mark_current_file_comments_seen();
            }
        }

        // ── Decide floating vs inline comments ──────────────────────────
        let file_diff = self.file_diffs[self.selected_file_index].clone();
        let file_idx = self.selected_file_index;

        let file_threads: Vec<_> = self
            .cached_threads
            .iter()
            .filter(|t| t.path == file_diff.path)
            .cloned()
            .collect();

        let file_drafts: Vec<_> = self
            .draft_comments
            .iter()
            .filter(|d| d.path == file_diff.path)
            .cloned()
            .collect();

        let available_width = ui.available_width();
        let has_any_comments =
            !file_threads.is_empty() || !file_drafts.is_empty() || self.commenting_line.is_some();
        let use_floating =
            available_width >= FLOATING_MIN_WIDTH && !self.diff_renderer.split_view();

        if use_floating && has_any_comments {
            self.show_diff_with_floating_comments(
                ui,
                &file_diff,
                file_idx,
                &file_threads,
                &file_drafts,
            );
        } else if use_floating {
            // Wide enough but no comments — render diff without any callback, full width
            self.diff_renderer
                .render_diff(ui, &file_diff, file_idx, theme, None);
            self.process_diff_actions(file_idx);
        } else {
            // Narrow pane — inline comments (original behavior)
            self.show_diff_with_inline_comments(ui, &file_diff, file_idx, &file_threads);
        }
    }

    /// Floating comments: render diff on the left, comment cards in a gutter on the right.
    /// Uses the same `ui.horizontal` + `allocate_ui_with_layout` pattern as the file-tree/diff
    /// split in `show_files_tab`, ensuring both columns share the same height.
    fn show_diff_with_floating_comments(
        &mut self,
        ui: &mut egui::Ui,
        file_diff: &crate::git::diff::FileDiff,
        file_idx: usize,
        file_threads: &[CommentThread],
        file_drafts: &[DraftComment],
    ) {
        let theme = self.theme;
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let gutter_width = GUTTER_WIDTH.min((available_width * 0.35).max(200.0));
        let diff_width = (available_width - gutter_width - GUTTER_GAP).max(300.0);

        // Pre-collect items and commenting state before the horizontal split
        // (we need these for both columns but &mut self is borrowed in the closure).
        let commenting_line = self.commenting_line;
        let file_path = file_diff.path.clone();

        ui.horizontal(|ui| {
            // ── Left column: diff ────────────────────────────────────────
            ui.allocate_ui_with_layout(
                egui::vec2(diff_width, available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.diff_renderer
                        .render_diff(ui, file_diff, file_idx, theme, None);
                },
            );

            // Capture line positions after the diff renders
            let line_y_positions: Vec<(usize, usize, f32)> =
                self.diff_renderer.line_y_positions().to_vec();
            let content_origin = self.diff_renderer.last_content_origin();
            let scroll_y = self.diff_renderer.scroll_offset_y();
            let line_height = self.diff_renderer.line_height();

            // Vertical separator
            let sep_rect = ui.available_rect_before_wrap();
            ui.painter().vline(
                sep_rect.left(),
                sep_rect.y_range(),
                egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.5)),
            );

            // ── Right column: floating comment gutter ────────────────────
            // Compute the Y offset between the gutter column top and the diff's
            // scroll-area viewport so cards line up with visible diff lines.
            ui.allocate_ui_with_layout(
                egui::vec2(gutter_width, available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    let items = build_floating_items(
                        file_threads,
                        file_drafts,
                        commenting_line,
                        file_idx,
                        &line_y_positions,
                    );

                    // The gutter column top in screen coords.
                    let gutter_top = ui.max_rect().top();
                    // Offset: how far below gutter_top the diff's visible content begins.
                    let y_offset = content_origin.y - gutter_top;

                    let cards = layout_floating_cards(
                        &items,
                        &line_y_positions,
                        scroll_y,
                        content_origin,
                        content_origin.y,
                        &self.floating_card_heights,
                    );

                    let accent = theme.accent_primary();
                    let gutter_left_edge = ui.max_rect().left();
                    let gutter_bottom = ui.max_rect().bottom();
                    let card_content_width = (gutter_width - 14.0).max(100.0);
                    let card_left = gutter_left_edge + 8.0;

                    // Clip all gutter painting to the column rect so nothing
                    // bleeds below into the keyboard-shortcuts bar.
                    let clip_rect = ui.max_rect();
                    let clipped_painter = ui.painter().with_clip_rect(clip_rect);

                    let mut pending_submit: Option<(String, usize, String)> = None;
                    let mut pending_cancel = false;
                    let mut pending_reply: Option<(usize, usize)> = None;

                    for (i, item) in items.iter().enumerate() {
                        let card = &cards[i];
                        let card_y = gutter_top + y_offset + card.actual_y;

                        // Skip cards fully outside the visible gutter area
                        if card_y > gutter_bottom || card_y + 200.0 < gutter_top {
                            continue;
                        }

                        // Max height the card can use before hitting the bottom
                        let max_card_h = (gutter_bottom - card_y - 4.0).max(20.0);

                        // ── Anchor connector ──
                        let ideal_screen_y = gutter_top + y_offset + card.ideal_y;
                        if (card.actual_y - card.ideal_y).abs() > 4.0 {
                            let anchor_y = ideal_screen_y + line_height / 2.0;
                            let card_center_y = card_y + 12.0;
                            clipped_painter.hline(
                                (gutter_left_edge - 4.0)..=(gutter_left_edge + 6.0),
                                anchor_y,
                                egui::Stroke::new(1.0, accent.gamma_multiply(0.15)),
                            );
                            let (top, bottom) = if anchor_y < card_center_y {
                                (anchor_y, card_center_y)
                            } else {
                                (card_center_y, anchor_y)
                            };
                            clipped_painter.vline(
                                gutter_left_edge + 6.0,
                                top..=bottom,
                                egui::Stroke::new(1.0, accent.gamma_multiply(0.15)),
                            );
                            clipped_painter.hline(
                                (gutter_left_edge + 6.0)..=card_left,
                                card_center_y,
                                egui::Stroke::new(1.0, accent.gamma_multiply(0.15)),
                            );
                        } else {
                            let anchor_y = card_y + line_height / 2.0;
                            clipped_painter.hline(
                                (gutter_left_edge - 4.0)..=card_left,
                                anchor_y,
                                egui::Stroke::new(1.0, accent.gamma_multiply(0.12)),
                            );
                        }

                        // ── Card content (height-clamped to gutter bottom) ──
                        let accent_shape_idx = clipped_painter.add(egui::Shape::Noop);

                        let card_rect = egui::Rect::from_min_size(
                            egui::pos2(card_left + 6.0, card_y + 2.0),
                            egui::vec2(card_content_width - 14.0, max_card_h),
                        );

                        let mut card_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(card_rect)
                                .layout(egui::Layout::top_down(egui::Align::LEFT)),
                        );
                        card_ui.set_clip_rect(clip_rect);

                        let card_start_y = card_ui.cursor().top();

                        let card_action = render_floating_card(
                            &mut card_ui,
                            theme,
                            item.thread.as_ref(),
                            &item.drafts,
                            item.is_composing,
                            card.line_num,
                            card.line_idx,
                            file_idx,
                            &file_path,
                            &mut self.comment_input,
                            &mut self.collapsed_threads,
                            &self.avatar_textures,
                        );

                        if card_action.submit.is_some() {
                            pending_submit = card_action.submit;
                        }
                        if card_action.cancel {
                            pending_cancel = true;
                        }
                        if card_action.start_reply.is_some() {
                            pending_reply = card_action.start_reply;
                        }

                        let card_end_y = card_ui.cursor().top();
                        let actual_card_height =
                            (card_end_y - card_start_y).max(20.0).min(max_card_h);

                        // Record measured height so next frame's layout avoids overlap
                        self.floating_card_heights
                            .insert(card.line_num, actual_card_height);

                        // Thin left accent bar only
                        let accent_bar = egui::Rect::from_min_size(
                            egui::pos2(card_left, card_y),
                            egui::vec2(3.0, actual_card_height + 4.0),
                        );
                        clipped_painter.set(
                            accent_shape_idx,
                            egui::Shape::rect_filled(accent_bar, 2.0, accent.gamma_multiply(0.4)),
                        );
                    }

                    // Process deferred floating card actions
                    if let Some((path, line, body)) = pending_submit {
                        self.post_single_comment(path, line, body);
                        self.comment_input.clear();
                        self.commenting_line = None;
                    }
                    if pending_cancel {
                        self.comment_input.clear();
                        self.commenting_line = None;
                    }
                    if let Some((fi, li)) = pending_reply {
                        self.commenting_line = Some((fi, li));
                    }
                },
            );
        });

        self.process_diff_actions(file_idx);
    }

    /// Inline comments: original behavior for narrow panes.
    fn show_diff_with_inline_comments(
        &mut self,
        ui: &mut egui::Ui,
        file_diff: &crate::git::diff::FileDiff,
        file_idx: usize,
        file_threads: &[CommentThread],
    ) {
        let theme = self.theme;
        let draft_comments = &self.draft_comments;
        let commenting_line = self.commenting_line;
        let comment_input = &mut self.comment_input;
        let collapsed_threads = &mut self.collapsed_threads;
        let avatar_textures = &self.avatar_textures;
        let mut pending_add_comment: Option<(String, usize, String)> = None;
        let mut clear_commenting = false;
        let mut pending_start_reply: Option<(usize, usize)> = None;

        self.diff_renderer.render_diff(
            ui,
            file_diff,
            file_idx,
            theme,
            Some(&mut |ui, line_idx, line: &DiffLine| {
                if let Some(new_line) = line.new_line_num {
                    render_inline_comments(
                        ui,
                        &file_diff.path,
                        new_line,
                        line_idx,
                        file_idx,
                        theme,
                        file_threads,
                        draft_comments,
                        commenting_line,
                        comment_input,
                        collapsed_threads,
                        &mut pending_add_comment,
                        &mut clear_commenting,
                        &mut pending_start_reply,
                        avatar_textures,
                    );
                }
            }),
        );

        // Process deferred comment actions
        if let Some((path, line, body)) = pending_add_comment {
            self.post_single_comment(path, line, body);
            self.comment_input.clear();
            self.commenting_line = None;
        }
        if clear_commenting {
            self.comment_input.clear();
            self.commenting_line = None;
        }

        if let Some((fi, li)) = pending_start_reply {
            self.commenting_line = Some((fi, li));
        }

        self.process_diff_actions(file_idx);
    }

    /// Process pending actions from the diff renderer (comment clicks, hunk expansion).
    fn process_diff_actions(&mut self, file_idx: usize) {
        // Process "+" comment button clicks
        if let Some((_file_idx, line_idx)) = self.diff_renderer.take_pending_comment() {
            self.commenting_line = Some((file_idx, line_idx));
        }

        // Process hunk expansion
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(hunk_idx) = self.diff_renderer.take_pending_expand() {
            if let Some(file_diff) = self.file_diffs.get_mut(self.selected_file_index) {
                self.diff_renderer.expand_context(file_diff, hunk_idx);
            }
        }
    }
}

// =============================================================================
// Floating comment helpers
// =============================================================================

/// An item to be rendered as a floating comment card in the gutter.
struct FloatingItem {
    line_num: usize,
    line_idx: usize,
    thread: Option<CommentThread>,
    drafts: Vec<DraftComment>,
    is_composing: bool,
}

/// Resolved Y layout for a floating card.
struct LayoutCard {
    ideal_y: f32,
    actual_y: f32,
    line_num: usize,
    line_idx: usize,
}

/// Collect all comment items that need floating cards for this file.
fn build_floating_items(
    file_threads: &[CommentThread],
    file_drafts: &[DraftComment],
    commenting_line: Option<(usize, usize)>,
    file_idx: usize,
    line_y_positions: &[(usize, usize, f32)],
) -> Vec<FloatingItem> {
    let mut items: Vec<FloatingItem> = Vec::new();
    let mut seen_lines: rustc_hash::FxHashSet<usize> = rustc_hash::FxHashSet::default();

    let composing_line_num = commenting_line.and_then(|(fi, li)| {
        if fi == file_idx {
            line_y_positions
                .iter()
                .find(|(idx, _, _)| *idx == li)
                .map(|(_, ln, _)| *ln)
        } else {
            None
        }
    });

    for thread in file_threads {
        seen_lines.insert(thread.line);
        let drafts: Vec<_> = file_drafts
            .iter()
            .filter(|d| d.line == thread.line)
            .cloned()
            .collect();
        items.push(FloatingItem {
            line_num: thread.line,
            line_idx: line_y_positions
                .iter()
                .find(|(_, ln, _)| *ln == thread.line)
                .map(|(idx, _, _)| *idx)
                .unwrap_or(0),
            thread: Some(thread.clone()),
            drafts,
            is_composing: composing_line_num == Some(thread.line),
        });
    }

    for draft in file_drafts {
        if !seen_lines.contains(&draft.line) {
            seen_lines.insert(draft.line);
            items.push(FloatingItem {
                line_num: draft.line,
                line_idx: line_y_positions
                    .iter()
                    .find(|(_, ln, _)| *ln == draft.line)
                    .map(|(idx, _, _)| *idx)
                    .unwrap_or(0),
                thread: None,
                drafts: vec![draft.clone()],
                is_composing: composing_line_num == Some(draft.line),
            });
        }
    }

    // Compose-only (new comment on a line with no existing thread/draft)
    if let Some((fi, li)) = commenting_line {
        if fi == file_idx {
            if let Some((_, ln, _)) = line_y_positions.iter().find(|(idx, _, _)| *idx == li) {
                if !seen_lines.contains(ln) {
                    items.push(FloatingItem {
                        line_num: *ln,
                        line_idx: li,
                        thread: None,
                        drafts: Vec::new(),
                        is_composing: true,
                    });
                }
            }
        }
    }

    items.sort_by_key(|item| item.line_num);
    items
}

/// Resolve Y positions for floating cards with collision avoidance.
///
/// Uses `last_actual_heights` from the previous frame so cards don't overlap.
/// On the first frame the estimates are generous; subsequent frames use measured values.
fn layout_floating_cards(
    items: &[FloatingItem],
    line_y_positions: &[(usize, usize, f32)],
    scroll_y: f32,
    content_origin: egui::Pos2,
    gutter_top: f32,
    last_actual_heights: &rustc_hash::FxHashMap<usize, f32>,
) -> Vec<LayoutCard> {
    let mut cards: Vec<LayoutCard> = Vec::new();
    let mut prev_bottom: f32 = 0.0;

    for item in items {
        let ideal_y = line_y_positions
            .iter()
            .find(|(_, ln, _)| *ln == item.line_num)
            .map(|(_, _, y)| *y - scroll_y + content_origin.y - gutter_top)
            .unwrap_or(0.0);

        // Use the measured height from the previous frame if available,
        // otherwise use a generous estimate to avoid overlap.
        let estimated_height = if let Some(&h) = last_actual_heights.get(&item.line_num) {
            h + 8.0 // small padding
        } else {
            let comment_count = item.thread.as_ref().map(|t| t.comments.len()).unwrap_or(0);
            let draft_count = item.drafts.len();
            let compose_height = if item.is_composing { 100.0 } else { 0.0 };
            // Generous: ~90px per comment (author line + wrapped body), 60px per draft
            (comment_count as f32 * 90.0) + (draft_count as f32 * 60.0) + compose_height + 30.0 // line badge + reply button + padding
        };

        let actual_y = ideal_y.max(prev_bottom + CARD_GAP);
        prev_bottom = actual_y + estimated_height;

        cards.push(LayoutCard {
            ideal_y,
            actual_y,
            line_num: item.line_num,
            line_idx: item.line_idx,
        });
    }

    cards
}

// =============================================================================
// Floating comment card rendering
// =============================================================================

/// Render a floating comment card (thread + drafts + compose) inside a gutter child UI.
/// Returns actions that the caller must process (submit, cancel, reply).
#[allow(clippy::too_many_arguments)]
fn render_floating_card(
    ui: &mut egui::Ui,
    theme: AppTheme,
    thread: Option<&CommentThread>,
    drafts: &[DraftComment],
    is_composing: bool,
    line_num: usize,
    line_idx: usize,
    file_idx: usize,
    file_path: &str,
    comment_input: &mut String,
    collapsed_threads: &mut rustc_hash::FxHashSet<(String, usize)>,
    avatar_textures: &rustc_hash::FxHashMap<String, egui::TextureHandle>,
) -> FloatingCardAction {
    let accent = theme.accent_primary();
    let thread_key = (file_path.to_string(), line_num);
    let card_width = ui.available_width();

    let mut action = FloatingCardAction {
        submit: None,
        cancel: false,
        start_reply: None,
    };

    ui.set_max_width(card_width);

    // ── Line number badge ──
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("L{line_num}"))
                .color(theme.text_secondary().gamma_multiply(0.6))
                .font(typography::monospace(typography::XS)),
        );
    });
    ui.add_space(2.0);

    // ── Thread comments ──
    if let Some(thread) = thread {
        let comments = &thread.comments;
        let is_collapsed = collapsed_threads.contains(&thread_key);
        let collapse_threshold = 3;
        let should_collapse = comments.len() > collapse_threshold;

        let visible_comments = if should_collapse && is_collapsed {
            &comments[..1]
        } else {
            comments
        };

        for (i, comment) in visible_comments.iter().enumerate() {
            if i > 0 {
                ui.add_space(2.0);
                let rect = ui.available_rect_before_wrap();
                ui.painter().hline(
                    rect.left()..=(rect.right()),
                    rect.top(),
                    egui::Stroke::new(0.5, theme.border_subtle()),
                );
                ui.add_space(2.0);
            }
            render_floating_comment_body(ui, theme, comment, avatar_textures);
        }

        if should_collapse {
            ui.add_space(2.0);
            let hidden = comments.len() - 1;
            let label = if is_collapsed {
                format!("Show {hidden} more")
            } else {
                "Collapse".to_string()
            };
            let btn = ui.add(
                egui::Button::new(RichText::new(label).size(typography::XS).color(accent))
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE),
            );
            if btn.clicked() {
                if is_collapsed {
                    collapsed_threads.remove(&thread_key);
                } else {
                    collapsed_threads.insert(thread_key.clone());
                }
            }
        }
    }

    // ── Draft comments ──
    for draft in drafts {
        if thread.is_some() || drafts.len() > 1 {
            ui.add_space(2.0);
            let rect = ui.available_rect_before_wrap();
            ui.painter().hline(
                rect.left()..=(rect.right()),
                rect.top(),
                egui::Stroke::new(0.5, accent.gamma_multiply(0.3)),
            );
            ui.add_space(2.0);
        }

        egui::Frame::new()
            .fill(accent.gamma_multiply(0.05))
            .corner_radius(3.0)
            .inner_margin(egui::Margin::symmetric(6, 4))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Draft")
                        .color(accent)
                        .font(typography::proportional(typography::XS))
                        .strong(),
                );
                ui.add_space(1.0);
                ui.label(
                    RichText::new(&draft.body)
                        .color(theme.text_primary().gamma_multiply(0.9))
                        .font(typography::proportional(typography::XS)),
                );
            });
    }

    // ── Compose input ──
    if is_composing {
        if thread.is_some() || !drafts.is_empty() {
            ui.add_space(4.0);
        }

        let response = ui.add(
            egui::TextEdit::multiline(comment_input)
                .hint_text(if thread.is_some() {
                    "Reply..."
                } else {
                    "Add a comment..."
                })
                .desired_rows(2)
                .desired_width(ui.available_width())
                .font(typography::proportional(typography::XS)),
        );

        if response.gained_focus() || comment_input.is_empty() {
            response.request_focus();
        }

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let submit_label = if thread.is_some() { "Reply" } else { "Comment" };
            let submit_btn = ui.add(
                egui::Button::new(
                    RichText::new(submit_label)
                        .size(typography::XS)
                        .color(theme.text_primary()),
                )
                .fill(accent.gamma_multiply(0.2))
                .corner_radius(3.0),
            );
            if submit_btn.clicked() && !comment_input.is_empty() {
                action.submit = Some((file_path.to_string(), line_num, comment_input.clone()));
            }

            let cancel_btn = ui.add(
                egui::Button::new(
                    RichText::new("Cancel")
                        .size(typography::XS)
                        .color(theme.text_secondary()),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            );
            if cancel_btn.clicked() {
                action.cancel = true;
            }

            ui.label(
                RichText::new("\u{2318}\u{23CE}")
                    .color(theme.text_secondary().gamma_multiply(0.4))
                    .font(typography::proportional(typography::XS)),
            );
        });
    } else if thread.is_some() {
        // Reply button
        ui.add_space(2.0);
        let reply_btn = ui.add(
            egui::Button::new(RichText::new("Reply").size(typography::XS).color(accent))
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
        );
        if reply_btn.clicked() {
            action.start_reply = Some((file_idx, line_idx));
        }
    }

    action
}

/// Render a single comment body inside a floating card (compact layout).
fn render_floating_comment_body(
    ui: &mut egui::Ui,
    theme: AppTheme,
    comment: &PrComment,
    avatar_textures: &rustc_hash::FxHashMap<String, egui::TextureHandle>,
) {
    // Author line: avatar + name + time
    ui.horizontal(|ui| {
        let avatar_size = 14.0;
        let (avatar_rect, _) =
            ui.allocate_exact_size(egui::vec2(avatar_size, avatar_size), egui::Sense::hover());
        let center = avatar_rect.center();
        let radius = avatar_size / 2.0;

        if let Some(texture) = avatar_textures.get(&comment.user.login) {
            let mut mesh = egui::Mesh::with_texture(texture.id());
            let segments = 20;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: center,
                uv: egui::pos2(0.5, 0.5),
                color: egui::Color32::WHITE,
            });
            for i in 0..=segments {
                let angle = std::f32::consts::TAU * i as f32 / segments as f32;
                let (sin, cos) = angle.sin_cos();
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: center + egui::vec2(cos * radius, sin * radius),
                    uv: egui::pos2(0.5 + cos * 0.5, 0.5 + sin * 0.5),
                    color: egui::Color32::WHITE,
                });
                if i > 0 {
                    mesh.indices.push(0);
                    mesh.indices.push(i);
                    mesh.indices.push(i + 1);
                }
            }
            ui.painter().add(egui::Shape::mesh(mesh));
        } else {
            let letter = comment
                .user
                .login
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            ui.painter()
                .circle_filled(center, radius, theme.accent_primary().gamma_multiply(0.2));
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                &letter,
                typography::proportional(7.0),
                theme.accent_primary(),
            );
        }

        ui.label(
            RichText::new(&comment.user.login)
                .color(theme.text_primary())
                .font(typography::proportional(typography::XS))
                .strong(),
        );
        ui.label(
            RichText::new(api::relative_time(&comment.created_at))
                .color(theme.text_secondary())
                .font(typography::proportional(typography::XS)),
        );
    });

    ui.add_space(1.0);

    // Comment body — compact, wrapping text
    crate::components::overlay::markdown_renderer::render_markdown(ui, &comment.body, theme);
}

// =============================================================================
// Inline comments (fallback for narrow panes)
// =============================================================================

/// Render threaded inline comments for a specific line (standalone function for borrow splitting).
#[allow(clippy::too_many_arguments)]
fn render_inline_comments(
    ui: &mut egui::Ui,
    file_path: &str,
    line_num: usize,
    line_idx: usize,
    file_idx: usize,
    theme: AppTheme,
    file_threads: &[CommentThread],
    draft_comments: &[DraftComment],
    commenting_line: Option<(usize, usize)>,
    comment_input: &mut String,
    collapsed_threads: &mut rustc_hash::FxHashSet<(String, usize)>,
    pending_add_comment: &mut Option<(String, usize, String)>,
    clear_commenting: &mut bool,
    pending_start_reply: &mut Option<(usize, usize)>,
    avatar_textures: &rustc_hash::FxHashMap<String, egui::TextureHandle>,
) {
    let thread = file_threads.iter().find(|t| t.line == line_num);

    let drafts: Vec<_> = draft_comments
        .iter()
        .filter(|c| c.path == file_path && c.line == line_num)
        .collect();

    let is_commenting = commenting_line == Some((file_idx, line_idx));
    let has_content = thread.is_some() || !drafts.is_empty() || is_commenting;

    if !has_content {
        return;
    }

    let thread_key = (file_path.to_string(), line_num);
    let accent = theme.accent_primary();

    ui.add_space(2.0);

    egui::Frame::new()
        .fill(theme.bg_elevated())
        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(0))
        .outer_margin(egui::Margin {
            left: 40,
            right: 8,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            let card_rect = ui.max_rect();
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    card_rect.left_top(),
                    egui::vec2(3.0, card_rect.height().max(1.0)),
                ),
                egui::CornerRadius {
                    nw: 4,
                    sw: 4,
                    ..Default::default()
                },
                accent.gamma_multiply(0.5),
            );

            ui.add_space(4.0);

            if let Some(thread) = thread {
                let comments = &thread.comments;
                let is_collapsed = collapsed_threads.contains(&thread_key);
                let collapse_threshold = 3;
                let should_collapse = comments.len() > collapse_threshold;

                let visible_comments = if should_collapse && is_collapsed {
                    &comments[..1]
                } else {
                    comments
                };

                for (i, comment) in visible_comments.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(2.0);
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().hline(
                            (rect.left() + 12.0)..=(rect.right() - 8.0),
                            rect.top(),
                            egui::Stroke::new(0.5, theme.border_subtle()),
                        );
                        ui.add_space(2.0);
                    }

                    render_inline_comment_body(ui, theme, comment, avatar_textures);
                }

                if should_collapse {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let hidden = comments.len() - 1;
                        let label = if is_collapsed {
                            format!("Show {hidden} more replies")
                        } else {
                            "Collapse".to_string()
                        };
                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(label).size(typography::XS).color(accent),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        );
                        if btn.clicked() {
                            if is_collapsed {
                                collapsed_threads.remove(&thread_key);
                            } else {
                                collapsed_threads.insert(thread_key.clone());
                            }
                        }
                    });
                }
            }

            for draft in &drafts {
                if thread.is_some() {
                    ui.add_space(2.0);
                    let rect = ui.available_rect_before_wrap();
                    ui.painter().hline(
                        (rect.left() + 12.0)..=(rect.right() - 8.0),
                        rect.top(),
                        egui::Stroke::new(0.5, accent.gamma_multiply(0.3)),
                    );
                    ui.add_space(2.0);
                }

                egui::Frame::new()
                    .fill(accent.gamma_multiply(0.05))
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 8,
                        top: 6,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Draft")
                                    .color(accent)
                                    .font(typography::proportional(typography::XS))
                                    .strong(),
                            );
                        });
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(&draft.body)
                                .color(theme.text_primary().gamma_multiply(0.9))
                                .font(typography::proportional(typography::XS)),
                        );
                    });
            }

            if is_commenting {
                if thread.is_some() || !drafts.is_empty() {
                    ui.add_space(2.0);
                    let rect = ui.available_rect_before_wrap();
                    ui.painter().hline(
                        (rect.left() + 12.0)..=(rect.right() - 8.0),
                        rect.top(),
                        egui::Stroke::new(0.5, accent.gamma_multiply(0.3)),
                    );
                    ui.add_space(2.0);
                }

                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 8,
                        top: 6,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        let response = ui.add(
                            egui::TextEdit::multiline(comment_input)
                                .hint_text(if thread.is_some() {
                                    "Reply..."
                                } else {
                                    "Add a comment..."
                                })
                                .desired_rows(2)
                                .desired_width(ui.available_width())
                                .font(typography::proportional(typography::SM)),
                        );

                        if response.gained_focus() || comment_input.is_empty() {
                            response.request_focus();
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let submit_label = if thread.is_some() {
                                "Reply"
                            } else {
                                "Add comment"
                            };
                            let submit_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(submit_label)
                                        .size(typography::XS)
                                        .color(theme.text_primary()),
                                )
                                .fill(accent.gamma_multiply(0.2))
                                .corner_radius(3.0),
                            );
                            if submit_btn.clicked() && !comment_input.is_empty() {
                                *pending_add_comment =
                                    Some((file_path.to_string(), line_num, comment_input.clone()));
                            }

                            let cancel_btn = ui.add(
                                egui::Button::new(
                                    RichText::new("Cancel")
                                        .size(typography::XS)
                                        .color(theme.text_secondary()),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE),
                            );
                            if cancel_btn.clicked() {
                                *clear_commenting = true;
                            }

                            ui.label(
                                RichText::new("\u{2318}\u{23CE} submit \u{2022} Esc cancel")
                                    .color(theme.text_secondary().gamma_multiply(0.5))
                                    .font(typography::proportional(typography::XS)),
                            );
                        });
                    });
            } else if thread.is_some() {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    let reply_btn = ui.add(
                        egui::Button::new(
                            RichText::new("Reply").size(typography::XS).color(accent),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE),
                    );
                    if reply_btn.clicked() {
                        *pending_start_reply = Some((file_idx, line_idx));
                    }
                });
            }

            ui.add_space(4.0);
        });

    ui.add_space(2.0);
}

/// Render a single comment within an inline thread card.
fn render_inline_comment_body(
    ui: &mut egui::Ui,
    theme: AppTheme,
    comment: &PrComment,
    avatar_textures: &rustc_hash::FxHashMap<String, egui::TextureHandle>,
) {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: 12,
            right: 8,
            top: 6,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let avatar_size = 16.0;
                let (avatar_rect, _) = ui.allocate_exact_size(
                    egui::vec2(avatar_size, avatar_size),
                    egui::Sense::hover(),
                );
                let center = avatar_rect.center();
                let radius = avatar_size / 2.0;

                if let Some(texture) = avatar_textures.get(&comment.user.login) {
                    let mut mesh = egui::Mesh::with_texture(texture.id());
                    let segments = 24;
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: center,
                        uv: egui::pos2(0.5, 0.5),
                        color: egui::Color32::WHITE,
                    });
                    for i in 0..=segments {
                        let angle = std::f32::consts::TAU * i as f32 / segments as f32;
                        let (sin, cos) = angle.sin_cos();
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: center + egui::vec2(cos * radius, sin * radius),
                            uv: egui::pos2(0.5 + cos * 0.5, 0.5 + sin * 0.5),
                            color: egui::Color32::WHITE,
                        });
                        if i > 0 {
                            mesh.indices.push(0);
                            mesh.indices.push(i);
                            mesh.indices.push(i + 1);
                        }
                    }
                    ui.painter().add(egui::Shape::mesh(mesh));
                } else {
                    let letter = comment
                        .user
                        .login
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string();
                    ui.painter().circle_filled(
                        center,
                        radius,
                        theme.accent_primary().gamma_multiply(0.2),
                    );
                    ui.painter().text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        &letter,
                        typography::proportional(8.0),
                        theme.accent_primary(),
                    );
                }

                ui.label(
                    RichText::new(&comment.user.login)
                        .color(theme.text_primary())
                        .font(typography::proportional(typography::XS))
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(api::relative_time(&comment.created_at))
                        .color(theme.text_secondary())
                        .font(typography::proportional(typography::XS)),
                );
            });
            ui.add_space(2.0);
            crate::components::overlay::markdown_renderer::render_markdown(
                ui,
                &comment.body,
                theme,
            );
        });
}
