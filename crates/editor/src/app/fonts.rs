//! Font setup for the editor.
//!
//! This module handles configuring custom fonts including Maple Mono,
//! Departure Mono, JetBrains Mono, Iosevka, and Nerd Fonts icons.

use crate::ui::settings_screen::EditorFont;

/// Set up fonts with all available fonts and Nerd Fonts icons.
/// The preferred font is set as highest priority in the font families.
pub fn setup_fonts(ctx: &egui::Context, preferred_font: EditorFont) {
    let mut fonts = egui::FontDefinitions::default();

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

    // Add Nerd Fonts icons
    egui_nerdfonts::add_to_fonts(&mut fonts, egui_nerdfonts::Variant::Regular);

    // Set the preferred font as highest priority
    let primary_font = preferred_font.font_family_name().to_owned();

    // Put preferred font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, primary_font.clone());

    // Put preferred font first (highest priority) for monospace too:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, primary_font);

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
