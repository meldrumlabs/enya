use egui::NumExt;
use re_build_info::BuildInfo;

use crate::{
    app::AppState,
    command::{CommandSender, UICommandSender},
};

use super::colors::text_color;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum Provider {
    #[default]
    Polygon,
    Binance,
}
impl Provider {
    pub fn name(&self) -> &'static str {
        match self {
            Provider::Polygon => "Polygon",
            Provider::Binance => "Binance",
        }
    }
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// Data provider API key.
    pub api_key: String,

    /// Which Data Provider to use.
    pub provider: Provider,

    pub ttadak_key: String,

    /// For advanced use (e.g., staging websocket SQL).
    pub staging_api_key: String,
}

pub fn show_settings_ui(
    ui: &mut egui::Ui,
    build_info: BuildInfo,
    app_state: &mut AppState,
    command_sender: &CommandSender,
) {
    egui::Frame {
        inner_margin: egui::Margin::same(5.0),
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
            egui::CollapsingHeader::new("Data Provider")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut app_state.settings.api_key)
                                .password(true)
                                .hint_text("API Key"),
                        );
                        egui::ComboBox::from_label("")
                            .width(100.0)
                            .selected_text(app_state.settings.provider.name())
                            .show_ui(ui, |ui| {
                                let polygon = ui.add(egui::SelectableLabel::new(
                                    matches!(app_state.settings.provider, Provider::Polygon),
                                    "Polygon",
                                ));
                                let binance = ui.add(egui::SelectableLabel::new(
                                    matches!(app_state.settings.provider, Provider::Binance),
                                    "Binance",
                                ));

                                if polygon.clicked() {
                                    app_state.settings.provider = Provider::Polygon;
                                }

                                if binance.clicked() {
                                    app_state.settings.provider = Provider::Binance;
                                }
                            });
                    });

                    ui.add_space(8.0);

                    if app_state.settings.api_key.is_empty() {
                        ui.colored_label(text_color, "Please configure your API key");
                    } else if ui.button("Save").clicked() {
                    }
                });

            egui::CollapsingHeader::new("Meldrum Pro")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut app_state.settings.ttadak_key)
                                .password(true)
                                .hint_text("API Key"),
                        );
                    });

                    ui.add_space(8.0);

                    if app_state.settings.api_key.is_empty() {
                        ui.colored_label(text_color, "Please configure your API key");
                    } else if ui.button("Save").clicked() {
                        // TODO: Verify Meldrum API key when this is a thing
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

            // egui::CollapsingHeader::new("Looks")
            //     .default_open(false)
            //     .show(ui, |ui| {
            //         egui::ComboBox::from_label("Theme")
            //             .width(100.0)
            //             .selected_text(app_state.theme.name())
            //             .show_ui(ui, |ui| {
            //                 let light = ui.add(egui::SelectableLabel::new(
            //                     matches!(app_state.theme, AppTheme::Light),
            //                     "Light",
            //                 ));
            //                 let dark = ui.add(egui::SelectableLabel::new(
            //                     matches!(app_state.theme, AppTheme::Dark),
            //                     "Dark",
            //                 ));

            //                 if light.clicked() {
            //                     app_state.theme = AppTheme::Light;
            //                 }
            //                 if dark.clicked() {
            //                     app_state.theme = AppTheme::Dark;
            //                 }
            //             });
            //     });
        });
    });
}

fn separator_with_some_space(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
