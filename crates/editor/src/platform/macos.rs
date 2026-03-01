//! macOS window vibrancy via NSVisualEffectView.
//!
//! Applies a translucent vibrancy effect to the window, allowing the desktop
//! to subtly show through the custom titlebar. The content area remains opaque
//! via egui panel fills.
//!
//! The effect view is inserted as a sibling of winit's content view inside the
//! window's frame view (NSThemeFrame), positioned behind the wgpu Metal
//! surface. Where egui paints semi-transparent pixels (the titlebar) the
//! vibrancy shows through; opaque panel fills hide it.

use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAutoresizingMaskOptions, NSColor, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowOrderingMode,
};
use objc2_foundation::{MainThreadMarker, ns_string};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Apply NSVisualEffectView vibrancy to the window behind the eframe content.
///
/// Configures the window for transparency and inserts a vibrancy view as a
/// sibling of winit's content view inside the window's frame view, positioned
/// *behind* the content view. This avoids replacing the content view (which
/// would break winit's instance variable access) while still putting vibrancy
/// behind the wgpu Metal surface.
///
/// Only the custom titlebar (rendered with a semi-transparent fill) reveals
/// the effect; all other panels paint opaque.
pub fn apply_vibrancy(cc: &eframe::CreationContext<'_>, is_dark: bool) {
    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("apply_vibrancy called off main thread, skipping");
        return;
    };

    let window_handle = match cc.window_handle() {
        Ok(handle) => handle,
        Err(e) => {
            log::warn!("Failed to get window handle for vibrancy: {e}");
            return;
        }
    };

    let RawWindowHandle::AppKit(appkit_handle) = window_handle.as_raw() else {
        log::warn!("Window handle is not AppKit, skipping vibrancy");
        return;
    };

    // Get the NSView from the raw handle pointer.
    let ns_view: Retained<NSView> =
        unsafe { Retained::retain(appkit_handle.ns_view.as_ptr().cast()) }
            .expect("NSView pointer was null");

    let Some(window) = ns_view.window() else {
        log::warn!("NSView has no window, skipping vibrancy");
        return;
    };

    let Some(content_view) = window.contentView() else {
        log::warn!("NSWindow has no content view, skipping vibrancy");
        return;
    };

    // The content view's superview is the window's frame view (NSThemeFrame).
    // We insert the effect view there as a sibling *behind* the content view
    // so winit's content view stays untouched.
    let Some(frame_view) = (unsafe { content_view.superview() }) else {
        log::warn!("Content view has no superview, skipping vibrancy");
        return;
    };

    unsafe {
        // Tell macOS this window participates in transparency compositing.
        // Without these the compositor treats the window as opaque and the
        // vibrancy effect is invisible.
        window.setOpaque(false);

        // Use near-zero alpha (0.0001) instead of clearColor() to preserve
        // the native window drop shadow. Fully transparent backgrounds break
        // the shadow compositing. This is the same trick Zed uses.
        let bg = NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.0001);
        window.setBackgroundColor(Some(&bg));
    }

    // Sync the window appearance with the editor theme so the vibrancy
    // blur renders with the correct dark/light tint.
    set_window_appearance(&window, is_dark);

    let frame = content_view.frame();
    let effect_view = unsafe { NSVisualEffectView::initWithFrame(mtm.alloc(), frame) };

    unsafe {
        // Selection material: colorless/neutral blur without imposing a tint,
        // so it works cleanly across all themes (dark, light, custom).
        effect_view.setMaterial(NSVisualEffectMaterial::Selection);
        effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        // Active state keeps vibrancy visible even when the window is
        // unfocused, matching Zed's behavior.
        effect_view.setState(NSVisualEffectState::Active);

        // Auto-resize with the window.
        effect_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::NSViewWidthSizable
                | NSAutoresizingMaskOptions::NSViewHeightSizable,
        );

        // Insert into the frame view as a sibling behind the content view.
        frame_view.addSubview_positioned_relativeTo(
            &effect_view,
            NSWindowOrderingMode::NSWindowBelow,
            Some(&content_view),
        );
    }

    log::info!("Applied macOS window vibrancy (Selection material)");
}

/// Sync the window's NSAppearance with the editor theme.
///
/// Call this when the effective theme changes (dark ↔ light) so the vibrancy
/// blur uses the correct appearance. Without this, a dark editor theme on a
/// light macOS system would show a light-tinted blur behind the titlebar.
pub fn sync_appearance(is_dark: bool) {
    unsafe {
        let app: Retained<objc2_app_kit::NSApplication> =
            objc2::msg_send_id![objc2::class!(NSApplication), sharedApplication];
        if let Some(window) = app.mainWindow() {
            set_window_appearance(&window, is_dark);
        }
    }
}

/// Set the NSAppearance on an NSWindow to match the editor's dark/light mode.
fn set_window_appearance(window: &objc2_app_kit::NSWindow, is_dark: bool) {
    let name = if is_dark {
        ns_string!("NSAppearanceNameVibrantDark")
    } else {
        ns_string!("NSAppearanceNameVibrantLight")
    };
    let appearance = NSAppearance::appearanceNamed(name);
    unsafe {
        window.setAppearance(appearance.as_deref());
    }
}
