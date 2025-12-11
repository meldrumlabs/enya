use egui::{
    Color32, CornerRadius, Stroke, Visuals,
    style::{Selection, TextCursorStyle, WidgetVisuals, Widgets},
};

pub fn white_theme() -> Visuals {
    let white = Color32::from_rgb(255, 255, 255);
    let black = Color32::from_rgb(0, 0, 0);
    //let black = Color32::from_rgb(180, 180, 180);
    let light_black = Color32::from_rgb(240, 240, 240);

    Visuals {
        dark_mode: false,
        override_text_color: Some(black),
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: white,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(1.0, black),
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(1.0, black),
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: white,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(1.0, black),
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(1.0, black),
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: light_black,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(1.5, black),
                corner_radius: CornerRadius::same(3),
                fg_stroke: Stroke::new(1.5, black),
                expansion: 1.0,
            },
            active: WidgetVisuals {
                bg_fill: light_black,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(2.0, black),
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(2.0, black),
                expansion: 1.0,
            },
            open: WidgetVisuals {
                bg_fill: light_black,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(1.0, black),
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(1.0, black),
                expansion: 0.0,
            },
        },
        selection: Selection {
            bg_fill: Color32::from_rgb(144, 209, 255),
            stroke: Stroke::new(1.0, Color32::from_rgb(0, 148, 255)),
        },
        window_fill: white,
        window_stroke: Stroke::new(1.0, black),
        ..Default::default()
    }
}

// pub fn black_theme() -> Visuals {
//     let black = Color32::from_rgb(0, 0, 0);
//     let white = Color32::from_rgb(255, 255, 255);

//     Visuals {
//         dark_mode: true,
//         override_text_color: Some(ORANGE),
//         widgets: Widgets {
//             noninteractive: WidgetVisuals {
//                 bg_fill: black,
//                 weak_bg_fill: black,
//                 bg_stroke: Stroke::new(1.0, black),
//                 rounding: Rounding::same(2.0),
//                 fg_stroke: Stroke::new(1.0, white),
//                 expansion: 0.0,
//             },
//             inactive: WidgetVisuals {
//                 bg_fill: black,
//                 weak_bg_fill: black,
//                 bg_stroke: Stroke::new(1.0, black),
//                 rounding: Rounding::same(2.0),
//                 fg_stroke: Stroke::new(1.0, white),
//                 expansion: 0.0,
//             },
//             hovered: WidgetVisuals {
//                 bg_fill: black,
//                 weak_bg_fill: black,
//                 bg_stroke: Stroke::new(1.5, white),
//                 rounding: Rounding::same(3.0),
//                 fg_stroke: Stroke::new(1.5, white),
//                 expansion: 1.0,
//             },
//             active: WidgetVisuals {
//                 bg_fill: black,
//                 weak_bg_fill: black,
//                 bg_stroke: Stroke::new(2.0, white),
//                 rounding: Rounding::same(2.0),
//                 fg_stroke: Stroke::new(2.0, white),
//                 expansion: 1.0,
//             },
//             open: WidgetVisuals {
//                 bg_fill: black,
//                 weak_bg_fill: black,
//                 bg_stroke: Stroke::new(1.0, white),
//                 rounding: Rounding::same(2.0),
//                 fg_stroke: Stroke::new(1.0, white),
//                 expansion: 0.0,
//             },
//         },
//         selection: Selection {
//             bg_fill: black,
//             stroke: Stroke::new(1.0, white),
//         },
//         window_rounding: Rounding::same(6.0),
//         window_fill: black,
//         window_stroke: Stroke::new(1.0, white),
//         ..Default::default()
//     }
// }

// pub fn black_theme() -> Visuals {
//     // Define colors: emphasize pure black, minimal grayscale
//     let pure_black = Color32::from_rgb(0, 0, 0); // Primary background
//     let dark_gray = Color32::from_rgb(20, 20, 20); // Minimal gray for floating elements
//     let medium_gray = Color32::from_rgb(40, 40, 40); // Hovered/active states
//     let light_gray = Color32::from_rgb(100, 100, 100); // Subdued text
//     let default_text = Color32::from_rgb(180, 180, 180); // Default text
//     let white = Color32::from_rgb(255, 255, 255); // Strong text

//     Visuals {
//         dark_mode: true,
//         override_text_color: None, // No orange override, use text hierarchy
//         widgets: Widgets {
//             noninteractive: WidgetVisuals {
//                 bg_fill: pure_black,                     // Full black for panels
//                 weak_bg_fill: pure_black,                // Consistent black
//                 bg_stroke: Stroke::new(1.0, dark_gray),  // Subtle stroke for separators
//                 rounding: Rounding::same(4.0),           // Rerun’s modern radius
//                 fg_stroke: Stroke::new(1.0, light_gray), // Subdued text
//                 expansion: 0.0,
//             },
//             inactive: WidgetVisuals {
//                 bg_fill: pure_black,                // No fill for checkboxes, etc.
//                 weak_bg_fill: Color32::TRANSPARENT, // No background for buttons
//                 bg_stroke: Stroke::NONE,            // Clean, no stroke
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, default_text), // Default text
//                 expansion: 0.0,
//             },
//             hovered: WidgetVisuals {
//                 bg_fill: medium_gray, // Subtle gray for hover feedback
//                 weak_bg_fill: medium_gray,
//                 bg_stroke: Stroke::NONE, // No stroke, like Rerun
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.5, default_text), // Slightly bolder
//                 expansion: 2.0,                            // Rerun’s hover expansion
//             },
//             active: WidgetVisuals {
//                 bg_fill: medium_gray, // Same as hover for consistency
//                 weak_bg_fill: medium_gray,
//                 bg_stroke: Stroke::NONE,
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(2.0, white), // Strong text for active
//                 expansion: 2.0,
//             },
//             open: WidgetVisuals {
//                 bg_fill: medium_gray, // Consistent with hover/active
//                 weak_bg_fill: medium_gray,
//                 bg_stroke: Stroke::NONE,
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, default_text),
//                 expansion: 2.0,
//             },
//         },
//         selection: Selection {
//             bg_fill: Color32::from_rgb(0, 60, 120), // Darker blue for selection
//             stroke: Stroke::new(2.0, Color32::from_rgb(120, 140, 200)), // Brighter blue
//         },
//         window_rounding: Rounding::same(6.0), // Rerun’s window radius
//         window_fill: dark_gray,               // Slightly lighter for tooltips/menus
//         window_stroke: Stroke::NONE,          // Rerun avoids window strokes
//         panel_fill: pure_black,               // Full black for panels
//         faint_bg_color: pure_black,           // No lighter stripes
//         extreme_bg_color: pure_black,         // Black for text edits, scroll bars
//         popup_shadow: egui::epaint::Shadow {
//             offset: [0.0, 15.0].into(),
//             blur: 50.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(128), // Rerun’s black shadow
//         },
//         window_shadow: egui::epaint::Shadow {
//             offset: [0.0, 15.0].into(),
//             blur: 50.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(128),
//         },
//         striped: false,        // No stripes, like Rerun
//         clip_rect_margin: 0.0, // Avoid glitches
//         ..Default::default()
//     }
// }

// pub fn gruvbox_theme() -> Visuals {
//     // Gruvbox dark palette
//     let bg0_h = Color32::from_rgb(29, 32, 33); // #1d2021 (hard dark background)
//     let bg0 = Color32::from_rgb(40, 40, 40); // #282828 (default background)
//     let bg1 = Color32::from_rgb(60, 56, 54); // #3c3836 (subtle highlight)
//     let fg0 = Color32::from_rgb(251, 241, 199); // #fbf1c7 (strong text)
//     let fg1 = Color32::from_rgb(235, 219, 178); // #ebdbb2 (default text)
//     let fg4 = Color32::from_rgb(168, 153, 132); // #a89984 (subdued text)
//     let blue = Color32::from_rgb(69, 133, 136); // #458588 (selection)
//     let yellow = Color32::from_rgb(215, 153, 33); // #d79921 (hover/active)
//     let orange = Color32::from_rgb(214, 93, 14); // #d65d0e (warning/active)
//     let red = Color32::from_rgb(204, 36, 29); // #cc241d (error)
//     let pure_black = Color32::from_rgb(0, 0, 0);

//     Visuals {
//         dark_mode: true,
//         override_text_color: None, // Use Gruvbox text hierarchy
//         widgets: Widgets {
//             noninteractive: WidgetVisuals {
//                 bg_fill: pure_black,              // Full black for panels
//                 weak_bg_fill: pure_black,         // Consistent with panels
//                 bg_stroke: Stroke::new(1.0, bg1), // Subtle separator lines
//                 rounding: Rounding::same(4.0),    // Rerun's modern look
//                 fg_stroke: Stroke::new(1.0, fg4), // Subdued text
//                 expansion: 0.0,
//             },
//             inactive: WidgetVisuals {
//                 bg_fill: pure_black,                // Full black for checkboxes, etc.
//                 weak_bg_fill: Color32::TRANSPARENT, // No button background
//                 bg_stroke: Stroke::NONE,            // Clean, like Rerun
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, fg1), // Default text
//                 expansion: 0.0,
//             },
//             hovered: WidgetVisuals {
//                 bg_fill: yellow, // Gruvbox yellow for hover feedback
//                 weak_bg_fill: yellow,
//                 bg_stroke: Stroke::NONE,
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.5, fg0), // Strong text
//                 expansion: 2.0,                   // Rerun's hover expansion
//             },
//             active: WidgetVisuals {
//                 bg_fill: orange, // Gruvbox orange for active state
//                 weak_bg_fill: orange,
//                 bg_stroke: Stroke::NONE,
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(2.0, fg0), // Strong text
//                 expansion: 2.0,
//             },
//             open: WidgetVisuals {
//                 bg_fill: yellow, // Consistent with hover
//                 weak_bg_fill: yellow,
//                 bg_stroke: Stroke::NONE,
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, fg1),
//                 expansion: 2.0,
//             },
//         },
//         selection: Selection {
//             bg_fill: blue, // Gruvbox blue for selections
//             stroke: Stroke::new(2.0, Color32::from_rgb(137, 180, 182)), // Lighter blue (#89b4b6)
//         },
//         window_rounding: Rounding::same(6.0), // Rerun's window corner radius
//         window_fill: bg0,                     // Slightly lighter for tooltips/menus
//         window_stroke: Stroke::NONE,          // Rerun's clean look
//         panel_fill: pure_black,               // Full black panels
//         faint_bg_color: pure_black,           // No gray for stripes
//         extreme_bg_color: pure_black,         // Text edits, scroll bars
//         popup_shadow: egui::epaint::Shadow {
//             offset: [0.0, 15.0].into(),
//             blur: 50.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(128), // Rerun's black-based shadow
//         },
//         window_shadow: egui::epaint::Shadow {
//             offset: [0.0, 15.0].into(),
//             blur: 50.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(128),
//         },
//         striped: false,        // Rerun's no-stripes approach
//         clip_rect_margin: 0.0, // Avoid glitches
//         error_fg_color: red,   // Gruvbox red for errors
//         warn_fg_color: orange, // Gruvbox orange for warnings
//         hyperlink_color: blue, // Gruvbox blue for links
//         ..Default::default()
//     }
// }

pub fn gruvbox_theme() -> Visuals {
    // Gruvbox dark palette (hard contrast)
    let bg0_h = Color32::from_rgb(29, 32, 33); // #1d2021 (hard dark background)
    let bg0 = Color32::from_rgb(40, 40, 40); // #282828 (default background)
    let bg1 = Color32::from_rgb(60, 56, 54); // #3c3836 (subtle highlight)
    let bg2 = Color32::from_rgb(80, 73, 69); // #504945 (floating elements)
    let fg0 = Color32::from_rgb(251, 241, 199); // #fbf1c7 (strong text)
    let fg1 = Color32::from_rgb(235, 219, 178); // #ebdbb2 (default text)
    let fg4 = Color32::from_rgb(168, 153, 132); // #a89984 (subdued text)
    let red = Color32::from_rgb(204, 36, 29); // #cc241d (error)
    let yellow = Color32::from_rgb(215, 153, 33); // #d79921 (hover)
    let blue = Color32::from_rgb(69, 133, 136); // #458588 (selection)
    let aqua = Color32::from_rgb(104, 157, 106); // #689d6a (alternative)
    let orange = Color32::from_rgb(214, 93, 14); // #d65d0e (active/warning)

    Visuals {
        dark_mode: true,
        override_text_color: None, // Use Gruvbox text hierarchy
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: bg0_h, // Hard dark background
                weak_bg_fill: bg0_h,
                bg_stroke: Stroke::new(1.0, bg1), // Subtle separator
                corner_radius: CornerRadius::same(2), // Subtle rounding for retro feel
                fg_stroke: Stroke::new(1.0, fg4), // Subdued text
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: bg0_h,                     // Consistent dark background
                weak_bg_fill: Color32::TRANSPARENT, // No button background
                bg_stroke: Stroke::new(1.0, bg1),   // Light stroke for definition
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(1.0, fg1), // Default text
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: yellow, // Gruvbox yellow for hover
                weak_bg_fill: aqua,
                bg_stroke: Stroke::new(1.5, bg1), // Slightly bolder stroke
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(1.5, fg0), // Strong text
                expansion: 1.0,                   // Subtle expansion
            },
            active: WidgetVisuals {
                bg_fill: orange, // Gruvbox orange for active
                weak_bg_fill: orange,
                bg_stroke: Stroke::new(1.5, bg1),
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(2.0, fg0), // Strong text
                expansion: 1.0,
            },
            open: WidgetVisuals {
                bg_fill: aqua, // Consistent with hover
                weak_bg_fill: aqua,
                bg_stroke: Stroke::new(1.0, bg1),
                corner_radius: CornerRadius::same(2),
                fg_stroke: Stroke::new(1.0, fg1), // Default text
                expansion: 1.0,
            },
        },
        selection: Selection {
            bg_fill: blue,                  // Gruvbox blue for selections
            stroke: Stroke::new(1.5, aqua), // Lighter aqua for stroke
        },
        window_fill: bg2,                     // Lighter bg for tooltips/menus
        window_stroke: Stroke::new(1.0, bg1), // Subtle window stroke
        panel_fill: bg0_h,                    // Hard dark background
        faint_bg_color: bg1,                  // Subtle highlight for stripes
        extreme_bg_color: bg0,                // Slightly lighter for text edits
        popup_shadow: egui::epaint::Shadow {
            offset: [2, 2],
            blur: 10,
            spread: 0,
            color: Color32::from_black_alpha(64), // Subtle shadow
        },
        window_shadow: egui::epaint::Shadow {
            offset: [2, 2],
            blur: 10,
            spread: 0,
            color: Color32::from_black_alpha(64),
        },
        striped: true, // Gruvbox often uses stripes for tables
        clip_rect_margin: 0.0,
        error_fg_color: red,   // Gruvbox red for errors
        warn_fg_color: orange, // Gruvbox orange for warnings
        hyperlink_color: blue, // Gruvbox blue for links

        // Blinking cursor for terminal aesthetic
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, fg0),
            blink: true,
            on_duration: 0.5,
            off_duration: 0.5,
            ..Default::default()
        },
        ..Default::default()
    }
}

// pub fn black_theme() -> Visuals {
//     // Define black and yellow palette
//     let pure_black = Color32::from_rgb(0, 0, 0);
//     let dark_black = Color32::from_rgb(8, 8, 8); // Very subtle lift from pure black
//     let soft_black = Color32::from_rgb(16, 16, 16); // Panel background
//     let charcoal = Color32::from_rgb(24, 24, 24); // Slightly lighter elements
//     let dark_gray = Color32::from_rgb(32, 32, 32); // Interactive elements
//     let medium_gray = Color32::from_rgb(48, 48, 48); // Hovered states
//     let light_gray = Color32::from_rgb(128, 128, 128); // Subdued text
//     let white = Color32::from_rgb(255, 255, 255); // Primary text

//     // Yellow accent colors
//     let yellow_primary = Color32::from_rgb(255, 215, 0); // Gold yellow
//     let yellow_bright = Color32::from_rgb(255, 235, 59); // Bright yellow
//     let yellow_dark = Color32::from_rgb(218, 165, 32); // Darker yellow

//     Visuals {
//         dark_mode: true,
//         override_text_color: None,
//         widgets: Widgets {
//             noninteractive: WidgetVisuals {
//                 bg_fill: pure_black, // Deep black panel background
//                 weak_bg_fill: soft_black,
//                 bg_stroke: Stroke::new(1.0, charcoal), // Subtle dark borders
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, light_gray), // Subdued text
//                 expansion: 0.0,
//             },
//             inactive: WidgetVisuals {
//                 bg_fill: dark_gray, // Dark interactive elements
//                 weak_bg_fill: Color32::TRANSPARENT,
//                 bg_stroke: Stroke::NONE,
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, white), // White text for contrast
//                 expansion: 0.0,
//             },
//             hovered: WidgetVisuals {
//                 bg_fill: medium_gray, // Lighter on hover
//                 weak_bg_fill: medium_gray,
//                 bg_stroke: Stroke::new(1.0, yellow_dark), // Yellow border on hover
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.5, yellow_bright), // Yellow text on hover
//                 expansion: 2.0,
//             },
//             active: WidgetVisuals {
//                 bg_fill: charcoal, // Darker when pressed
//                 weak_bg_fill: charcoal,
//                 bg_stroke: Stroke::new(2.0, yellow_primary), // Strong yellow border
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(2.0, yellow_primary), // Yellow text when active
//                 expansion: 2.0,
//             },
//             open: WidgetVisuals {
//                 bg_fill: medium_gray,
//                 weak_bg_fill: medium_gray,
//                 bg_stroke: Stroke::new(1.0, yellow_dark),
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, yellow_bright),
//                 expansion: 2.0,
//             },
//         },
//         selection: Selection {
//             bg_fill: Color32::from_rgba_unmultiplied(255, 215, 0, 60), // Semi-transparent yellow
//             stroke: Stroke::new(2.0, yellow_primary),                  // Yellow selection border
//         },
//         window_rounding: Rounding::same(6.0),
//         window_fill: charcoal, // Floating windows in charcoal
//         window_stroke: Stroke::new(1.0, yellow_dark), // Subtle yellow window borders
//         panel_fill: soft_black, // Main panels in deep black
//         faint_bg_color: dark_black, // Very subtle background variations
//         extreme_bg_color: pure_black, // Text inputs, scrollbars in pure black
//         popup_shadow: egui::epaint::Shadow {
//             offset: [0.0, 8.0].into(),
//             blur: 24.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(200), // Stronger shadow for contrast
//         },
//         window_shadow: egui::epaint::Shadow {
//             offset: [0.0, 8.0].into(),
//             blur: 24.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(200),
//         },
//         striped: false,
//         clip_rect_margin: 0.0,
//         ..Default::default()
//     }
// }

// pub fn black_theme() -> Visuals {
//     let dark_black = Color32::from_rgb(8, 8, 8); // #080808 <- Recommended background match
//     let soft_black = Color32::from_rgb(16, 16, 16); // #101010
//     let charcoal = Color32::from_rgb(24, 24, 24); // #181818
//     let dark_gray = Color32::from_rgb(32, 32, 32); // #202020
//     let medium_gray = Color32::from_rgb(48, 48, 48); // #303030
//     let light_gray = Color32::from_rgb(128, 128, 128); // #808080
//     let white = Color32::from_rgb(255, 255, 255); // Primary text

//     // Yellow accent colors
//     let yellow_primary = Color32::from_rgb(255, 215, 0); // Gold yellow
//     let yellow_bright = Color32::from_rgb(255, 235, 59); // Bright yellow
//     let yellow_dark = Color32::from_rgb(218, 165, 32); // Darker yellow

//     Visuals {
//         dark_mode: true,
//         override_text_color: None,
//         widgets: Widgets {
//             noninteractive: WidgetVisuals {
//                 // Use the lifted black for deep background areas
//                 bg_fill: dark_black,
//                 weak_bg_fill: soft_black,
//                 bg_stroke: Stroke::new(1.0, charcoal),
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, light_gray),
//                 expansion: 0.0,
//             },
//             inactive: WidgetVisuals {
//                 bg_fill: dark_gray,
//                 weak_bg_fill: Color32::TRANSPARENT,
//                 bg_stroke: Stroke::NONE,
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, white),
//                 expansion: 0.0,
//             },
//             hovered: WidgetVisuals {
//                 bg_fill: medium_gray,
//                 weak_bg_fill: medium_gray,
//                 bg_stroke: Stroke::new(1.0, yellow_dark),
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.5, yellow_bright),
//                 expansion: 2.0,
//             },
//             active: WidgetVisuals {
//                 bg_fill: charcoal,
//                 weak_bg_fill: charcoal,
//                 bg_stroke: Stroke::new(2.0, yellow_primary),
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(2.0, yellow_primary),
//                 expansion: 2.0,
//             },
//             open: WidgetVisuals {
//                 bg_fill: medium_gray,
//                 weak_bg_fill: medium_gray,
//                 bg_stroke: Stroke::new(1.0, yellow_dark),
//                 rounding: Rounding::same(4.0),
//                 fg_stroke: Stroke::new(1.0, yellow_bright),
//                 expansion: 2.0,
//             },
//         },
//         selection: Selection {
//             bg_fill: Color32::from_rgba_unmultiplied(255, 215, 0, 60),
//             stroke: Stroke::new(2.0, yellow_primary),
//         },
//         window_rounding: Rounding::same(6.0),
//         window_fill: charcoal,
//         window_stroke: Stroke::new(1.0, yellow_dark),
//         // 🔄 Change: Set main panel background to dark_black
//         panel_fill: dark_black,
//         faint_bg_color: dark_black,
//         // 🔄 Change: Set extreme background (e.g., scrollbars, text inputs) to dark_black
//         extreme_bg_color: dark_black,
//         popup_shadow: egui::epaint::Shadow {
//             offset: [0.0, 8.0].into(),
//             blur: 24.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(200),
//         },
//         window_shadow: egui::epaint::Shadow {
//             offset: [0.0, 8.0].into(),
//             blur: 24.0,
//             spread: 0.0,
//             color: Color32::from_black_alpha(200),
//         },
//         striped: false,
//         clip_rect_margin: 0.0,
//         ..Default::default()
//     }
// }

pub fn black_theme() -> Visuals {
    // --- Obsidian Glass Design System ---
    // A cohesive dark theme inspired by Linear, Vercel, and Raycast.
    // Premium feel with subtle depth and emerald accents.
    use super::palette::{accent, bg, border, highlight, semantic, text};

    // Subtle corner radius for modern feel (not too rounded, not harsh)
    let corner_radius = CornerRadius::same(4);

    // Soft shadow for elevated elements
    let soft_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };

    Visuals {
        dark_mode: true,
        override_text_color: None,

        // --- Widget Visuals ---
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: bg::SURFACE,
                weak_bg_fill: bg::SURFACE,
                bg_stroke: Stroke::new(1.0, border::SUBTLE),
                corner_radius,
                fg_stroke: Stroke::new(1.0, text::PRIMARY),
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: bg::ELEVATED,
                weak_bg_fill: Color32::TRANSPARENT,
                bg_stroke: Stroke::new(1.0, border::SUBTLE),
                corner_radius,
                fg_stroke: Stroke::new(1.0, text::PRIMARY),
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: bg::HOVER,
                weak_bg_fill: bg::HOVER,
                bg_stroke: Stroke::new(1.0, border::DEFAULT),
                corner_radius,
                fg_stroke: Stroke::new(1.0, text::PRIMARY),
                expansion: 1.0,
            },
            active: WidgetVisuals {
                bg_fill: accent::PRIMARY,
                weak_bg_fill: accent::PRIMARY,
                bg_stroke: Stroke::new(1.0, accent::PRIMARY),
                corner_radius,
                fg_stroke: Stroke::new(1.0, bg::BASE), // Dark text on accent
                expansion: 0.0,
            },
            open: WidgetVisuals {
                bg_fill: bg::ELEVATED,
                weak_bg_fill: bg::ELEVATED,
                bg_stroke: Stroke::new(1.0, accent::PRIMARY),
                corner_radius,
                fg_stroke: Stroke::new(1.0, accent::PRIMARY),
                expansion: 1.0,
            },
        },

        // --- Selection ---
        selection: Selection {
            bg_fill: highlight::SELECTION,
            stroke: Stroke::new(1.0, accent::PRIMARY),
        },

        // --- Window & Panel ---
        window_fill: bg::SURFACE,
        window_stroke: Stroke::new(1.0, border::SUBTLE),
        panel_fill: bg::BASE,
        faint_bg_color: bg::SURFACE,
        extreme_bg_color: bg::BASE,

        // --- Shadows ---
        popup_shadow: soft_shadow,
        window_shadow: soft_shadow,

        // --- Semantic colors ---
        error_fg_color: semantic::ERROR,
        warn_fg_color: semantic::WARNING,
        hyperlink_color: accent::PRIMARY,

        // --- Text cursor ---
        text_cursor: TextCursorStyle {
            stroke: Stroke::new(2.0, accent::PRIMARY),
            blink: true,
            on_duration: 0.5,
            off_duration: 0.5,
            ..Default::default()
        },

        striped: false,
        clip_rect_margin: 0.0,
        ..Default::default()
    }
}
