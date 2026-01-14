//! Floating panes that hover above the tile layout.
//!
//! Floating panes are detachable, draggable windows that can be used for:
//! - Quick ad-hoc queries without disrupting the main layout
//! - Reference panels (docs, runbooks) during investigations
//! - Comparison views (yesterday's metrics over today's)
//! - Scratch space that doesn't pollute saved layouts

use egui::{Id, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::components::Component;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;

/// Unique identifier for a floating pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FloatingPaneId(pub u64);

/// Minimum size for floating panes.
const MIN_FLOATING_SIZE: Vec2 = Vec2::new(300.0, 200.0);

/// Default size for new floating panes.
const DEFAULT_FLOATING_SIZE: Vec2 = Vec2::new(500.0, 350.0);

/// Height of the title bar.
const TITLE_BAR_HEIGHT: f32 = 32.0;

/// Size of title bar buttons.
const BUTTON_SIZE: f32 = 24.0;

/// Distance from screen edge to trigger snapping (in pixels).
const SNAP_THRESHOLD: f32 = 20.0;

/// Margin from screen edge when snapped.
const SNAP_MARGIN: f32 = 8.0;

/// Duration of animations in seconds.
const ANIMATION_DURATION: f32 = 0.15;

/// Animation types for floating panes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AnimationKind {
    /// Pane is appearing (scale up, fade in).
    Appearing,
    /// Pane is minimizing (shrink to title bar).
    Minimizing,
    /// Pane is expanding from minimized.
    Expanding,
}

/// Smooth easing function (ease-out cubic).
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A pane that floats above the tile layout.
pub struct FloatingPane {
    /// Unique identifier for the floating pane.
    pub id: FloatingPaneId,
    /// The component being displayed.
    pub component: Box<dyn Component>,
    /// Current position (screen coordinates, top-left of title bar).
    pub position: Pos2,
    /// Current size (including title bar).
    pub size: Vec2,
    /// Whether this pane is minimized (collapsed to title bar only).
    pub minimized: bool,
    /// Z-index for stacking order (higher = on top).
    pub z_index: u32,
    /// Whether this pane is pinned (stays on top).
    pub pinned: bool,
    /// Whether this pane was created as scratch (auto-close on escape).
    pub is_scratch: bool,
    /// Optional name override (defaults to component name).
    pub custom_name: Option<String>,
    /// Whether being resized from the right edge.
    resize_right: bool,
    /// Whether being resized from the bottom edge.
    resize_bottom: bool,
    /// Whether being resized from the left edge.
    resize_left: bool,
    /// Whether being resized from the top edge.
    resize_top: bool,
    /// Whether this pane is maximized (fills the viewport).
    maximized: bool,
    /// Stored position/size before maximizing (for restore).
    pre_maximize_rect: Option<Rect>,
    /// Current animation state.
    animation: Option<(AnimationKind, Instant)>,
    /// Animation progress (0.0 to 1.0, for rendering).
    animation_progress: f32,
    /// Target height for minimize/expand animations.
    animation_target_height: f32,
    /// Start height for minimize/expand animations.
    animation_start_height: f32,
}

impl FloatingPane {
    /// Create a new floating pane with default size at the given position.
    pub fn new(id: FloatingPaneId, component: Box<dyn Component>, position: Pos2) -> Self {
        Self::with_size(id, component, position, DEFAULT_FLOATING_SIZE)
    }

    /// Create a new floating pane with a specific size.
    pub fn with_size(
        id: FloatingPaneId,
        component: Box<dyn Component>,
        position: Pos2,
        size: Vec2,
    ) -> Self {
        // Ensure minimum size constraints
        let size = Vec2::new(
            size.x.max(MIN_FLOATING_SIZE.x),
            size.y.max(MIN_FLOATING_SIZE.y),
        );
        Self {
            id,
            component,
            position,
            size,
            minimized: false,
            z_index: 0,
            pinned: false,
            is_scratch: false,
            custom_name: None,
            resize_right: false,
            resize_bottom: false,
            resize_left: false,
            resize_top: false,
            maximized: false,
            pre_maximize_rect: None,
            // Start with appearing animation
            animation: Some((AnimationKind::Appearing, Instant::now())),
            animation_progress: 0.0,
            animation_target_height: size.y,
            animation_start_height: size.y,
        }
    }

    /// Update animation state. Returns true if animation is in progress.
    pub fn update_animation(&mut self) -> bool {
        let Some((_kind, start)) = self.animation else {
            return false;
        };

        let elapsed = start.elapsed().as_secs_f32();
        let progress = (elapsed / ANIMATION_DURATION).min(1.0);
        self.animation_progress = ease_out_cubic(progress);

        if progress >= 1.0 {
            // Animation complete
            self.animation = None;
            self.animation_progress = 1.0;
            false
        } else {
            true
        }
    }

    /// Start a minimize animation.
    pub fn start_minimize(&mut self) {
        self.animation_start_height = self.size.y;
        self.animation_target_height = TITLE_BAR_HEIGHT;
        self.animation = Some((AnimationKind::Minimizing, Instant::now()));
        self.animation_progress = 0.0;
        self.minimized = true;
    }

    /// Start an expand (un-minimize) animation.
    pub fn start_expand(&mut self) {
        self.animation_start_height = TITLE_BAR_HEIGHT;
        self.animation_target_height = self.size.y;
        self.animation = Some((AnimationKind::Expanding, Instant::now()));
        self.animation_progress = 0.0;
        self.minimized = false;
    }

    /// Get the current animated height for rendering.
    pub fn animated_height(&self) -> f32 {
        // During minimize/expand animations, interpolate between start and target
        if let Some((AnimationKind::Minimizing | AnimationKind::Expanding, _)) = self.animation {
            let start = self.animation_start_height;
            let target = self.animation_target_height;
            return start + (target - start) * self.animation_progress;
        }

        // Default height based on minimized state
        if self.minimized {
            TITLE_BAR_HEIGHT
        } else {
            self.size.y
        }
    }

    /// Get the current scale for appearing animation.
    pub fn animated_scale(&self) -> f32 {
        if let Some((AnimationKind::Appearing, _)) = self.animation {
            0.95 + 0.05 * self.animation_progress
        } else {
            1.0
        }
    }

    /// Get the current opacity for appearing animation.
    pub fn animated_opacity(&self) -> f32 {
        if let Some((AnimationKind::Appearing, _)) = self.animation {
            self.animation_progress
        } else {
            1.0
        }
    }

    /// Check if currently animating.
    pub fn is_animating(&self) -> bool {
        self.animation.is_some()
    }

    /// Toggle maximized state. Returns true if now maximized.
    pub fn toggle_maximize(&mut self, viewport: Rect) -> bool {
        if self.maximized {
            // Restore to pre-maximize size
            if let Some(rect) = self.pre_maximize_rect.take() {
                self.position = rect.min;
                self.size = rect.size();
            }
            self.maximized = false;
            false
        } else {
            // Save current rect and maximize
            self.pre_maximize_rect = Some(Rect::from_min_size(self.position, self.size));
            self.position = viewport.min + Vec2::new(SNAP_MARGIN, SNAP_MARGIN);
            self.size = viewport.size() - Vec2::new(SNAP_MARGIN * 2.0, SNAP_MARGIN * 2.0);
            self.maximized = true;
            true
        }
    }

    /// Check if this pane is maximized.
    pub fn is_maximized(&self) -> bool {
        self.maximized
    }

    /// Create a new scratch floating pane (auto-closes on escape).
    pub fn new_scratch(id: FloatingPaneId, component: Box<dyn Component>, position: Pos2) -> Self {
        let mut pane = Self::new(id, component, position);
        pane.is_scratch = true;
        pane
    }

    /// Get the display name for this pane.
    pub fn name(&self) -> String {
        self.custom_name
            .clone()
            .unwrap_or_else(|| self.component.name())
    }

    /// Get the content area rect (below title bar).
    pub fn content_rect(&self) -> Rect {
        if self.minimized {
            Rect::NOTHING
        } else {
            Rect::from_min_size(
                self.position + Vec2::new(0.0, TITLE_BAR_HEIGHT),
                Vec2::new(self.size.x, self.size.y - TITLE_BAR_HEIGHT),
            )
        }
    }

    /// Get the full rect including title bar.
    pub fn full_rect(&self) -> Rect {
        let height = if self.minimized {
            TITLE_BAR_HEIGHT
        } else {
            self.size.y
        };
        Rect::from_min_size(self.position, Vec2::new(self.size.x, height))
    }
}

/// Result from showing a floating pane.
#[derive(Debug, Clone, PartialEq)]
pub enum FloatingPaneAction {
    /// No action needed.
    None,
    /// Close this floating pane.
    Close,
    /// Dock this pane back into the tile layout.
    Dock,
    /// Bring this pane to the front (update z-index).
    BringToFront,
    /// Toggle minimized state.
    ToggleMinimize,
    /// Toggle pinned state.
    TogglePin,
    /// Toggle maximized state (needs viewport rect to be handled).
    ToggleMaximize,
}

/// Render a floating pane and return any resulting action.
///
/// The `viewport` rect is used for maximize and edge snapping.
pub fn show_floating_pane(
    pane: &mut FloatingPane,
    ctx: &egui::Context,
    theme: AppTheme,
    is_focused: bool,
    viewport: Rect,
) -> FloatingPaneAction {
    let mut action = FloatingPaneAction::None;

    // Update animation state
    let is_animating = pane.update_animation();
    if is_animating {
        ctx.request_repaint();
    }

    // Determine layer order based on pinned state
    let order = if pane.pinned {
        egui::Order::Foreground
    } else {
        egui::Order::Middle
    };

    let pane_id = pane.id;

    // Get animated values
    let scale = pane.animated_scale();
    let opacity = pane.animated_opacity();
    let animated_height = pane.animated_height();

    // Calculate the exact size for this pane using animated height
    let pane_size = Vec2::new(pane.size.x, animated_height);

    // Calculate scaled position for appearing animation (scale from center)
    let scaled_position = if scale < 1.0 {
        let center = pane.position + pane_size / 2.0;
        let scaled_size = pane_size * scale;
        center - scaled_size / 2.0
    } else {
        pane.position
    };

    // Use egui::Area for the floating window with constrained size
    let _area_response = egui::Area::new(Id::new(("floating_pane", pane_id.0)))
        .fixed_pos(scaled_position)
        .order(order)
        .show(ctx, |ui| {
            // Apply scale to the size
            let scaled_pane_size = pane_size * scale;

            // Constrain the UI to the exact pane size
            ui.set_min_size(scaled_pane_size);
            ui.set_max_size(scaled_pane_size);

            // Apply opacity to colors
            let opacity_byte = (opacity * 255.0) as u8;

            // Glass effect: semi-transparent background with subtle tint
            // Uses a slightly lighter, more translucent version of bg_surface
            let base_color = theme.bg_surface();
            let glass_color = egui::Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                ((base_color.a() as f32 * 0.92) as u8).min(opacity_byte), // Slightly more transparent
            );

            // Shadow with animated opacity
            let shadow_alpha = (60.0 * opacity) as u8;

            let frame = egui::Frame::NONE
                .fill(glass_color)
                .corner_radius(8.0 * scale)
                .shadow(egui::Shadow {
                    spread: 0,
                    blur: (12.0 * scale) as u8,
                    offset: [0, (4.0 * scale) as i8],
                    color: egui::Color32::from_black_alpha(shadow_alpha),
                })
                .inner_margin(0.0);

            frame.show(ui, |ui| {
                // Constrain frame content to exact size
                ui.set_min_size(scaled_pane_size);
                ui.set_max_size(scaled_pane_size);

                // Apply opacity fade to the UI
                if opacity < 1.0 {
                    ui.set_opacity(opacity);
                }

                let available_width = pane.size.x * scale;

                // Title bar
                let title_action =
                    show_title_bar(ui, pane, theme, is_focused, available_width / scale);
                if title_action != FloatingPaneAction::None {
                    action = title_action;
                }

                // Content area (if not minimized and height allows)
                let show_content = !pane.minimized && animated_height > TITLE_BAR_HEIGHT + 10.0;
                if show_content {
                    let content_height = (animated_height - TITLE_BAR_HEIGHT).max(0.0) * scale;
                    // Use exact size allocation so content fills the space perfectly
                    let (content_rect, _response) = ui.allocate_exact_size(
                        Vec2::new(available_width, content_height),
                        egui::Sense::hover(),
                    );

                    // Render component in the allocated rect with strict bounds
                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
                    child_ui.set_min_size(content_rect.size());
                    child_ui.set_max_size(content_rect.size());
                    child_ui.set_clip_rect(content_rect);
                    pane.component.show(&mut child_ui);
                }
            });
        });

    // Handle dragging the title bar to move the pane
    let title_bar_rect =
        Rect::from_min_size(pane.position, Vec2::new(pane.size.x, TITLE_BAR_HEIGHT));
    let title_bar_response = ctx.input(|i| {
        i.pointer.any_down()
            && title_bar_rect.contains(i.pointer.interact_pos().unwrap_or_default())
    });

    if title_bar_response {
        action = FloatingPaneAction::BringToFront;
    }

    // Handle double-click on title bar to maximize/restore
    let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
    if let Some(pos) = pointer_pos {
        if title_bar_rect.contains(pos) {
            // Check for double-click
            let double_clicked = ctx.input(|i| {
                i.pointer
                    .button_double_clicked(egui::PointerButton::Primary)
            });
            if double_clicked {
                action = FloatingPaneAction::ToggleMaximize;
            }
        }
    }

    // Handle drag movement with edge snapping
    if let Some(pos) = pointer_pos {
        if ctx.input(|i| i.pointer.any_pressed()) && title_bar_rect.contains(pos) {
            // Start dragging - bring to front, and exit maximized state
            if pane.maximized {
                // Restore to normal size when starting to drag a maximized pane
                if let Some(rect) = pane.pre_maximize_rect.take() {
                    pane.size = rect.size();
                }
                pane.maximized = false;
            }
            action = FloatingPaneAction::BringToFront;
        }

        if ctx.input(|i| i.pointer.any_down()) {
            let delta = ctx.input(|i| i.pointer.delta());
            if delta != Vec2::ZERO && title_bar_rect.contains(pos - delta) {
                let mut new_pos = pane.position + delta;

                // Screen edge snapping
                let pane_rect = Rect::from_min_size(new_pos, pane.size);

                // Snap to left edge
                if (pane_rect.left() - viewport.left()).abs() < SNAP_THRESHOLD {
                    new_pos.x = viewport.left() + SNAP_MARGIN;
                }
                // Snap to right edge
                if (pane_rect.right() - viewport.right()).abs() < SNAP_THRESHOLD {
                    new_pos.x = viewport.right() - pane.size.x - SNAP_MARGIN;
                }
                // Snap to top edge
                if (pane_rect.top() - viewport.top()).abs() < SNAP_THRESHOLD {
                    new_pos.y = viewport.top() + SNAP_MARGIN;
                }
                // Snap to bottom edge
                if (pane_rect.bottom() - viewport.bottom()).abs() < SNAP_THRESHOLD {
                    new_pos.y = viewport.bottom() - pane.size.y - SNAP_MARGIN;
                }

                pane.position = new_pos;
            }
        }
    }

    // Handle resize from edges/corners (disabled when maximized)
    if !pane.maximized {
        handle_resize(pane, ctx);
    }

    action
}

/// Show the title bar with buttons and return any action.
fn show_title_bar(
    ui: &mut egui::Ui,
    pane: &mut FloatingPane,
    theme: AppTheme,
    is_focused: bool,
    width: f32,
) -> FloatingPaneAction {
    let mut action = FloatingPaneAction::None;

    let title_bar_rect = ui.allocate_space(Vec2::new(width, TITLE_BAR_HEIGHT)).1;

    // Background
    let bg_color = if is_focused {
        theme.bg_elevated()
    } else {
        theme.bg_surface()
    };
    // Only round the top corners of the title bar
    let title_rounding = egui::CornerRadius {
        nw: 8,
        ne: 8,
        sw: 0,
        se: 0,
    };
    ui.painter()
        .rect_filled(title_bar_rect, title_rounding, bg_color);

    // Bottom border
    ui.painter().line_segment(
        [title_bar_rect.left_bottom(), title_bar_rect.right_bottom()],
        Stroke::new(1.0, theme.border_subtle()),
    );

    // Pin indicator
    let pin_rect = Rect::from_min_size(
        title_bar_rect.left_top() + Vec2::new(8.0, (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2.0),
        Vec2::splat(BUTTON_SIZE),
    );

    let pin_response = ui.allocate_rect(pin_rect, egui::Sense::click());
    let pin_color = if pane.pinned {
        theme.accent_primary()
    } else if pin_response.hovered() {
        theme.text_secondary()
    } else {
        theme.text_tertiary()
    };

    // Pin icon (simplified pushpin)
    let pin_center = pin_rect.center();
    ui.painter().circle_filled(pin_center, 4.0, pin_color);
    ui.painter().line_segment(
        [pin_center, pin_center + Vec2::new(0.0, 6.0)],
        Stroke::new(2.0, pin_color),
    );

    if pin_response.clicked() {
        action = FloatingPaneAction::TogglePin;
    }

    // Title text - use relative offset from title bar left edge
    // Pin button: 8px margin + 24px button + 8px spacing = 40px
    let title_offset = 8.0 + BUTTON_SIZE + 8.0;
    let buttons_width = 4.0 * (BUTTON_SIZE + 4.0) + 8.0; // 4 buttons with spacing

    let title_pos = Pos2::new(
        title_bar_rect.left() + title_offset,
        title_bar_rect.center().y,
    );
    let title_color = if is_focused {
        theme.text_primary()
    } else {
        theme.text_secondary()
    };

    ui.painter().text(
        title_pos,
        egui::Align2::LEFT_CENTER,
        pane.name(),
        typography::body(),
        title_color,
    );

    // Buttons (right side): minimize, maximize (future), dock, close
    let buttons_start = title_bar_rect.right() - buttons_width;
    let button_y = title_bar_rect.top() + (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2.0;

    // Minimize button
    let minimize_rect =
        Rect::from_min_size(Pos2::new(buttons_start, button_y), Vec2::splat(BUTTON_SIZE));
    let minimize_response = ui.allocate_rect(minimize_rect, egui::Sense::click());
    let minimize_color = if minimize_response.hovered() {
        theme.text_primary()
    } else {
        theme.text_tertiary()
    };

    // Draw minimize icon (horizontal line)
    ui.painter().line_segment(
        [
            minimize_rect.center() - Vec2::new(5.0, 0.0),
            minimize_rect.center() + Vec2::new(5.0, 0.0),
        ],
        Stroke::new(2.0, minimize_color),
    );

    if minimize_response.clicked() {
        action = FloatingPaneAction::ToggleMinimize;
    }

    // Dock button
    let dock_rect = Rect::from_min_size(
        Pos2::new(buttons_start + BUTTON_SIZE + 4.0, button_y),
        Vec2::splat(BUTTON_SIZE),
    );
    let dock_response = ui.allocate_rect(dock_rect, egui::Sense::click());
    let dock_color = if dock_response.hovered() {
        theme.text_primary()
    } else {
        theme.text_tertiary()
    };

    // Draw dock icon (arrow pointing down into box)
    let dock_center = dock_rect.center();
    // Box outline
    ui.painter().rect_stroke(
        Rect::from_center_size(dock_center + Vec2::new(0.0, 2.0), Vec2::new(10.0, 8.0)),
        2.0,
        Stroke::new(1.5, dock_color),
        StrokeKind::Inside,
    );
    // Arrow
    ui.painter().line_segment(
        [
            dock_center - Vec2::new(0.0, 4.0),
            dock_center + Vec2::new(0.0, 2.0),
        ],
        Stroke::new(1.5, dock_color),
    );

    if dock_response.clicked() {
        action = FloatingPaneAction::Dock;
    }

    // Close button
    let close_rect = Rect::from_min_size(
        Pos2::new(buttons_start + 2.0 * (BUTTON_SIZE + 4.0), button_y),
        Vec2::splat(BUTTON_SIZE),
    );
    let close_response = ui.allocate_rect(close_rect, egui::Sense::click());
    let close_color = if close_response.hovered() {
        theme.semantic_error()
    } else {
        theme.text_tertiary()
    };

    // Draw X icon
    let close_center = close_rect.center();
    let x_offset = 5.0;
    ui.painter().line_segment(
        [
            close_center - Vec2::new(x_offset, x_offset),
            close_center + Vec2::new(x_offset, x_offset),
        ],
        Stroke::new(2.0, close_color),
    );
    ui.painter().line_segment(
        [
            close_center + Vec2::new(-x_offset, x_offset),
            close_center + Vec2::new(x_offset, -x_offset),
        ],
        Stroke::new(2.0, close_color),
    );

    if close_response.clicked() {
        action = FloatingPaneAction::Close;
    }

    action
}

/// Handle resize from edges and corners.
fn handle_resize(pane: &mut FloatingPane, ctx: &egui::Context) {
    if pane.minimized {
        return;
    }

    let full_rect = pane.full_rect();
    let edge_width = 6.0;
    let corner_size = 12.0;

    // Define edge/corner rects
    let right_edge = Rect::from_min_size(
        Pos2::new(
            full_rect.right() - edge_width,
            full_rect.top() + corner_size,
        ),
        Vec2::new(edge_width, full_rect.height() - 2.0 * corner_size),
    );
    let bottom_edge = Rect::from_min_size(
        Pos2::new(
            full_rect.left() + corner_size,
            full_rect.bottom() - edge_width,
        ),
        Vec2::new(full_rect.width() - 2.0 * corner_size, edge_width),
    );
    let bottom_right = Rect::from_min_size(
        Pos2::new(
            full_rect.right() - corner_size,
            full_rect.bottom() - corner_size,
        ),
        Vec2::splat(corner_size),
    );

    let pointer_pos = ctx.input(|i| i.pointer.interact_pos());

    if let Some(pos) = pointer_pos {
        // Check if starting resize
        if ctx.input(|i| i.pointer.any_pressed()) {
            if bottom_right.contains(pos) {
                pane.resize_right = true;
                pane.resize_bottom = true;
            } else if right_edge.contains(pos) {
                pane.resize_right = true;
            } else if bottom_edge.contains(pos) {
                pane.resize_bottom = true;
            }
        }

        // Apply resize delta
        if ctx.input(|i| i.pointer.any_down()) {
            let delta = ctx.input(|i| i.pointer.delta());
            if delta != Vec2::ZERO {
                if pane.resize_right {
                    pane.size.x = (pane.size.x + delta.x).max(MIN_FLOATING_SIZE.x);
                }
                if pane.resize_bottom {
                    pane.size.y = (pane.size.y + delta.y).max(MIN_FLOATING_SIZE.y);
                }
            }
        }

        // End resize
        if ctx.input(|i| i.pointer.any_released()) {
            pane.resize_right = false;
            pane.resize_bottom = false;
            pane.resize_left = false;
            pane.resize_top = false;
        }

        // Set cursor based on hover
        if bottom_right.contains(pos) || (pane.resize_right && pane.resize_bottom) {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        } else if right_edge.contains(pos) || pane.resize_right {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if bottom_edge.contains(pos) || pane.resize_bottom {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
    }
}

/// Manager for all floating panes in a workspace.
pub struct FloatingPaneManager {
    /// All floating panes.
    pub panes: Vec<FloatingPane>,
    /// Counter for generating unique IDs.
    next_id: u64,
    /// Currently focused floating pane.
    focused: Option<FloatingPaneId>,
    /// Next z-index to assign.
    next_z_index: u32,
}

impl Default for FloatingPaneManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FloatingPaneManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            panes: Vec::new(),
            next_id: 1,
            focused: None,
            next_z_index: 1,
        }
    }

    /// Check if there are any floating panes.
    pub fn has_panes(&self) -> bool {
        !self.panes.is_empty()
    }

    /// Get the number of floating panes.
    pub fn count(&self) -> usize {
        self.panes.len()
    }

    /// Check if any floating pane is focused.
    pub fn is_focused(&self) -> bool {
        self.focused.is_some()
    }

    /// Get the focused floating pane ID.
    pub fn focused_id(&self) -> Option<FloatingPaneId> {
        self.focused
    }

    /// Set focus to a specific floating pane.
    pub fn set_focus(&mut self, id: Option<FloatingPaneId>) {
        self.focused = id;
        if let Some(id) = id {
            self.bring_to_front(id);
        }
    }

    /// Clear focus from floating panes.
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Create a new floating pane and add it to the manager.
    pub fn add_pane(&mut self, component: Box<dyn Component>, position: Pos2) -> FloatingPaneId {
        self.add_pane_with_size(component, position, DEFAULT_FLOATING_SIZE)
    }

    /// Create a new floating pane with a specific size.
    pub fn add_pane_with_size(
        &mut self,
        component: Box<dyn Component>,
        position: Pos2,
        size: Vec2,
    ) -> FloatingPaneId {
        let id = FloatingPaneId(self.next_id);
        self.next_id += 1;

        let mut pane = FloatingPane::with_size(id, component, position, size);
        pane.z_index = self.next_z_index;
        self.next_z_index += 1;

        self.panes.push(pane);
        self.focused = Some(id);

        id
    }

    /// Create a new scratch floating pane.
    pub fn add_scratch_pane(
        &mut self,
        component: Box<dyn Component>,
        position: Pos2,
    ) -> FloatingPaneId {
        let id = FloatingPaneId(self.next_id);
        self.next_id += 1;

        let mut pane = FloatingPane::new_scratch(id, component, position);
        pane.z_index = self.next_z_index;
        self.next_z_index += 1;

        self.panes.push(pane);
        self.focused = Some(id);

        id
    }

    /// Remove a floating pane by ID and return its component.
    pub fn remove_pane(&mut self, id: FloatingPaneId) -> Option<Box<dyn Component>> {
        if let Some(idx) = self.panes.iter().position(|p| p.id == id) {
            let pane = self.panes.remove(idx);

            // Clear focus if we removed the focused pane
            if self.focused == Some(id) {
                self.focused = self.panes.last().map(|p| p.id);
            }

            Some(pane.component)
        } else {
            None
        }
    }

    /// Bring a pane to the front (highest z-index).
    pub fn bring_to_front(&mut self, id: FloatingPaneId) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == id) {
            pane.z_index = self.next_z_index;
            self.next_z_index += 1;
        }
    }

    /// Cycle focus to the next floating pane. Returns true if cycled within floating panes.
    pub fn cycle_focus_next(&mut self) -> bool {
        if self.panes.is_empty() {
            return false;
        }

        match self.focused {
            Some(current_id) => {
                let current_idx = self.panes.iter().position(|p| p.id == current_id);
                if let Some(idx) = current_idx {
                    let next_idx = (idx + 1) % self.panes.len();
                    self.focused = Some(self.panes[next_idx].id);
                    self.bring_to_front(self.panes[next_idx].id);
                    true
                } else {
                    self.focused = Some(self.panes[0].id);
                    true
                }
            }
            None => {
                self.focused = Some(self.panes[0].id);
                self.bring_to_front(self.panes[0].id);
                true
            }
        }
    }

    /// Cycle focus to the previous floating pane.
    pub fn cycle_focus_prev(&mut self) -> bool {
        if self.panes.is_empty() {
            return false;
        }

        match self.focused {
            Some(current_id) => {
                let current_idx = self.panes.iter().position(|p| p.id == current_id);
                if let Some(idx) = current_idx {
                    let prev_idx = if idx == 0 {
                        self.panes.len() - 1
                    } else {
                        idx - 1
                    };
                    self.focused = Some(self.panes[prev_idx].id);
                    self.bring_to_front(self.panes[prev_idx].id);
                    true
                } else {
                    self.focused = Some(self.panes[0].id);
                    true
                }
            }
            None => {
                let last_idx = self.panes.len() - 1;
                self.focused = Some(self.panes[last_idx].id);
                self.bring_to_front(self.panes[last_idx].id);
                true
            }
        }
    }

    /// Close any scratch panes (called on Escape).
    pub fn close_scratch_panes(&mut self) -> Vec<Box<dyn Component>> {
        // Partition panes into scratch and non-scratch (O(n) instead of O(n²))
        let (scratch, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.panes)
            .into_iter()
            .partition(|p| p.is_scratch);
        self.panes = kept;

        // Clear focus if we closed the focused pane
        if let Some(focused_id) = self.focused {
            if !self.panes.iter().any(|p| p.id == focused_id) {
                self.focused = self.panes.last().map(|p| p.id);
            }
        }

        scratch.into_iter().map(|p| p.component).collect()
    }

    /// Render all floating panes and handle actions.
    ///
    /// The `viewport` rect is used for maximize and edge snapping.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        theme: AppTheme,
        viewport: Rect,
    ) -> Vec<(FloatingPaneId, FloatingPaneAction)> {
        let mut actions = Vec::new();

        // Sort by z_index for rendering order
        let mut indices: Vec<usize> = (0..self.panes.len()).collect();
        indices.sort_by_key(|&i| self.panes[i].z_index);

        for idx in indices {
            let pane = &mut self.panes[idx];
            let is_focused = self.focused == Some(pane.id);
            let pane_id = pane.id;

            let action = show_floating_pane(pane, ctx, theme, is_focused, viewport);

            if action != FloatingPaneAction::None {
                actions.push((pane_id, action));
            }
        }

        actions
    }

    /// Toggle maximize state for a pane.
    pub fn toggle_maximize(&mut self, id: FloatingPaneId, viewport: Rect) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == id) {
            pane.toggle_maximize(viewport);
        }
    }

    /// Toggle minimize state for a pane with animation.
    pub fn toggle_minimize(&mut self, id: FloatingPaneId) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == id) {
            if pane.minimized {
                pane.start_expand();
            } else {
                pane.start_minimize();
            }
        }
    }

    /// Auto-arrange all floating panes in a tiled grid layout.
    ///
    /// Arranges panes in a grid that fits within the viewport, with equal spacing.
    pub fn arrange_panes(&mut self, viewport: Rect) {
        let count = self.panes.len();
        if count == 0 {
            return;
        }

        // Calculate grid dimensions
        let cols = (count as f32).sqrt().ceil() as usize;
        let rows = count.div_ceil(cols);

        // Calculate cell size with margins
        let margin = SNAP_MARGIN;
        let gap = 12.0; // Gap between panes
        let available_width = viewport.width() - margin * 2.0 - gap * (cols - 1) as f32;
        let available_height = viewport.height() - margin * 2.0 - gap * (rows - 1) as f32;
        let cell_width = available_width / cols as f32;
        let cell_height = available_height / rows as f32;

        // Ensure minimum size
        let pane_width = cell_width.max(MIN_FLOATING_SIZE.x);
        let pane_height = cell_height.max(MIN_FLOATING_SIZE.y);

        // Position each pane
        for (i, pane) in self.panes.iter_mut().enumerate() {
            let col = i % cols;
            let row = i / cols;

            let x = viewport.left() + margin + (col as f32) * (pane_width + gap);
            let y = viewport.top() + margin + (row as f32) * (pane_height + gap);

            pane.position = Pos2::new(x, y);
            pane.size = Vec2::new(pane_width, pane_height);

            // Un-maximize and un-minimize for consistent appearance
            pane.maximized = false;
            pane.pre_maximize_rect = None;
            if pane.minimized {
                pane.start_expand();
            }
        }
    }

    /// Apply theme to all floating pane components.
    pub fn set_theme(&mut self, theme: AppTheme) {
        for pane in &mut self.panes {
            pane.component.set_theme(theme);
        }
    }

    /// Apply API key to all floating pane components.
    pub fn set_api_key(&mut self, key: &str) {
        for pane in &mut self.panes {
            pane.component.set_api_key(key);
        }
    }

    /// Get a mutable reference to the focused pane's component.
    pub fn focused_component_mut(&mut self) -> Option<&mut Box<dyn Component>> {
        let focused_id = self.focused?;
        self.panes
            .iter_mut()
            .find(|p| p.id == focused_id)
            .map(|p| &mut p.component)
    }
}
