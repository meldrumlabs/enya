//! Keyboard and mouse input handling for the terminal.

use egui::{Key, Modifiers};
use ghostty_vt::KeyModifiers;

/// Convert egui modifiers to ghostty KeyModifiers.
pub fn egui_to_ghostty_modifiers(mods: &Modifiers) -> KeyModifiers {
    KeyModifiers {
        shift: mods.shift,
        control: mods.ctrl,
        alt: mods.alt,
        super_key: mods.mac_cmd || mods.command,
    }
}

/// Convert an egui key to a ghostty key name for encoding.
pub fn egui_key_to_name(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::ArrowUp => "up",
        Key::ArrowDown => "down",
        Key::ArrowLeft => "left",
        Key::ArrowRight => "right",
        Key::Home => "home",
        Key::End => "end",
        Key::PageUp => "pageup",
        Key::PageDown => "pagedown",
        Key::Insert => "insert",
        Key::Delete => "delete",
        Key::Backspace => "backspace",
        Key::Enter => "enter",
        Key::Tab => "tab",
        Key::Escape => "escape",
        Key::F1 => "f1",
        Key::F2 => "f2",
        Key::F3 => "f3",
        Key::F4 => "f4",
        Key::F5 => "f5",
        Key::F6 => "f6",
        Key::F7 => "f7",
        Key::F8 => "f8",
        Key::F9 => "f9",
        Key::F10 => "f10",
        Key::F11 => "f11",
        Key::F12 => "f12",
        _ => return None,
    })
}

/// Encode a character with control modifier.
pub fn encode_ctrl_char(c: char) -> Option<u8> {
    // Ctrl+A through Ctrl+Z map to 0x01-0x1A
    let c_lower = c.to_ascii_lowercase();
    if c_lower.is_ascii_lowercase() {
        Some((c_lower as u8) - b'a' + 1)
    } else {
        match c {
            '[' | '3' => Some(0x1B), // Escape
            '\\' | '4' => Some(0x1C),
            ']' | '5' => Some(0x1D),
            '^' | '6' => Some(0x1E),
            '_' | '7' => Some(0x1F),
            '@' | '2' => Some(0x00),
            _ => None,
        }
    }
}

/// Encode text input for the terminal.
///
/// Handles special cases like control characters and Alt prefixing.
pub fn encode_text_input(text: &str, modifiers: &Modifiers) -> Vec<u8> {
    let mut result = Vec::new();

    for c in text.chars() {
        if modifiers.ctrl && !modifiers.alt {
            // Control character
            if let Some(ctrl_byte) = encode_ctrl_char(c) {
                result.push(ctrl_byte);
                continue;
            }
        }

        if modifiers.alt {
            // Alt prefix: send ESC before the character
            result.push(0x1B);
        }

        // Encode character as UTF-8
        let mut buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut buf);
        result.extend_from_slice(encoded.as_bytes());
    }

    result
}

/// Mouse button for terminal mouse reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

impl MouseButton {
    /// Get the button code for SGR mouse reporting.
    pub fn sgr_code(self) -> u8 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::WheelUp => 64,
            MouseButton::WheelDown => 65,
        }
    }
}

/// Encode a mouse event in SGR format.
pub fn encode_sgr_mouse(
    button: MouseButton,
    col: u16,
    row: u16,
    pressed: bool,
    modifiers: &Modifiers,
) -> Vec<u8> {
    let mut code = button.sgr_code();

    // Add modifier bits
    if modifiers.shift {
        code += 4;
    }
    if modifiers.alt {
        code += 8;
    }
    if modifiers.ctrl {
        code += 16;
    }

    let terminator = if pressed { 'M' } else { 'm' };

    format!("\x1b[<{code};{col};{row}{terminator}").into_bytes()
}
