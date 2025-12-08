use egui::NumExt;
use enya_build_info::BuildInfo;

use crate::{
    app::AppState,
    command::{CommandSender, UICommandSender},
};

use super::colors::text_color;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub api_key: String,
    /// Recent plots that were opened (metric name, timestamp)
    #[serde(default)]
    pub recent_plots: Vec<RecentPlotEntry>,
    /// Recent workspaces that were accessed
    #[serde(default)]
    pub recent_workspaces: Vec<WorkspaceEntry>,
}

/// Entry for a recently opened plot/chart
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentPlotEntry {
    /// Display name for the plot
    pub name: String,
    /// The metric name or query identifier
    pub metric_name: String,
    /// Unix timestamp of when it was last opened
    pub timestamp: i64,
    /// Whether this is a custom query (vs a metric)
    #[serde(default)]
    pub is_query: bool,
}

/// Entry for a recently accessed workspace
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceEntry {
    /// Display name for the workspace
    pub name: String,
    /// Description or path of the workspace
    pub description: String,
    /// Unix timestamp of when it was last accessed
    pub timestamp: i64,
}

impl AppSettings {
    /// Maximum number of recent plots to keep
    pub const MAX_RECENT_PLOTS: usize = 10;
    /// Maximum number of recent workspaces to keep
    pub const MAX_RECENT_WORKSPACES: usize = 8;

    /// Add a recent plot entry, updating timestamp if it already exists
    pub fn add_recent_plot(&mut self, name: String, metric_name: String, is_query: bool) {
        let timestamp = crate::util::now_unix_secs();

        // Remove existing entry with same metric_name
        self.recent_plots.retain(|p| p.metric_name != metric_name);

        // Add new entry at the front
        self.recent_plots.insert(
            0,
            RecentPlotEntry {
                name,
                metric_name,
                timestamp,
                is_query,
            },
        );

        // Trim to max size
        self.recent_plots.truncate(Self::MAX_RECENT_PLOTS);
    }

    /// Add a recent workspace entry, updating timestamp if it already exists
    pub fn add_recent_workspace(&mut self, name: String, description: String) {
        let timestamp = crate::util::now_unix_secs();

        // Remove existing entry with same name
        self.recent_workspaces.retain(|w| w.name != name);

        // Add new entry at the front
        self.recent_workspaces.insert(
            0,
            WorkspaceEntry {
                name,
                description,
                timestamp,
            },
        );

        // Trim to max size
        self.recent_workspaces.truncate(Self::MAX_RECENT_WORKSPACES);
    }
}

pub fn show_settings_ui(
    ui: &mut egui::Ui,
    build_info: BuildInfo,
    app_state: &mut AppState,
    command_sender: &CommandSender,
) {
    egui::Frame {
        inner_margin: egui::Margin::same(5),
        ..Default::default()
    }
    .show(ui, |ui| {
        const MAX_WIDTH: f32 = 600.0;
        const MIN_WIDTH: f32 = 300.0;

        let centering_margin = ((ui.available_width() - MAX_WIDTH) / 2.0).at_least(0.0);
        let max_rect = ui.max_rect().expand2(-centering_margin * egui::Vec2::X);
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(max_rect));

        egui::ScrollArea::both()
            .auto_shrink(false)
            .show(&mut child_ui, |ui| {
                ui.set_min_width(MIN_WIDTH);
                show_settings_ui_impl(ui, build_info, app_state, command_sender);
            });

        if ui.input_mut(|ui| ui.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            command_sender.send_ui(crate::command::UICommand::CloseSettings);
        }
    });
}

pub fn show_settings_ui_impl(
    ui: &mut egui::Ui,
    build_info: BuildInfo,
    app_state: &mut AppState,
    command_sender: &CommandSender,
) {
    let image = egui::Image::new(egui::include_image!("../../assets/logo.png"));
    let text_color = text_color(app_state.theme);

    ui.vertical_centered_justified(|ui| {
        ui.add(image.max_width(250.0).max_height(250.0));
    });

    ui.add_space(40.0);

    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("Settings")
                .strong()
                .size(24.0)
                .color(text_color),
        );

        ui.allocate_ui_with_layout(
            egui::Vec2::X * ui.available_width(),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                use crate::ui::UiExt;
                if ui
                    .small_icon_button(&crate::ui::icons::CLOSE_ICON)
                    .clicked()
                {
                    command_sender.send_ui(crate::command::UICommand::CloseSettings);
                }
            },
        )
    });

    separator_with_some_space(ui);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            egui::CollapsingHeader::new("Enya Pro")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut app_state.settings.api_key)
                                .password(true)
                                .hint_text("API Key"),
                        );
                    });

                    ui.add_space(8.0);

                    if app_state.settings.api_key.is_empty() {
                        ui.colored_label(text_color, "Please configure your API key");
                    } else if ui.button("Save").clicked() {
                        // TODO: Verify Enya API key when this is a thing
                    }
                });

            egui::CollapsingHeader::new("Build")
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new("build_info_grid")
                        .striped(true)
                        .spacing(egui::vec2(10.0, 4.0))
                        .show(ui, |ui| {
                            ui.label("Version:");
                            // Assuming CrateVersion implements Display.
                            ui.label(build_info.version.to_string());
                            ui.end_row();

                            ui.label("Rustc Version:");
                            ui.label(build_info.rustc_version);
                            ui.end_row();

                            ui.label("LLVM Version:");
                            ui.label(build_info.llvm_version);
                            ui.end_row();
                            ui.label("Target Triple:");
                            ui.label(build_info.target_triple);
                            ui.end_row();
                        });
                });
        });
    });
}

fn separator_with_some_space(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
