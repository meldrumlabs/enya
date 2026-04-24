//! Command palette for the PR review pane.
//!
//! Provides a searchable overlay of actions that can be performed in the
//! current view (list or detail).

use super::{PrListSegment, PrReviewPane, PrReviewView};
use crate::ui::typography;

/// A command that can be executed from the command palette.
#[derive(Debug, Clone)]
pub enum PrCommand {
    /// Jump to a specific file in the detail view.
    JumpToFile { file_index: usize, path: String },
    /// Jump to a specific comment thread.
    JumpToComment { path: String, line: usize, id: u64 },
    /// Mark all files as reviewed.
    MarkAllReviewed,
    /// Open the submit review panel.
    SubmitReview,
    /// Go back to the PR list.
    BackToList,
    /// Open the current PR in GitHub.
    OpenInGithub,
    /// Refresh the current view.
    Refresh,
    /// Switch the list view inbox segment.
    SwitchSegment(PrListSegment),
}

impl PrCommand {
    /// Human-readable label shown in the palette.
    pub fn label(&self) -> String {
        match self {
            PrCommand::JumpToFile { path, .. } => format!("Open file: {path}"),
            PrCommand::JumpToComment { path, line, .. } => {
                format!("Jump to comment: {path}:{line}")
            }
            PrCommand::MarkAllReviewed => "Mark all files reviewed".to_string(),
            PrCommand::SubmitReview => "Submit review".to_string(),
            PrCommand::BackToList => "Back to list".to_string(),
            PrCommand::OpenInGithub => "Open in GitHub".to_string(),
            PrCommand::Refresh => "Refresh".to_string(),
            PrCommand::SwitchSegment(PrListSegment::NeedsReview) => {
                "Switch to Needs Review".to_string()
            }
            PrCommand::SwitchSegment(PrListSegment::MyPrs) => "Switch to My PRs".to_string(),
            PrCommand::SwitchSegment(PrListSegment::All) => "Switch to All".to_string(),
        }
    }
}

/// Build the list of available commands for the current pane state.
pub fn build_commands(pane: &PrReviewPane) -> Vec<PrCommand> {
    let mut cmds = Vec::new();

    match pane.view {
        PrReviewView::List => {
            cmds.push(PrCommand::Refresh);
            cmds.push(PrCommand::SwitchSegment(PrListSegment::NeedsReview));
            cmds.push(PrCommand::SwitchSegment(PrListSegment::MyPrs));
            cmds.push(PrCommand::SwitchSegment(PrListSegment::All));
        }
        PrReviewView::Detail => {
            cmds.push(PrCommand::BackToList);
            cmds.push(PrCommand::SubmitReview);
            cmds.push(PrCommand::MarkAllReviewed);
            cmds.push(PrCommand::OpenInGithub);

            for (idx, file_diff) in pane.file_diffs.iter().enumerate() {
                cmds.push(PrCommand::JumpToFile {
                    file_index: idx,
                    path: file_diff.path.clone(),
                });
            }

            for thread in &pane.cached_threads {
                if let Some(first) = thread.comments.first() {
                    cmds.push(PrCommand::JumpToComment {
                        path: thread.path.clone(),
                        line: thread.line,
                        id: first.id,
                    });
                }
            }
        }
    }

    cmds
}

/// Show the command palette overlay. Returns the selected command if the user
/// confirms with Enter.
pub fn show_command_palette(
    pane: &mut PrReviewPane,
    ui: &mut egui::Ui,
) -> Option<PrCommand> {
    let theme = pane.theme;
    let id = ui.id().with("pr_command_palette");

    // Full-screen semi-transparent backdrop
    let screen_rect = ui.ctx().content_rect();
    ui.painter().rect_filled(
        screen_rect,
        0.0,
        theme.bg_base().gamma_multiply(0.6),
    );

    // Centered card
    let card_width = 480.0;
    let card_pos = egui::pos2(
        screen_rect.center().x - card_width / 2.0,
        screen_rect.center().y - 150.0,
    );

    // Build and filter commands before the area so we can use them for keyboard nav
    let all_commands = build_commands(pane);
    let query = pane.command_palette_query.to_lowercase();
    let filtered: Vec<usize> = all_commands
        .iter()
        .enumerate()
        .filter(|(_, cmd)| {
            if query.is_empty() {
                true
            } else {
                cmd.label().to_lowercase().contains(&query)
            }
        })
        .map(|(i, _)| i)
        .collect();

    if pane.command_palette_selected >= filtered.len() && !filtered.is_empty() {
        pane.command_palette_selected = filtered.len().saturating_sub(1);
    }

    let mut clicked_cmd: Option<PrCommand> = None;

    egui::Area::new(id)
        .order(egui::Order::Tooltip)
        .fixed_pos(card_pos)
        .show(ui.ctx(), |ui| {
            crate::components::util::OverlayStyle::elevated_card(theme)
                .frame()
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(card_width);

                    // Text input
                    let text_edit = egui::TextEdit::singleline(&mut pane.command_palette_query)
                        .hint_text("Type a command...")
                        .desired_width(card_width - 24.0)
                        .font(typography::proportional(typography::SM));
                    let te_resp = ui.add(text_edit);
                    te_resp.request_focus();

                    ui.add_space(8.0);

                    // Render list
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for (vis_idx, &cmd_idx) in filtered.iter().enumerate() {
                                let cmd = &all_commands[cmd_idx];
                                let is_selected = vis_idx == pane.command_palette_selected;
                                let row_height = 28.0;
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), row_height),
                                    egui::Sense::click(),
                                );

                                if is_selected || response.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        4.0,
                                        if is_selected {
                                            theme.accent_primary().gamma_multiply(0.15)
                                        } else {
                                            theme.text_primary().gamma_multiply(0.04)
                                        },
                                    );
                                }

                                let galley = ui.painter().layout_no_wrap(
                                    cmd.label(),
                                    typography::proportional(typography::SM),
                                    if is_selected {
                                        theme.text_primary()
                                    } else {
                                        theme.text_secondary()
                                    },
                                );
                                ui.painter().galley(
                                    egui::pos2(
                                        rect.left() + 12.0,
                                        rect.center().y - galley.size().y / 2.0,
                                    ),
                                    galley,
                                    theme.text_primary(),
                                );

                                if response.clicked() {
                                    clicked_cmd = Some(cmd.clone());
                                }
                            }
                        });
                });
        });

    // Keyboard navigation
    ui.ctx().input(|input| {
        if input.key_pressed(egui::Key::ArrowDown) {
            let count = filtered.len();
            if count > 0 {
                pane.command_palette_selected =
                    (pane.command_palette_selected + 1).min(count - 1);
            }
        }
        if input.key_pressed(egui::Key::ArrowUp) {
            pane.command_palette_selected = pane.command_palette_selected.saturating_sub(1);
        }
        if input.key_pressed(egui::Key::Escape) {
            pane.command_palette_active = false;
            pane.command_palette_query.clear();
            pane.command_palette_selected = 0;
        }
        if input.key_pressed(egui::Key::Enter) && !filtered.is_empty() {
            pane.command_palette_execute = true;
        }
    });

    clicked_cmd
}
