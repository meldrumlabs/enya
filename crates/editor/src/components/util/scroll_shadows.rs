//! Scroll Shadows - VS Code style fade gradients for scrollable content.
//!
//! Renders subtle gradient shadows at the top and bottom edges of scroll areas
//! to indicate when there's more content in that direction.

use egui::{Color32, Mesh, Rect, Ui, Vec2};

/// Configuration for scroll shadow rendering.
#[derive(Clone, Copy)]
pub struct ScrollShadowConfig {
    /// Height of the shadow gradient in pixels.
    pub shadow_height: f32,
    /// Base color for the shadow (usually matches background).
    pub color: Color32,
    /// Maximum opacity at the edge (0.0 to 1.0).
    pub max_opacity: f32,
}

impl Default for ScrollShadowConfig {
    fn default() -> Self {
        Self {
            shadow_height: 24.0,
            color: Color32::BLACK,
            max_opacity: 0.4,
        }
    }
}

impl ScrollShadowConfig {
    /// Create a new config with the given shadow color.
    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Create a new config with the given shadow height.
    pub fn with_height(mut self, height: f32) -> Self {
        self.shadow_height = height;
        self
    }

    /// Create a new config with the given max opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.max_opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

/// Scroll state for determining which shadows to show.
#[derive(Clone, Copy, Default)]
pub struct ScrollState {
    /// Whether there's content above the visible area.
    pub can_scroll_up: bool,
    /// Whether there's content below the visible area.
    pub can_scroll_down: bool,
}

impl ScrollState {
    /// Create scroll state from an egui ScrollArea output.
    pub fn from_scroll_output(content_size: Vec2, inner_rect: Rect, offset: Vec2) -> Self {
        let can_scroll_up = offset.y > 1.0;
        let can_scroll_down = offset.y + inner_rect.height() < content_size.y - 1.0;

        Self {
            can_scroll_up,
            can_scroll_down,
        }
    }
}

/// Render scroll shadows on a given rect.
///
/// Call this after rendering your ScrollArea content.
pub fn render_scroll_shadows(
    ui: &mut Ui,
    rect: Rect,
    state: ScrollState,
    config: ScrollShadowConfig,
) {
    let painter = ui.painter();

    // Top shadow (fades from opaque to transparent going down)
    if state.can_scroll_up {
        let top_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), config.shadow_height));

        let mut mesh = Mesh::default();

        // Top edge (opaque)
        let top_color = Color32::from_rgba_unmultiplied(
            config.color.r(),
            config.color.g(),
            config.color.b(),
            (config.max_opacity * 255.0) as u8,
        );

        // Bottom edge (transparent)
        let bottom_color = Color32::TRANSPARENT;

        // Build gradient quad
        let tl = mesh.vertices.len() as u32;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: top_rect.left_top(),
            uv: egui::epaint::WHITE_UV,
            color: top_color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: top_rect.right_top(),
            uv: egui::epaint::WHITE_UV,
            color: top_color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: top_rect.right_bottom(),
            uv: egui::epaint::WHITE_UV,
            color: bottom_color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: top_rect.left_bottom(),
            uv: egui::epaint::WHITE_UV,
            color: bottom_color,
        });

        mesh.indices
            .extend_from_slice(&[tl, tl + 1, tl + 2, tl, tl + 2, tl + 3]);

        painter.add(egui::Shape::mesh(mesh));
    }

    // Bottom shadow (fades from transparent to opaque going down)
    if state.can_scroll_down {
        let bottom_rect = Rect::from_min_size(
            rect.left_bottom() - Vec2::new(0.0, config.shadow_height),
            Vec2::new(rect.width(), config.shadow_height),
        );

        let mut mesh = Mesh::default();

        // Top edge (transparent)
        let top_color = Color32::TRANSPARENT;

        // Bottom edge (opaque)
        let bottom_color = Color32::from_rgba_unmultiplied(
            config.color.r(),
            config.color.g(),
            config.color.b(),
            (config.max_opacity * 255.0) as u8,
        );

        // Build gradient quad
        let tl = mesh.vertices.len() as u32;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: bottom_rect.left_top(),
            uv: egui::epaint::WHITE_UV,
            color: top_color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: bottom_rect.right_top(),
            uv: egui::epaint::WHITE_UV,
            color: top_color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: bottom_rect.right_bottom(),
            uv: egui::epaint::WHITE_UV,
            color: bottom_color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: bottom_rect.left_bottom(),
            uv: egui::epaint::WHITE_UV,
            color: bottom_color,
        });

        mesh.indices
            .extend_from_slice(&[tl, tl + 1, tl + 2, tl, tl + 2, tl + 3]);

        painter.add(egui::Shape::mesh(mesh));
    }
}

/// A helper wrapper for ScrollArea that automatically renders shadows.
///
/// Usage:
/// ```ignore
/// let scroll_output = ScrollAreaWithShadows::new(theme)
///     .show(ui, |ui| {
///         // Your scrollable content
///     });
/// ```
pub struct ScrollAreaWithShadows {
    config: ScrollShadowConfig,
    id_salt: Option<egui::Id>,
    max_height: Option<f32>,
    auto_shrink: [bool; 2],
}

impl ScrollAreaWithShadows {
    /// Create a new scroll area with shadows using theme colors.
    pub fn new(bg_color: Color32) -> Self {
        Self {
            config: ScrollShadowConfig::default().with_color(bg_color),
            id_salt: None,
            max_height: None,
            auto_shrink: [true, true],
        }
    }

    /// Set a custom shadow config.
    pub fn with_config(mut self, config: ScrollShadowConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the ID salt for the scroll area.
    pub fn id_salt(mut self, id: impl std::hash::Hash) -> Self {
        self.id_salt = Some(egui::Id::new(id));
        self
    }

    /// Set the maximum height.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Set auto-shrink behavior.
    pub fn auto_shrink(mut self, auto_shrink: [bool; 2]) -> Self {
        self.auto_shrink = auto_shrink;
        self
    }

    /// Show the scroll area with content.
    pub fn show<R>(
        self,
        ui: &mut Ui,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        let mut scroll_area = egui::ScrollArea::vertical().auto_shrink(self.auto_shrink);

        if let Some(id) = self.id_salt {
            scroll_area = scroll_area.id_salt(id);
        }

        if let Some(height) = self.max_height {
            scroll_area = scroll_area.max_height(height);
        }

        let output = scroll_area.show(ui, add_contents);

        // Calculate scroll state
        let state = ScrollState::from_scroll_output(
            output.content_size,
            output.inner_rect,
            output.state.offset,
        );

        // Render shadows on top of the scroll area
        render_scroll_shadows(ui, output.inner_rect, state, self.config);

        output
    }
}
