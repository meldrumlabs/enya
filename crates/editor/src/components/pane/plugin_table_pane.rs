//! Plugin table pane component for displaying custom data from plugins.
//!
//! This pane renders tabular data provided by Lua plugins, allowing plugins
//! to create custom views for data fetched via HTTP or generated locally.
//!
//! ## Premium Features
//!
//! - **Row hover and selection** with themed backgrounds
//! - **Status badges** for values like "healthy", "error", "warning"
//! - **Accent border** for selected rows with subtle glow effect
//! - **Column separators** for better readability
//! - **Smooth transitions** between states

use std::any::Any;

use egui::{Color32, RichText, ScrollArea, Vec2};
use enya_plugin::{CustomTableConfig, CustomTableData};

use crate::components::util::id_generator::next_id_usize;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

// ============================================================================
// Constants for consistent styling
// ============================================================================

const ROW_HEIGHT: f32 = 32.0;
const HEADER_HEIGHT: f32 = 40.0;
const PADDING: f32 = 16.0;
const INNER_PADDING: f32 = 12.0;
const CORNER_RADIUS: f32 = 8.0;
const SMALL_CORNER_RADIUS: f32 = 4.0;
const MIN_COLUMN_WIDTH: f32 = 80.0;
const DEFAULT_COLUMN_WIDTH: f32 = 140.0;
const ACCENT_BORDER_WIDTH: f32 = 3.0;
const GLOW_WIDTH: f32 = 12.0;

/// A plugin table pane that displays custom tabular data from plugins.
///
/// This pane shows data in a scrollable table with:
/// - Configurable columns (name, key, width)
/// - Rows of cell data with hover/selection states
/// - Status badge support for special values
/// - Optional auto-refresh support
/// - Error display if data fetch fails
pub struct PluginTablePane {
    /// Unique identifier for this pane
    id: usize,
    /// Display name for the pane (from config title)
    name: String,
    /// Current theme
    theme: AppTheme,
    /// Configuration for this pane type
    config: CustomTableConfig,
    /// Current data to display
    data: CustomTableData,
    /// Whether data is currently loading
    is_loading: bool,
    /// Currently selected row index
    selected_index: Option<usize>,
    /// Currently hovered row index
    hovered_index: Option<usize>,
}

impl PluginTablePane {
    /// Create a new plugin table pane with the given configuration and initial data.
    pub fn new(config: CustomTableConfig, data: CustomTableData) -> Self {
        Self {
            id: next_id_usize(),
            name: config.title.clone(),
            theme: AppTheme::default(),
            config,
            data,
            is_loading: false,
            selected_index: None,
            hovered_index: None,
        }
    }

    /// Get the pane type name (for matching updates by type).
    pub fn pane_type(&self) -> &str {
        &self.config.name
    }

    /// Set new data for this pane.
    pub fn set_data(&mut self, data: CustomTableData) {
        self.data = data;
        self.is_loading = false;
        // Reset selection if data changed
        if let Some(idx) = self.selected_index {
            if idx >= self.data.rows.len() {
                self.selected_index = None;
            }
        }
    }

    /// Set the loading state.
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Render the table pane.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let accent = theme.accent_primary();
        let available_size = ui.available_size();

        // Calculate column widths
        let column_widths: Vec<f32> = self.calculate_column_widths(available_size.x);

        // Draw main background with rounded corners
        let main_rect = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(main_rect, CORNER_RADIUS, theme.bg_surface());

        ui.add_space(PADDING);

        // Status bar (row count, loading, or error)
        self.draw_status_bar(ui, theme);

        ui.add_space(8.0);

        // Table content area
        let table_height = available_size.y - HEADER_HEIGHT - PADDING * 2.5;

        // Reset hover state before rendering
        self.hovered_index = None;

        ScrollArea::vertical()
            .max_height(table_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                // Column headers with background
                self.draw_column_headers(ui, &column_widths, theme);

                ui.add_space(4.0);

                // Data rows or empty state
                if self.data.rows.is_empty() && self.data.error.is_none() {
                    self.draw_empty_state(ui, theme);
                } else {
                    let row_count = self.data.rows.len();
                    for row_idx in 0..row_count {
                        self.draw_row(ui, row_idx, &column_widths, theme, accent);
                    }
                }
            });
    }

    /// Draw the status bar (loading, error, or row count) as a premium badge.
    fn draw_status_bar(&self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.horizontal(|ui| {
            // Right-aligned status badge
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(INNER_PADDING);

                if self.is_loading {
                    // Loading badge
                    let badge_color = theme.text_tertiary();
                    let badge_bg = if theme.is_dark() {
                        badge_color.gamma_multiply(0.12)
                    } else {
                        badge_color.gamma_multiply(0.08)
                    };

                    egui::Frame::new()
                        .fill(badge_bg)
                        .corner_radius(SMALL_CORNER_RADIUS)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.spinner();
                                ui.label(
                                    RichText::new("Loading")
                                        .color(theme.text_secondary())
                                        .size(11.0),
                                );
                            });
                        });
                } else if let Some(error) = &self.data.error {
                    // Error badge
                    let error_color = theme.semantic_error();
                    let badge_bg = if theme.is_dark() {
                        error_color.gamma_multiply(0.15)
                    } else {
                        error_color.gamma_multiply(0.10)
                    };

                    let response = egui::Frame::new()
                        .fill(badge_bg)
                        .corner_radius(SMALL_CORNER_RADIUS)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("{} Error", semantic_icons::status::ERROR))
                                    .color(error_color)
                                    .size(11.0),
                            );
                        });
                    response.response.on_hover_text(error);
                } else {
                    // Row count badge
                    let count = self.data.rows.len();
                    let badge_color = theme.accent_primary();
                    let badge_bg = if theme.is_dark() {
                        badge_color.gamma_multiply(0.12)
                    } else {
                        badge_color.gamma_multiply(0.08)
                    };

                    egui::Frame::new()
                        .fill(badge_bg)
                        .corner_radius(SMALL_CORNER_RADIUS)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            let count_text = if count == 1 {
                                format!("{} 1 row", semantic_icons::file::DATA)
                            } else {
                                format!("{} {} rows", semantic_icons::file::DATA, count)
                            };
                            ui.label(RichText::new(count_text).color(badge_color).size(11.0));
                        });
                }
            });
        });
    }

    /// Calculate column widths based on available space.
    fn calculate_column_widths(&self, available_width: f32) -> Vec<f32> {
        let num_columns = self.config.columns.len();
        if num_columns == 0 {
            return Vec::new();
        }

        let usable_width = available_width - PADDING * 2.0 - ACCENT_BORDER_WIDTH;

        // Check if any columns have fixed widths
        let mut widths: Vec<f32> = self
            .config
            .columns
            .iter()
            .map(|col| col.width_f32().unwrap_or(0.0))
            .collect();

        let fixed_total: f32 = widths.iter().filter(|w| **w > 0.0).sum();
        let flex_count = widths.iter().filter(|w| **w == 0.0).count();

        if flex_count > 0 {
            // Distribute remaining space to flex columns
            let remaining = (usable_width - fixed_total).max(0.0);
            let flex_width = (remaining / flex_count as f32).max(MIN_COLUMN_WIDTH);

            for w in widths.iter_mut() {
                if *w == 0.0 {
                    *w = flex_width;
                }
            }
        }

        // Ensure minimum widths
        for w in widths.iter_mut() {
            if *w < MIN_COLUMN_WIDTH {
                *w = MIN_COLUMN_WIDTH;
            }
        }

        widths
    }

    /// Draw column headers with background.
    fn draw_column_headers(&self, ui: &mut egui::Ui, column_widths: &[f32], theme: AppTheme) {
        let header_rect = ui.available_rect_before_wrap();
        let header_rect = egui::Rect::from_min_size(
            header_rect.min,
            Vec2::new(ui.available_width(), HEADER_HEIGHT - 8.0),
        );

        let painter = ui.painter();

        // Header background
        painter.rect_filled(
            header_rect,
            SMALL_CORNER_RADIUS,
            if theme.is_dark() {
                theme.bg_elevated().gamma_multiply(0.8)
            } else {
                theme.bg_base().gamma_multiply(0.95)
            },
        );

        // Draw headers using painter (same approach as rows for perfect alignment)
        let base_x = header_rect.left() + INNER_PADDING + ACCENT_BORDER_WIDTH;
        let center_y = header_rect.center().y;
        let mut x_offset = base_x;

        for (idx, col) in self.config.columns.iter().enumerate() {
            let width = column_widths
                .get(idx)
                .copied()
                .unwrap_or(DEFAULT_COLUMN_WIDTH);

            // Draw header text
            let galley = painter.layout_no_wrap(
                col.name.to_uppercase(),
                egui::FontId::proportional(10.0),
                theme.text_tertiary(),
            );

            painter.galley(
                egui::pos2(x_offset, center_y - galley.size().y / 2.0),
                galley,
                theme.text_tertiary(),
            );

            x_offset += width;

            // Column separator (except last)
            if idx < self.config.columns.len() - 1 {
                painter.vline(
                    x_offset - 4.0,
                    egui::Rangef::new(header_rect.top() + 6.0, header_rect.bottom() - 6.0),
                    egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.3)),
                );
            }
        }

        // Allocate space for the header
        ui.allocate_space(Vec2::new(ui.available_width(), HEADER_HEIGHT - 8.0));
    }

    /// Draw empty state message.
    fn draw_empty_state(&self, ui: &mut egui::Ui, theme: AppTheme) {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(semantic_icons::status::INFO)
                    .color(theme.text_tertiary())
                    .size(32.0),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new("No data available")
                    .color(theme.text_tertiary())
                    .size(14.0),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Use the plugin commands to fetch data")
                    .color(theme.text_tertiary().gamma_multiply(0.7))
                    .size(12.0),
            );
        });
    }

    /// Draw a single data row with premium styling.
    fn draw_row(
        &mut self,
        ui: &mut egui::Ui,
        row_idx: usize,
        column_widths: &[f32],
        theme: AppTheme,
        accent: Color32,
    ) {
        let is_selected = self.selected_index == Some(row_idx);
        let is_hovered = self.hovered_index == Some(row_idx);
        let row = &self.data.rows[row_idx];

        // Calculate row background color
        let bg_color = if is_selected {
            if theme.is_dark() {
                accent.gamma_multiply(0.18)
            } else {
                accent.gamma_multiply(0.12)
            }
        } else if is_hovered {
            if theme.is_dark() {
                accent.gamma_multiply(0.08)
            } else {
                accent.gamma_multiply(0.05)
            }
        } else if row_idx % 2 == 1 {
            // Subtle alternating rows
            if theme.is_dark() {
                theme.bg_surface().gamma_multiply(1.08)
            } else {
                theme.bg_surface().gamma_multiply(0.97)
            }
        } else {
            Color32::TRANSPARENT
        };

        // Row rectangle
        let row_rect = ui.available_rect_before_wrap();
        let row_rect =
            egui::Rect::from_min_size(row_rect.min, Vec2::new(ui.available_width(), ROW_HEIGHT));

        // Draw row background
        ui.painter().rect_filled(row_rect, 0.0, bg_color);

        // Selected row: accent left border with glow
        if is_selected {
            let border_rect = egui::Rect::from_min_size(
                row_rect.left_top(),
                egui::vec2(ACCENT_BORDER_WIDTH, row_rect.height()),
            );
            ui.painter().rect_filled(border_rect, 0.0, accent);

            // Subtle glow effect (dark themes)
            if theme.is_dark() {
                let glow_rect = egui::Rect::from_min_size(
                    egui::pos2(row_rect.left() + ACCENT_BORDER_WIDTH, row_rect.top()),
                    egui::vec2(GLOW_WIDTH, row_rect.height()),
                );
                ui.painter()
                    .rect_filled(glow_rect, 0.0, accent.gamma_multiply(0.06));
            }
        }

        // Interactive row response
        let response = ui.allocate_rect(row_rect, egui::Sense::click());

        if response.hovered() {
            self.hovered_index = Some(row_idx);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            self.selected_index = if self.selected_index == Some(row_idx) {
                None // Deselect if clicking same row
            } else {
                Some(row_idx)
            };
        }

        // Draw cell contents
        let painter = ui.painter();
        let base_x = row_rect.left() + INNER_PADDING + ACCENT_BORDER_WIDTH;
        let center_y = row_rect.center().y;
        let text_color = theme.text_primary();

        let mut x_offset = base_x;

        for (idx, col) in self.config.columns.iter().enumerate() {
            let width = column_widths
                .get(idx)
                .copied()
                .unwrap_or(DEFAULT_COLUMN_WIDTH);
            let key = col.data_key();
            let value = row.get(key).unwrap_or("-");

            // Check if this value should be rendered as a status badge
            if let Some((badge_text, badge_color)) = get_status_badge(value, theme) {
                // Render as a badge
                let badge_galley = painter.layout_no_wrap(
                    badge_text.to_string(),
                    egui::FontId::proportional(10.0),
                    badge_color,
                );

                let badge_width = badge_galley.size().x + 12.0;
                let badge_height = badge_galley.size().y + 6.0;

                let badge_rect = egui::Rect::from_min_size(
                    egui::pos2(x_offset, center_y - badge_height / 2.0),
                    egui::vec2(badge_width, badge_height),
                );

                // Badge background
                let badge_bg = if theme.is_dark() {
                    badge_color.gamma_multiply(0.15)
                } else {
                    badge_color.gamma_multiply(0.12)
                };
                painter.rect_filled(badge_rect, SMALL_CORNER_RADIUS, badge_bg);

                // Badge text
                painter.galley(
                    egui::pos2(
                        badge_rect.center().x - badge_galley.size().x / 2.0,
                        badge_rect.center().y - badge_galley.size().y / 2.0,
                    ),
                    badge_galley,
                    badge_color,
                );
            } else {
                // Render as regular text
                let galley = painter.layout_no_wrap(
                    value.to_string(),
                    egui::FontId::proportional(13.0),
                    text_color,
                );

                painter.galley(
                    egui::pos2(x_offset, center_y - galley.size().y / 2.0),
                    galley,
                    text_color,
                );
            }

            x_offset += width;

            // Column separator
            if idx < self.config.columns.len() - 1 {
                painter.vline(
                    x_offset - 4.0,
                    egui::Rangef::new(row_rect.top() + 6.0, row_rect.bottom() - 6.0),
                    egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.2)),
                );
            }
        }

        // Allocate space for the row
        ui.allocate_space(Vec2::new(ui.available_width(), ROW_HEIGHT));
    }
}

/// Check if a value should be rendered as a status badge.
/// Returns the badge text and color if applicable.
fn get_status_badge(value: &str, theme: AppTheme) -> Option<(&'static str, Color32)> {
    let lower = value.to_lowercase();

    // Success states
    if lower == "healthy"
        || lower == "ok"
        || lower == "success"
        || lower == "active"
        || lower == "running"
        || lower == "up"
        || lower == "online"
    {
        return Some(("●", theme.semantic_success()));
    }

    // Warning states
    if lower == "warning"
        || lower == "warn"
        || lower == "degraded"
        || lower == "pending"
        || lower == "slow"
    {
        return Some(("●", theme.semantic_warning()));
    }

    // Error states
    if lower == "error"
        || lower == "failed"
        || lower == "critical"
        || lower == "down"
        || lower == "offline"
        || lower == "unhealthy"
    {
        return Some(("●", theme.semantic_error()));
    }

    // Info states
    if lower == "info" || lower == "unknown" || lower == "maintenance" || lower == "paused" {
        return Some(("●", theme.semantic_info()));
    }

    None
}

/// Implement Component trait so PluginTablePane can be used in the dashboard.
impl crate::components::Component for PluginTablePane {
    fn show(&mut self, ui: &mut egui::Ui) {
        PluginTablePane::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn label(&self) -> egui::RichText {
        egui::RichText::new(format!("{} {}", semantic_icons::file::DATA, self.name))
    }

    fn description(&self) -> &str {
        "Custom table from plugin"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Component;
    use std::collections::BTreeMap;

    fn create_test_config() -> CustomTableConfig {
        CustomTableConfig {
            name: "test-table".to_string(),
            title: "Test Table".to_string(),
            columns: vec![
                enya_plugin::TableColumnConfig::new("Name".to_string()),
                enya_plugin::TableColumnConfig::new("Status".to_string()),
            ],
            refresh_interval: 0,
            plugin_name: "test-plugin".to_string(),
        }
    }

    #[test]
    fn test_pane_creation() {
        let config = create_test_config();
        let data = CustomTableData::with_rows(vec![]);
        let pane = PluginTablePane::new(config.clone(), data);

        assert_eq!(pane.name(), "Test Table");
        assert_eq!(pane.pane_type(), "test-table");
    }

    #[test]
    fn test_set_data_resets_invalid_selection() {
        let config = create_test_config();
        let mut cells = BTreeMap::new();
        cells.insert("name".to_string(), "Test".to_string());
        let row = enya_plugin::CustomTableRow { cells };

        let data = CustomTableData::with_rows(vec![row]);
        let mut pane = PluginTablePane::new(config, data);

        // Select the only row
        pane.selected_index = Some(0);

        // Set empty data - selection should reset
        pane.set_data(CustomTableData::with_rows(vec![]));
        assert_eq!(pane.selected_index, None);
    }

    #[test]
    fn test_status_badge_detection() {
        let theme = AppTheme::Dark;

        // Success states
        assert!(get_status_badge("healthy", theme).is_some());
        assert!(get_status_badge("HEALTHY", theme).is_some());
        assert!(get_status_badge("running", theme).is_some());

        // Warning states
        assert!(get_status_badge("warning", theme).is_some());
        assert!(get_status_badge("degraded", theme).is_some());

        // Error states
        assert!(get_status_badge("error", theme).is_some());
        assert!(get_status_badge("failed", theme).is_some());

        // Regular values - no badge
        assert!(get_status_badge("some text", theme).is_none());
        assert!(get_status_badge("123", theme).is_none());
    }

    #[test]
    fn test_column_width_calculation() {
        let config = create_test_config();
        let data = CustomTableData::with_rows(vec![]);
        let pane = PluginTablePane::new(config, data);

        let widths = pane.calculate_column_widths(500.0);
        assert_eq!(widths.len(), 2);
        // Both columns should get equal width (no fixed widths specified)
        assert!((widths[0] - widths[1]).abs() < 0.1);
    }
}
