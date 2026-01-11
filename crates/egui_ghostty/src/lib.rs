//! Terminal emulator widget for egui using ghostty's VT library.
//!
//! This crate provides a terminal emulator that can be embedded in egui applications.
//! It uses ghostty's virtual terminal library for accurate terminal emulation and
//! portable-pty for cross-platform PTY support.
//!
//! # Example
//!
//! ```ignore
//! use egui_ghostty::{TerminalWidget, TerminalConfig};
//!
//! let config = TerminalConfig::default();
//! let mut terminal = TerminalWidget::new(config, "/bin/zsh").unwrap();
//!
//! // In your egui render loop:
//! terminal.show(ui);
//! ```

mod colors;
mod config;
mod input;
mod session;
mod widget;

pub use colors::{AnsiColor, ColorScheme};
pub use config::TerminalConfig;
pub use session::TerminalSession;
pub use widget::{TerminalResponse, TerminalWidget};

/// Re-export ghostty types that users might need.
pub use ghostty_vt::{KeyModifiers, Rgb};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_scheme_default() {
        let scheme = ColorScheme::default();
        // Verify foreground and background are set
        assert_ne!(scheme.foreground.r, 0);
        assert_ne!(scheme.foreground.g, 0);
        assert_ne!(scheme.foreground.b, 0);
        // Background should be dark
        assert!(scheme.background.r < 100);
        assert!(scheme.background.g < 100);
        assert!(scheme.background.b < 100);
    }

    #[test]
    fn test_color_scheme_ansi_colors() {
        let scheme = ColorScheme::default();
        // First 16 colors should return from the colors array
        for i in 0..16 {
            let color = scheme.ansi(i);
            assert_eq!(color, scheme.colors[i as usize]);
        }
    }

    #[test]
    fn test_color_scheme_extended_colors() {
        let scheme = ColorScheme::default();
        // Color cube (16-231): 6x6x6 colors
        let color = scheme.ansi(16); // First color in cube (black)
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);

        // Grayscale (232-255)
        let gray_start = scheme.ansi(232);
        assert_eq!(gray_start.r, 8);
        assert_eq!(gray_start.g, 8);
        assert_eq!(gray_start.b, 8);

        let gray_end = scheme.ansi(255);
        assert_eq!(gray_end.r, 238);
        assert_eq!(gray_end.g, 238);
        assert_eq!(gray_end.b, 238);
    }

    #[test]
    fn test_color_scheme_nord() {
        let scheme = ColorScheme::nord();
        // Nord background is #2E3440
        assert_eq!(scheme.background.r, 0x2E);
        assert_eq!(scheme.background.g, 0x34);
        assert_eq!(scheme.background.b, 0x40);
    }

    #[test]
    fn test_terminal_config_default() {
        let config = TerminalConfig::default();
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
        assert!(config.block_cursor);
        assert_eq!(config.cursor_blink_interval, 0.5);
    }

    #[test]
    fn test_terminal_config_builder() {
        let config = TerminalConfig::with_size(120, 40)
            .with_block_cursor(false)
            .with_colors(Rgb::new(0, 0, 0), Rgb::new(255, 255, 255));
        assert_eq!(config.cols, 120);
        assert_eq!(config.rows, 40);
        assert!(!config.block_cursor);
        assert_eq!(config.default_fg.r, 0);
        assert_eq!(config.default_bg.r, 255);
    }

    #[test]
    fn test_rgb_to_color32() {
        use colors::rgb_to_color32;
        let rgb = Rgb::new(100, 150, 200);
        let color = rgb_to_color32(rgb);
        assert_eq!(color.r(), 100);
        assert_eq!(color.g(), 150);
        assert_eq!(color.b(), 200);
    }
}
