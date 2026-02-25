//! UI rendering methods for the workspace.
//!
//! This module handles rendering the filtered view, custom scrollbar,
//! collapsible sections, and scroll-to-focused-tile functionality.

use egui::{Color32, RichText, Vec2};
use egui_tiles::Tile;

use super::{FocusTarget, SECTION_CONTENT_PADDING, SECTION_PANE_GAP, SectionConfig, Workspace};
use crate::components::{Buffer, QueryPane};
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use egui_tiles::TileId;

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
                    ui.label(RichText::new(after).color(normal_color).size(size).strong());
                }
            });
        }
        _ => {
            // No match, render normally
            ui.label(RichText::new(text).color(normal_color).size(size).strong());
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
                                                let match_range = self
                                                    .viewport_filter
                                                    .find_match_range(&display_name);
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

    /// Check if a tile is selected in visual-multi mode
    fn is_tile_selected_in_visual_multi(&self, tile_id: TileId) -> bool {
        self.visual_multi_state
            .as_ref()
            .is_some_and(|state| state.selected_tile_ids.contains(&tile_id))
    }

    /// Check if we're in visual-multi mode
    fn is_in_visual_multi_mode(&self) -> bool {
        self.visual_multi_state.is_some()
    }

    /// Draw visual-multi selection indicator for a pane
    fn draw_visual_multi_selection(&self, ui: &egui::Ui, rect: egui::Rect, theme: AppTheme) {
        // Selection tint using theme accent
        let selection_color = theme.accent_primary().gamma_multiply(0.15);

        // Fill the entire pane with a subtle selection tint
        ui.painter().rect_filled(rect, 4.0, selection_color);

        // Draw selection border
        let border_color = theme.accent_primary();
        let border_width = 2.0;
        let inset_rect = rect.shrink(border_width / 2.0);
        ui.painter().rect_stroke(
            inset_rect,
            4.0,
            egui::Stroke::new(border_width, border_color),
            egui::StrokeKind::Outside,
        );
    }

    /// Collect tile IDs that are selected in visual-multi mode within a section.
    /// Returns empty vec if not in visual-multi mode.
    fn get_selected_tiles(&self, tile_ids: &[TileId]) -> Vec<TileId> {
        if self.is_in_visual_multi_mode() {
            tile_ids
                .iter()
                .filter(|&&id| self.is_tile_selected_in_visual_multi(id))
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Draw focus and selection overlays for a pane.
    /// Handles both visual-multi selection indicator and focus border.
    fn draw_pane_overlays(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        is_focused: bool,
        is_selected: bool,
        is_visual_multi: bool,
        theme: AppTheme,
    ) {
        // Draw visual-multi selection indicator first (underneath focus border)
        if is_selected {
            self.draw_visual_multi_selection(ui, rect, theme);
        }

        // Draw focus border (brighter in visual-multi mode)
        if is_focused {
            let focus_color = if is_visual_multi {
                theme.accent_hover()
            } else {
                theme.accent_primary()
            };
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(2.0, focus_color),
                egui::StrokeKind::Inside,
            );
        }
    }

    /// Render sections with collapsible headers (Grafana-style)
    ///
    /// This is called when the workspace uses the sections format instead of
    /// the legacy flat panes format. Each section renders as:
    /// - A clickable header with collapse indicator (▼/▶), name, and pane count
    /// - Section content (panes) when expanded, laid out according to section layout
    #[profiling::function]
    pub(super) fn render_sections(&mut self, ui: &mut egui::Ui) {
        let theme = self.behavior.theme();

        // Update section renderer theme
        self.section_renderer.set_theme(theme);

        // Capture the full available width once so that section content
        // (e.g. horizontal pane layouts) cannot shrink subsequent headers.
        let full_width = ui.available_width();

        // Check if we need to scroll to the focused element
        let should_scroll = self.section_scroll_to_focus;
        let current_focus = self.section_focus;
        self.section_scroll_to_focus = false;

        // Get all pane tile IDs in order (they match section pane order)
        let pane_tile_ids = self.get_pane_tile_ids();

        // Clone section configs to avoid borrow conflicts
        let section_configs = self.section_configs.clone();

        // Track which pane index we're at as we iterate through sections
        let mut pane_offset = 0;

        // Render each section
        for (section_idx, section_config) in section_configs.iter().enumerate() {
            let section_pane_count = section_config.panes.len();

            // Get or create section state
            let section_state = self
                .section_states
                .get(section_idx)
                .cloned()
                .unwrap_or_default();

            // Check if this section header is focused
            let header_focused = self.section_focus == FocusTarget::SectionHeader(section_idx);

            // Render section header
            let header_response = self.section_renderer.render_header(
                ui,
                section_config,
                &section_state,
                header_focused,
                full_width,
            );

            // Scroll to focused header
            if should_scroll && current_focus == FocusTarget::SectionHeader(section_idx) {
                ui.scroll_to_rect(header_response.rect, Some(egui::Align::Center));
            }

            // Handle header click - toggle collapsed state
            if header_response.clicked() {
                if let Some(state) = self.section_states.get_mut(section_idx) {
                    state.collapsed = !state.collapsed;
                }
            }

            // Render section content if not collapsed
            if !section_state.collapsed {
                // Get the tile IDs for this section's panes
                let section_tile_ids: Vec<_> = pane_tile_ids
                    .iter()
                    .skip(pane_offset)
                    .take(section_pane_count)
                    .copied()
                    .collect();

                // Render content based on section layout
                self.render_section_content(ui, section_config, &section_tile_ids, section_idx);

                // Scroll to focused pane within this section
                if should_scroll {
                    if let FocusTarget::Pane { section, .. } = current_focus {
                        if section == section_idx {
                            // Scroll to the bottom of this section's content
                            let rect = egui::Rect::from_min_size(
                                ui.next_widget_position(),
                                egui::Vec2::ZERO,
                            );
                            ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        }
                    }
                }
            }

            // Move to next section's panes
            pane_offset += section_pane_count;

            // Add spacing between sections
            ui.add_space(8.0);
        }
    }

    /// Render section content (panes) based on the section's layout type
    fn render_section_content(
        &mut self,
        ui: &mut egui::Ui,
        section: &SectionConfig,
        tile_ids: &[egui_tiles::TileId],
        section_idx: usize,
    ) {
        use super::config::SectionLayout;

        if tile_ids.is_empty() {
            return;
        }

        let theme = self.behavior.theme();

        ui.add_space(SECTION_CONTENT_PADDING);

        match section.layout {
            SectionLayout::Horizontal => {
                self.render_section_horizontal(ui, tile_ids, section_idx, theme);
            }
            SectionLayout::Vertical => {
                self.render_section_vertical(ui, tile_ids, section_idx, theme);
            }
            SectionLayout::Grid => {
                let columns = section.columns.unwrap_or(2).max(1);
                self.render_section_grid(ui, tile_ids, section_idx, columns, theme);
            }
            SectionLayout::Tabs => {
                // Clone section to avoid borrow conflict with render method
                let section = section.clone();
                self.render_section_tabs(ui, tile_ids, &section, section_idx, theme);
            }
        }

        ui.add_space(SECTION_CONTENT_PADDING);
    }

    /// Render panes in horizontal layout (side by side)
    fn render_section_horizontal(
        &mut self,
        ui: &mut egui::Ui,
        tile_ids: &[egui_tiles::TileId],
        section_idx: usize,
        theme: AppTheme,
    ) {
        let available_width = ui.available_width() - SECTION_CONTENT_PADDING * 2.0;
        let pane_count = tile_ids.len();
        let total_gaps = (pane_count.saturating_sub(1)) as f32 * SECTION_PANE_GAP;
        let pane_width =
            ((available_width - total_gaps) / pane_count as f32).max(super::MIN_PANE_SIZE);
        let pane_height = super::SECTION_PANE_HEIGHT;

        let is_visual_multi = self.is_in_visual_multi_mode();
        let selected_tiles = self.get_selected_tiles(tile_ids);

        ui.horizontal(|ui| {
            // Disable implicit item spacing – we handle gaps explicitly.
            ui.spacing_mut().item_spacing.x = 0.0;

            for (pane_idx, &tile_id) in tile_ids.iter().enumerate() {
                let is_focused = self.section_focus
                    == FocusTarget::Pane {
                        section: section_idx,
                        pane: pane_idx,
                    };
                let is_selected = selected_tiles.contains(&tile_id);

                // Reserve exact space so all cells in the row have identical
                // dimensions, then render into a child UI at that rect.
                let (cell_rect, _) = ui.allocate_exact_size(
                    Vec2::new(pane_width, pane_height),
                    egui::Sense::hover(),
                );
                let mut child_ui =
                    ui.new_child(egui::UiBuilder::new().max_rect(cell_rect));
                child_ui.set_clip_rect(cell_rect);
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                    component.set_theme(theme);
                    component.show(&mut child_ui);
                }

                self.draw_pane_overlays(
                    ui,
                    cell_rect,
                    is_focused,
                    is_selected,
                    is_visual_multi,
                    theme,
                );

                if pane_idx < pane_count - 1 {
                    ui.add_space(SECTION_PANE_GAP);
                }
            }
        });
    }

    /// Render panes in vertical layout (stacked)
    fn render_section_vertical(
        &mut self,
        ui: &mut egui::Ui,
        tile_ids: &[egui_tiles::TileId],
        section_idx: usize,
        theme: AppTheme,
    ) {
        let pane_height = super::SECTION_PANE_HEIGHT;
        let is_visual_multi = self.is_in_visual_multi_mode();
        let selected_tiles = self.get_selected_tiles(tile_ids);

        ui.vertical(|ui| {
            let available_width = ui.available_width();
            for (pane_idx, &tile_id) in tile_ids.iter().enumerate() {
                let is_focused = self.section_focus
                    == FocusTarget::Pane {
                        section: section_idx,
                        pane: pane_idx,
                    };
                let is_selected = selected_tiles.contains(&tile_id);

                let response = ui.allocate_ui(Vec2::new(available_width, pane_height), |ui| {
                    let fixed_rect = ui.max_rect();
                    if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                        component.set_theme(theme);
                        component.show(ui);
                    }
                    fixed_rect
                });

                self.draw_pane_overlays(
                    ui,
                    response.inner,
                    is_focused,
                    is_selected,
                    is_visual_multi,
                    theme,
                );

                if pane_idx < tile_ids.len() - 1 {
                    ui.add_space(SECTION_PANE_GAP);
                }
            }
        });
    }

    /// Render panes in grid layout
    fn render_section_grid(
        &mut self,
        ui: &mut egui::Ui,
        tile_ids: &[egui_tiles::TileId],
        section_idx: usize,
        columns: usize,
        theme: AppTheme,
    ) {
        let available_width = ui.available_width() - SECTION_CONTENT_PADDING * 2.0;
        let total_gaps = (columns.saturating_sub(1)) as f32 * SECTION_PANE_GAP;
        let cell_width =
            ((available_width - total_gaps) / columns as f32).max(super::MIN_PANE_SIZE);
        let cell_height = super::SECTION_GRID_CELL_HEIGHT;

        let is_visual_multi = self.is_in_visual_multi_mode();
        let selected_tiles = self.get_selected_tiles(tile_ids);

        ui.vertical(|ui| {
            for row_start in (0..tile_ids.len()).step_by(columns) {
                ui.horizontal(|ui| {
                    // Disable implicit item spacing – we handle gaps explicitly.
                    ui.spacing_mut().item_spacing.x = 0.0;

                    for col in 0..columns {
                        let pane_idx = row_start + col;
                        if pane_idx >= tile_ids.len() {
                            break;
                        }

                        let tile_id = tile_ids[pane_idx];
                        let is_focused = self.section_focus
                            == FocusTarget::Pane {
                                section: section_idx,
                                pane: pane_idx,
                            };
                        let is_selected = selected_tiles.contains(&tile_id);

                        // Reserve exact space so all cells in the row have
                        // identical dimensions, then render into a child UI.
                        let (cell_rect, _) = ui.allocate_exact_size(
                            Vec2::new(cell_width, cell_height),
                            egui::Sense::hover(),
                        );
                        let mut child_ui =
                            ui.new_child(egui::UiBuilder::new().max_rect(cell_rect));
                        child_ui.set_clip_rect(cell_rect);
                        if let Some(Tile::Pane(component)) =
                            self.viewport_tree.tiles.get_mut(tile_id)
                        {
                            component.set_theme(theme);
                            component.show(&mut child_ui);
                        }

                        self.draw_pane_overlays(
                            ui,
                            cell_rect,
                            is_focused,
                            is_selected,
                            is_visual_multi,
                            theme,
                        );

                        if col < columns - 1 && pane_idx + 1 < tile_ids.len() {
                            ui.add_space(SECTION_PANE_GAP);
                        }
                    }
                });

                if row_start + columns < tile_ids.len() {
                    ui.add_space(SECTION_PANE_GAP);
                }
            }
        });
    }

    /// Render panes in tabbed layout
    fn render_section_tabs(
        &mut self,
        ui: &mut egui::Ui,
        tile_ids: &[egui_tiles::TileId],
        section: &SectionConfig,
        section_idx: usize,
        theme: AppTheme,
    ) {
        if tile_ids.is_empty() {
            return;
        }

        // Determine active tab from focus (or default to first)
        let active_tab = if let FocusTarget::Pane { section: s, pane } = self.section_focus {
            if s == section_idx {
                pane.min(tile_ids.len() - 1)
            } else {
                0
            }
        } else {
            0
        };

        let is_visual_multi = self.is_in_visual_multi_mode();

        // Tab bar
        ui.horizontal(|ui| {
            for (pane_idx, &tile_id) in tile_ids.iter().enumerate() {
                let is_active = pane_idx == active_tab;

                // Get pane name from config or generate default
                let pane_name = if pane_idx < section.panes.len()
                    && !section.panes[pane_idx].name.is_empty()
                {
                    section.panes[pane_idx].name.clone()
                } else if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        query_pane.name().to_string()
                    } else {
                        format!("Pane {}", pane_idx + 1)
                    }
                } else {
                    format!("Pane {}", pane_idx + 1)
                };

                // Check if this tab is selected in visual-multi mode
                let is_selected = self.is_tile_selected_in_visual_multi(tile_id);

                let button_fill = if is_selected {
                    theme.accent_primary().gamma_multiply(0.3)
                } else if is_active {
                    theme.bg_elevated()
                } else {
                    theme.bg_surface()
                };

                let button = egui::Button::new(&pane_name).fill(button_fill);

                if ui.add(button).clicked() {
                    self.section_focus = FocusTarget::Pane {
                        section: section_idx,
                        pane: pane_idx,
                    };
                }
            }
        });

        ui.add_space(SECTION_PANE_GAP);

        // Render active tab content
        if let Some(&tile_id) = tile_ids.get(active_tab) {
            let is_focused = self.section_focus
                == FocusTarget::Pane {
                    section: section_idx,
                    pane: active_tab,
                };
            let is_selected = self.is_tile_selected_in_visual_multi(tile_id);

            let pane_height = super::SECTION_PANE_HEIGHT;
            let available_width = ui.available_width();

            let response = ui.allocate_ui(Vec2::new(available_width, pane_height), |ui| {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                    component.set_theme(theme);
                    component.show(ui);
                }
                ui.min_rect()
            });

            self.draw_pane_overlays(
                ui,
                response.inner,
                is_focused,
                is_selected,
                is_visual_multi,
                theme,
            );
        }
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

    /// Render empty workspace hint when there are no panes (Neovim-style intro)
    #[profiling::function]
    pub(super) fn render_empty_workspace_hint(&self, ui: &mut egui::Ui) {
        let theme = self.behavior.theme();
        let text_col = text_color(theme);
        let muted_col = text_col.gamma_multiply(0.5);
        let subtle_col = text_col.gamma_multiply(0.35);
        let tilde_col = text_col.gamma_multiply(0.25); // Very subtle tilde color

        let line_height = typography::MD + 4.0;

        // Calculate how many tilde lines to show above content for vertical centering
        let available_height = ui.available_height();
        let top_padding_lines = ((available_height * 0.30) / line_height) as usize;

        // Render tilde lines for top padding
        for _ in 0..top_padding_lines {
            render_tilde_line(ui, tilde_col, line_height);
        }

        // Empty line before title
        render_tilde_line(ui, tilde_col, line_height);

        // Title: "Enya" in large monospace (with tilde)
        render_centered_title_with_tilde(ui, "Enya", text_col, tilde_col);

        // Tagline (with tilde)
        render_centered_tagline_with_tilde(ui, "A Builder's Best Friend", subtle_col, tilde_col);

        // Version (with tilde)
        let version = format!("version {}", env!("CARGO_PKG_VERSION"));
        render_centered_tagline_with_tilde(ui, &version, muted_col, tilde_col);

        // Empty line after version
        render_tilde_line(ui, tilde_col, line_height);
        render_tilde_line(ui, tilde_col, line_height);

        // Build hint lines - Neovim style: "type  :command    description"
        // Format: (key, description)
        #[cfg(not(target_arch = "wasm32"))]
        let hints: &[(&str, &str)] = &[
            ("Space+f", "fuzzy finder"),
            ("aa", "ask AI agent"),
            ("?", "help"),
            (":", "commands"),
        ];

        #[cfg(target_arch = "wasm32")]
        let hints: &[(&str, &str)] = &[
            ("Space+f", "fuzzy finder"),
            ("?", "help"),
            (":", "commands"),
        ];

        // Render hints as centered block with aligned columns (with tildes)
        render_centered_hints_block_with_tilde(ui, hints, text_col, muted_col, tilde_col);

        // Fill remaining space with tilde lines
        let remaining_height = ui.available_height();
        let remaining_lines = (remaining_height / line_height) as usize;
        for _ in 0..remaining_lines {
            render_tilde_line(ui, tilde_col, line_height);
        }
    }
}

/// Render a tilde line (Neovim-style empty line marker)
fn render_tilde_line(ui: &mut egui::Ui, color: Color32, line_height: f32) {
    let font = egui::FontId::monospace(typography::MD);
    let galley = {
        let painter = ui.painter();
        painter.layout_no_wrap("~".to_string(), font, color)
    };

    let available_width = ui.available_width();

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(available_width, line_height),
        egui::Sense::hover(),
    );

    let painter = ui.painter();
    // Center the tilde vertically within the line height
    let y_offset = (line_height - galley.size().y) / 2.0;
    painter.galley(
        egui::pos2(rect.left(), rect.top() + y_offset),
        galley,
        color,
    );
}

/// Render centered title text (large monospace) with tilde marker
fn render_centered_title_with_tilde(
    ui: &mut egui::Ui,
    text: &str,
    color: Color32,
    tilde_color: Color32,
) {
    let font = egui::FontId::monospace(typography::HEADING + 4.0);
    let tilde_font = egui::FontId::monospace(typography::MD);

    let (galley, tilde_galley) = {
        let painter = ui.painter();
        let g = painter.layout_no_wrap(text.to_string(), font, color);
        let t = painter.layout_no_wrap("~".to_string(), tilde_font, tilde_color);
        (g, t)
    };

    let available_width = ui.available_width();
    let start_x = ((available_width - galley.size().x) / 2.0).max(0.0);

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(available_width, galley.size().y),
        egui::Sense::hover(),
    );

    let painter = ui.painter();
    // Tilde on left, vertically centered
    let tilde_y = rect.top() + (galley.size().y - tilde_galley.size().y) / 2.0;
    painter.galley(egui::pos2(rect.left(), tilde_y), tilde_galley, tilde_color);
    // Centered title
    painter.galley(egui::pos2(rect.left() + start_x, rect.top()), galley, color);
}

/// Render centered tagline (smaller proportional) with tilde marker
fn render_centered_tagline_with_tilde(
    ui: &mut egui::Ui,
    text: &str,
    color: Color32,
    tilde_color: Color32,
) {
    let font = egui::FontId::proportional(typography::SM);
    let tilde_font = egui::FontId::monospace(typography::MD);

    let (galley, tilde_galley) = {
        let painter = ui.painter();
        let g = painter.layout_no_wrap(text.to_string(), font, color);
        let t = painter.layout_no_wrap("~".to_string(), tilde_font, tilde_color);
        (g, t)
    };

    let available_width = ui.available_width();
    let start_x = ((available_width - galley.size().x) / 2.0).max(0.0);
    let line_height = galley.size().y.max(tilde_galley.size().y);

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(available_width, line_height),
        egui::Sense::hover(),
    );

    let painter = ui.painter();
    // Tilde on left
    let tilde_y = rect.top() + (line_height - tilde_galley.size().y) / 2.0;
    painter.galley(egui::pos2(rect.left(), tilde_y), tilde_galley, tilde_color);
    // Centered tagline
    let text_y = rect.top() + (line_height - galley.size().y) / 2.0;
    painter.galley(egui::pos2(rect.left() + start_x, text_y), galley, color);
}

/// Render a centered block of hints with aligned columns (Neovim-style) with tilde markers
fn render_centered_hints_block_with_tilde(
    ui: &mut egui::Ui,
    hints: &[(&str, &str)],
    key_color: Color32,
    desc_color: Color32,
    tilde_color: Color32,
) {
    let font = egui::FontId::monospace(typography::MD);
    let col_spacing = 16.0;

    // Measure widths in a scoped block to release the painter borrow
    let (max_key_width, max_desc_width) = {
        let painter = ui.painter();
        let max_key = hints
            .iter()
            .map(|(key, _)| {
                painter
                    .layout_no_wrap(format!("type  {key}"), font.clone(), key_color)
                    .size()
                    .x
            })
            .fold(0.0_f32, |a, b| a.max(b));

        let max_desc = hints
            .iter()
            .map(|(_, desc)| {
                painter
                    .layout_no_wrap(desc.to_string(), font.clone(), desc_color)
                    .size()
                    .x
            })
            .fold(0.0_f32, |a, b| a.max(b));

        (max_key, max_desc)
    };

    let total_width = max_key_width + col_spacing + max_desc_width;
    let available_width = ui.available_width();
    let block_start_x = ((available_width - total_width) / 2.0).max(0.0);

    // Render each hint line
    for (key, desc) in hints {
        let type_key_text = format!("type  {key}");

        // Create galleys in scoped block
        let (key_galley, desc_galley, tilde_galley, row_height) = {
            let painter = ui.painter();
            let kg = painter.layout_no_wrap(type_key_text, font.clone(), key_color);
            let dg = painter.layout_no_wrap(desc.to_string(), font.clone(), desc_color);
            let tg = painter.layout_no_wrap("~".to_string(), font.clone(), tilde_color);
            let h = kg.size().y.max(dg.size().y);
            (kg, dg, tg, h)
        };

        // Allocate space
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, row_height + 4.0),
            egui::Sense::hover(),
        );

        // Draw tilde on left
        let painter = ui.painter();
        let tilde_y = rect.top() + (row_height - tilde_galley.size().y) / 2.0;
        painter.galley(egui::pos2(rect.left(), tilde_y), tilde_galley, tilde_color);

        // Draw key
        painter.galley(
            egui::pos2(rect.left() + block_start_x, rect.top()),
            key_galley,
            key_color,
        );

        // Draw description
        painter.galley(
            egui::pos2(
                rect.left() + block_start_x + max_key_width + col_spacing,
                rect.top(),
            ),
            desc_galley,
            desc_color,
        );
    }
}
