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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_egui_key_to_name() {
        assert_eq!(egui_key_to_name(Key::ArrowUp), Some("up"));
        assert_eq!(egui_key_to_name(Key::ArrowDown), Some("down"));
        assert_eq!(egui_key_to_name(Key::Enter), Some("enter"));
        assert_eq!(egui_key_to_name(Key::Escape), Some("escape"));
        assert_eq!(egui_key_to_name(Key::Tab), Some("tab"));
        assert_eq!(egui_key_to_name(Key::F1), Some("f1"));
        assert_eq!(egui_key_to_name(Key::F12), Some("f12"));
        // Regular letter keys don't have names
        assert_eq!(egui_key_to_name(Key::A), None);
    }

    #[test]
    fn test_encode_ctrl_char() {
        // Ctrl+A = 0x01, Ctrl+Z = 0x1A
        assert_eq!(encode_ctrl_char('a'), Some(0x01));
        assert_eq!(encode_ctrl_char('A'), Some(0x01));
        assert_eq!(encode_ctrl_char('z'), Some(0x1A));
        assert_eq!(encode_ctrl_char('Z'), Some(0x1A));
        // Ctrl+C = 0x03
        assert_eq!(encode_ctrl_char('c'), Some(0x03));
        // Ctrl+[ = Escape
        assert_eq!(encode_ctrl_char('['), Some(0x1B));
        // Non-control characters
        assert_eq!(encode_ctrl_char('1'), None);
    }

    #[test]
    fn test_encode_text_input_simple() {
        let modifiers = Modifiers::NONE;
        let result = encode_text_input("hello", &modifiers);
        assert_eq!(result, b"hello");
    }

    #[test]
    fn test_encode_text_input_ctrl() {
        let modifiers = Modifiers::CTRL;
        // Ctrl+C = 0x03
        let result = encode_text_input("c", &modifiers);
        assert_eq!(result, vec![0x03]);
    }

    #[test]
    fn test_encode_text_input_alt() {
        let modifiers = Modifiers::ALT;
        // Alt+x should send ESC + x
        let result = encode_text_input("x", &modifiers);
        assert_eq!(result, vec![0x1B, b'x']);
    }

    #[test]
    fn test_encode_text_input_unicode() {
        let modifiers = Modifiers::NONE;
        let result = encode_text_input("日本語", &modifiers);
        assert_eq!(result, "日本語".as_bytes());
    }

    #[test]
    fn test_mouse_button_sgr_code() {
        assert_eq!(MouseButton::Left.sgr_code(), 0);
        assert_eq!(MouseButton::Middle.sgr_code(), 1);
        assert_eq!(MouseButton::Right.sgr_code(), 2);
        assert_eq!(MouseButton::WheelUp.sgr_code(), 64);
        assert_eq!(MouseButton::WheelDown.sgr_code(), 65);
    }

    #[test]
    fn test_encode_sgr_mouse_press() {
        let modifiers = Modifiers::NONE;
        let result = encode_sgr_mouse(MouseButton::Left, 10, 5, true, &modifiers);
        assert_eq!(result, b"\x1b[<0;10;5M");
    }

    #[test]
    fn test_encode_sgr_mouse_release() {
        let modifiers = Modifiers::NONE;
        let result = encode_sgr_mouse(MouseButton::Left, 10, 5, false, &modifiers);
        assert_eq!(result, b"\x1b[<0;10;5m");
    }

    #[test]
    fn test_encode_sgr_mouse_with_modifiers() {
        let modifiers = Modifiers {
            shift: true,
            ctrl: true,
            alt: false,
            ..Default::default()
        };
        // Shift adds 4, Ctrl adds 16
        let result = encode_sgr_mouse(MouseButton::Left, 1, 1, true, &modifiers);
        assert_eq!(result, b"\x1b[<20;1;1M");
    }

    #[test]
    fn test_egui_to_ghostty_modifiers() {
        let egui_mods = Modifiers {
            shift: true,
            ctrl: true,
            alt: true,
            mac_cmd: true,
            ..Default::default()
        };
        let ghostty_mods = egui_to_ghostty_modifiers(&egui_mods);
        assert!(ghostty_mods.shift);
        assert!(ghostty_mods.control);
        assert!(ghostty_mods.alt);
        assert!(ghostty_mods.super_key);
    }
}
