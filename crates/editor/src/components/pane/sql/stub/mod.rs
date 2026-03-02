//! Stub for SQL pane - shows a message when SQL support is unavailable,
//! but can render snapshot data in read-only mode on WASM.

mod snapshot_card;
mod snapshot_plan;

use egui::RichText;

use crate::components::Component;
use crate::components::util::id_generator::next_id_usize;
use crate::ui::theme::AppTheme;

use super::SqlPaneAction;

/// Per-cell UI state.
#[derive(Debug, Clone, Default)]
struct CellViewState {
    table_page: usize,
}

/// SQL pane stub for builds without SQL support.
///
/// Shows snapshot data in read-only mode when loaded from a snapshot,
/// otherwise shows a message explaining that SQL support requires the native app.
pub struct SqlPane {
    id: usize,
    theme: AppTheme,
    /// The single snapshot cell (if loaded from snapshot).
    snapshot_cell: Option<enya_config::SnapshotQueryCell>,
    /// View state for the snapshot cell.
    cell_view_state: CellViewState,
    /// Whether snapshot data has been loaded.
    has_snapshot: bool,
}

impl SqlPane {
    /// Create a new SQL pane stub.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            id: next_id_usize(),
            theme,
            snapshot_cell: None,
            cell_view_state: CellViewState::default(),
            has_snapshot: false,
        }
    }

    /// Take the pending action, if any (stub always returns None).
    pub fn take_action(&mut self) -> SqlPaneAction {
        SqlPaneAction::None
    }

    /// Synchronize connections from Settings (no-op on WASM stub).
    pub fn sync_connections(
        &mut self,
        _definitions: &[crate::ui::settings_screen::FlightSqlConnection],
    ) {
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
        self.snapshot_cell
            .as_ref()
            .map(|cell| enya_config::SnapshotSqlPane {
                cells: vec![cell.clone()],
            })
    }

    /// Load snapshot data into the SQL pane for read-only display.
    pub fn load_snapshot_data(&mut self, data: &enya_config::SnapshotSqlPane) {
        // Take the last non-info cell from the snapshot
        self.snapshot_cell = data
            .cells
            .iter()
            .rev()
            .find(|c| c.kind != enya_config::SnapshotCellKind::Info)
            .cloned();
        self.cell_view_state = CellViewState::default();
        self.has_snapshot = true;

        log::info!(
            "Loaded SQL snapshot data: {} cells (showing last result)",
            data.cells.len()
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

    /// Render the single snapshot cell.
    fn show_notebook(&mut self, ui: &mut egui::Ui) {
        let avail_width = ui.available_width();
        let max_width = avail_width.min(900.0);

        ui.allocate_ui_with_layout(
            egui::vec2(avail_width, ui.available_height()),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.set_max_width(max_width);
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(cell) = &self.snapshot_cell {
                            let state = &mut self.cell_view_state;
                            let card_actions =
                                snapshot_card::render_snapshot_card(ui, cell, 0, state, self.theme);

                            for action in card_actions {
                                match action {
                                    snapshot_card::CardAction::Collapse => {}
                                    snapshot_card::CardAction::NextPage => {
                                        self.cell_view_state.table_page += 1;
                                    }
                                    snapshot_card::CardAction::PrevPage => {
                                        if self.cell_view_state.table_page > 0 {
                                            self.cell_view_state.table_page -= 1;
                                        }
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
        } else if self.snapshot_cell.is_none() {
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
