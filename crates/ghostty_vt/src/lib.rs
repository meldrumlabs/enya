//! Safe Rust bindings for the Ghostty virtual terminal library.
//!
//! This crate provides a safe, ergonomic interface to ghostty's terminal emulator core,
//! wrapping the raw FFI bindings from `ghostty_vt_sys`.

use std::ffi::c_void;
use std::fmt;
use std::ptr::NonNull;

/// Error type for terminal operations.
#[derive(Debug)]
pub enum Error {
    /// Failed to create a new terminal instance.
    CreateFailed,
    /// Failed to feed bytes to the terminal.
    FeedFailed(i32),
    /// Failed to scroll the viewport.
    ScrollFailed(i32),
    /// Failed to dump viewport content.
    DumpFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::CreateFailed => write!(f, "terminal create failed"),
            Error::FeedFailed(code) => write!(f, "terminal feed failed: {code}"),
            Error::ScrollFailed(code) => write!(f, "terminal scroll failed: {code}"),
            Error::DumpFailed => write!(f, "terminal dump failed"),
        }
    }
}

impl std::error::Error for Error {}

/// A terminal emulator instance.
///
/// Wraps the underlying ghostty terminal and provides safe access to its functionality.
pub struct Terminal {
    ptr: NonNull<c_void>,
}

// SAFETY: The underlying ghostty terminal is thread-safe when accessed through
// the C API, as each operation is atomic.
unsafe impl Send for Terminal {}

/// An RGB color value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Create a new RGB color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// White color (255, 255, 255).
    pub const WHITE: Self = Self::new(255, 255, 255);

    /// Black color (0, 0, 0).
    pub const BLACK: Self = Self::new(0, 0, 0);
}

/// Style information for a single cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellStyle {
    /// Foreground color.
    pub fg: Rgb,
    /// Background color.
    pub bg: Rgb,
    /// Style flags (bold, italic, underline, etc.).
    pub flags: u8,
}

impl CellStyle {
    /// Check if the inverse flag is set.
    pub fn is_inverse(&self) -> bool {
        self.flags & 0x01 != 0
    }

    /// Check if the bold flag is set.
    pub fn is_bold(&self) -> bool {
        self.flags & 0x02 != 0
    }

    /// Check if the italic flag is set.
    pub fn is_italic(&self) -> bool {
        self.flags & 0x04 != 0
    }

    /// Check if the underline flag is set.
    pub fn is_underline(&self) -> bool {
        self.flags & 0x08 != 0
    }

    /// Check if the faint flag is set.
    pub fn is_faint(&self) -> bool {
        self.flags & 0x10 != 0
    }

    /// Check if the invisible flag is set.
    pub fn is_invisible(&self) -> bool {
        self.flags & 0x20 != 0
    }

    /// Check if the strikethrough flag is set.
    pub fn is_strikethrough(&self) -> bool {
        self.flags & 0x40 != 0
    }
}

/// A run of cells with the same style.
///
/// Style runs are an optimization for rendering, grouping consecutive cells
/// that share the same styling attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleRun {
    /// Starting column (1-indexed).
    pub start_col: u16,
    /// Ending column (1-indexed, inclusive).
    pub end_col: u16,
    /// Foreground color.
    pub fg: Rgb,
    /// Background color.
    pub bg: Rgb,
    /// Style flags.
    pub flags: u8,
}

impl StyleRun {
    /// Check if the inverse flag is set.
    pub fn is_inverse(&self) -> bool {
        self.flags & 0x01 != 0
    }

    /// Check if the bold flag is set.
    pub fn is_bold(&self) -> bool {
        self.flags & 0x02 != 0
    }

    /// Check if the italic flag is set.
    pub fn is_italic(&self) -> bool {
        self.flags & 0x04 != 0
    }

    /// Check if the underline flag is set.
    pub fn is_underline(&self) -> bool {
        self.flags & 0x08 != 0
    }

    /// Check if the faint flag is set.
    pub fn is_faint(&self) -> bool {
        self.flags & 0x10 != 0
    }

    /// Check if the strikethrough flag is set.
    pub fn is_strikethrough(&self) -> bool {
        self.flags & 0x40 != 0
    }
}

/// Keyboard modifier state.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
}

impl KeyModifiers {
    /// Convert modifiers to a bitmask for the FFI.
    fn bits(self) -> u16 {
        let mut bits = 0u16;
        if self.shift {
            bits |= 0x0001;
        }
        if self.control {
            bits |= 0x0002;
        }
        if self.alt {
            bits |= 0x0004;
        }
        if self.super_key {
            bits |= 0x0008;
        }
        bits
    }
}

/// Encode a named key with modifiers into terminal escape sequences.
///
/// Supported key names:
/// - Arrow keys: "up", "down", "left", "right"
/// - Navigation: "home", "end", "pageup"/"page_up", "pagedown"/"page_down"
/// - Editing: "insert", "delete", "backspace"
/// - Control: "enter", "tab", "escape"
/// - Function keys: "f1" through "f12"
///
/// Returns `None` if the key name is not recognized.
pub fn encode_key_named(name: &str, modifiers: KeyModifiers) -> Option<Vec<u8>> {
    if name.is_empty() {
        return None;
    }

    let bytes = unsafe {
        ghostty_vt_sys::ghostty_vt_encode_key_named(name.as_ptr(), name.len(), modifiers.bits())
    };
    if bytes.ptr.is_null() || bytes.len == 0 {
        return None;
    }

    let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
    let out = slice.to_vec();
    unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
    Some(out)
}

impl Terminal {
    /// Create a new terminal with the specified dimensions.
    ///
    /// # Arguments
    /// * `cols` - Number of columns (width in characters)
    /// * `rows` - Number of rows (height in lines)
    pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
        let ptr = unsafe { ghostty_vt_sys::ghostty_vt_terminal_new(cols, rows) };
        let ptr = NonNull::new(ptr).ok_or(Error::CreateFailed)?;
        Ok(Self { ptr })
    }

    /// Set the default foreground and background colors.
    pub fn set_default_colors(&mut self, fg: Rgb, bg: Rgb) {
        unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_set_default_colors(
                self.ptr.as_ptr(),
                fg.r,
                fg.g,
                fg.b,
                bg.r,
                bg.g,
                bg.b,
            )
        }
    }

    /// Set an ANSI palette color (0-255).
    ///
    /// This updates the terminal's internal palette and triggers a redraw,
    /// causing all cells using this color index to display the new color.
    pub fn set_ansi_color(&mut self, index: u8, color: Rgb) {
        unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_set_ansi_color(
                self.ptr.as_ptr(),
                index,
                color.r,
                color.g,
                color.b,
            )
        }
    }

    /// Feed bytes into the terminal for processing.
    ///
    /// This is the primary input method - bytes from PTY output should be fed here.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let rc = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_feed(self.ptr.as_ptr(), bytes.as_ptr(), bytes.len())
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(Error::FeedFailed(rc))
        }
    }

    /// Resize the terminal to new dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Error> {
        let rc =
            unsafe { ghostty_vt_sys::ghostty_vt_terminal_resize(self.ptr.as_ptr(), cols, rows) };
        if rc == 0 {
            Ok(())
        } else {
            Err(Error::ScrollFailed(rc))
        }
    }

    /// Dump the entire viewport as a string.
    pub fn dump_viewport(&self) -> Result<String, Error> {
        let bytes = unsafe { ghostty_vt_sys::ghostty_vt_terminal_dump_viewport(self.ptr.as_ptr()) };
        if bytes.ptr.is_null() {
            return Err(Error::DumpFailed);
        }

        let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
        let s = String::from_utf8_lossy(slice).into_owned();
        unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
        Ok(s)
    }

    /// Dump a single viewport row as a string.
    ///
    /// Row is 0-indexed from the top of the viewport.
    pub fn dump_viewport_row(&self, row: u16) -> Result<String, Error> {
        let bytes = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_dump_viewport_row(self.ptr.as_ptr(), row)
        };
        if bytes.ptr.is_null() {
            return Err(Error::DumpFailed);
        }

        let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
        let s = String::from_utf8_lossy(slice).into_owned();
        unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
        Ok(s)
    }

    /// Get per-cell style information for a viewport row.
    pub fn dump_viewport_row_cell_styles(&self, row: u16) -> Result<Vec<CellStyle>, Error> {
        let bytes = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_dump_viewport_row_cell_styles(
                self.ptr.as_ptr(),
                row,
            )
        };
        if bytes.ptr.is_null() {
            return Err(Error::DumpFailed);
        }
        if bytes.len == 0 {
            unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
            return Ok(Vec::new());
        }
        if bytes.len % 8 != 0 {
            unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
            return Err(Error::DumpFailed);
        }

        let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
        let mut out = Vec::with_capacity(bytes.len / 8);
        for chunk in slice.chunks_exact(8) {
            out.push(CellStyle {
                fg: Rgb {
                    r: chunk[0],
                    g: chunk[1],
                    b: chunk[2],
                },
                bg: Rgb {
                    r: chunk[3],
                    g: chunk[4],
                    b: chunk[5],
                },
                flags: chunk[6],
            });
        }

        unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
        Ok(out)
    }

    /// Get style runs for a viewport row.
    ///
    /// Style runs group consecutive cells with the same styling, which is more
    /// efficient for rendering than per-cell styles.
    pub fn dump_viewport_row_style_runs(&self, row: u16) -> Result<Vec<StyleRun>, Error> {
        let bytes = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_dump_viewport_row_style_runs(self.ptr.as_ptr(), row)
        };
        if bytes.ptr.is_null() {
            return Err(Error::DumpFailed);
        }
        if bytes.len == 0 {
            unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
            return Ok(Vec::new());
        }
        if bytes.len % 12 != 0 {
            unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
            return Err(Error::DumpFailed);
        }

        let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
        let mut out = Vec::with_capacity(bytes.len / 12);
        for chunk in slice.chunks_exact(12) {
            out.push(StyleRun {
                start_col: u16::from_ne_bytes([chunk[0], chunk[1]]),
                end_col: u16::from_ne_bytes([chunk[2], chunk[3]]),
                fg: Rgb {
                    r: chunk[4],
                    g: chunk[5],
                    b: chunk[6],
                },
                bg: Rgb {
                    r: chunk[7],
                    g: chunk[8],
                    b: chunk[9],
                },
                flags: chunk[10],
            });
        }

        unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
        Ok(out)
    }

    /// Take the list of dirty (modified) viewport rows.
    ///
    /// This consumes the dirty state - subsequent calls will return empty
    /// until more rows are modified.
    pub fn take_dirty_viewport_rows(&mut self, rows: u16) -> Result<Vec<u16>, Error> {
        let bytes = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_take_dirty_viewport_rows(self.ptr.as_ptr(), rows)
        };
        if bytes.ptr.is_null() || bytes.len == 0 {
            return Ok(Vec::new());
        }
        if bytes.len % 2 != 0 {
            unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
            return Err(Error::DumpFailed);
        }

        let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
        let mut out = Vec::with_capacity(bytes.len / 2);
        for chunk in slice.chunks_exact(2) {
            out.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
        Ok(out)
    }

    /// Take the viewport scroll delta since the last call.
    ///
    /// Returns the number of lines scrolled (positive = down, negative = up).
    pub fn take_viewport_scroll_delta(&mut self) -> i32 {
        unsafe { ghostty_vt_sys::ghostty_vt_terminal_take_viewport_scroll_delta(self.ptr.as_ptr()) }
    }

    /// Get the current cursor position.
    ///
    /// Returns `(col, row)` where both are 1-indexed.
    pub fn cursor_position(&self) -> Option<(u16, u16)> {
        let mut col: u16 = 0;
        let mut row: u16 = 0;
        let ok = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_cursor_position(
                self.ptr.as_ptr(),
                &mut col as *mut u16,
                &mut row as *mut u16,
            )
        };
        ok.then_some((col, row))
    }

    /// Get the hyperlink URL at the specified position.
    ///
    /// Col and row are 1-indexed.
    pub fn hyperlink_at(&self, col: u16, row: u16) -> Option<String> {
        let bytes = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_hyperlink_at(self.ptr.as_ptr(), col, row)
        };
        if bytes.ptr.is_null() || bytes.len == 0 {
            return None;
        }

        let slice = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
        let s = String::from_utf8_lossy(slice).into_owned();
        unsafe { ghostty_vt_sys::ghostty_vt_bytes_free(bytes) };
        Some(s)
    }

    /// Scroll the viewport by the specified number of lines.
    ///
    /// Positive values scroll down (show older content), negative scroll up.
    pub fn scroll_viewport(&mut self, delta_lines: i32) -> Result<(), Error> {
        let rc = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_scroll_viewport(self.ptr.as_ptr(), delta_lines)
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(Error::ScrollFailed(rc))
        }
    }

    /// Scroll the viewport to the top of the scrollback buffer.
    pub fn scroll_viewport_top(&mut self) -> Result<(), Error> {
        let rc =
            unsafe { ghostty_vt_sys::ghostty_vt_terminal_scroll_viewport_top(self.ptr.as_ptr()) };
        if rc == 0 {
            Ok(())
        } else {
            Err(Error::ScrollFailed(rc))
        }
    }

    /// Scroll the viewport to the bottom (most recent output).
    pub fn scroll_viewport_bottom(&mut self) -> Result<(), Error> {
        let rc = unsafe {
            ghostty_vt_sys::ghostty_vt_terminal_scroll_viewport_bottom(self.ptr.as_ptr())
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(Error::ScrollFailed(rc))
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        unsafe { ghostty_vt_sys::ghostty_vt_terminal_free(self.ptr.as_ptr()) }
    }
}

/// Convenience function to create a new terminal.
pub fn terminal_new(cols: u16, rows: u16) -> Result<Terminal, Error> {
    Terminal::new(cols, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_constants() {
        assert_eq!(Rgb::WHITE, Rgb::new(255, 255, 255));
        assert_eq!(Rgb::BLACK, Rgb::new(0, 0, 0));
    }

    #[test]
    fn test_key_modifiers_bits() {
        let mods = KeyModifiers {
            shift: true,
            control: true,
            alt: false,
            super_key: false,
        };
        assert_eq!(mods.bits(), 0x0003);
    }

    #[test]
    fn test_cell_style_flags() {
        let style = CellStyle {
            fg: Rgb::WHITE,
            bg: Rgb::BLACK,
            flags: 0x0F, // inverse, bold, italic, underline
        };
        assert!(style.is_inverse());
        assert!(style.is_bold());
        assert!(style.is_italic());
        assert!(style.is_underline());
        assert!(!style.is_faint());
        assert!(!style.is_invisible());
        assert!(!style.is_strikethrough());
    }
}
