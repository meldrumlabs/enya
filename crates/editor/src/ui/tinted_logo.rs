//! Tintable logo texture with overlay blend for depth-preserving color tinting.
//!
//! This module provides a cached texture system for the Enya logo:
//! - For the Emerald theme (default): uses the original branded logo
//! - For the Light theme (ink/paper): uses the grayscale logo directly (ink on paper)
//! - For other themes: applies an overlay blend to a grayscale version,
//!   preserving depth and shading while tinting with the theme's accent color.

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};

use super::theme::AppTheme;

/// Raw bytes of the original branded logo PNG (for Emerald theme)
const LOGO_BYTES_ORIGINAL: &[u8] = include_bytes!("../../assets/logo.png");

/// Raw bytes of the tintable (grayscale) logo PNG (for other themes)
const LOGO_BYTES_TINTABLE: &[u8] = include_bytes!("../../assets/enya_tintable.png");

/// Cached tinted logo texture that updates when theme changes
pub struct TintedLogo {
    texture: Option<TextureHandle>,
    current_theme: Option<AppTheme>,
}

impl Default for TintedLogo {
    fn default() -> Self {
        Self::new()
    }
}

impl TintedLogo {
    pub fn new() -> Self {
        Self {
            texture: None,
            current_theme: None,
        }
    }

    /// Get or create the logo texture for the current theme.
    ///
    /// - For Emerald: returns the original branded logo
    /// - For other themes: returns an overlay-blended tinted version
    ///
    /// Returns a texture handle that can be used with `egui::Image::from_texture()`.
    pub fn get(&mut self, ctx: &Context, theme: AppTheme) -> &TextureHandle {
        // Regenerate texture if theme changed
        if self.current_theme != Some(theme) || self.texture.is_none() {
            let image = load_logo_for_theme(theme, 1.0);
            self.texture = Some(ctx.load_texture(
                format!("enya_logo_{}", theme.name()),
                image,
                TextureOptions::LINEAR,
            ));
            self.current_theme = Some(theme);
        }

        self.texture.as_ref().unwrap()
    }

    /// Get the texture with a custom opacity multiplier applied.
    /// Useful for subtle/watermark effects.
    /// For Emerald theme, applies opacity to the original logo.
    /// For other themes, applies opacity to the tint color.
    pub fn get_with_opacity(
        &mut self,
        ctx: &Context,
        theme: AppTheme,
        opacity: f32,
    ) -> TextureHandle {
        let image = load_logo_for_theme(theme, opacity);
        ctx.load_texture(
            format!("enya_logo_{}_{:.2}", theme.name(), opacity),
            image,
            TextureOptions::LINEAR,
        )
    }
}

/// Get a logo texture for stateless contexts.
/// - For Emerald: returns the original branded logo
/// - For other themes: returns an overlay-blended tinted version
///
/// This creates/retrieves a texture with a name based on the theme, so egui's
/// internal texture cache will automatically reuse it across frames.
/// For components that hold state, prefer using `TintedLogo` struct for better control.
pub fn get_tinted_logo(ctx: &Context, theme: AppTheme) -> TextureHandle {
    let image = load_logo_for_theme(theme, 1.0);
    ctx.load_texture(
        format!("enya_logo_{}", theme.name()),
        image,
        TextureOptions::LINEAR,
    )
}

/// Get a logo texture with custom opacity for stateless contexts.
pub fn get_tinted_logo_with_opacity(ctx: &Context, theme: AppTheme, opacity: f32) -> TextureHandle {
    let image = load_logo_for_theme(theme, opacity);
    ctx.load_texture(
        format!("enya_logo_{}_{:.2}", theme.name(), opacity),
        image,
        TextureOptions::LINEAR,
    )
}

/// Load the appropriate logo for the given theme.
/// - For Emerald: loads the original branded logo (with optional opacity)
/// - For Light: loads the grayscale logo as-is (ink on paper aesthetic)
/// - For other themes: loads the tintable logo with overlay blend tinting
fn load_logo_for_theme(theme: AppTheme, opacity: f32) -> ColorImage {
    if theme == AppTheme::Emerald {
        load_original_logo(opacity)
    } else if theme == AppTheme::Light {
        // Light uses ink/paper aesthetic - use grayscale logo directly
        load_grayscale_logo(opacity)
    } else {
        let tint = theme.accent_primary().gamma_multiply(opacity);
        load_tinted_logo(tint)
    }
}

/// Load the original branded logo (for Emerald theme).
/// Optionally applies opacity by multiplying the alpha channel.
fn load_original_logo(opacity: f32) -> ColorImage {
    let image = image::load_from_memory(LOGO_BYTES_ORIGINAL)
        .expect("Failed to load original logo")
        .to_rgba8();

    let size = [image.width() as usize, image.height() as usize];
    let pixels: Vec<Color32> = image
        .pixels()
        .map(|pixel| {
            let alpha = (pixel[3] as f32 * opacity).clamp(0.0, 255.0) as u8;
            Color32::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], alpha)
        })
        .collect();

    ColorImage::new(size, pixels)
}

/// Load the grayscale logo as-is (for Light theme's ink/paper aesthetic).
/// The grayscale values represent ink intensity on paper.
/// Optionally applies opacity by multiplying the alpha channel.
fn load_grayscale_logo(opacity: f32) -> ColorImage {
    let image = image::load_from_memory(LOGO_BYTES_TINTABLE)
        .expect("Failed to load tintable logo")
        .to_rgba8();

    let size = [image.width() as usize, image.height() as usize];
    let pixels: Vec<Color32> = image
        .pixels()
        .map(|pixel| {
            let gray = pixel[0]; // Grayscale value
            let alpha = (pixel[3] as f32 * opacity).clamp(0.0, 255.0) as u8;
            Color32::from_rgba_unmultiplied(gray, gray, gray, alpha)
        })
        .collect();

    ColorImage::new(size, pixels)
}

/// Load the tintable logo and apply overlay blend tinting.
///
/// The overlay blend formula preserves depth:
/// - Dark areas (gray < 0.5): result = 2 * gray * tint
/// - Light areas (gray >= 0.5): result = 1 - 2 * (1 - gray) * (1 - tint)
///
/// This keeps deep blacks dark while applying the tint color to mid-tones and highlights.
fn load_tinted_logo(tint: Color32) -> ColorImage {
    let image = image::load_from_memory(LOGO_BYTES_TINTABLE)
        .expect("Failed to load tintable logo")
        .to_rgba8();

    let size = [image.width() as usize, image.height() as usize];
    let mut pixels: Vec<Color32> = Vec::with_capacity(size[0] * size[1]);

    let tint_r = tint.r() as f32 / 255.0;
    let tint_g = tint.g() as f32 / 255.0;
    let tint_b = tint.b() as f32 / 255.0;

    for pixel in image.pixels() {
        let gray = pixel[0] as f32 / 255.0;
        let alpha = pixel[3];

        // Overlay blend formula
        let blend = |g: f32, t: f32| -> u8 {
            let result = if g < 0.5 {
                2.0 * g * t
            } else {
                1.0 - 2.0 * (1.0 - g) * (1.0 - t)
            };
            (result * 255.0).clamp(0.0, 255.0) as u8
        };

        pixels.push(Color32::from_rgba_unmultiplied(
            blend(gray, tint_r),
            blend(gray, tint_g),
            blend(gray, tint_b),
            alpha,
        ));
    }

    ColorImage::new(size, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_original_logo() {
        // Test that original logo loads without panicking
        let image = load_original_logo(1.0);

        // Should have non-zero dimensions
        assert!(image.size[0] > 0);
        assert!(image.size[1] > 0);

        // Pixel count should match dimensions
        assert_eq!(image.pixels.len(), image.size[0] * image.size[1]);
    }

    #[test]
    fn test_load_tinted_logo() {
        // Test that tintable logo loads without panicking
        let tint = Color32::from_rgb(136, 192, 208); // Nord blue
        let image = load_tinted_logo(tint);

        // Should have non-zero dimensions
        assert!(image.size[0] > 0);
        assert!(image.size[1] > 0);

        // Pixel count should match dimensions
        assert_eq!(image.pixels.len(), image.size[0] * image.size[1]);
    }

    #[test]
    fn test_overlay_blend_preserves_blacks() {
        // Deep blacks should stay dark regardless of tint
        let tint = Color32::from_rgb(255, 0, 0); // Bright red
        let image = load_tinted_logo(tint);

        // Find a fully transparent or very dark pixel
        // The blend of gray=0 with any tint should be 0 (2 * 0 * t = 0)
        // So any originally black pixels should remain black
        let has_dark_pixels = image
            .pixels
            .iter()
            .any(|p| p.r() < 30 && p.g() < 30 && p.b() < 30);
        assert!(has_dark_pixels, "Should preserve dark areas");
    }

    #[test]
    fn test_emerald_uses_original() {
        // Emerald theme should use the original logo
        let image = load_logo_for_theme(AppTheme::Emerald, 1.0);

        // Should match the original logo dimensions
        let original = load_original_logo(1.0);
        assert_eq!(image.size, original.size);
    }

    #[test]
    fn test_non_emerald_uses_tinted() {
        // Nord theme should use tinted version
        let nord_image = load_logo_for_theme(AppTheme::Nord, 1.0);

        // Should load successfully with valid dimensions
        assert!(nord_image.size[0] > 0);
        assert!(nord_image.size[1] > 0);
        assert_eq!(
            nord_image.pixels.len(),
            nord_image.size[0] * nord_image.size[1]
        );
    }

    #[test]
    fn test_light_uses_grayscale() {
        // Light theme should use grayscale logo (ink on paper)
        let light_image = load_logo_for_theme(AppTheme::Light, 1.0);

        // Should load successfully with valid dimensions
        assert!(light_image.size[0] > 0);
        assert!(light_image.size[1] > 0);
        assert_eq!(
            light_image.pixels.len(),
            light_image.size[0] * light_image.size[1]
        );

        // Verify it's grayscale (R == G == B for all pixels)
        for pixel in &light_image.pixels {
            assert_eq!(pixel.r(), pixel.g(), "Grayscale should have R == G");
            assert_eq!(pixel.g(), pixel.b(), "Grayscale should have G == B");
        }
    }
}
