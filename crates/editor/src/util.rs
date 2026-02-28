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

/// Compute responsive overlay width that fits any screen size.
///
/// Uses [`overlay_content_rect`] so the result accounts for the sidebar.
/// `preferred_min` is the minimum on large screens but automatically reduces
/// on small screens (e.g. WASM at 1.5× zoom on a laptop) so the overlay
/// never exceeds 95% of the available content width.
pub fn overlay_width(ctx: &egui::Context, fraction: f32, preferred_min: f32, max: f32) -> f32 {
    let available = overlay_content_rect(ctx).width();
    let effective_min = preferred_min.min(available * 0.95).max(200.0);
    (available * fraction).clamp(effective_min, max)
}

/// Compute responsive overlay height that fits any screen size.
///
/// `preferred_min` is the minimum on large screens but automatically reduces
/// on small screens so the overlay never exceeds 90% of the available height.
pub fn overlay_height(ctx: &egui::Context, fraction: f32, preferred_min: f32, max: f32) -> f32 {
    let available = overlay_content_rect(ctx).height();
    let effective_min = preferred_min.min(available * 0.90).max(100.0);
    (available * fraction).clamp(effective_min, max)
}

/// Compute overlay height with only a max cap (no minimum).
///
/// For overlays that use the `(fraction * available).min(max)` pattern where
/// height has no lower bound.
pub fn overlay_max_height(ctx: &egui::Context, fraction: f32, max: f32) -> f32 {
    let available = overlay_content_rect(ctx).height();
    (available * fraction).min(max)
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

#[cfg(test)]
mod tests {
    // Test the overlay sizing math directly (without egui context).
    fn compute_width(available: f32, fraction: f32, preferred_min: f32, max: f32) -> f32 {
        let effective_min = preferred_min.min(available * 0.95).max(200.0);
        (available * fraction).clamp(effective_min, max)
    }

    fn compute_height(available: f32, fraction: f32, preferred_min: f32, max: f32) -> f32 {
        let effective_min = preferred_min.min(available * 0.90).max(100.0);
        (available * fraction).clamp(effective_min, max)
    }

    #[test]
    fn large_screen_uses_max_cap() {
        // 1920px wide, UnifiedFinder: fraction * available exceeds max
        let w = compute_width(1920.0, 0.80, 800.0, 1200.0);
        assert_eq!(w, 1200.0);
    }

    #[test]
    fn medium_screen_uses_fraction() {
        // 1100px content area, fraction result is between min and max
        let w = compute_width(1100.0, 0.80, 800.0, 1200.0);
        assert_eq!(w, 880.0);
    }

    #[test]
    fn small_wasm_screen_reduces_min() {
        // 712px content area (13" MacBook WASM with sidebar open)
        let w = compute_width(712.0, 0.80, 800.0, 1200.0);
        // effective_min = 800.min(712*0.95) = 676.4
        // fraction = 712 * 0.80 = 569.6 → clamped up to 676.4
        assert!((w - 676.4).abs() < 0.1);
        assert!(w < 712.0, "overlay must fit within available space");
    }

    #[test]
    fn very_small_screen_uses_floor() {
        let w = compute_width(150.0, 0.80, 800.0, 1200.0);
        // effective_min = 800.min(142.5).max(200) = 200
        assert_eq!(w, 200.0);
    }

    #[test]
    fn height_responsive_on_small_screen() {
        // 400px available height, overlay wants min 500
        let h = compute_height(400.0, 0.70, 500.0, 700.0);
        // effective_min = 500.min(360).max(100) = 360
        // fraction = 280 → clamped up to 360
        assert_eq!(h, 360.0);
    }

    #[test]
    fn height_large_screen() {
        let h = compute_height(1000.0, 0.70, 500.0, 700.0);
        // fraction = 700, clamped to max 700
        assert_eq!(h, 700.0);
    }
}
