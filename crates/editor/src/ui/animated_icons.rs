//! Animated icon helpers — loading spinners, pulsing sync indicators, etc.
//!
//! Provides lightweight animation for status icons using opacity/color cycling
//! (no rotation, since egui doesn't support rotated text easily).

use egui::{Color32, RichText, Ui};

/// Render a loading/thinking icon with a gentle opacity pulse.
///
/// The icon cycles between full brightness and ~60% brightness
/// using a sine wave tied to wall-clock time.
pub fn loading_icon(ui: &mut Ui, icon: &str, base_color: Color32, time: f32) {
    let pulse = (time * 3.0).sin() * 0.5 + 0.5; // 0..1 oscillation
    let alpha = 0.6 + pulse * 0.4; // 0.6..1.0
    let color = base_color.gamma_multiply(alpha);
    ui.label(RichText::new(icon).color(color));
}

/// Render a sync/refresh icon with a subtle "breathing" glow.
///
/// Slower than loading pulse — feels like a heartbeat rather than a spinner.
pub fn sync_icon(ui: &mut Ui, icon: &str, base_color: Color32, time: f32) {
    let pulse = (time * 2.0).sin() * 0.5 + 0.5;
    let alpha = 0.7 + pulse * 0.3; // 0.7..1.0
    let color = base_color.gamma_multiply(alpha);
    ui.label(RichText::new(icon).color(color));
}

/// Render a recording/active indicator with a sharp blink.
///
/// Good for "live" indicators (recording, streaming, etc.).
pub fn recording_icon(ui: &mut Ui, icon: &str, base_color: Color32, time: f32) {
    let blink = if (time * 2.0) % 1.0 < 0.5 { 1.0 } else { 0.4 };
    let color = base_color.gamma_multiply(blink);
    ui.label(RichText::new(icon).color(color));
}
