use egui::{
    Color32, Rounding, Stroke, Visuals,
    style::{Selection, WidgetVisuals, Widgets},
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
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(1.0, black),
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: white,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(1.0, black),
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(1.0, black),
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: light_black,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(1.5, black),
                rounding: Rounding::same(3.0),
                fg_stroke: Stroke::new(1.5, black),
                expansion: 1.0,
            },
            active: WidgetVisuals {
                bg_fill: light_black,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(2.0, black),
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(2.0, black),
                expansion: 1.0,
            },
            open: WidgetVisuals {
                bg_fill: light_black,
                weak_bg_fill: light_black,
                bg_stroke: Stroke::new(1.0, black),
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(1.0, black),
                expansion: 0.0,
            },
        },
        selection: Selection {
            bg_fill: Color32::from_rgb(144, 209, 255),
            stroke: Stroke::new(1.0, Color32::from_rgb(0, 148, 255)),
        },
        window_rounding: Rounding::same(6.0),
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

pub fn black_theme() -> Visuals {
    // Define grayscale palette inspired by Rerun's color_table
    let s100 = Color32::from_rgb(28, 37, 38); // Very dark gray (panel background, top bar)
    let s150 = Color32::from_rgb(42, 52, 54); // Slightly lighter (bottom bar, faint bg)
    let s250 = Color32::from_rgb(74, 84, 86); // Strokes, floating elements
    let s300 = Color32::from_rgb(90, 100, 102); // Inactive widget fill
    let s325 = Color32::from_rgb(100, 110, 112); // Hovered/active widget fill
    let s550 = Color32::from_rgb(150, 160, 162); // Subdued text
    let s775 = Color32::from_rgb(200, 210, 212); // Default text
    let s1000 = Color32::from_rgb(255, 255, 255); // Strong text
    let pure_black = Color32::from_rgb(0, 0, 0);

    Visuals {
        dark_mode: true,
        override_text_color: None, // Remove ORANGE override to use grayscale text
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: s100,                     // Panel-like background
                weak_bg_fill: s100,                // Consistent with panel
                bg_stroke: Stroke::new(1.0, s250), // Subtle separator lines
                rounding: Rounding::same(4.0),     // Slightly larger for modern look
                fg_stroke: Stroke::new(1.0, s550), // Subdued text for non-interactive
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: s300,                      // Slightly lighter for checkboxes, etc.
                weak_bg_fill: Color32::TRANSPARENT, // No background for buttons
                bg_stroke: Stroke::NONE,            // No stroke for inactive buttons
                rounding: Rounding::same(4.0),
                fg_stroke: Stroke::new(1.0, s775), // Default text
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: s325, // Subtle highlight for hover
                weak_bg_fill: s325,
                bg_stroke: Stroke::NONE, // No stroke to keep clean
                rounding: Rounding::same(4.0),
                fg_stroke: Stroke::new(1.5, s775), // Slightly bolder text
                expansion: 2.0,                    // Rerun's expansion for hover feedback
            },
            active: WidgetVisuals {
                bg_fill: s325, // Same as hover for consistency
                weak_bg_fill: s325,
                bg_stroke: Stroke::NONE,
                rounding: Rounding::same(4.0),
                fg_stroke: Stroke::new(2.0, s1000), // Strong text for active state
                expansion: 2.0,
            },
            open: WidgetVisuals {
                bg_fill: s325, // Consistent with hover/active
                weak_bg_fill: s325,
                bg_stroke: Stroke::NONE,
                rounding: Rounding::same(4.0),
                fg_stroke: Stroke::new(1.0, s775),
                expansion: 2.0,
            },
        },
        selection: Selection {
            bg_fill: Color32::from_rgb(0, 82, 165), // Rerun's blue(S350)-like color
            stroke: Stroke::new(2.0, Color32::from_rgb(173, 184, 255)), // Brighter blue
        },
        window_rounding: Rounding::same(6.0), // Matches Rerun's window_corner_radius
        window_fill: s250,                    // Floating elements like tooltips/menus
        window_stroke: Stroke::NONE,          // Rerun avoids window strokes
        panel_fill: s100,                     // Main panel background
        faint_bg_color: s150,                 // For zebra stripes or subtle backgrounds
        extreme_bg_color: pure_black,         // Text edits, scroll bars
        popup_shadow: egui::epaint::Shadow {
            offset: [0.0, 15.0].into(),
            blur: 50.0,
            spread: 0.0,
            color: Color32::from_black_alpha(128), // Rerun's black-based shadow
        },
        window_shadow: egui::epaint::Shadow {
            offset: [0.0, 15.0].into(),
            blur: 50.0,
            spread: 0.0,
            color: Color32::from_black_alpha(128),
        },
        striped: false,        // Disable stripes like Rerun
        clip_rect_margin: 0.0, // Avoid visual glitches
        ..Default::default()
    }
}

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
                rounding: Rounding::same(2.0),    // Subtle rounding for retro feel
                fg_stroke: Stroke::new(1.0, fg4), // Subdued text
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: bg0_h,                     // Consistent dark background
                weak_bg_fill: Color32::TRANSPARENT, // No button background
                bg_stroke: Stroke::new(1.0, bg1),   // Light stroke for definition
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(1.0, fg1), // Default text
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: yellow, // Gruvbox yellow for hover
                weak_bg_fill: aqua,
                bg_stroke: Stroke::new(1.5, bg1), // Slightly bolder stroke
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(1.5, fg0), // Strong text
                expansion: 1.0,                   // Subtle expansion
            },
            active: WidgetVisuals {
                bg_fill: orange, // Gruvbox orange for active
                weak_bg_fill: orange,
                bg_stroke: Stroke::new(1.5, bg1),
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(2.0, fg0), // Strong text
                expansion: 1.0,
            },
            open: WidgetVisuals {
                bg_fill: aqua, // Consistent with hover
                weak_bg_fill: aqua,
                bg_stroke: Stroke::new(1.0, bg1),
                rounding: Rounding::same(2.0),
                fg_stroke: Stroke::new(1.0, fg1), // Default text
                expansion: 1.0,
            },
        },
        selection: Selection {
            bg_fill: blue,                  // Gruvbox blue for selections
            stroke: Stroke::new(1.5, aqua), // Lighter aqua for stroke
        },
        window_rounding: Rounding::same(4.0), // Moderate rounding for windows
        window_fill: bg2,                     // Lighter bg for tooltips/menus
        window_stroke: Stroke::new(1.0, bg1), // Subtle window stroke
        panel_fill: bg0_h,                    // Hard dark background
        faint_bg_color: bg1,                  // Subtle highlight for stripes
        extreme_bg_color: bg0,                // Slightly lighter for text edits
        popup_shadow: egui::epaint::Shadow {
            offset: [2.0, 2.0].into(),
            blur: 10.0,
            spread: 0.0,
            color: Color32::from_black_alpha(64), // Subtle shadow
        },
        window_shadow: egui::epaint::Shadow {
            offset: [2.0, 2.0].into(),
            blur: 10.0,
            spread: 0.0,
            color: Color32::from_black_alpha(64),
        },
        striped: true, // Gruvbox often uses stripes for tables
        clip_rect_margin: 0.0,
        error_fg_color: red,   // Gruvbox red for errors
        warn_fg_color: orange, // Gruvbox orange for warnings
        hyperlink_color: blue, // Gruvbox blue for links
        ..Default::default()
    }
}
