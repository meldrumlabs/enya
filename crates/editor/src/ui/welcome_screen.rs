use egui::NumExt;

use crate::app::AppState;

use super::colors::text_color;

pub fn welcome_section_ui(ui: &mut egui::Ui, app_state: &AppState) {
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
                show_welcome_section_ui(ui, app_state);
            });
    });
}

pub fn show_welcome_section_ui(ui: &mut egui::Ui, app_state: &AppState) {
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
}
