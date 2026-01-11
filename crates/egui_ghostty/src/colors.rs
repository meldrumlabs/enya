//! ANSI color palette and color scheme support.

use egui::Color32;
use ghostty_vt::Rgb;

/// Standard ANSI color indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AnsiColor {
    Black = 0,
    Red = 1,
    Green = 2,
    Yellow = 3,
    Blue = 4,
    Magenta = 5,
    Cyan = 6,
    White = 7,
    BrightBlack = 8,
    BrightRed = 9,
    BrightGreen = 10,
    BrightYellow = 11,
    BrightBlue = 12,
    BrightMagenta = 13,
    BrightCyan = 14,
    BrightWhite = 15,
}

/// A terminal color scheme with the standard 16 ANSI colors.
#[derive(Clone, Debug)]
pub struct ColorScheme {
    /// The 16 standard ANSI colors.
    pub colors: [Rgb; 16],
    /// Foreground color.
    pub foreground: Rgb,
    /// Background color.
    pub background: Rgb,
    /// Cursor color.
    pub cursor: Rgb,
    /// Selection background color.
    pub selection_bg: Rgb,
}

impl Default for ColorScheme {
    fn default() -> Self {
        // Default to a dark theme similar to Catppuccin Mocha
        Self {
            colors: [
                // Normal colors
                Rgb::new(0x45, 0x47, 0x5A), // Black
                Rgb::new(0xF3, 0x8B, 0xA8), // Red
                Rgb::new(0xA6, 0xE3, 0xA1), // Green
                Rgb::new(0xF9, 0xE2, 0xAF), // Yellow
                Rgb::new(0x89, 0xB4, 0xFA), // Blue
                Rgb::new(0xF5, 0xC2, 0xE7), // Magenta
                Rgb::new(0x94, 0xE2, 0xD5), // Cyan
                Rgb::new(0xBA, 0xC2, 0xDE), // White
                // Bright colors
                Rgb::new(0x58, 0x5B, 0x70), // Bright Black
                Rgb::new(0xF3, 0x8B, 0xA8), // Bright Red
                Rgb::new(0xA6, 0xE3, 0xA1), // Bright Green
                Rgb::new(0xF9, 0xE2, 0xAF), // Bright Yellow
                Rgb::new(0x89, 0xB4, 0xFA), // Bright Blue
                Rgb::new(0xF5, 0xC2, 0xE7), // Bright Magenta
                Rgb::new(0x94, 0xE2, 0xD5), // Bright Cyan
                Rgb::new(0xA6, 0xAD, 0xC8), // Bright White
            ],
            foreground: Rgb::new(0xCD, 0xD6, 0xF4),
            background: Rgb::new(0x1E, 0x1E, 0x2E),
            cursor: Rgb::new(0xF5, 0xE0, 0xDC),
            selection_bg: Rgb::new(0x45, 0x47, 0x5A),
        }
    }
}

impl ColorScheme {
    /// Get an ANSI color by index.
    pub fn ansi(&self, index: u8) -> Rgb {
        if index < 16 {
            self.colors[index as usize]
        } else {
            // Extended 256-color palette - compute from index
            self.extended_color(index)
        }
    }

    /// Compute extended 256-color palette color.
    fn extended_color(&self, index: u8) -> Rgb {
        if index < 16 {
            return self.colors[index as usize];
        }

        if index < 232 {
            // Color cube: 6x6x6
            let idx = index - 16;
            let r = idx / 36;
            let g = (idx % 36) / 6;
            let b = idx % 6;
            Rgb::new(
                if r == 0 { 0 } else { 55 + r * 40 },
                if g == 0 { 0 } else { 55 + g * 40 },
                if b == 0 { 0 } else { 55 + b * 40 },
            )
        } else {
            // Grayscale: 24 shades
            let gray = 8 + (index - 232) * 10;
            Rgb::new(gray, gray, gray)
        }
    }

    /// Create a Nord-inspired color scheme.
    pub fn nord() -> Self {
        Self {
            colors: [
                Rgb::new(0x3B, 0x42, 0x52), // Black
                Rgb::new(0xBF, 0x61, 0x6A), // Red
                Rgb::new(0xA3, 0xBE, 0x8C), // Green
                Rgb::new(0xEB, 0xCB, 0x8B), // Yellow
                Rgb::new(0x81, 0xA1, 0xC1), // Blue
                Rgb::new(0xB4, 0x8E, 0xAD), // Magenta
                Rgb::new(0x88, 0xC0, 0xD0), // Cyan
                Rgb::new(0xE5, 0xE9, 0xF0), // White
                Rgb::new(0x4C, 0x56, 0x6A), // Bright Black
                Rgb::new(0xBF, 0x61, 0x6A), // Bright Red
                Rgb::new(0xA3, 0xBE, 0x8C), // Bright Green
                Rgb::new(0xEB, 0xCB, 0x8B), // Bright Yellow
                Rgb::new(0x81, 0xA1, 0xC1), // Bright Blue
                Rgb::new(0xB4, 0x8E, 0xAD), // Bright Magenta
                Rgb::new(0x8F, 0xBC, 0xBB), // Bright Cyan
                Rgb::new(0xEC, 0xEF, 0xF4), // Bright White
            ],
            foreground: Rgb::new(0xEC, 0xEF, 0xF4),
            background: Rgb::new(0x2E, 0x34, 0x40),
            cursor: Rgb::new(0xD8, 0xDE, 0xE9),
            selection_bg: Rgb::new(0x43, 0x4C, 0x5E),
        }
    }
}

/// Convert ghostty Rgb to egui Color32.
pub fn rgb_to_color32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Convert egui Color32 to ghostty Rgb.
#[allow(dead_code)]
pub fn color32_to_rgb(color: Color32) -> Rgb {
    Rgb::new(color.r(), color.g(), color.b())
}
