//! Font setup for the editor.
//!
//! This module handles configuring custom fonts including Maple Mono,
//! Departure Mono, JetBrains Mono, Iosevka, and Nerd Fonts icons.

use crate::ui::settings_screen::EditorFont;

/// Set up fonts with all available fonts and Nerd Fonts icons.
/// The preferred font is set as highest priority in the font families.
pub fn setup_fonts(ctx: &egui::Context, preferred_font: EditorFont) {
    // Start with empty fonts since we disabled default_fonts feature
    let mut fonts = egui::FontDefinitions::empty();

    // Add Maple Mono font
    fonts.font_data.insert(
        "maple_mono".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/MapleMono-Regular.otf"))
            .into(),
    );

    // Add Departure Mono font
    fonts.font_data.insert(
        "departure_mono".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/DepartureMono-Regular.otf"
        ))
        .into(),
    );

    // Add JetBrains Mono font
    fonts.font_data.insert(
        "jetbrains_mono".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/JetBrainsMono-Regular.ttf"
        ))
        .into(),
    );

    // Add Iosevka font
    fonts.font_data.insert(
        "iosevka".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/Iosevka-Regular.ttf"))
            .into(),
    );

    // Add Nerd Fonts icons font data
    fonts.font_data.insert(
        "nerdfonts".to_owned(),
        egui_nerdfonts::Variant::Regular.font_data().into(),
    );

    // Get the preferred font name
    let primary_font = preferred_font.font_family_name().to_owned();

    // Build font list: preferred font first, then fallbacks, then nerdfonts for icons
    let mut font_list = vec![primary_font.clone()];

    // Add other fonts as fallbacks (skip if it's the same as primary)
    for font in ["maple_mono", "departure_mono", "jetbrains_mono", "iosevka"] {
        if font != primary_font {
            font_list.push(font.to_owned());
        }
    }

    // Add nerdfonts last for icon fallback
    font_list.push("nerdfonts".to_owned());

    // Set up font families - since we start from empty, we need to populate them
    fonts
        .families
        .insert(egui::FontFamily::Proportional, font_list.clone());
    fonts
        .families
        .insert(egui::FontFamily::Monospace, font_list);

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
