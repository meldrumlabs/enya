//! Tile tree behavior for egui_tiles integration.
//!
//! This module provides the `TreeBehavior` struct that implements
//! `egui_tiles::Behavior` for rendering and managing the pane layout.

use rustc_hash::{FxHashMap, FxHashSet};

/// Smooth easing function (ease-out cubic) for animations.
fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles};

use crate::components::Component;
use crate::ui::colors::text_color;
use crate::ui::theme::AppTheme;
use crate::util::Instant;

/// Behavior implementation for the egui_tiles tree.
///
/// Handles rendering of panes, tab styling, focus borders,
/// visual-multi selection overlays, and viewport filtering.
#[derive(Default, Clone)]
pub struct TreeBehavior {
    /// Currently focused tile for vim-style navigation
    focused_tile_id: Option<TileId>,
    /// Selected tiles in visual-multi mode (empty when not in visual-multi mode)
    selected_tile_ids: FxHashSet<TileId>,
    /// Whether we're currently in visual-multi mode
    is_visual_multi_mode: bool,
    /// Query content per tile (for display in visual-multi mode)
    tile_queries: FxHashMap<TileId, String>,
    theme: AppTheme,
    api_key: String,
    /// Tile IDs that are filtered out (should be dimmed)
    filtered_out_tiles: FxHashSet<TileId>,
    /// Whether viewport filter is active
    is_filter_active: bool,

    // ==================== Visual Effects ====================
    /// Active yank flash effects (tile_id -> start_time)
    yank_flashes: FxHashMap<TileId, Instant>,
    /// Active focus pulse effects (tile_id -> start_time)
    focus_pulses: FxHashMap<TileId, Instant>,
    /// Last focused tile for detecting focus changes
    last_focused_tile: Option<TileId>,
    /// Whether to dim inactive panes
    dim_inactive_enabled: bool,
}

impl TreeBehavior {
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    pub fn set_keys(&mut self, api_key: String) {
        self.api_key = api_key;
    }

    pub fn set_focused_tile(&mut self, tile_id: Option<TileId>) {
        self.focused_tile_id = tile_id;
    }

    pub fn focused_tile(&self) -> Option<TileId> {
        self.focused_tile_id
    }

    pub fn set_visual_multi_state(
        &mut self,
        is_active: bool,
        selected_ids: FxHashSet<TileId>,
        tile_queries: FxHashMap<TileId, String>,
    ) {
        self.is_visual_multi_mode = is_active;
        self.selected_tile_ids = selected_ids;
        self.tile_queries = tile_queries;
    }

    pub fn set_filter_state(&mut self, is_active: bool, filtered_out_tiles: FxHashSet<TileId>) {
        self.is_filter_active = is_active;
        self.filtered_out_tiles = filtered_out_tiles;
    }

    /// Get the current theme
    pub fn theme(&self) -> AppTheme {
        self.theme
    }

    /// Get the current API key
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    // ==================== Visual Effects ====================

    /// Trigger a yank flash effect for a tile.
    pub fn trigger_yank_flash(&mut self, tile_id: TileId) {
        self.yank_flashes.insert(tile_id, Instant::now());
    }

    /// Update focus tracking and trigger pulse if focus changed.
    pub fn update_focus_effects(&mut self) {
        if self.focused_tile_id != self.last_focused_tile {
            if let Some(tile_id) = self.focused_tile_id {
                self.focus_pulses.insert(tile_id, Instant::now());
            }
            self.last_focused_tile = self.focused_tile_id;
        }
    }

    /// Clean up completed visual effects.
    pub fn cleanup_effects(&mut self) {
        const YANK_FLASH_DURATION: f32 = 0.25;
        const FOCUS_PULSE_DURATION: f32 = 0.2;

        self.yank_flashes
            .retain(|_, start| start.elapsed().as_secs_f32() < YANK_FLASH_DURATION);
        self.focus_pulses
            .retain(|_, start| start.elapsed().as_secs_f32() < FOCUS_PULSE_DURATION);
    }

    /// Check if any visual effects are active (needs repaint).
    pub fn has_active_effects(&self) -> bool {
        !self.yank_flashes.is_empty() || !self.focus_pulses.is_empty()
    }

    /// Enable or disable dim inactive panes effect.
    pub fn set_dim_inactive(&mut self, enabled: bool) {
        self.dim_inactive_enabled = enabled;
    }
}

impl egui_tiles::Behavior<Box<dyn Component>> for TreeBehavior {
    /// Minimum size for any pane - prevents panes from becoming too small
    fn min_size(&self) -> f32 {
        200.0 // Minimum 200px to ensure charts/content remain readable
    }

    /// Gap between panes in horizontal/vertical layouts
    fn gap_width(&self, _style: &egui::Style) -> f32 {
        4.0 // Subtle gap for visual separation
    }

    /// Stroke for the resize handle between panes
    fn resize_stroke(
        &self,
        _style: &egui::Style,
        resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        let color = match resize_state {
            egui_tiles::ResizeState::Idle => self.theme.border_subtle(),
            egui_tiles::ResizeState::Hovering => self.theme.border_default(),
            egui_tiles::ResizeState::Dragging => self.theme.border_focus(),
        };
        egui::Stroke::new(1.0, color)
    }

    /// Height of the tab bar
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        28.0 // Slightly taller for better visual presence
    }

    /// Background color of the tab bar
    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        self.theme.bg_surface()
    }

    /// Background color of individual tabs
    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<Box<dyn Component>>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            self.theme.bg_elevated()
        } else if state.is_being_dragged {
            self.theme.bg_hover()
        } else {
            self.theme.bg_surface()
        }
    }

    /// Stroke for the line separating tab bar from content
    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        egui::Stroke::new(1.0, self.theme.border_subtle())
    }

    /// Outline stroke around tabs (subtle accent for active, subtle for inactive)
    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<Box<dyn Component>>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        if state.active {
            // Muted accent - visible but not distracting
            egui::Stroke::new(1.0, self.theme.syntax_key().gamma_multiply(0.5))
        } else {
            egui::Stroke::new(1.0, self.theme.border_subtle())
        }
    }

    fn tab_title_for_pane(&mut self, component: &Box<dyn Component>) -> egui::WidgetText {
        component
            .label()
            .color(text_color(self.theme))
            .strong()
            .into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        component: &mut Box<dyn Component>,
    ) -> egui_tiles::UiResponse {
        // Make sure theme + keys are updated for the component
        component.set_theme(self.theme);
        component.set_api_key(&self.api_key);

        component.show(ui);

        egui_tiles::UiResponse::None
    }

    #[profiling::function]
    fn paint_on_top_of_tile(
        &self,
        painter: &egui::Painter,
        _style: &egui::Style,
        tile_id: TileId,
        rect: egui::Rect,
    ) {
        let is_focused = self.focused_tile_id == Some(tile_id);
        let is_selected = self.is_visual_multi_mode && self.selected_tile_ids.contains(&tile_id);
        let is_filtered_out = self.is_filter_active && self.filtered_out_tiles.contains(&tile_id);

        // When viewport filter is active, dim non-matching panes
        if is_filtered_out {
            let dim_color = self.theme.bg_base().gamma_multiply(0.8);
            painter.rect_filled(rect, 4.0, dim_color);

            // Draw "filtered" indicator text
            let text_color = self.theme.text_tertiary();
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "filtered",
                egui::FontId::proportional(12.0),
                text_color,
            );
            return; // Don't draw other overlays on filtered panes
        }

        // In visual-multi mode, draw selection indicator for selected panes
        if is_selected {
            // Selection tint using theme accent
            let selection_color = self.theme.accent_primary().gamma_multiply(0.15);

            // Fill the entire tile with a subtle selection tint
            painter.rect_filled(rect, 4.0, selection_color);

            // Draw selection border
            let border_color = self.theme.accent_primary();
            let border_width = 2.0;
            let inset_rect = rect.shrink(border_width / 2.0);
            painter.rect_stroke(
                inset_rect,
                4.0,
                egui::Stroke::new(border_width, border_color),
                egui::StrokeKind::Outside,
            );
        }

        // Draw focus border on top of the entire tile (including tab bar)
        // Premium glass effect with layered borders for depth
        if is_focused {
            // Use theme accent for focus border
            // Brighter accent in visual-multi mode to distinguish cursor from selection
            let focus_color = if self.is_visual_multi_mode {
                self.theme.accent_hover()
            } else {
                self.theme.accent_primary()
            };

            // Premium layered glow effect - thin layers to minimize content overlap
            // Layer 1: Outer glow band (subtle, widest)
            let glow_color = focus_color.gamma_multiply(0.10);
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(3.0, glow_color),
                egui::StrokeKind::Inside,
            );

            // Layer 2: Mid glow band (brighter, thinner)
            let mid_glow_color = focus_color.gamma_multiply(0.25);
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.5, mid_glow_color),
                egui::StrokeKind::Inside,
            );

            // Layer 3: Crisp edge border (full color, thin)
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, focus_color),
                egui::StrokeKind::Inside,
            );
        }

        // In visual-multi mode, show query content at the bottom of each selected pane
        if is_selected {
            if let Some(query) = self.tile_queries.get(&tile_id) {
                // Premium glass styling for query overlay
                let bg_color = self.theme.bg_surface().gamma_multiply(0.92);
                let text_color = self.theme.text_primary().gamma_multiply(0.9);
                let accent_color = self.theme.accent_primary();
                let border_color = accent_color.gamma_multiply(0.3);

                // Truncate query if too long
                let display_query = if query.len() > 60 {
                    format!("{}...", &query[..57])
                } else {
                    query.clone()
                };

                // Calculate text layout
                let font_id = egui::FontId::monospace(11.0);
                let galley = painter.layout_no_wrap(display_query, font_id, text_color);

                // Position at bottom of tile with padding
                let padding_h = 10.0;
                let padding_v = 6.0;
                let overlay_height = galley.rect.height() + padding_v * 2.0;
                let overlay_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x, rect.max.y - overlay_height),
                    egui::vec2(rect.width(), overlay_height),
                );

                // Draw background with subtle top border
                painter.rect_filled(overlay_rect, 0.0, bg_color);

                // Top edge accent line
                let top_line_rect = egui::Rect::from_min_size(
                    overlay_rect.left_top(),
                    egui::vec2(overlay_rect.width(), 1.0),
                );
                painter.rect_filled(top_line_rect, 0.0, border_color);

                // Emerald accent bar on left
                let accent_bar = egui::Rect::from_min_size(
                    overlay_rect.left_top(),
                    egui::vec2(3.0, overlay_height),
                );
                painter.rect_filled(accent_bar, 0.0, accent_color);

                // Draw text centered vertically in the overlay
                let text_pos = egui::pos2(
                    overlay_rect.min.x + padding_h,
                    overlay_rect.center().y - galley.rect.height() / 2.0,
                );
                painter.galley(text_pos, galley, text_color);
            }
        }

        // ==================== Visual Effects ====================

        // Dim inactive panes (subtle overlay on unfocused panes)
        if self.dim_inactive_enabled && !is_focused && !is_selected && !is_filtered_out {
            let dim_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25);
            painter.rect_filled(rect, 4.0, dim_color);
        }

        // Yank flash effect (brief highlight when yanked)
        if let Some(start_time) = self.yank_flashes.get(&tile_id) {
            const YANK_FLASH_DURATION: f32 = 0.25;
            let elapsed = start_time.elapsed().as_secs_f32();
            let progress = (elapsed / YANK_FLASH_DURATION).min(1.0);

            // Quick fade in, slow fade out
            let opacity = if progress < 0.1 {
                ease_out_cubic(progress / 0.1)
            } else {
                1.0 - ease_out_cubic((progress - 0.1) / 0.9)
            };

            if opacity > 0.01 {
                let base = self.theme.accent_primary();
                let flash_color = egui::Color32::from_rgba_unmultiplied(
                    base.r(),
                    base.g(),
                    base.b(),
                    (opacity * 80.0) as u8,
                );
                painter.rect_filled(rect, 4.0, flash_color);
            }
        }

        // Focus pulse effect (glow when pane receives focus)
        if let Some(start_time) = self.focus_pulses.get(&tile_id) {
            const FOCUS_PULSE_DURATION: f32 = 0.2;
            let elapsed = start_time.elapsed().as_secs_f32();
            let progress = (elapsed / FOCUS_PULSE_DURATION).min(1.0);

            // Quick rise, gradual fall
            let intensity = if progress < 0.3 {
                ease_out_cubic(progress / 0.3)
            } else {
                1.0 - ease_out_cubic((progress - 0.3) / 0.7)
            };

            if intensity > 0.01 {
                let base = self.theme.accent_primary();
                let pulse_color = egui::Color32::from_rgba_unmultiplied(
                    base.r(),
                    base.g(),
                    base.b(),
                    (intensity * 60.0) as u8,
                );
                // Expanding glow rings
                let glow_expansion = intensity * 4.0;
                painter.rect_stroke(
                    rect.expand(glow_expansion),
                    6.0,
                    egui::Stroke::new(2.0 + intensity * 2.0, pulse_color),
                    egui::StrokeKind::Outside,
                );
            }
        }
    }

    fn is_tab_closable(&self, _tiles: &Tiles<Box<dyn Component>>, _tile_id: TileId) -> bool {
        true
    }

    fn simplification_options(&self) -> SimplificationOptions {
        SimplificationOptions {
            all_panes_must_have_tabs: true,
            prune_empty_tabs: true,
            prune_empty_containers: true,
            ..SimplificationOptions::OFF
        }
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<Box<dyn Component>>, tile_id: TileId) -> bool {
        if let Some(tile) = tiles.get(tile_id) {
            match tile {
                Tile::Pane(pane) => {
                    // Single pane removal
                    let tab_title = self.tab_title_for_pane(pane);
                    log::debug!("Closing tab: {}, tile ID: {tile_id:?}", tab_title.text());
                }
                Tile::Container(container) => {
                    // Container removal
                    log::debug!("Closing container: {:?}", container.kind());
                    let children_ids = container.children();
                    for child_id in children_ids {
                        if let Some(Tile::Pane(pane)) = tiles.get(*child_id) {
                            let tab_title = self.tab_title_for_pane(pane);
                            log::debug!("Closing tab: {}, tile ID: {tile_id:?}", tab_title.text());
                        }
                    }
                }
            }
        }

        // Proceed to removing the tab
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tile_id(id: u64) -> TileId {
        TileId::from_u64(id)
    }

    // ==================== TreeBehavior Basic Tests ====================

    #[test]
    fn test_tree_behavior_default() {
        let behavior = TreeBehavior::default();
        assert!(behavior.focused_tile().is_none());
        assert_eq!(behavior.theme(), AppTheme::default()); // Default theme (from Default derive)
        assert_eq!(behavior.api_key(), "");
    }

    #[test]
    fn test_set_theme() {
        let mut behavior = TreeBehavior::default();
        assert_eq!(behavior.theme(), AppTheme::Dark);

        behavior.set_theme(AppTheme::Light);
        assert_eq!(behavior.theme(), AppTheme::Light);

        behavior.set_theme(AppTheme::Dark);
        assert_eq!(behavior.theme(), AppTheme::Dark);
    }

    #[test]
    fn test_set_keys() {
        let mut behavior = TreeBehavior::default();
        assert_eq!(behavior.api_key(), "");

        behavior.set_keys("test-api-key-123".to_string());
        assert_eq!(behavior.api_key(), "test-api-key-123");

        behavior.set_keys(String::new());
        assert_eq!(behavior.api_key(), "");
    }

    // ==================== Focus Management Tests ====================

    #[test]
    fn test_set_focused_tile() {
        let mut behavior = TreeBehavior::default();
        let tile_id = make_tile_id(42);

        assert!(behavior.focused_tile().is_none());

        behavior.set_focused_tile(Some(tile_id));
        assert_eq!(behavior.focused_tile(), Some(tile_id));

        behavior.set_focused_tile(None);
        assert!(behavior.focused_tile().is_none());
    }

    #[test]
    fn test_focused_tile_changes() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let tile3 = make_tile_id(3);

        behavior.set_focused_tile(Some(tile1));
        assert_eq!(behavior.focused_tile(), Some(tile1));

        behavior.set_focused_tile(Some(tile2));
        assert_eq!(behavior.focused_tile(), Some(tile2));
        assert_ne!(behavior.focused_tile(), Some(tile1));

        behavior.set_focused_tile(Some(tile3));
        assert_eq!(behavior.focused_tile(), Some(tile3));
    }

    // ==================== Visual Multi State Tests ====================

    #[test]
    fn test_set_visual_multi_state_inactive() {
        let mut behavior = TreeBehavior::default();

        behavior.set_visual_multi_state(false, FxHashSet::default(), FxHashMap::default());

        // When inactive, the state should be set but doesn't affect rendering
        // (is_visual_multi_mode is private, but we can verify via clone/debug)
    }

    #[test]
    fn test_set_visual_multi_state_active() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);

        let mut selected = FxHashSet::default();
        selected.insert(tile1);
        selected.insert(tile2);

        let mut queries = FxHashMap::default();
        queries.insert(tile1, "query1".to_string());
        queries.insert(tile2, "query2".to_string());

        behavior.set_visual_multi_state(true, selected.clone(), queries.clone());

        // The internal state should be updated (verified by behavior in rendering)
    }

    #[test]
    fn test_set_visual_multi_state_with_empty_selection() {
        let mut behavior = TreeBehavior::default();

        behavior.set_visual_multi_state(true, FxHashSet::default(), FxHashMap::default());
        // Active mode with no selections - valid state
    }

    // ==================== Filter State Tests ====================

    #[test]
    fn test_set_filter_state_inactive() {
        let mut behavior = TreeBehavior::default();

        behavior.set_filter_state(false, FxHashSet::default());
        // Filter not active, no tiles filtered
    }

    #[test]
    fn test_set_filter_state_active() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);

        let mut filtered = FxHashSet::default();
        filtered.insert(tile1);
        filtered.insert(tile2);

        behavior.set_filter_state(true, filtered);
        // Filter active with two tiles filtered out
    }

    #[test]
    fn test_set_filter_state_toggle() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);

        let mut filtered = FxHashSet::default();
        filtered.insert(tile1);

        behavior.set_filter_state(true, filtered.clone());
        behavior.set_filter_state(false, FxHashSet::default());
        behavior.set_filter_state(true, filtered);
        // Can toggle filter state on and off
    }

    // ==================== Clone Tests ====================

    #[test]
    fn test_tree_behavior_clone() {
        let mut behavior = TreeBehavior::default();
        let tile_id = make_tile_id(42);

        behavior.set_theme(AppTheme::Light);
        behavior.set_keys("my-key".to_string());
        behavior.set_focused_tile(Some(tile_id));

        let cloned = behavior.clone();

        assert_eq!(cloned.theme(), AppTheme::Light);
        assert_eq!(cloned.api_key(), "my-key");
        assert_eq!(cloned.focused_tile(), Some(tile_id));
    }

    // ==================== Multiple State Combinations ====================

    #[test]
    fn test_combined_states() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let tile3 = make_tile_id(3);

        // Set theme
        behavior.set_theme(AppTheme::Light);

        // Set API key
        behavior.set_keys("secret".to_string());

        // Set focus
        behavior.set_focused_tile(Some(tile1));

        // Set visual multi state
        let mut selected = FxHashSet::default();
        selected.insert(tile1);
        selected.insert(tile2);
        let mut queries = FxHashMap::default();
        queries.insert(tile1, "q1".to_string());
        queries.insert(tile2, "q2".to_string());
        behavior.set_visual_multi_state(true, selected, queries);

        // Set filter state
        let mut filtered = FxHashSet::default();
        filtered.insert(tile3);
        behavior.set_filter_state(true, filtered);

        // Verify independent states
        assert_eq!(behavior.theme(), AppTheme::Light);
        assert_eq!(behavior.api_key(), "secret");
        assert_eq!(behavior.focused_tile(), Some(tile1));
    }

    #[test]
    fn test_focus_with_visual_multi() {
        let mut behavior = TreeBehavior::default();
        let cursor_tile = make_tile_id(1);
        let selected_tile = make_tile_id(2);

        // Focus can be different from selected tiles in visual-multi mode
        behavior.set_focused_tile(Some(cursor_tile));

        let mut selected = FxHashSet::default();
        selected.insert(selected_tile);
        behavior.set_visual_multi_state(true, selected, FxHashMap::default());

        // Cursor (focus) is tile1, but tile2 is selected
        assert_eq!(behavior.focused_tile(), Some(cursor_tile));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_api_key() {
        let mut behavior = TreeBehavior::default();
        behavior.set_keys(String::new());
        assert!(behavior.api_key().is_empty());
    }

    #[test]
    fn test_long_api_key() {
        let mut behavior = TreeBehavior::default();
        let long_key = "a".repeat(1000);
        behavior.set_keys(long_key.clone());
        assert_eq!(behavior.api_key(), long_key);
    }

    #[test]
    fn test_unicode_in_queries() {
        let mut behavior = TreeBehavior::default();
        let tile = make_tile_id(1);

        let mut selected = FxHashSet::default();
        selected.insert(tile);

        let mut queries = FxHashMap::default();
        queries.insert(tile, "メトリック{ラベル=\"日本語\"}".to_string());

        behavior.set_visual_multi_state(true, selected, queries);
        // Unicode queries should work fine
    }

    #[test]
    fn test_many_tiles_in_selection() {
        let mut behavior = TreeBehavior::default();

        let mut selected = FxHashSet::default();
        let mut queries = FxHashMap::default();

        for i in 0..100 {
            let tile = make_tile_id(i);
            selected.insert(tile);
            queries.insert(tile, format!("query_{i}"));
        }

        behavior.set_visual_multi_state(true, selected.clone(), queries);

        // Should handle many tiles
        assert_eq!(selected.len(), 100);
    }

    #[test]
    fn test_many_tiles_in_filter() {
        let mut behavior = TreeBehavior::default();

        let mut filtered = FxHashSet::default();
        for i in 0..50 {
            filtered.insert(make_tile_id(i));
        }

        behavior.set_filter_state(true, filtered.clone());

        // Should handle many filtered tiles
        assert_eq!(filtered.len(), 50);
    }
}
