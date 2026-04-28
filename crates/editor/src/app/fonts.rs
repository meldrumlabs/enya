//! Font setup for the editor.
//!
//! This module handles configuring custom fonts including Maple Mono,
//! Departure Mono, JetBrains Mono, Iosevka, Geist Mono, and Nerd Fonts icons.

use crate::ui::settings_screen::EditorFont;

/// Set up fonts with all available fonts and Nerd Fonts icons.
/// The preferred font is set as highest priority in the font families.
///
/// `custom_fonts` is a list of `(display_name, path)` for all user-loaded fonts.
/// We preload every custom font so that the settings page can preview any of
/// them without requiring a separate `setup_fonts` call first.
pub fn setup_fonts(
    ctx: &egui::Context,
    preferred_font: &EditorFont,
    custom_fonts: &[(String, String)],
) {
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

    // Add Geist Mono font
    fonts.font_data.insert(
        "geist_mono".to_owned(),
        egui::FontData::from_static(include_bytes!("../../assets/fonts/GeistMono-Regular.otf"))
            .into(),
    );

    // Add Nerd Fonts icons font data
    fonts.font_data.insert(
        "nerdfonts".to_owned(),
        egui_nerdfonts::Variant::Regular.font_data().into(),
    );

    // ── Preload every user-loaded custom font ───────────────────────────
    // This ensures the settings page can preview any custom font in the same
    // frame it is clicked, before the app loop has a chance to call us again.
    for (name, path) in custom_fonts {
        let family_key = format!("custom_{name}");
        if fonts.font_data.contains_key(&family_key) {
            continue; // already loaded (e.g. same file added twice)
        }
        match std::fs::read(path) {
            Ok(bytes) => {
                fonts
                    .font_data
                    .insert(family_key.clone(), egui::FontData::from_owned(bytes).into());
                fonts.families.insert(
                    egui::FontFamily::Name(family_key.clone().into()),
                    vec![family_key, "nerdfonts".to_owned()],
                );
            }
            Err(e) => {
                log::warn!("Failed to load custom font '{name}' from {path}: {e}");
            }
        }
    }

    // Determine primary font family name
    let primary_font = preferred_font.font_family_name();

    // If the preferred custom font failed to load above, fall back to default
    let effective_primary = if preferred_font.is_custom()
        && !fonts.font_data.contains_key(&primary_font)
    {
        log::warn!("Preferred custom font '{primary_font}' not loaded, falling back to default");
        EditorFont::default().font_family_name()
    } else {
        primary_font
    };

    // Build font list: preferred font first, then fallbacks, then nerdfonts for icons
    let mut font_list = vec![effective_primary.clone()];

    // Add other built-in fonts as fallbacks (skip if it's the same as primary)
    for font in EditorFont::all_builtins() {
        let name = font.font_family_name();
        if name != effective_primary {
            font_list.push(name);
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

    // Register each built-in font as a named family for direct access (e.g., in style picker previews)
    for font in EditorFont::all_builtins() {
        let font_name = font.font_family_name();
        fonts.families.insert(
            egui::FontFamily::Name(font_name.clone().into()),
            vec![font_name, "nerdfonts".to_owned()],
        );
    }

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
