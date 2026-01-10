//! UI rendering methods for the workspace.
//!
//! This module handles rendering the filtered view, custom scrollbar,
//! and scroll-to-focused-tile functionality.

use egui::RichText;
use egui_tiles::Tile;

use super::Workspace;
use crate::components::{Buffer, QueryPane};
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

impl Workspace {
    /// Render only matching panes when viewport filter is active
    #[profiling::function]
    pub(super) fn render_filtered_view(&mut self, ui: &mut egui::Ui) {
        let theme = self.behavior.theme();

        // Get matching pane IDs with their names - matches on query content AND tag
        let matching_panes: Vec<(egui_tiles::TileId, String)> = self
            .get_pane_tile_ids()
            .into_iter()
            .filter_map(|tile_id| {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        // Match on query content OR tag OR name
                        if self.viewport_filter.matches(query_pane.saved_query())
                            || self.viewport_filter.matches(query_pane.tag())
                            || self.viewport_filter.matches(query_pane.name())
                        {
                            let name = if !query_pane.tag().is_empty() {
                                query_pane.tag().to_string()
                            } else {
                                query_pane.name().to_string()
                            };
                            return Some((tile_id, name));
                        }
                        return None;
                    }
                    if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                        if self.viewport_filter.matches(buffer.saved_content())
                            || self.viewport_filter.matches(buffer.name())
                        {
                            return Some((tile_id, buffer.name().to_string()));
                        }
                        return None;
                    }
                }
                Some((tile_id, String::new()))
            })
            .collect();

        if matching_panes.is_empty() {
            // Show "no matches" message
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("No panes match the filter")
                        .color(text_color(theme).gamma_multiply(0.5))
                        .size(16.0),
                );
            });
            return;
        }

        // Calculate layout - prefer vertical stacking (1 column)
        let available = ui.available_size();
        let pane_count = matching_panes.len();

        // Use single column for vertical stacking, only use 2 columns if many panes and wide screen
        let columns = if pane_count == 1 || available.x < 800.0 {
            1
        } else if pane_count >= 4 && available.x >= 1200.0 {
            2
        } else {
            1
        };

        let header_height = 28.0;
        let pane_spacing = 12.0;
        let rows = pane_count.div_ceil(columns);

        let pane_width = (available.x - (columns as f32 - 1.0) * pane_spacing) / columns as f32;
        // Calculate pane height - account for headers
        let total_header_height = rows as f32 * header_height;
        let total_spacing = (rows as f32 - 1.0) * pane_spacing;
        let pane_height =
            ((available.y - total_header_height - total_spacing) / rows as f32).max(180.0);

        let text_col = text_color(theme);
        let accent = theme.accent_primary();

        egui::ScrollArea::vertical()
            .id_salt("filtered_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("filtered_panes_grid")
                    .num_columns(columns)
                    .spacing([pane_spacing, pane_spacing])
                    .show(ui, |ui| {
                        for (idx, (tile_id, pane_name)) in matching_panes.iter().enumerate() {
                            if let Some(Tile::Pane(component)) =
                                self.viewport_tree.tiles.get_mut(*tile_id)
                            {
                                component.set_theme(theme);
                                component.set_api_key(self.behavior.api_key());

                                // Render pane with header showing the name
                                ui.allocate_ui(
                                    egui::vec2(pane_width, pane_height + header_height),
                                    |ui| {
                                        ui.vertical(|ui| {
                                            // Pane header with name
                                            ui.horizontal(|ui| {
                                                ui.add_space(4.0);
                                                ui.label(
                                                    RichText::new(semantic_icons::action::CHART)
                                                        .color(accent)
                                                        .size(typography::MD),
                                                );
                                                ui.add_space(4.0);
                                                let display_name = if pane_name.is_empty() {
                                                    format!("Pane {}", idx + 1)
                                                } else {
                                                    pane_name.clone()
                                                };
                                                ui.label(
                                                    RichText::new(display_name)
                                                        .color(text_col)
                                                        .size(typography::MD)
                                                        .strong(),
                                                );
                                            });
                                            ui.add_space(4.0);

                                            // Pane content
                                            ui.allocate_ui(
                                                egui::vec2(pane_width, pane_height),
                                                |ui| {
                                                    component.show(ui);
                                                },
                                            );
                                        });
                                    },
                                );
                            }

                            // End row after 'columns' panes
                            if (idx + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    /// Draw nvim-style scrollbar indicator in the scrollbar gutter
    pub(super) fn draw_scrollbar(
        &self,
        painter: &egui::Painter,
        gutter_rect: egui::Rect,
        theme: AppTheme,
    ) {
        // Only draw if content is taller than visible area
        if self.viewport_content_height <= self.viewport_visible_height {
            return;
        }

        // Scrollbar dimensions - slim and elegant
        let scrollbar_width = 4.0;
        let margin_vertical = 8.0;
        let scrollbar_x = gutter_rect.center().x - scrollbar_width / 2.0;

        // Calculate scrollbar track area
        let track_top = gutter_rect.top() + margin_vertical;
        let track_bottom = gutter_rect.bottom() - margin_vertical;
        let track_height = track_bottom - track_top;

        // Calculate thumb position and size
        let visible_ratio = self.viewport_visible_height / self.viewport_content_height;
        let thumb_height = (track_height * visible_ratio).max(24.0); // Minimum thumb size

        let max_scroll = self.viewport_content_height - self.viewport_visible_height;
        let scroll_ratio = if max_scroll > 0.0 {
            (self.viewport_scroll_offset / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_top = track_top + (track_height - thumb_height) * scroll_ratio;

        let track_rect = egui::Rect::from_min_size(
            egui::pos2(scrollbar_x, track_top),
            egui::vec2(scrollbar_width, track_height),
        );

        let thumb_rect = egui::Rect::from_min_size(
            egui::pos2(scrollbar_x, thumb_top),
            egui::vec2(scrollbar_width, thumb_height),
        );

        // Theme-aware colors
        let track_color = theme.scrollbar_track();
        let thumb_color = theme.scrollbar_thumb();
        let thumb_highlight = theme.scrollbar_thumb_highlight();

        // Draw track with rounded ends
        painter.rect_filled(track_rect, scrollbar_width / 2.0, track_color);

        // Draw thumb with subtle gradient effect (using layered rectangles)
        // Base thumb
        painter.rect_filled(thumb_rect, scrollbar_width / 2.0, thumb_color);

        // Inner highlight (slightly smaller, brighter) for depth
        let highlight_inset = 0.5;
        let highlight_rect = thumb_rect.shrink2(egui::vec2(highlight_inset, 1.0));
        painter.rect_filled(
            highlight_rect,
            (scrollbar_width - highlight_inset * 2.0) / 2.0,
            thumb_highlight,
        );

        // Top cap highlight for a glossy effect
        let cap_height = 3.0_f32.min(thumb_height / 4.0);
        let cap_rect =
            egui::Rect::from_min_size(thumb_rect.min, egui::vec2(scrollbar_width, cap_height));
        let cap_color = theme.scrollbar_cap();
        painter.rect_filled(cap_rect, scrollbar_width / 2.0, cap_color);
    }

    /// Scroll viewport to make the focused tile visible
    pub(super) fn scroll_to_focused_tile(&mut self, ctx: &egui::Context) {
        let focused_id = match self.behavior.focused_tile() {
            Some(id) => id,
            None => return,
        };

        // Get all pane IDs in order
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.is_empty() {
            return;
        }

        // Find the index of the focused pane
        let focused_index = match pane_ids.iter().position(|&id| id == focused_id) {
            Some(idx) => idx,
            None => return,
        };

        // Calculate approximate position of the focused tile
        // Assume each pane takes equal height for simplicity
        let pane_count = pane_ids.len();
        if pane_count == 0 {
            return;
        }

        let pane_height = self.viewport_content_height / pane_count as f32;
        let target_top = focused_index as f32 * pane_height;
        let target_bottom = target_top + pane_height;

        // Calculate scroll target to bring tile into view (with some padding)
        let padding = 20.0;
        let view_top = self.viewport_scroll_offset;
        let view_bottom = view_top + self.viewport_visible_height;

        if target_top < view_top + padding {
            // Tile is above the visible area, scroll up
            self.viewport_scroll_target = (target_top - padding).max(0.0);
            ctx.request_repaint();
        } else if target_bottom > view_bottom - padding {
            // Tile is below the visible area, scroll down
            let max_scroll = (self.viewport_content_height - self.viewport_visible_height).max(0.0);
            self.viewport_scroll_target =
                (target_bottom - self.viewport_visible_height + padding).clamp(0.0, max_scroll);
            ctx.request_repaint();
        }
    }
}
