//! WASM stub for SQL pane - shows "Native App Required" message.
//!
//! SQL panes require Flight SQL connectivity which is only available
//! in the native desktop app.

use egui::RichText;

use crate::components::Component;
use crate::components::util::id_generator::next_id_usize;
use crate::ui::theme::AppTheme;

/// Action returned by the SQL pane (stub version has no actions).
#[derive(Debug, Clone, PartialEq)]
pub enum SqlPaneAction {
    None,
}

/// SQL pane stub for WASM builds.
///
/// Shows a "Native App Required" message since Flight SQL
/// connectivity is not available in browsers.
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

    fn set_api_key(&mut self, _key: &str) {
        // No-op for stub
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // No-op for stub
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
                RichText::new("Native App Required")
                    .color(text_primary)
                    .size(18.0)
                    .strong(),
            );

            ui.add_space(8.0);

            // Description
            ui.label(
                RichText::new(
                    "SQL panes with Flight SQL connectivity\nare only available in the native desktop app.",
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
