//! UI rendering methods for the workspace.
//!
//! This module handles rendering the filtered view, custom scrollbar,
//! and scroll-to-focused-tile functionality.

use egui::{Color32, RichText};
use egui_tiles::Tile;

use super::Workspace;
use crate::components::util::query_executor::Backend;
use crate::components::{Buffer, QueryPane};
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Render a centered hint with key and description (e.g. "Space+f to explore metrics")
fn render_centered_hint(
    ui: &mut egui::Ui,
    key_text: &str,
    description: &str,
    key_color: Color32,
    desc_color: Color32,
) {
    let font_key = egui::FontId::monospace(typography::MD);
    let font_desc = egui::FontId::proportional(typography::MD);
    let spacing = 4.0;

    // Layout text to measure widths (use temporary painter reference)
    let (key_galley, desc_galley) = {
        let painter = ui.painter();
        let key = painter.layout_no_wrap(key_text.to_string(), font_key, key_color);
        let desc = painter.layout_no_wrap(description.to_string(), font_desc, desc_color);
        (key, desc)
    };

    let total_width = key_galley.size().x + spacing + desc_galley.size().x;
    let row_height = key_galley.size().y.max(desc_galley.size().y);
    let available_width = ui.available_width();
    let start_x = ((available_width - total_width) / 2.0).max(0.0);

    // Allocate space for the row
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(available_width, row_height), egui::Sense::hover());

    // Draw key text
    let painter = ui.painter();
    painter.galley(
        egui::pos2(rect.left() + start_x, rect.top()),
        key_galley.clone(),
        key_color,
    );

    // Draw description
    painter.galley(
        egui::pos2(
            rect.left() + start_x + key_galley.size().x + spacing,
            rect.top(),
        ),
        desc_galley,
        desc_color,
    );
}

/// Render centered text (single style)
fn render_centered_text(ui: &mut egui::Ui, text: &str, color: Color32, font: egui::FontId) {
    let galley = {
        let painter = ui.painter();
        painter.layout_no_wrap(text.to_string(), font, color)
    };

    let available_width = ui.available_width();
    let start_x = ((available_width - galley.size().x) / 2.0).max(0.0);

    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(available_width, galley.size().y), egui::Sense::hover());

    let painter = ui.painter();
    painter.galley(egui::pos2(rect.left() + start_x, rect.top()), galley, color);
}

/// Render text with a highlighted portion (for filter matches)
fn render_highlighted_text(
    ui: &mut egui::Ui,
    text: &str,
    match_range: Option<(usize, usize)>,
    normal_color: Color32,
    highlight_color: Color32,
    size: f32,
) {
    match match_range {
        Some((start, end)) if start < text.len() && end <= text.len() => {
            // Split text into before, match, and after
            let before = &text[..start];
            let matched = &text[start..end];
            let after = &text[end..];

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if !before.is_empty() {
                    ui.label(
                        RichText::new(before)
                            .color(normal_color)
                            .size(size)
                            .strong(),
                    );
                }
                ui.label(
                    RichText::new(matched)
                        .color(highlight_color)
                        .size(size)
                        .strong()
                        .underline(),
                );
                if !after.is_empty() {
                    ui.label(
                        RichText::new(after)
                            .color(normal_color)
                            .size(size)
                            .strong(),
                    );
                }
            });
        }
        _ => {
            // No match, render normally
            ui.label(
                RichText::new(text)
                    .color(normal_color)
                    .size(size)
                    .strong(),
            );
        }
    }
}

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
                                            // Pane header with name (highlighted if matches filter)
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
                                                // Find match range for highlighting
                                                let match_range =
                                                    self.viewport_filter.find_match_range(&display_name);
                                                render_highlighted_text(
                                                    ui,
                                                    &display_name,
                                                    match_range,
                                                    text_col,
                                                    accent,
                                                    typography::MD,
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

    /// Render empty workspace hint when there are no panes
    #[profiling::function]
    pub(super) fn render_empty_workspace_hint(&self, ui: &mut egui::Ui) {
        let theme = self.behavior.theme();
        let text_col = text_color(theme);
        let muted_col = text_col.gamma_multiply(0.5);
        let subtle_col = text_col.gamma_multiply(0.35);
        let accent = theme.accent_primary();

        // Calculate vertical centering
        let available_height = ui.available_height();
        ui.add_space(available_height * 0.35);

        // Connection status (only show when connected)
        if self.query_executor.is_connected() {
            let endpoint = match self.query_executor.backend() {
                Backend::Prometheus(endpoint) => endpoint.clone(),
                Backend::Demo => "Demo Mode".to_string(),
            };

            // Truncate long endpoints for display
            let display_endpoint = if endpoint.len() > 40 {
                format!("{}...", &endpoint[..37])
            } else {
                endpoint
            };

            render_centered_text(
                ui,
                &format!("{} {}", semantic_icons::status::CONNECTED, display_endpoint),
                accent,
                egui::FontId::proportional(typography::SM),
            );

            ui.add_space(4.0);

            // Metric count
            let metric_count = self.query_executor.metric_names().len();
            let metric_text = if metric_count == 1 {
                "1 metric available".to_string()
            } else {
                format!("{} metrics available", metric_count)
            };
            render_centered_text(
                ui,
                &metric_text,
                subtle_col,
                egui::FontId::proportional(typography::SM),
            );

            ui.add_space(20.0);
        }

        // Main hint: Space+f to explore metrics
        render_centered_hint(ui, "Space+f", "to explore metrics", text_col, muted_col);

        // Native-only: Agent mode hint
        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.add_space(8.0);
            render_centered_hint(
                ui,
                "aa",
                "to enter Agent mode and create plots",
                text_col,
                muted_col,
            );
        }

        ui.add_space(8.0);

        // Help hint (same styling as other hints)
        render_centered_hint(ui, "?", "for help", text_col, muted_col);
    }
}
