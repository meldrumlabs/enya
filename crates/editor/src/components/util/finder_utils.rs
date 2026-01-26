//! Common utilities for overlay and finder components.
//!
//! This module provides shared functionality for modal overlays including:
//! - Theme-aware colors and styling (`OverlayStyle`, `FinderColors`, `OverlayColors`)
//! - Text highlighting for fuzzy matching
//! - Keyboard navigation helpers
//! - Common UI elements (separators, key badges, backdrops)
//! - Preview data generation for demo mode

use egui::{Color32, Key, RichText, Stroke, TextFormat, text::LayoutJob};

use crate::ui::active_theme::ActiveThemeColors;
use crate::ui::colors::text_color;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::pane::time_series_chart::DataPoint;

/// Overlay style variants for modal/popup components
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum OverlayStyleVariant {
    /// Frosted glass: transparent background with soft edges (default)
    #[default]
    FrostedGlass,
    /// Minimal flat: solid background, no shadows
    MinimalFlat,
    /// Subtle neon: solid background with glowing accent border
    SubtleNeon,
    /// Premium glass: enhanced transparency with inner glow
    PremiumGlass,
}

/// Styling configuration for overlay/modal components
pub struct OverlayStyle {
    /// Background color
    pub bg: Color32,
    /// Border color
    pub border: Color32,
    /// Corner radius
    pub corner_radius: f32,
    /// Shadow configuration
    pub shadow: egui::epaint::Shadow,
    /// Border stroke width
    pub stroke_width: f32,
    /// Inner highlight color (top edge glow for glass effect)
    pub inner_highlight: Option<Color32>,
}

impl OverlayStyle {
    /// Create overlay style for the given theme and variant
    pub fn new(theme: AppTheme, variant: OverlayStyleVariant) -> Self {
        match variant {
            OverlayStyleVariant::FrostedGlass => Self::frosted_glass(theme),
            OverlayStyleVariant::MinimalFlat => Self::minimal_flat(theme),
            OverlayStyleVariant::SubtleNeon => Self::subtle_neon(theme),
            OverlayStyleVariant::PremiumGlass => Self::premium_glass(theme),
        }
    }

    /// Frosted glass style: semi-transparent background with soft edges
    /// Now enhanced with inner highlight for premium feel
    pub fn frosted_glass(theme: AppTheme) -> Self {
        let bg = theme.overlay_bg();
        let border = theme.overlay_border();
        let inner_highlight = Some(theme.overlay_highlight());

        Self {
            bg,
            border,
            corner_radius: 14.0, // Slightly more rounded for premium feel
            stroke_width: 1.0,
            shadow: egui::epaint::Shadow {
                offset: [0, 8],
                blur: 32,
                spread: 0,
                color: Color32::from_black_alpha(80), // Deeper shadow for more lift
            },
            inner_highlight,
        }
    }

    /// Frosted glass style using active theme colors (builtin or custom)
    pub fn frosted_glass_active(colors: &ActiveThemeColors) -> Self {
        Self {
            bg: colors.overlay_bg,
            border: colors.overlay_border,
            corner_radius: 14.0,
            stroke_width: 1.0,
            shadow: egui::epaint::Shadow {
                offset: [0, 8],
                blur: 32,
                spread: 0,
                color: Color32::from_black_alpha(80),
            },
            inner_highlight: Some(colors.overlay_highlight),
        }
    }

    /// Minimal flat style: solid background, no shadows
    pub fn minimal_flat(theme: AppTheme) -> Self {
        let bg = theme.bg_surface();
        let border = theme.border_default();

        Self {
            bg,
            border,
            corner_radius: 6.0, // Match the premium 6px radius
            stroke_width: 1.0,
            shadow: egui::epaint::Shadow::NONE,
            inner_highlight: None,
        }
    }

    /// Subtle neon style: solid background with glowing accent border
    pub fn subtle_neon(theme: AppTheme) -> Self {
        let bg = theme.bg_base();
        let glow_color = theme.accent_primary();

        Self {
            bg,
            border: glow_color,
            corner_radius: 6.0,
            stroke_width: 1.5,
            shadow: egui::epaint::Shadow {
                offset: [0, 0],
                blur: 20,
                spread: 3,
                color: glow_color.gamma_multiply(0.5),
            },
            inner_highlight: None,
        }
    }

    /// Premium glass style: enhanced transparency with inner glow and deep shadows
    pub fn premium_glass(theme: AppTheme) -> Self {
        let bg = theme.overlay_bg_deep();
        let border = theme.overlay_border();
        let inner_highlight = Some(theme.overlay_highlight_strong());

        Self {
            bg,
            border,
            corner_radius: 16.0, // More rounded for premium
            stroke_width: 1.0,
            shadow: egui::epaint::Shadow {
                offset: [0, 12],
                blur: 48,
                spread: 4,
                color: Color32::from_black_alpha(100), // Deep ambient shadow
            },
            inner_highlight,
        }
    }

    /// Apply this style to an egui Frame
    pub fn apply_to_frame(&self, frame: egui::Frame) -> egui::Frame {
        frame
            .fill(self.bg)
            .stroke(egui::Stroke::new(self.stroke_width, self.border))
            .corner_radius(self.corner_radius)
            .shadow(self.shadow)
    }

    /// Create a new styled egui Frame
    pub fn frame(&self) -> egui::Frame {
        self.apply_to_frame(egui::Frame::new().inner_margin(0.0))
    }

    /// Get the inner highlight color if available
    pub fn inner_highlight(&self) -> Option<Color32> {
        self.inner_highlight
    }

    /// Draw inner highlight effect on top of the frame
    /// Call this after the frame content is rendered to add the glass edge effect
    pub fn draw_inner_highlight(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        if let Some(highlight_color) = self.inner_highlight {
            // Draw a subtle gradient line at the top edge for the glass reflection effect
            let highlight_rect = egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(1.0, 1.0),
                egui::vec2(rect.width() - 2.0, 1.5),
            );
            ui.painter()
                .rect_filled(highlight_rect, self.corner_radius - 1.0, highlight_color);
        }
    }
}

/// Theme-aware colors for finder modals
pub struct FinderColors {
    /// Background color for the modal
    pub bg: Color32,
    /// Border color
    pub border: Color32,
    /// Separator line color
    pub separator: Color32,
    /// Highlight color for matched text
    pub highlight: Color32,
    /// Background for selected row
    pub selected_bg: Color32,
    /// Background for hovered row
    pub hover_bg: Color32,
    /// Preview pane background
    pub preview_bg: Color32,
    /// Panel background (for side-by-side preview)
    pub panel_bg: Color32,
}

impl FinderColors {
    /// Create finder colors for the given theme
    pub fn new(theme: AppTheme) -> Self {
        Self {
            bg: theme.bg_surface(),
            border: theme.border_default(),
            separator: theme.border_subtle(),
            highlight: theme.highlight_match_text(),
            selected_bg: theme.bg_selected(),
            hover_bg: theme.bg_hover(),
            preview_bg: theme.bg_elevated(),
            panel_bg: theme.bg_base(),
        }
    }
}

/// Keyboard input state for finder navigation
pub struct FinderKeyboardInput {
    /// Navigate up in the list
    pub navigate_up: bool,
    /// Navigate down in the list
    pub navigate_down: bool,
    /// Confirm selection
    pub confirm: bool,
    /// Close the finder
    pub escape: bool,
    /// Toggle preview pane
    pub toggle_preview: bool,
    /// Cycle to next mode (Tab key)
    pub cycle_mode: bool,
}

impl FinderKeyboardInput {
    /// Read keyboard input from the egui context
    pub fn read(ctx: &egui::Context) -> Self {
        ctx.input(|i| Self {
            navigate_up: i.key_pressed(Key::ArrowUp) || (i.key_pressed(Key::K) && i.modifiers.ctrl),
            navigate_down: i.key_pressed(Key::ArrowDown)
                || (i.key_pressed(Key::J) && i.modifiers.ctrl)
                || (i.key_pressed(Key::N) && i.modifiers.ctrl),
            confirm: i.key_pressed(Key::Enter),
            escape: i.key_pressed(Key::Escape),
            toggle_preview: i.key_pressed(Key::P) && i.modifiers.ctrl,
            cycle_mode: i.key_pressed(Key::Tab) && !i.modifiers.shift,
        })
    }
}

/// Create a text galley with highlighted match positions
pub fn create_highlighted_text(
    ui: &egui::Ui,
    text: &str,
    positions: &[usize],
    normal_color: Color32,
    highlight_color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::default();
    let font_id = typography::proportional(typography::XL);

    for (i, ch) in text.chars().enumerate() {
        let color = if positions.contains(&i) {
            highlight_color
        } else {
            normal_color
        };

        let format = TextFormat {
            font_id: font_id.clone(),
            color,
            ..Default::default()
        };

        job.append(&ch.to_string(), 0.0, format);
    }

    ui.fonts_mut(|f| f.layout_job(job))
}

/// Generate deterministic demo preview data based on an item name
///
/// Returns a vector of `DataPoint`s representing a sine wave pattern
/// that is unique but reproducible for a given item name.
pub fn generate_demo_preview_data(item_name: &str) -> Vec<DataPoint> {
    let seed: u64 = item_name.bytes().map(|b| b as u64).sum();
    let now = 1_700_000_000.0;
    let duration = 3600.0; // 1 hour
    let num_points = 60;

    (0..num_points)
        .map(|i| {
            let t = now + (i as f64 / num_points as f64) * duration;
            // Create a unique but deterministic wave pattern based on seed
            let phase = (seed % 10) as f64 * 0.3;
            let amplitude = 20.0 + (seed % 30) as f64;
            let base = 50.0 + amplitude * ((t / 300.0) + phase).sin();
            let noise = ((t * (17.0 + (seed % 7) as f64)).sin()) * 5.0;
            DataPoint {
                timestamp: t,
                value: base + noise,
            }
        })
        .collect()
}

/// Get the chart line color for the given theme
pub fn chart_color(theme: AppTheme) -> Color32 {
    theme.chart_color(0)
}

/// Render keyboard hints footer for finder modals
pub fn render_keyboard_hints(ui: &mut egui::Ui, hint_color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("↑↓")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("navigate")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("↵")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("select")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("ctrl+p")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("preview")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("esc")
                .color(hint_color)
                .size(typography::SM),
        );
        ui.label(
            egui::RichText::new("close")
                .color(hint_color)
                .size(typography::SM),
        );
    });
}

// =============================================================================
// Shared Overlay Colors
// =============================================================================

/// Common theme-aware colors for overlay components.
///
/// This provides a single source of truth for colors used across
/// modal overlays, reducing duplication and ensuring consistency.
pub struct OverlayColors {
    /// Primary text color
    pub text: Color32,
    /// Muted text color (60% opacity)
    pub muted_text: Color32,
    /// Very muted text color (40% opacity)
    pub faint_text: Color32,
    /// Accent color for highlights
    pub accent: Color32,
    /// Separator line color
    pub separator: Color32,
    /// Elevated background color (for input fields, cards)
    pub elevated_bg: Color32,
    /// Badge/key background color
    pub badge_bg: Color32,
}

impl OverlayColors {
    /// Create overlay colors for the given theme
    pub fn new(theme: AppTheme) -> Self {
        let text = text_color(theme);
        Self {
            text,
            muted_text: text.gamma_multiply(0.6),
            faint_text: text.gamma_multiply(0.4),
            accent: theme.accent_hover(),
            separator: theme.border_subtle(),
            elevated_bg: theme.bg_elevated(),
            badge_bg: theme.bg_hover(),
        }
    }

    /// Create overlay colors from ActiveThemeColors (for custom themes)
    pub fn from_active(colors: &ActiveThemeColors) -> Self {
        Self {
            text: colors.text_primary,
            muted_text: colors.text_primary.gamma_multiply(0.6),
            faint_text: colors.text_primary.gamma_multiply(0.4),
            accent: colors.accent_hover,
            separator: colors.border_subtle,
            elevated_bg: colors.bg_elevated,
            badge_bg: colors.bg_hover,
        }
    }
}

// =============================================================================
// Shared UI Elements
// =============================================================================

/// Draw a horizontal separator line at the current cursor position.
///
/// This is a common pattern used across all overlay components to
/// visually separate sections.
pub fn draw_separator(ui: &mut egui::Ui, theme: AppTheme) {
    let separator_color = theme.border_subtle();
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        Stroke::new(1.0, separator_color),
    );
}

/// Draw a horizontal separator line with a specific color.
pub fn draw_separator_colored(ui: &mut egui::Ui, color: Color32) {
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        Stroke::new(1.0, color),
    );
}

/// Render a keyboard hint with pill badge styling.
///
/// This renders a compact key badge followed by a description label,
/// commonly used in overlay footers to show available keybindings.
/// Example: `[j/k] nav` or `[Enter] load`
pub fn render_keyboard_hint_pill(
    ui: &mut egui::Ui,
    key: &str,
    desc: &str,
    muted_text: Color32,
    text_col: Color32,
) {
    // Key badge with pill background
    let badge_padding = egui::Vec2::new(6.0, 2.0);
    let font = typography::monospace(typography::SM);
    let galley = ui
        .painter()
        .layout_no_wrap(key.to_string(), font.clone(), text_col);
    let badge_size = galley.size() + badge_padding * 2.0;

    let (badge_rect, _) = ui.allocate_exact_size(badge_size, egui::Sense::hover());

    // Draw pill background
    ui.painter()
        .rect_filled(badge_rect, 4.0, text_col.gamma_multiply(0.08));
    // Draw border
    ui.painter().rect_stroke(
        badge_rect,
        4.0,
        Stroke::new(1.0, text_col.gamma_multiply(0.15)),
        egui::StrokeKind::Inside,
    );
    // Draw key text centered
    ui.painter().galley(
        badge_rect.center() - galley.size() / 2.0,
        galley,
        text_col.gamma_multiply(0.9),
    );

    ui.add_space(4.0);
    ui.label(
        RichText::new(desc)
            .color(muted_text)
            .font(typography::proportional(typography::SM)),
    );
}

/// Render a keyboard key badge (like `⌘K` or `Enter`).
///
/// This renders a styled badge with the key text, commonly used
/// in tutorials, which-key popups, and keyboard hints.
/// Enhanced with premium styling including subtle gradient and refined borders.
pub fn render_key_badge(ui: &mut egui::Ui, key: &str, bg_color: Color32, text_color: Color32) {
    let font = typography::monospace(typography::MD);
    let text = RichText::new(key).color(text_color).font(font);

    // Premium key badge with subtle depth
    let response = egui::Frame::new()
        .fill(bg_color)
        .corner_radius(5.0) // Slightly more rounded
        .inner_margin(egui::Margin::symmetric(7, 3))
        .stroke(Stroke::new(1.0, text_color.gamma_multiply(0.15)))
        .shadow(egui::epaint::Shadow {
            offset: [0, 1],
            blur: 2,
            spread: 0,
            color: Color32::from_black_alpha(20),
        })
        .show(ui, |ui| {
            ui.label(text);
        });

    // Draw subtle top highlight for 3D effect
    let rect = response.response.rect;
    let highlight_rect = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(rect.width() - 2.0, 1.0),
    );
    ui.painter().rect_filled(
        highlight_rect,
        4.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 15),
    );
}

/// Render a keyboard key badge with larger padding (for tutorials).
/// Enhanced with premium styling.
pub fn render_key_badge_large(
    ui: &mut egui::Ui,
    key: &str,
    bg_color: Color32,
    text_color: Color32,
) {
    let font = typography::monospace(typography::MD);
    let text = RichText::new(key).color(text_color).font(font);

    let response = egui::Frame::new()
        .fill(bg_color)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .stroke(Stroke::new(1.0, text_color.gamma_multiply(0.15)))
        .shadow(egui::epaint::Shadow {
            offset: [0, 2],
            blur: 4,
            spread: 0,
            color: Color32::from_black_alpha(25),
        })
        .show(ui, |ui| {
            ui.label(text);
        });

    // Draw subtle top highlight for 3D effect
    let rect = response.response.rect;
    let highlight_rect = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(1.0, 1.0),
        egui::vec2(rect.width() - 2.0, 1.0),
    );
    ui.painter().rect_filled(
        highlight_rect,
        5.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 20),
    );
}

// =============================================================================
// Stat Badge Rendering
// =============================================================================

/// Render a simple stat badge (e.g., "128 rows", "5 cols").
///
/// Uses muted colors from OverlayColors for a subtle appearance.
pub fn render_stat_badge(ui: &mut egui::Ui, text: &str, colors: &OverlayColors) {
    egui::Frame::new()
        .fill(colors.badge_bg)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .color(colors.muted_text)
                    .font(typography::proportional(typography::XS)),
            );
        });
}

/// Render a stat badge with an icon prefix (e.g., clock icon + "123ms").
///
/// The icon is rendered in faint_text color, the value in muted_text.
pub fn render_stat_badge_with_icon(
    ui: &mut egui::Ui,
    icon: &str,
    value: &str,
    colors: &OverlayColors,
) {
    egui::Frame::new()
        .fill(colors.badge_bg)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.label(RichText::new(icon).color(colors.faint_text).size(10.0));
                ui.label(
                    RichText::new(value)
                        .color(colors.muted_text)
                        .font(typography::proportional(typography::XS)),
                );
            });
        });
}

/// Render a colored badge with custom fill and text color.
///
/// Useful for diff stats, status indicators, etc. The fill is automatically
/// made semi-transparent (15% opacity) and a subtle stroke is added.
pub fn render_colored_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.15))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.5)))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(color).size(11.0));
        });
}

// =============================================================================
// Split Panel Rendering
// =============================================================================

/// Render a split-panel header with left and right labels.
///
/// Commonly used for diff views to show "staging" vs "production" etc.
pub fn render_split_header(
    ui: &mut egui::Ui,
    left_label: &str,
    right_label: &str,
    left_color: Color32,
    right_color: Color32,
    separator_color: Color32,
) {
    let available_width = ui.available_width();
    let side_width = (available_width - 12.0) / 2.0;

    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(left_label).color(left_color).strong());
            },
        );
        ui.add_space(4.0);
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(right_label).color(right_color).strong());
            },
        );
    });

    // Separator below headers
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        Stroke::new(1.0, separator_color),
    );
    ui.add_space(4.0);
}

/// Render side-by-side panels with a vertical separator.
///
/// Takes closures for rendering the left and right content.
/// The `id_salt` is used to create unique IDs for scroll areas.
pub fn render_split_panels<L, R>(
    ui: &mut egui::Ui,
    height: f32,
    separator_color: Color32,
    id_salt: &str,
    left_content: L,
    right_content: R,
) where
    L: FnOnce(&mut egui::Ui),
    R: FnOnce(&mut egui::Ui),
{
    let available_width = ui.available_width();
    let side_width = (available_width - 12.0) / 2.0;

    ui.horizontal(|ui| {
        // Left panel
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.set_max_width(side_width);
                egui::ScrollArea::vertical()
                    .id_salt(format!("{id_salt}_left"))
                    .auto_shrink([false, false])
                    .show(ui, left_content);
            },
        );

        // Center separator
        let separator_rect = ui.available_rect_before_wrap();
        ui.painter().vline(
            separator_rect.left(),
            separator_rect.y_range(),
            Stroke::new(1.0, separator_color),
        );
        ui.add_space(4.0);

        // Right panel
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.set_max_width(side_width);
                egui::ScrollArea::vertical()
                    .id_salt(format!("{id_salt}_right"))
                    .auto_shrink([false, false])
                    .show(ui, right_content);
            },
        );
    });
}

/// Draw a semi-transparent backdrop overlay covering the entire screen.
///
/// This is used by modals like the buffer editor and multi-edit overlay
/// to dim the background content. Enhanced with subtle radial gradient
/// for a premium depth effect.
#[allow(deprecated)]
pub fn draw_backdrop(ctx: &egui::Context, theme: AppTheme, id_suffix: &str) {
    let screen_rect = ctx.screen_rect();
    egui::Area::new(egui::Id::new(format!("{id_suffix}_backdrop")))
        .fixed_pos(screen_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            // Premium backdrop with slight vignette effect
            let backdrop_color = theme.backdrop_color();
            ui.painter().rect_filled(screen_rect, 0.0, backdrop_color);

            // Add subtle vignette at edges for depth (dark themes)
            if let Some(vignette_color) = theme.backdrop_vignette() {
                // Top edge vignette
                let top_rect = egui::Rect::from_min_size(
                    screen_rect.min,
                    egui::vec2(screen_rect.width(), 80.0),
                );
                ui.painter().rect_filled(top_rect, 0.0, vignette_color);
                // Bottom edge vignette
                let bottom_rect = egui::Rect::from_min_size(
                    egui::pos2(screen_rect.min.x, screen_rect.max.y - 80.0),
                    egui::vec2(screen_rect.width(), 80.0),
                );
                ui.painter().rect_filled(bottom_rect, 0.0, vignette_color);
            }
        });
}

/// Draw a premium backdrop with accent glow
///
/// Similar to draw_backdrop but with a subtle accent glow in the center
/// for a more branded, luxurious feel.
#[allow(deprecated)]
pub fn draw_premium_backdrop(ctx: &egui::Context, theme: AppTheme, id_suffix: &str) {
    let screen_rect = ctx.screen_rect();
    egui::Area::new(egui::Id::new(format!("{id_suffix}_premium_backdrop")))
        .fixed_pos(screen_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            // Base backdrop
            let backdrop_color = theme.backdrop_color_strong();
            ui.painter().rect_filled(screen_rect, 0.0, backdrop_color);

            // Subtle accent glow in the center (where modal will appear)
            if let Some(glow_color) = theme.backdrop_accent_glow() {
                let center = screen_rect.center();
                let glow_rect = egui::Rect::from_center_size(center, egui::vec2(400.0, 300.0));
                ui.painter().rect_filled(glow_rect, 100.0, glow_color);
            }
        });
}
