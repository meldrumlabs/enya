use egui::{Color32, NumExt};

use crate::{
    app::AppState,
    command::{CommandSender, UICommandSender},
};

use super::colors::{apply_button_theme, text_color};

//pub(super) const MIN_COLUMN_WIDTH: f32 = 250.0;
pub(super) const API_KEY_URL: &str = "https://polygon.io/dashboard/signup";

pub fn welcome_section_ui(ui: &mut egui::Ui, app_state: &AppState, command_sender: &CommandSender) {
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
            egui::RichText::new("Ttadak")
                .strong()
                .size(24.0)
                .color(theme_color),
        );
        ui.add_space(10.0);

        // let horizontal_scroll = ui.available_width() < 40.0 * 2.0 + MIN_COLUMN_WIDTH;
        // kvet (max_width, max_height) = if horizontal_scroll {
        //     (350.0, 350.0)
        // } else {
        //     (500.0, 500.0)
        // };

        // ui.add(
        //     egui::Image::new(egui::include_image!("../../assets/graphic-hero.svg"))
        //         .max_width(max_width)
        //         .max_height(max_height),
        // );
    });

    ui.add_space(30.0);

    ui.vertical_centered(|ui| {
        if ui
            .add(apply_button_theme(
                app_state.theme,
                egui::Button::new(
                    egui::RichText::new("Create API Key")
                        .strong()
                        .color(Color32::WHITE)
                        .size(16.0),
                )
                .min_size(egui::Vec2::new(220.0, 25.0)),
            ))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            ui.ctx().open_url(egui::output::OpenUrl {
                url: API_KEY_URL.to_owned(),
                new_tab: true,
            });
        }

        ui.add_space(9.0);

        if ui
            .add(apply_button_theme(
                app_state.theme,
                egui::Button::new(
                    egui::RichText::new("Settings")
                        .strong()
                        .color(Color32::WHITE)
                        .size(16.0),
                )
                //.sense(egui::Sense::hover())
                .min_size(egui::Vec2::new(220.0, 25.0)),
            ))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            // Make it open in the next frame
            command_sender.send_ui(crate::command::UICommand::Settings)
        }

        ui.add_space(9.0);

        if ui
            .add(apply_button_theme(
                app_state.theme,
                egui::Button::new(
                    egui::RichText::new("Open")
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
            command_sender.send_ui(crate::command::UICommand::Open);
        }
    });

    ui.add_space(25.0);
}
