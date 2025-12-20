//! Font setup for the editor.
//!
//! This module handles configuring custom fonts including DepartureMono
//! and Nerd Fonts icons.

/// Set up fonts with both DepartureMono and Nerd Fonts icons
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Add DepartureMono font
    fonts.font_data.insert(
        "departure_mono".to_owned(),
        egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/DepartureMono-Regular.otf"
        ))
        .into(),
    );

    // Add Nerd Fonts icons
    egui_nerdfonts::add_to_fonts(&mut fonts, egui_nerdfonts::Variant::Regular);

    // Put DepartureMono first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "departure_mono".to_owned());

    // Put DepartureMono first (highest priority) for monospace too:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "departure_mono".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
