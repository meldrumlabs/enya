//! Ambient background orbs — slow-moving soft gradient circles behind the workspace.
//!
//! Adds depth and life to dark themes without interfering with readability.
//! Orbs are drawn at very low opacity using radial-gradient meshes.

use egui::{Color32, Mesh, Pos2, Rect, Vec2};
use egui::epaint::Vertex;

use crate::ui::theme::AppTheme;

/// An individual ambient orb.
#[derive(Debug, Clone)]
struct Orb {
    /// Normalized base position [0..1] relative to screen size.
    base_pos: Vec2,
    /// Normalized radius (fraction of screen height).
    radius_norm: f32,
    /// Movement speed factor.
    speed: f32,
    /// Phase offset for animation.
    phase: f32,
}

/// Ambient background orbs renderer.
///
/// Create once and reuse; call [`AmbientOrbs::draw`] each frame.
#[derive(Debug, Clone)]
pub struct AmbientOrbs {
    orbs: Vec<Orb>,
}

impl Default for AmbientOrbs {
    fn default() -> Self {
        Self::new()
    }
}

impl AmbientOrbs {
    /// Create the default set of orbs.
    pub fn new() -> Self {
        Self {
            // Positions, radii and phases are carefully tuned so the orbs
            // drift through different screen quadrants over time without
            // clustering too often.
            orbs: vec![
                Orb {
                    base_pos: Vec2::new(0.25, 0.35),
                    radius_norm: 0.45,
                    speed: 0.18,
                    phase: 0.0,
                },
                Orb {
                    base_pos: Vec2::new(0.75, 0.60),
                    radius_norm: 0.38,
                    speed: 0.14,
                    phase: 2.1,
                },
                Orb {
                    base_pos: Vec2::new(0.50, 0.80),
                    radius_norm: 0.52,
                    speed: 0.11,
                    phase: 4.3,
                },
            ],
        }
    }

    /// Draw the orbs on the given rect.
    ///
    /// `time` should be seconds since app start (or any monotonic clock).
    /// The orbs are only rendered for dark themes; on light themes this
    /// is a no-op.
    pub fn draw(&self, painter: &egui::Painter, rect: Rect, theme: AppTheme, time: f32) {
        if theme.is_light() {
            return;
        }

        let colors = theme_orb_colors(theme);
        let screen_h = rect.height();

        for (i, orb) in self.orbs.iter().enumerate() {
            let color = colors[i % colors.len()];

            // Organic drift: each orb follows a lazy Lissajous-like path.
            let dx = (time * orb.speed + orb.phase).sin() * 0.12;
            let dy = (time * orb.speed * 0.7 + orb.phase + 1.0).sin() * 0.10;

            let pos = Pos2::new(
                rect.min.x + (orb.base_pos.x + dx) * rect.width(),
                rect.min.y + (orb.base_pos.y + dy) * screen_h,
            );

            let radius_px = orb.radius_norm * screen_h;
            let alpha = 0.06f32; // Very subtle: 6% opacity at center

            draw_soft_circle(painter, pos, radius_px, color, alpha);
        }
    }
}

/// Theme-specific orb palette.
///
/// Colors are muted, desaturated variants of each theme's accent so they
/// read as ambient light rather than UI elements.
fn theme_orb_colors(theme: AppTheme) -> Vec<Color32> {
    match theme {
        AppTheme::Meldrum => vec![
            Color32::from_rgb(200, 120, 40),  // Warm amber
            Color32::from_rgb(180, 90, 30),   // Deeper orange
            Color32::from_rgb(160, 100, 60),  // Rust
        ],
        AppTheme::Void => vec![
            Color32::from_rgb(80, 50, 160),   // Deep violet
            Color32::from_rgb(60, 40, 120),   // Darker violet
            Color32::from_rgb(100, 60, 180),  // Lighter violet
        ],
        AppTheme::Neon => vec![
            Color32::from_rgb(160, 40, 120),  // Magenta
            Color32::from_rgb(40, 120, 140),  // Cyan
            Color32::from_rgb(140, 40, 160),  // Purple
        ],
        AppTheme::Midnight => vec![
            Color32::from_rgb(40, 80, 160),   // Deep blue
            Color32::from_rgb(30, 60, 130),   // Darker blue
            Color32::from_rgb(50, 100, 180),  // Lighter blue
        ],
        AppTheme::Aurora => vec![
            Color32::from_rgb(60, 160, 120),  // Teal
            Color32::from_rgb(80, 140, 100),  // Muted teal
            Color32::from_rgb(100, 180, 140), // Light teal
        ],
        AppTheme::Ayu => vec![
            Color32::from_rgb(180, 140, 60),  // Amber
            Color32::from_rgb(160, 120, 50),  // Darker amber
            Color32::from_rgb(200, 160, 80),  // Light amber
        ],
        AppTheme::Graphite => vec![
            Color32::from_rgb(180, 100, 40),  // Orange
            Color32::from_rgb(160, 80, 30),   // Darker orange
            Color32::from_rgb(200, 120, 60),  // Light orange
        ],
        AppTheme::RosePine => vec![
            Color32::from_rgb(140, 100, 160), // Soft purple
            Color32::from_rgb(120, 80, 140),  // Darker purple
            Color32::from_rgb(160, 120, 180), // Light purple
        ],
        AppTheme::Everforest => vec![
            Color32::from_rgb(100, 140, 80),  // Sage
            Color32::from_rgb(80, 120, 60),   // Darker sage
            Color32::from_rgb(120, 160, 100), // Light sage
        ],
        AppTheme::Catppuccin => vec![
            Color32::from_rgb(120, 140, 200), // Soft blue
            Color32::from_rgb(100, 120, 180), // Darker blue
            Color32::from_rgb(140, 160, 220), // Light blue
        ],
        AppTheme::Arrakis => vec![
            Color32::from_rgb(160, 120, 60),  // Sand
            Color32::from_rgb(140, 100, 50),  // Darker sand
            Color32::from_rgb(180, 140, 80),  // Light sand
        ],
        AppTheme::Ink => vec![
            Color32::from_rgb(120, 120, 140), // Silver
            Color32::from_rgb(100, 100, 120), // Darker silver
            Color32::from_rgb(140, 140, 160), // Light silver
        ],
        AppTheme::Onyx => vec![
            Color32::from_rgb(160, 140, 60),  // Gold
            Color32::from_rgb(140, 120, 50),  // Darker gold
            Color32::from_rgb(180, 160, 80),  // Light gold
        ],
        // Dark / System / Custom fall back to a subtle emerald set
        _ => vec![
            Color32::from_rgb(40, 140, 100),  // Emerald
            Color32::from_rgb(30, 120, 80),   // Darker emerald
            Color32::from_rgb(60, 160, 120),  // Lighter emerald
        ],
    }
}

/// Draw a soft radial-gradient circle using a triangle-fan mesh.
///
/// `center_alpha` is the opacity at the center (0..1). The edge is always
/// fully transparent.
fn draw_soft_circle(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    center_alpha: f32,
) {
    if radius <= 0.0 || center_alpha <= 0.0 {
        return;
    }

    let segments = 32;
    let mut mesh = Mesh::default();

    // Center vertex
    let center_idx = mesh.vertices.len() as u32;
    let center_color = Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (center_alpha * 255.0).min(255.0) as u8,
    );
    mesh.vertices.push(Vertex {
        pos: center,
        uv: egui::epaint::WHITE_UV,
        color: center_color,
    });

    // Perimeter vertices (fully transparent)
    let edge_color = Color32::TRANSPARENT;
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let pos = center + Vec2::new(angle.cos(), angle.sin()) * radius;
        mesh.vertices.push(Vertex {
            pos,
            uv: egui::epaint::WHITE_UV,
            color: edge_color,
        });
    }

    // Triangle fan indices
    for i in 0..segments {
        mesh.indices.push(center_idx);
        mesh.indices.push(center_idx + 1 + i);
        mesh.indices.push(center_idx + 1 + i + 1);
    }

    painter.add(egui::Shape::mesh(mesh));
}
