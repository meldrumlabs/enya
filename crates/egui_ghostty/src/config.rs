//! Terminal configuration.

use ghostty_vt::Rgb;

/// Configuration for a terminal instance.
#[derive(Clone, Copy, Debug)]
pub struct TerminalConfig {
    /// Number of columns (width in characters).
    pub cols: u16,
    /// Number of rows (height in lines).
    pub rows: u16,
    /// Default foreground color.
    pub default_fg: Rgb,
    /// Default background color.
    pub default_bg: Rgb,
    /// Whether to track and update the window title from OSC sequences.
    pub update_window_title: bool,
    /// Cursor blink interval in seconds (0 to disable blinking).
    pub cursor_blink_interval: f32,
    /// Whether to show a block cursor (vs beam cursor).
    pub block_cursor: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            default_fg: Rgb::new(0xFF, 0xFF, 0xFF),
            default_bg: Rgb::new(0x1E, 0x1E, 0x2E), // Catppuccin-ish dark
            update_window_title: true,
            cursor_blink_interval: 0.5,
            block_cursor: true,
        }
    }
}

impl TerminalConfig {
    /// Create a config with custom dimensions.
    pub fn with_size(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            ..Default::default()
        }
    }

    /// Set the default colors.
    pub fn with_colors(mut self, fg: Rgb, bg: Rgb) -> Self {
        self.default_fg = fg;
        self.default_bg = bg;
        self
    }

    /// Set cursor style.
    pub fn with_block_cursor(mut self, block: bool) -> Self {
        self.block_cursor = block;
        self
    }
}
