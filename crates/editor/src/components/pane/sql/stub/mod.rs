//! Stub for SQL pane - shows a message when SQL support is unavailable,
//! but can render snapshot data in read-only mode on WASM.

mod snapshot_card;
mod snapshot_plan;

use egui::RichText;

use crate::components::Component;
use crate::components::util::id_generator::next_id_usize;
use crate::ui::theme::AppTheme;

use super::SqlPaneAction;

/// Which content tab is active in an expanded cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CellTab {
    #[default]
    Table,
    Plan,
}

/// Per-cell UI state.
#[derive(Debug, Clone)]
struct CellViewState {
    expanded: bool,
    active_tab: CellTab,
    table_page: usize,
}

impl Default for CellViewState {
    fn default() -> Self {
        Self {
            expanded: false,
            active_tab: CellTab::Table,
            table_page: 0,
        }
    }
}

/// SQL pane stub for builds without SQL support.
///
/// Shows snapshot data in read-only mode when loaded from a snapshot,
/// otherwise shows a message explaining that SQL support requires the native app.
pub struct SqlPane {
    id: usize,
    theme: AppTheme,
    /// Snapshot cells loaded from snapshot data.
    snapshot_cells: Vec<enya_config::SnapshotQueryCell>,
    /// Per-cell view state.
    cell_states: Vec<CellViewState>,
    /// Currently selected cell index.
    selected_cell: Option<usize>,
    /// Whether snapshot data has been loaded.
    has_snapshot: bool,
}

impl SqlPane {
    /// Create a new SQL pane stub.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            id: next_id_usize(),
            theme,
            snapshot_cells: Vec::new(),
            cell_states: Vec::new(),
            selected_cell: None,
            has_snapshot: false,
        }
    }

    /// Take the pending action, if any (stub always returns None).
    pub fn take_action(&mut self) -> SqlPaneAction {
        SqlPaneAction::None
    }

    /// Get an inline table from query results (stub always returns None).
    pub fn get_inline_table(
        &self,
        _query: Option<&str>,
    ) -> Option<crate::components::pane::inline_content::InlineTable> {
        None
    }

    /// Extract snapshot data from the SQL pane.
    pub fn extract_snapshot_data(&self) -> Option<enya_config::SnapshotSqlPane> {
        if self.snapshot_cells.is_empty() {
            None
        } else {
            Some(enya_config::SnapshotSqlPane {
                cells: self.snapshot_cells.clone(),
            })
        }
    }

    /// Load snapshot data into the SQL pane for read-only display.
    pub fn load_snapshot_data(&mut self, data: &enya_config::SnapshotSqlPane) {
        self.snapshot_cells = data.cells.clone();
        self.cell_states = data
            .cells
            .iter()
            .map(|_| CellViewState::default())
            .collect();
        self.has_snapshot = true;

        // Auto-expand the last cell
        if let Some(last) = self.cell_states.last_mut() {
            last.expanded = true;
            self.selected_cell = Some(self.cell_states.len() - 1);
        }

        log::info!(
            "Loaded SQL snapshot data: {} cells",
            self.snapshot_cells.len()
        );
    }

    /// Show the "SQL Feature Not Available" message.
    fn show_not_available(&self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);

            ui.label(
                RichText::new(egui_nerdfonts::regular::DESKTOP)
                    .color(accent)
                    .size(48.0),
            );

            ui.add_space(16.0);

            ui.label(
                RichText::new("SQL Feature Not Available")
                    .color(text_primary)
                    .size(18.0)
                    .strong(),
            );

            ui.add_space(8.0);

            ui.label(
                RichText::new(
                    "SQL panes require the native desktop app\nbuilt with the 'sql' feature enabled.",
                )
                .color(text_secondary)
                .size(13.0),
            );

            ui.add_space(24.0);

            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 200.0) / 2.0);
                ui.label(
                    RichText::new("Download at ")
                        .color(text_secondary.gamma_multiply(0.7))
                        .size(11.0),
                );
                ui.label(RichText::new("enya.build").color(accent).size(11.0));
            });
        });
    }

    /// Show the empty snapshot state.
    fn show_empty_state(&self, ui: &mut egui::Ui) {
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.label(
                RichText::new(egui_nerdfonts::regular::CODE_BRACES)
                    .color(accent.gamma_multiply(0.5))
                    .size(48.0),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new("No SQL queries in this snapshot")
                    .color(text_secondary)
                    .size(14.0),
            );
        });
    }

    /// Render the notebook cell list from snapshot data.
    fn show_notebook(&mut self, ui: &mut egui::Ui) {
        // Centered max-width layout
        let max_width = 900.0;
        let avail_width = ui.available_width();
        let side_pad = ((avail_width - max_width) / 2.0).max(0.0);

        ui.add_space(side_pad);
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(0, 8))
            .show(ui, |ui| {
                ui.set_max_width(max_width);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Render each cell as a card
                        let mut actions: Vec<(usize, snapshot_card::CardAction)> = Vec::new();

                        for (idx, cell) in self.snapshot_cells.iter().enumerate() {
                            let state = &mut self.cell_states[idx];
                            let is_selected = self.selected_cell == Some(idx);
                            let cell_number = idx + 1;

                            let card_actions = snapshot_card::render_snapshot_card(
                                ui,
                                cell,
                                idx,
                                state,
                                self.theme,
                                is_selected,
                                cell_number,
                            );

                            for action in card_actions {
                                actions.push((idx, action));
                            }

                            ui.add_space(4.0);
                        }

                        // Apply actions
                        for (idx, action) in actions {
                            match action {
                                snapshot_card::CardAction::Select => {
                                    self.selected_cell = Some(idx);
                                }
                                snapshot_card::CardAction::Expand => {
                                    // Collapse all others
                                    for (i, s) in self.cell_states.iter_mut().enumerate() {
                                        s.expanded = i == idx;
                                    }
                                    self.selected_cell = Some(idx);
                                }
                                snapshot_card::CardAction::Collapse => {
                                    self.cell_states[idx].expanded = false;
                                }
                                snapshot_card::CardAction::SetTab(tab) => {
                                    self.cell_states[idx].active_tab = tab;
                                }
                                snapshot_card::CardAction::NextPage => {
                                    self.cell_states[idx].table_page += 1;
                                }
                                snapshot_card::CardAction::PrevPage => {
                                    if self.cell_states[idx].table_page > 0 {
                                        self.cell_states[idx].table_page -= 1;
                                    }
                                }
                            }
                        }
                    });
            });
    }
}

/// Format milliseconds for display.
pub(super) fn format_ms(ms: u64) -> String {
    if ms == 0 {
        "0µs".to_string()
    } else if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.2}s", ms as f64 / 1000.0)
    }
}

impl Component for SqlPane {
    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        "SQL".to_string()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn label(&self) -> egui::RichText {
        RichText::new("SQL")
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        if !self.has_snapshot {
            self.show_not_available(ui);
        } else if self.snapshot_cells.is_empty() {
            self.show_empty_state(ui);
        } else {
            self.show_notebook(ui);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
