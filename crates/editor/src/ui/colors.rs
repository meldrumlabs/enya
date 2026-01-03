use egui::Color32;

use crate::ui::theme::AppTheme;

#[inline]
pub fn text_color(theme: AppTheme) -> Color32 {
    theme.text_primary()
}

#[inline]
pub fn button_color(theme: AppTheme) -> Color32 {
    theme.text_primary()
}

pub fn apply_button_theme(theme: AppTheme, button: egui::Button<'_>) -> egui::Button<'_> {
    button
        .fill(theme.bg_surface())
        .stroke(egui::Stroke::new(1.0, theme.border_subtle()))
}

pub const ENYA_WHITE: Color32 = Color32::from_rgb(255, 255, 255);
pub const ENYA_DARK: Color32 = Color32::from_rgb(0, 0, 0);
