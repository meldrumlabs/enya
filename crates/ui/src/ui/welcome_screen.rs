use egui::{Color32, NumExt};

use crate::{
    app::AppState,
    command::{CommandSender, UICommandSender},
};

use super::colors::{apply_button_theme, text_color};

pub fn welcome_section_ui(ui: &mut egui::Ui, app_state: &AppState, command_sender: &CommandSender) {
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
                show_welcome_section_ui(ui, app_state, command_sender);
            });
    });
}

pub fn show_welcome_section_ui(
    ui: &mut egui::Ui,
    app_state: &AppState,
    command_sender: &CommandSender,
) {
    ui.vertical_centered_justified(|ui| {
        let image = egui::Image::new(egui::include_image!("../../assets/logo.png"));

        let theme_color = text_color(app_state.theme);

        ui.add(image.max_width(250.0).max_height(250.0));
        ui.heading(
            egui::RichText::new("Enya")
                .strong()
                .size(24.0)
                .color(theme_color),
        );
        ui.add_space(10.0);
    });

    ui.add_space(30.0);

    ui.vertical_centered(|ui| {
        if ui
            .add(apply_button_theme(
                app_state.theme,
                egui::Button::new(
                    egui::RichText::new("Settings")
                        .strong()
                        .color(Color32::WHITE)
                        .size(16.0),
                )
                .min_size(egui::Vec2::new(220.0, 25.0)),
            ))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            // Make it open in the next frame
            command_sender.send_ui(crate::command::UICommand::Settings)
        }
    });
}
