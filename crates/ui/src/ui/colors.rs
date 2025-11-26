use egui::Color32;

use crate::theme::AppTheme;

#[inline]
pub fn text_color(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Light => ENYA_WHITE,
        AppTheme::Dark => ENYA_WHITE,
    }
}

#[inline]
pub fn button_color(theme: AppTheme) -> Color32 {
    match theme {
        AppTheme::Light => ENYA_WHITE,
        AppTheme::Dark => ENYA_WHITE,
    }
}

pub fn apply_button_theme(theme: AppTheme, button: egui::Button<'_>) -> egui::Button<'_> {
    match theme {
        AppTheme::Light => button.fill(ENYA_WHITE),
        AppTheme::Dark => button
            .fill(ENYA_DARK)
            .stroke(egui::Stroke::new(1.0, ENYA_WHITE.gamma_multiply(0.086))),
    }
}

pub const ENYA_WHITE: Color32 = Color32::from_rgb(255, 255, 255);
pub const ENYA_DARK: Color32 = Color32::from_rgb(0, 0, 0);
