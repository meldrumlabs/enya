//! Section renderer for collapsible section headers and content.
//!
//! This module provides the `SectionRenderer` struct that handles rendering
//! of Grafana-style collapsible sections with headers and pane content.

use egui::{Response, Ui, Vec2};

use crate::ui::theme::AppTheme;

use super::config::{SectionConfig, SectionLayout};
use super::input::{FocusTarget, SectionState};

/// Height of section headers
pub const SECTION_HEADER_HEIGHT: f32 = 32.0;

/// Padding around section content
pub const SECTION_CONTENT_PADDING: f32 = 8.0;

/// Gap between panes in a section
pub const SECTION_PANE_GAP: f32 = 8.0;

/// Minimum pane height/width
pub const MIN_PANE_SIZE: f32 = 100.0;

/// Default height for panes in horizontal/vertical/tabs layouts (enough for legend + chart)
pub const SECTION_PANE_HEIGHT: f32 = 280.0;

/// Height for grid cells (slightly shorter for compact view)
pub const SECTION_GRID_CELL_HEIGHT: f32 = 220.0;

/// Renders section headers and content for collapsible sections.
///
/// The renderer uses the current theme to style headers with:
/// - Collapse indicator (▼ expanded, ▶ collapsed)
/// - Section name
/// - Pane count badge
/// - Focus border when header is focused
#[derive(Clone, Default)]
pub struct SectionRenderer {
    theme: AppTheme,
}

impl SectionRenderer {
    /// Create a new section renderer with the given theme
    pub fn new(theme: AppTheme) -> Self {
        Self { theme }
    }

    /// Update the theme used for rendering
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Get the current theme
    pub fn theme(&self) -> AppTheme {
        self.theme
    }

    /// Render a section header.
    ///
    /// Returns the Response for click/hover detection.
    pub fn render_header(
        &self,
        ui: &mut Ui,
        section: &SectionConfig,
        state: &SectionState,
        focused: bool,
        width: f32,
    ) -> Response {
        let desired_size = Vec2::new(width, SECTION_HEADER_HEIGHT);

        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Background
            let bg_color = if response.hovered() {
                self.theme.bg_hover()
            } else {
                self.theme.bg_surface()
            };
            painter.rect_filled(rect, 4.0, bg_color);

            // Focus border
            if focused {
                painter.rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(2.0, self.theme.accent_primary()),
                    egui::StrokeKind::Inside,
                );
            } else {
                painter.rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(1.0, self.theme.border_subtle()),
                    egui::StrokeKind::Inside,
                );
            }

            // Collapse indicator
            let indicator = if state.collapsed { "▶" } else { "▼" };
            let indicator_pos = rect.left_center() + Vec2::new(12.0, 0.0);
            painter.text(
                indicator_pos,
                egui::Align2::LEFT_CENTER,
                indicator,
                egui::FontId::proportional(14.0),
                self.theme.text_secondary(),
            );

            // Section name
            let name_pos = rect.left_center() + Vec2::new(32.0, 0.0);
            painter.text(
                name_pos,
                egui::Align2::LEFT_CENTER,
                &section.name,
                egui::FontId::proportional(14.0),
                self.theme.text_primary(),
            );

            // Pane count badge
            let pane_count = section.panes.len();
            let badge_text = format!(
                "[{} pane{}]",
                pane_count,
                if pane_count == 1 { "" } else { "s" }
            );
            let badge_pos = rect.right_center() - Vec2::new(12.0, 0.0);
            painter.text(
                badge_pos,
                egui::Align2::RIGHT_CENTER,
                badge_text,
                egui::FontId::proportional(12.0),
                self.theme.text_tertiary(),
            );
        }

        response
    }

    /// Render section content (panes) based on the section's layout.
    ///
    /// This is called when the section is expanded.
    pub fn render_content<F>(
        &self,
        ui: &mut Ui,
        section: &SectionConfig,
        focus: FocusTarget,
        section_idx: usize,
        mut render_pane: F,
    ) where
        F: FnMut(&mut Ui, usize, bool),
    {
        if section.panes.is_empty() {
            return;
        }

        ui.add_space(SECTION_CONTENT_PADDING);

        match section.layout {
            SectionLayout::Horizontal => {
                self.render_horizontal(ui, section, focus, section_idx, &mut render_pane);
            }
            SectionLayout::Vertical => {
                self.render_vertical(ui, section, focus, section_idx, &mut render_pane);
            }
            SectionLayout::Grid => {
                self.render_grid(ui, section, focus, section_idx, &mut render_pane);
            }
            SectionLayout::Tabs => {
                self.render_tabs(ui, section, focus, section_idx, &mut render_pane);
            }
        }

        ui.add_space(SECTION_CONTENT_PADDING);
    }

    /// Render panes in horizontal layout (side by side)
    fn render_horizontal<F>(
        &self,
        ui: &mut Ui,
        section: &SectionConfig,
        focus: FocusTarget,
        section_idx: usize,
        render_pane: &mut F,
    ) where
        F: FnMut(&mut Ui, usize, bool),
    {
        let available_width = ui.available_width() - SECTION_CONTENT_PADDING * 2.0;
        let pane_count = section.panes.len();
        let total_gaps = (pane_count.saturating_sub(1)) as f32 * SECTION_PANE_GAP;
        let pane_width = ((available_width - total_gaps) / pane_count as f32).max(MIN_PANE_SIZE);

        ui.horizontal(|ui| {
            for pane_idx in 0..pane_count {
                let is_focused = focus
                    == FocusTarget::Pane {
                        section: section_idx,
                        pane: pane_idx,
                    };

                ui.allocate_ui(Vec2::new(pane_width, ui.available_height()), |ui| {
                    if is_focused {
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            egui::Stroke::new(2.0, self.theme.accent_primary()),
                            egui::StrokeKind::Inside,
                        );
                    }
                    render_pane(ui, pane_idx, is_focused);
                });

                if pane_idx < pane_count - 1 {
                    ui.add_space(SECTION_PANE_GAP);
                }
            }
        });
    }

    /// Render panes in vertical layout (stacked)
    fn render_vertical<F>(
        &self,
        ui: &mut Ui,
        section: &SectionConfig,
        focus: FocusTarget,
        section_idx: usize,
        render_pane: &mut F,
    ) where
        F: FnMut(&mut Ui, usize, bool),
    {
        let pane_count = section.panes.len();

        ui.vertical(|ui| {
            for pane_idx in 0..pane_count {
                let is_focused = focus
                    == FocusTarget::Pane {
                        section: section_idx,
                        pane: pane_idx,
                    };

                ui.allocate_ui(Vec2::new(ui.available_width(), MIN_PANE_SIZE * 2.0), |ui| {
                    if is_focused {
                        let rect = ui.available_rect_before_wrap();
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            egui::Stroke::new(2.0, self.theme.accent_primary()),
                            egui::StrokeKind::Inside,
                        );
                    }
                    render_pane(ui, pane_idx, is_focused);
                });

                if pane_idx < pane_count - 1 {
                    ui.add_space(SECTION_PANE_GAP);
                }
            }
        });
    }

    /// Render panes in grid layout
    fn render_grid<F>(
        &self,
        ui: &mut Ui,
        section: &SectionConfig,
        focus: FocusTarget,
        section_idx: usize,
        render_pane: &mut F,
    ) where
        F: FnMut(&mut Ui, usize, bool),
    {
        let columns = section.columns.unwrap_or(2).max(1);
        let pane_count = section.panes.len();
        let available_width = ui.available_width() - SECTION_CONTENT_PADDING * 2.0;
        let total_gaps = (columns.saturating_sub(1)) as f32 * SECTION_PANE_GAP;
        let cell_width = ((available_width - total_gaps) / columns as f32).max(MIN_PANE_SIZE);

        ui.vertical(|ui| {
            for row_start in (0..pane_count).step_by(columns) {
                ui.horizontal(|ui| {
                    for col in 0..columns {
                        let pane_idx = row_start + col;
                        if pane_idx >= pane_count {
                            break;
                        }

                        let is_focused = focus
                            == FocusTarget::Pane {
                                section: section_idx,
                                pane: pane_idx,
                            };

                        ui.allocate_ui(Vec2::new(cell_width, MIN_PANE_SIZE * 2.0), |ui| {
                            if is_focused {
                                let rect = ui.available_rect_before_wrap();
                                ui.painter().rect_stroke(
                                    rect,
                                    4.0,
                                    egui::Stroke::new(2.0, self.theme.accent_primary()),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            render_pane(ui, pane_idx, is_focused);
                        });

                        if col < columns - 1 && pane_idx + 1 < pane_count {
                            ui.add_space(SECTION_PANE_GAP);
                        }
                    }
                });

                if row_start + columns < pane_count {
                    ui.add_space(SECTION_PANE_GAP);
                }
            }
        });
    }

    /// Render panes in tabbed layout
    fn render_tabs<F>(
        &self,
        ui: &mut Ui,
        section: &SectionConfig,
        focus: FocusTarget,
        section_idx: usize,
        render_pane: &mut F,
    ) where
        F: FnMut(&mut Ui, usize, bool),
    {
        let pane_count = section.panes.len();
        if pane_count == 0 {
            return;
        }

        // Determine active tab from focus
        let active_tab = if let FocusTarget::Pane { section: s, pane } = focus {
            if s == section_idx {
                pane.min(pane_count - 1)
            } else {
                0
            }
        } else {
            0
        };

        // Tab bar
        ui.horizontal(|ui| {
            for pane_idx in 0..pane_count {
                let is_active = pane_idx == active_tab;
                let pane_name = if section.panes[pane_idx].name.is_empty() {
                    format!("Pane {}", pane_idx + 1)
                } else {
                    section.panes[pane_idx].name.clone()
                };

                let button = egui::Button::new(&pane_name).fill(if is_active {
                    self.theme.bg_elevated()
                } else {
                    self.theme.bg_surface()
                });

                if ui.add(button).clicked() {
                    // Tab click handling would be done by the parent
                }
            }
        });

        ui.add_space(SECTION_PANE_GAP);

        // Render active tab content
        let is_focused = focus
            == FocusTarget::Pane {
                section: section_idx,
                pane: active_tab,
            };

        if is_focused {
            let rect = ui.available_rect_before_wrap();
            ui.painter().rect_stroke(
                rect.shrink(2.0),
                4.0,
                egui::Stroke::new(2.0, self.theme.accent_primary()),
                egui::StrokeKind::Inside,
            );
        }

        render_pane(ui, active_tab, is_focused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_renderer_default() {
        let renderer = SectionRenderer::default();
        assert_eq!(renderer.theme(), AppTheme::default());
    }

    #[test]
    fn test_section_renderer_new() {
        let renderer = SectionRenderer::new(AppTheme::Light);
        assert_eq!(renderer.theme(), AppTheme::Light);
    }

    #[test]
    fn test_section_renderer_set_theme() {
        let mut renderer = SectionRenderer::default();
        renderer.set_theme(AppTheme::Midnight);
        assert_eq!(renderer.theme(), AppTheme::Midnight);
    }

    #[test]
    fn test_constants() {
        assert_eq!(SECTION_HEADER_HEIGHT, 32.0);
        assert_eq!(SECTION_CONTENT_PADDING, 8.0);
        assert_eq!(SECTION_PANE_GAP, 8.0);
        assert_eq!(MIN_PANE_SIZE, 100.0);
    }
}
