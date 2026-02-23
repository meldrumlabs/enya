// Re-export web-time's Instant for WASM, std::time::Instant for native
// web-time is a drop-in replacement that works in browsers (used by egui/eframe/rerun)
#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::disallowed_types)]
pub use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;

/// Get the current Unix timestamp in seconds.
/// Works on both native and WASM platforms.
#[inline]
#[allow(clippy::disallowed_types)]
pub fn now_unix_secs() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// Get the current Unix timestamp in seconds as f64 (with sub-second precision).
/// Works on both native and WASM platforms.
#[inline]
#[allow(clippy::disallowed_types)]
pub fn now_unix_secs_f64() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
}

/// Get the current Unix timestamp in nanoseconds.
/// Works on both native and WASM platforms.
#[inline]
#[allow(clippy::disallowed_types)]
pub fn now_unix_nanos() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }
}

/// Returns the rect that centered overlays should constrain to.
///
/// When the project sidebar is visible, the rect is offset to the right by the
/// sidebar width so overlays center within the content area. When the sidebar is
/// hidden, this returns `ctx.available_rect()` (the full area between titlebar
/// and statusbar).
pub fn overlay_content_rect(ctx: &egui::Context) -> egui::Rect {
    let sidebar_w: f32 = ctx
        .data(|d| d.get_temp(egui::Id::new("sidebar_width")))
        .unwrap_or(0.0);
    let mut rect = ctx.available_rect();
    rect.min.x += sidebar_w;
    rect
}

pub fn png_to_icon_data(png_bytes: &[u8]) -> egui::IconData {
    let image = image::load_from_memory(png_bytes).unwrap();
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.into_rgba8().to_vec();
    egui::IconData {
        width: size[0] as u32,
        height: size[1] as u32,
        rgba,
    }
}
