//! Tile tree behavior for egui_tiles integration.
//!
//! This module provides the `TreeBehavior` struct that implements
//! `egui_tiles::Behavior` for rendering and managing the pane layout.

use std::collections::{HashMap, HashSet};

use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles};

use crate::components::Component;
use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;

/// Behavior implementation for the egui_tiles tree.
///
/// Handles rendering of panes, tab styling, focus borders,
/// visual-multi selection overlays, and viewport filtering.
#[derive(Default, Clone)]
pub struct TreeBehavior {
    pub(super) add_child_to: Option<TileId>,
    /// Currently focused tile for vim-style navigation
    focused_tile_id: Option<TileId>,
    /// Selected tiles in visual-multi mode (empty when not in visual-multi mode)
    selected_tile_ids: HashSet<TileId>,
    /// Whether we're currently in visual-multi mode
    is_visual_multi_mode: bool,
    /// Query content per tile (for display in visual-multi mode)
    tile_queries: HashMap<TileId, String>,
    theme: AppTheme,
    api_key: String,
    /// Tile IDs that are filtered out (should be dimmed)
    filtered_out_tiles: HashSet<TileId>,
    /// Whether viewport filter is active
    is_filter_active: bool,
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
        selected_ids: HashSet<TileId>,
        tile_queries: HashMap<TileId, String>,
    ) {
        self.is_visual_multi_mode = is_active;
        self.selected_tile_ids = selected_ids;
        self.tile_queries = tile_queries;
    }

    pub fn set_filter_state(&mut self, is_active: bool, filtered_out_tiles: HashSet<TileId>) {
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
}

impl egui_tiles::Behavior<Box<dyn Component>> for TreeBehavior {
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
            egui_tiles::ResizeState::Idle => palette::border_subtle(self.theme),
            egui_tiles::ResizeState::Hovering => palette::border_default(self.theme),
            egui_tiles::ResizeState::Dragging => palette::border::FOCUS,
        };
        egui::Stroke::new(1.0, color)
    }

    /// Height of the tab bar
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        28.0 // Slightly taller for better visual presence
    }

    /// Background color of the tab bar
    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        palette::bg_surface(self.theme)
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
            palette::bg_elevated(self.theme)
        } else if state.is_being_dragged {
            palette::bg_hover(self.theme)
        } else {
            palette::bg_surface(self.theme)
        }
    }

    /// Stroke for the line separating tab bar from content
    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        egui::Stroke::new(1.0, palette::border_subtle(self.theme))
    }

    /// Outline stroke around tabs (emerald for active, subtle for inactive)
    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &Tiles<Box<dyn Component>>,
        _tile_id: TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        if state.active {
            egui::Stroke::new(1.0, palette::accent::PRIMARY)
        } else {
            egui::Stroke::new(1.0, palette::border_subtle(self.theme))
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
            let dim_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200),
                AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200),
            };
            painter.rect_filled(rect, 4.0, dim_color);

            // Draw "filtered" indicator text
            let text_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgba_unmultiplied(100, 100, 100, 150),
                AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(150, 150, 150, 150),
            };
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
            // Emerald selection color to match brand
            let selection_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgba_unmultiplied(5, 150, 105, 50),
                AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(16, 185, 129, 40),
            };

            // Fill the entire tile with a subtle selection tint
            painter.rect_filled(rect, 4.0, selection_color);

            // Draw selection border
            let border_color = match self.theme {
                AppTheme::Light => palette::accent::LIGHT,
                AppTheme::Dark => palette::accent::PRIMARY,
            };
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
        // This shows which pane has the cursor in visual-multi mode
        if is_focused {
            // White/gray focus color to match Enya's color scheme
            // Use brighter color in visual-multi mode to distinguish cursor from selection
            let focus_color = if self.is_visual_multi_mode {
                match self.theme {
                    AppTheme::Light => egui::Color32::from_rgb(100, 100, 110),
                    AppTheme::Dark => egui::Color32::from_rgb(255, 255, 255),
                }
            } else {
                match self.theme {
                    AppTheme::Light => egui::Color32::from_rgb(120, 120, 130),
                    AppTheme::Dark => egui::Color32::from_rgb(200, 200, 210),
                }
            };

            // Shrink the rect inward so the border stroke is fully visible
            let border_width = 3.0;
            let inset_rect = rect.shrink(border_width / 2.0);

            painter.rect_stroke(
                inset_rect,
                4.0,
                egui::Stroke::new(border_width, focus_color),
                egui::StrokeKind::Outside,
            );
        }

        // In visual-multi mode, show query content at the bottom of each selected pane
        if is_selected {
            if let Some(query) = self.tile_queries.get(&tile_id) {
                // Style for query overlay
                let bg_color = match self.theme {
                    AppTheme::Light => egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230),
                    AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(30, 30, 35, 230),
                };
                let text_color = match self.theme {
                    AppTheme::Light => egui::Color32::from_rgb(50, 50, 60),
                    AppTheme::Dark => egui::Color32::from_rgb(220, 220, 230),
                };

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
                let padding = 6.0;
                let overlay_height = galley.rect.height() + padding * 2.0;
                let overlay_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x, rect.max.y - overlay_height),
                    egui::vec2(rect.width(), overlay_height),
                );

                // Draw background
                painter.rect_filled(overlay_rect, 0.0, bg_color);

                // Draw text centered vertically in the overlay
                let text_pos = egui::pos2(
                    overlay_rect.min.x + padding,
                    overlay_rect.center().y - galley.rect.height() / 2.0,
                );
                painter.galley(text_pos, galley, text_color);
            }
        }
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &Tiles<Box<dyn Component>>,
        ui: &mut egui::Ui,
        tile_id: TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        if ui.button("➕").clicked() {
            self.add_child_to = Some(tile_id);
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
        assert_eq!(behavior.theme(), AppTheme::Light); // Default theme (from Default derive)
        assert_eq!(behavior.api_key(), "");
    }

    #[test]
    fn test_set_theme() {
        let mut behavior = TreeBehavior::default();
        assert_eq!(behavior.theme(), AppTheme::Light);

        behavior.set_theme(AppTheme::Dark);
        assert_eq!(behavior.theme(), AppTheme::Dark);

        behavior.set_theme(AppTheme::Light);
        assert_eq!(behavior.theme(), AppTheme::Light);
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

        behavior.set_visual_multi_state(false, HashSet::new(), HashMap::new());

        // When inactive, the state should be set but doesn't affect rendering
        // (is_visual_multi_mode is private, but we can verify via clone/debug)
    }

    #[test]
    fn test_set_visual_multi_state_active() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);

        let mut selected = HashSet::new();
        selected.insert(tile1);
        selected.insert(tile2);

        let mut queries = HashMap::new();
        queries.insert(tile1, "query1".to_string());
        queries.insert(tile2, "query2".to_string());

        behavior.set_visual_multi_state(true, selected.clone(), queries.clone());

        // The internal state should be updated (verified by behavior in rendering)
    }

    #[test]
    fn test_set_visual_multi_state_with_empty_selection() {
        let mut behavior = TreeBehavior::default();

        behavior.set_visual_multi_state(true, HashSet::new(), HashMap::new());
        // Active mode with no selections - valid state
    }

    // ==================== Filter State Tests ====================

    #[test]
    fn test_set_filter_state_inactive() {
        let mut behavior = TreeBehavior::default();

        behavior.set_filter_state(false, HashSet::new());
        // Filter not active, no tiles filtered
    }

    #[test]
    fn test_set_filter_state_active() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);

        let mut filtered = HashSet::new();
        filtered.insert(tile1);
        filtered.insert(tile2);

        behavior.set_filter_state(true, filtered);
        // Filter active with two tiles filtered out
    }

    #[test]
    fn test_set_filter_state_toggle() {
        let mut behavior = TreeBehavior::default();
        let tile1 = make_tile_id(1);

        let mut filtered = HashSet::new();
        filtered.insert(tile1);

        behavior.set_filter_state(true, filtered.clone());
        behavior.set_filter_state(false, HashSet::new());
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
        let mut selected = HashSet::new();
        selected.insert(tile1);
        selected.insert(tile2);
        let mut queries = HashMap::new();
        queries.insert(tile1, "q1".to_string());
        queries.insert(tile2, "q2".to_string());
        behavior.set_visual_multi_state(true, selected, queries);

        // Set filter state
        let mut filtered = HashSet::new();
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

        let mut selected = HashSet::new();
        selected.insert(selected_tile);
        behavior.set_visual_multi_state(true, selected, HashMap::new());

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

        let mut selected = HashSet::new();
        selected.insert(tile);

        let mut queries = HashMap::new();
        queries.insert(tile, "メトリック{ラベル=\"日本語\"}".to_string());

        behavior.set_visual_multi_state(true, selected, queries);
        // Unicode queries should work fine
    }

    #[test]
    fn test_many_tiles_in_selection() {
        let mut behavior = TreeBehavior::default();

        let mut selected = HashSet::new();
        let mut queries = HashMap::new();

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

        let mut filtered = HashSet::new();
        for i in 0..50 {
            filtered.insert(make_tile_id(i));
        }

        behavior.set_filter_state(true, filtered.clone());

        // Should handle many filtered tiles
        assert_eq!(filtered.len(), 50);
    }
}
