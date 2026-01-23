use egui::{
    Color32, CornerRadius, Stroke, Visuals,
    style::{Selection, TextCursorStyle, WidgetVisuals, Widgets},
};

/// Create a dark theme with the given accent colors
pub fn dark_theme(theme: super::theme::AppTheme) -> Visuals {
    // --- Premium Obsidian Glass Design System ---
    // A luxurious dark theme with refined depth and configurable accents.
    // Designed for Departure Mono typography and high-end developer experience.
    use super::palette::semantic;

    // Theme-aware colors
    let accent_primary_color = theme.accent_primary();
    let accent_hover_color = theme.accent_hover();
    let selection_color = theme.accent_selection();
    let focus_border_color = theme.border_focus();

    // Theme-aware background colors
    let bg_base = theme.bg_base();
    let bg_surface = theme.bg_surface();
    let bg_elevated = theme.bg_elevated();
    let bg_hover = theme.bg_hover();

    // Theme-aware border colors
    let border_subtle = theme.border_subtle();

    // Theme-aware text colors
    let text_primary = theme.text_primary();

    // Premium corner radius - subtle but refined (not too rounded, not harsh)
    let corner_radius = CornerRadius::same(6);

    // Layered shadow system for premium depth perception
    // Primary shadow provides the main elevation effect
    let soft_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(100), // Slightly stronger for depth
    };

    Visuals {
        dark_mode: true,
        override_text_color: None,

        // --- Premium Widget Visuals ---
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: bg_surface,
                weak_bg_fill: bg_surface,
                bg_stroke: Stroke::new(1.0, border_subtle),
                corner_radius,
                fg_stroke: Stroke::new(1.0, text_primary),
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: bg_elevated,
                weak_bg_fill: Color32::TRANSPARENT,
                bg_stroke: Stroke::new(1.0, border_subtle),
                corner_radius,
                fg_stroke: Stroke::new(1.0, text_primary),
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: bg_hover,
                weak_bg_fill: bg_hover,
                bg_stroke: Stroke::new(1.0, focus_border_color), // More prominent on hover
                corner_radius,
                fg_stroke: Stroke::new(1.0, text_primary),
                expansion: 1.5, // Slightly more expansion for premium feel
            },
            active: WidgetVisuals {
                bg_fill: accent_primary_color,
                weak_bg_fill: accent_primary_color,
                bg_stroke: Stroke::new(1.5, accent_hover_color), // Glow effect on active
                corner_radius,
                fg_stroke: Stroke::new(1.5, bg_base), // Dark text on accent
                expansion: 0.5,
            },
            open: WidgetVisuals {
                bg_fill: bg_elevated,
                weak_bg_fill: bg_elevated,
                bg_stroke: Stroke::new(1.5, accent_primary_color), // Stronger accent border
                corner_radius,
                fg_stroke: Stroke::new(1.0, accent_hover_color), // Brighter accent text
                expansion: 1.0,
            },
        },

        // --- Premium Selection ---
        selection: Selection {
            bg_fill: selection_color,
            stroke: Stroke::new(1.5, accent_primary_color), // Slightly stronger selection border
        },

        // --- Window & Panel ---
        window_fill: bg_surface,
        window_stroke: Stroke::new(1.0, border_subtle),
        panel_fill: bg_base,
        faint_bg_color: bg_surface,
        extreme_bg_color: bg_base,

        // --- Premium Shadows ---
        popup_shadow: soft_shadow,
        window_shadow: soft_shadow,

        // --- Semantic colors ---
        error_fg_color: semantic::ERROR,
        warn_fg_color: semantic::WARNING,
        hyperlink_color: accent_hover_color, // Brighter for better visibility

        // --- Premium Text Cursor ---
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.5, accent_primary_color), // Slightly thicker for premium feel
            blink: true,
            on_duration: 0.6, // Slightly slower blink for elegance
            off_duration: 0.4,
            ..Default::default()
        },

        striped: false,
        clip_rect_margin: 0.0,
        ..Default::default()
    }
}

/// Create a light theme with the given accent colors
pub fn light_theme(theme: super::theme::AppTheme) -> Visuals {
    use super::palette::{accent_primary, accent_selection, light_bg, light_border, semantic};

    let accent_primary_color = accent_primary(theme);
    let selection_color = accent_selection(theme);

    Visuals {
        dark_mode: false,
        widgets: Widgets::light(),
        selection: Selection {
            bg_fill: selection_color,
            stroke: Stroke::new(1.0, accent_primary_color),
        },

        hyperlink_color: accent_primary_color,

        faint_bg_color: light_bg::SURFACE,
        extreme_bg_color: light_bg::ELEVATED,
        code_bg_color: light_bg::ELEVATED,

        warn_fg_color: semantic::WARNING,
        error_fg_color: semantic::ERROR,

        window_shadow: egui::epaint::Shadow {
            offset: [10, 20],
            blur: 15,
            spread: 0,
            color: Color32::from_black_alpha(25),
        },
        window_fill: light_bg::BASE,
        window_stroke: Stroke::new(1.0, light_border::DEFAULT),

        panel_fill: light_bg::BASE,

        popup_shadow: egui::epaint::Shadow {
            offset: [6, 10],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(25),
        },

        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, accent_primary_color),
            blink: true,
            on_duration: 0.5,
            off_duration: 0.5,
            ..Default::default()
        },
        ..Visuals::light()
    }
}
