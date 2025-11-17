use egui::Color32;

use crate::theme::AppTheme;

#[inline]
pub fn text_color(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Light => ORANGE,
        AppTheme::Dark => ORANGE,
    }
}

#[inline]
pub fn button_color(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Light => ORANGE,
        AppTheme::Dark => ORANGE,
    }
}

pub fn apply_button_theme(theme: AppTheme, button: egui::Button<'_>) -> egui::Button<'_> {
    match theme {
        AppTheme::Light => button.fill(POLYGON_PURPLE),
        AppTheme::Dark => {
            button
                .fill(egui::Color32::from_rgb(26, 29, 30))
                .stroke(egui::Stroke::new(
                    1.0,
                    ORANGE.gamma_multiply(0.086),
                    //egui::Color32::WHITE.gamma_multiply(0.086),
                ))
        }
    }
}

pub const POLYGON_PURPLE: Color32 = Color32::from_rgb(95, 92, 255);
pub const POLYGON_WHITE: Color32 = Color32::WHITE;

pub const ORANGE: Color32 = Color32::from_rgb(255, 106, 0);
