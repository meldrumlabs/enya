//! Stub for SQL pane - shows a message when SQL support is unavailable.
//!
//! This stub is used when:
//! - Running in WASM (browsers don't support Flight SQL)
//! - The `sql` feature is disabled (to reduce dependencies)

use egui::RichText;

use crate::components::Component;
use crate::components::util::id_generator::next_id_usize;
use crate::ui::theme::AppTheme;

/// Action returned by the SQL pane (stub version has no actions).
#[derive(Debug, Clone, PartialEq)]
pub enum SqlPaneAction {
    None,
}

/// SQL pane stub for builds without SQL support.
///
/// Shows a message explaining that SQL support requires the native app
/// with the `sql` feature enabled.
pub struct SqlPane {
    id: usize,
    theme: AppTheme,
}

impl SqlPane {
    /// Create a new SQL pane stub.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            id: next_id_usize(),
            theme,
        }
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
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);

            // Desktop icon
            ui.label(
                RichText::new(egui_nerdfonts::regular::DESKTOP)
                    .color(accent)
                    .size(48.0),
            );

            ui.add_space(16.0);

            // Title
            ui.label(
                RichText::new("SQL Feature Not Available")
                    .color(text_primary)
                    .size(18.0)
                    .strong(),
            );

            ui.add_space(8.0);

            // Description
            ui.label(
                RichText::new(
                    "SQL panes require the native desktop app\nbuilt with the 'sql' feature enabled.",
                )
                .color(text_secondary)
                .size(13.0),
            );

            ui.add_space(24.0);

            // Download hint
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
