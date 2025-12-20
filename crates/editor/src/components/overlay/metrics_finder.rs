//! MetricsFinder - A telescope/fzf-style finder modal for metrics with tag preview.
//!
//! This module provides a finder modal for searching and selecting metrics,
//! with a preview pane showing available tags/labels. It uses the generic
//! [`Finder<T>`] abstraction for consistent behavior with other finder modals.
//!
//! # Features
//!
//! - Fuzzy search across all available metrics
//! - Preview pane showing metric metadata and available tags
//! - Category icons based on metric naming conventions
//! - Support for both demo mode and live Prometheus connections
//!
//! # Usage
//!
//! ```ignore
//! let mut finder = MetricsFinder::new();
//! finder.set_items(vec![
//!     MetricItem {
//!         name: "http_requests_total".into(),
//!         category: "http".into(),
//!         description: Some("Total HTTP requests".into()),
//!         unit: Some("requests".into()),
//!         tags: HashMap::new(),
//!         series_count: 42,
//!     },
//! ]);
//! finder.open();
//!
//! // In render loop:
//! if let Some(metric) = finder.show(ctx) {
//!     create_pane_for_metric(&metric);
//! }
//! ```

use std::collections::{HashMap, HashSet};

use egui::{Color32, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::typography;

use crate::components::util::finder::{Finder, FinderConfig, FinderItem, FinderResult};
use crate::components::util::finder_utils::FinderColors;

/// A metric item that can be searched in the metrics finder.
///
/// Contains all the information needed to display and filter metrics,
/// including metadata like category, description, and available tags.
#[derive(Debug, Clone)]
pub struct MetricItem {
    /// Metric name (e.g., "http_requests_total").
    pub name: String,
    /// Metric category (e.g., "http", "system", "app").
    pub category: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Optional unit of measurement (e.g., "bytes", "seconds").
    pub unit: Option<String>,
    /// Tags/labels associated with this metric (key -> set of values).
    pub tags: HashMap<String, HashSet<String>>,
    /// Number of active time series for this metric.
    pub series_count: usize,
}

impl FinderItem for MetricItem {
    fn search_text(&self) -> &str {
        &self.name
    }

    fn icon(&self) -> &'static str {
        semantic_icons::metric_type_icon(&self.name)
    }

    fn secondary_text(&self) -> Option<String> {
        Some(format!("[{}]", self.category))
    }
}

/// A telescope/fzf-style finder modal for metrics with tag preview.
///
/// This wraps the generic [`Finder<MetricItem>`] and adds a custom preview
/// pane that shows metric metadata and available tags/labels.
pub struct MetricsFinder {
    /// The underlying generic finder.
    finder: Finder<MetricItem>,
}

impl Default for MetricsFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsFinder {
    /// Creates a new metrics finder.
    pub fn new() -> Self {
        let config = FinderConfig {
            placeholder: "Search metrics...",
            icon: semantic_icons::action::SEARCH,
            show_preview: true,
            empty_message: "No results found",
            no_items_message: "No metrics available",
        };

        Self {
            finder: Finder::new(config),
        }
    }

    /// Sets the UI theme for styling.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.finder.set_theme(theme);
    }

    /// Returns `true` if the finder is currently visible.
    pub fn is_open(&self) -> bool {
        self.finder.is_open()
    }

    /// Opens the metrics finder modal.
    pub fn open(&mut self) {
        self.finder.open();
    }

    /// Closes the metrics finder modal.
    pub fn close(&mut self) {
        self.finder.close();
    }

    /// Sets the metrics to search through.
    pub fn set_items(&mut self, items: Vec<MetricItem>) {
        self.finder.set_items(items);
    }

    /// Toggle preview pane visibility.
    pub fn toggle_preview(&mut self) {
        self.finder.toggle_preview();
    }

    /// Get the currently selected metric item (if any).
    pub fn selected_item(&self) -> Option<&MetricItem> {
        self.finder.selected_item()
    }

    /// Get the currently selected metric name (if any).
    pub fn selected_metric_name(&self) -> Option<&str> {
        self.selected_item().map(|item| item.name.as_str())
    }

    /// Update tags for a specific metric in the items list.
    ///
    /// This is used to update the preview with fetched per-metric labels.
    pub fn update_metric_tags(
        &mut self,
        metric_name: &str,
        tags: HashMap<String, HashSet<String>>,
    ) {
        // Update in the items list
        if let Some(item) = self
            .finder
            .items_mut()
            .iter_mut()
            .find(|i| i.name == metric_name)
        {
            item.tags = tags.clone();
        }
        self.finder.mark_needs_refresh();
    }

    /// Shows the metrics finder modal.
    ///
    /// Returns `Some(metric)` if the user selected a metric this frame.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<MetricItem> {
        let theme = self.finder.theme();
        self.finder.show_with_preview(ctx, |ui, result, colors| {
            Self::render_preview(ui, result, colors, theme);
        })
    }

    /// Renders the preview pane content for the selected metric.
    fn render_preview(
        ui: &mut egui::Ui,
        result: &FinderResult<MetricItem>,
        colors: &FinderColors,
        theme: AppTheme,
    ) {
        let text_col = text_color(theme);
        let tag_key_color = match theme {
            AppTheme::Light => Color32::from_rgb(50, 120, 180), // blue
            AppTheme::Dark => Color32::from_rgb(97, 175, 239),  // light blue
        };
        let tag_value_color = match theme {
            AppTheme::Light => Color32::from_rgb(80, 140, 80), // green
            AppTheme::Dark => Color32::from_rgb(152, 195, 121), // light green
        };

        // Header with metric info
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(result.item.icon())
                    .color(text_col)
                    .size(typography::HEADING),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(result.item.search_text())
                    .color(text_col)
                    .strong()
                    .size(typography::XL),
            );
        });

        // Category and unit line
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Category: {}", result.item.category))
                    .color(text_col.gamma_multiply(0.5))
                    .size(typography::SM),
            );
            if let Some(unit) = &result.item.unit {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!("Unit: {unit}"))
                        .color(text_col.gamma_multiply(0.5))
                        .size(typography::SM),
                );
            }
        });

        // Series count
        if result.item.series_count > 0 {
            ui.add_space(2.0);
            ui.label(
                RichText::new(format!("{} active series", result.item.series_count))
                    .color(text_col.gamma_multiply(0.5))
                    .size(typography::SM),
            );
        }

        ui.add_space(8.0);

        // Separator line
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );
        ui.add_space(8.0);

        // Description section (if available)
        if let Some(desc) = &result.item.description {
            ui.label(
                RichText::new("Description")
                    .color(text_col.gamma_multiply(0.6))
                    .size(typography::XS),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(desc)
                    .color(text_col.gamma_multiply(0.8))
                    .size(typography::MD),
            );
            ui.add_space(12.0);
        }

        // Tags section
        ui.label(
            RichText::new("Available Tags")
                .color(text_col.gamma_multiply(0.6))
                .size(typography::XS),
        );
        ui.add_space(6.0);

        if result.item.tags.is_empty() {
            // Show placeholder
            let remaining = ui.available_height();
            ui.allocate_space(egui::vec2(0.0, remaining / 3.0));
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No tags available")
                        .color(text_col.gamma_multiply(0.4))
                        .italics()
                        .size(typography::MD),
                );
            });
        } else {
            // Show tags in a scrollable area
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Sort tag keys for consistent display
                    let mut tag_keys: Vec<_> = result.item.tags.keys().collect();
                    tag_keys.sort();

                    // Check if this is "placeholder only" mode (all tags have just "..." as value)
                    let is_placeholder_only = result
                        .item
                        .tags
                        .values()
                        .all(|values| values.len() == 1 && values.contains("..."));

                    if is_placeholder_only {
                        // Compact display: just show label names in a list
                        for key in tag_keys.iter().take(10) {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("•").color(tag_key_color).size(typography::MD),
                                );
                                ui.label(
                                    RichText::new(*key)
                                        .color(tag_key_color)
                                        .size(typography::MD),
                                );
                            });
                        }
                        if tag_keys.len() > 10 {
                            ui.label(
                                RichText::new(format!("  ... and {} more", tag_keys.len() - 10))
                                    .color(text_col.gamma_multiply(0.4))
                                    .italics()
                                    .size(typography::XS),
                            );
                        }
                    } else {
                        // Full display with values
                        for (idx, key) in tag_keys.iter().enumerate() {
                            if let Some(values) = result.item.tags.get(*key) {
                                // Tag key
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{key}:"))
                                            .color(tag_key_color)
                                            .size(typography::MD)
                                            .strong(),
                                    );
                                });

                                // Tag values (show up to 5, with ellipsis if more)
                                let mut sorted_values: Vec<_> = values.iter().collect();
                                sorted_values.sort();
                                let display_count = sorted_values.len().min(5);
                                let has_more = sorted_values.len() > 5;

                                ui.indent(egui::Id::new(("tag_values", idx)), |ui| {
                                    for value in sorted_values.iter().take(display_count) {
                                        ui.label(
                                            RichText::new(format!("• {value}"))
                                                .color(tag_value_color)
                                                .size(typography::SM),
                                        );
                                    }
                                    if has_more {
                                        ui.label(
                                            RichText::new(format!(
                                                "  ... and {} more",
                                                sorted_values.len() - 5
                                            ))
                                            .color(text_col.gamma_multiply(0.4))
                                            .italics()
                                            .size(typography::XS),
                                        );
                                    }
                                });

                                ui.add_space(6.0);
                            }
                        }
                    }
                });
        }
    }
}
