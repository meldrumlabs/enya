use egui::RichText;

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

/// What kind of item is being inspected
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorTarget {
    /// No selection
    None,
    /// A metric from the tree
    Metric {
        name: String,
        description: Option<String>,
        unit: Option<String>,
        tags: Vec<(String, Vec<String>)>,
        series_count: usize,
    },
    /// A data point on a chart (for future use)
    DataPoint {
        metric_name: String,
        timestamp: f64,
        value: f64,
        tags: Vec<(String, String)>,
    },
}

impl Default for InspectorTarget {
    fn default() -> Self {
        Self::None
    }
}

/// Statistics for a metric series
#[derive(Debug, Clone, Default)]
pub struct MetricStats {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub avg: Option<f64>,
    pub last: Option<f64>,
    pub count: usize,
}

impl MetricStats {
    /// Create demo stats for testing
    pub fn demo() -> Self {
        Self {
            min: Some(42.5),
            max: Some(98.7),
            avg: Some(67.3),
            last: Some(72.1),
            count: 120,
        }
    }
}

/// A collapsible inspector panel for the right side of the dashboard
pub struct InspectorPanel {
    /// Whether the panel is visible/expanded
    visible: bool,
    /// Current theme
    theme: AppTheme,
    /// What is currently being inspected
    target: InspectorTarget,
    /// Statistics for the current metric (if available)
    stats: Option<MetricStats>,
    /// Panel width
    width: f32,
}

impl Default for InspectorPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl InspectorPanel {
    /// Default panel width
    const DEFAULT_WIDTH: f32 = 280.0;
    /// Minimum panel width
    const MIN_WIDTH: f32 = 200.0;
    /// Maximum panel width
    const MAX_WIDTH: f32 = 400.0;

    pub fn new() -> Self {
        Self {
            visible: false, // Hidden by default
            theme: AppTheme::default(),
            target: InspectorTarget::None,
            stats: None,
            width: Self::DEFAULT_WIDTH,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the panel is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Toggle panel visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Show the panel
    pub fn open(&mut self) {
        self.visible = true;
    }

    /// Hide the panel
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Set what is being inspected
    pub fn set_target(&mut self, target: InspectorTarget) {
        self.target = target;
        // Auto-show panel when something is selected
        if !matches!(self.target, InspectorTarget::None) {
            self.visible = true;
        }
    }

    /// Set statistics for the current metric
    pub fn set_stats(&mut self, stats: Option<MetricStats>) {
        self.stats = stats;
    }

    /// Get the current target
    pub fn target(&self) -> &InspectorTarget {
        &self.target
    }

    /// Clear the current selection
    pub fn clear(&mut self) {
        self.target = InspectorTarget::None;
        self.stats = None;
    }

    /// Render the panel (call this in the parent layout)
    /// Returns true if visible, so parent can adjust layout
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if !self.visible {
            return;
        }

        let text_color = text_color(self.theme);

        egui::SidePanel::right("inspector_panel")
            .resizable(true)
            .default_width(self.width)
            .width_range(Self::MIN_WIDTH..=Self::MAX_WIDTH)
            .show_inside(ui, |ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} Inspector", egui_phosphor::regular::INFO))
                            .color(text_color)
                            .strong(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new(egui_phosphor::regular::X).color(text_color))
                            .on_hover_text("Close inspector")
                            .clicked()
                        {
                            self.visible = false;
                        }
                    });
                });

                ui.separator();
                ui.add_space(8.0);

                // Content based on target
                match &self.target {
                    InspectorTarget::None => {
                        self.show_empty_state(ui, text_color);
                    }
                    InspectorTarget::Metric { .. } => {
                        self.show_metric_details(ui, text_color);
                    }
                    InspectorTarget::DataPoint { .. } => {
                        self.show_data_point_details(ui, text_color);
                    }
                }
            });
    }

    fn show_empty_state(&self, ui: &mut egui::Ui, text_color: egui::Color32) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                RichText::new(egui_phosphor::regular::CURSOR_CLICK)
                    .color(text_color.gamma_multiply(0.3))
                    .size(48.0),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new("Select a metric to inspect")
                    .color(text_color.gamma_multiply(0.5))
                    .italics(),
            );
        });
    }

    fn show_metric_details(&self, ui: &mut egui::Ui, text_color: egui::Color32) {
        let InspectorTarget::Metric {
            name,
            description,
            unit,
            tags,
            series_count,
        } = &self.target
        else {
            return;
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Metric name
                ui.label(
                    RichText::new(format!("{} Metric", egui_phosphor::regular::CHART_LINE))
                        .color(text_color.gamma_multiply(0.6))
                        .small(),
                );
                ui.label(RichText::new(name).color(text_color).strong());
                ui.add_space(12.0);

                // Description
                if let Some(desc) = description {
                    ui.label(
                        RichText::new("Description")
                            .color(text_color.gamma_multiply(0.6))
                            .small(),
                    );
                    ui.label(RichText::new(desc).color(text_color));
                    ui.add_space(12.0);
                }

                // Unit
                if let Some(u) = unit {
                    self.show_field(ui, "Unit", u, text_color);
                }

                // Series count
                self.show_field(ui, "Series", &series_count.to_string(), text_color);

                // Statistics section
                if let Some(stats) = &self.stats {
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.label(
                        RichText::new(format!("{} Statistics", egui_phosphor::regular::CHART_BAR))
                            .color(text_color)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    egui::Grid::new("stats_grid")
                        .num_columns(2)
                        .spacing([16.0, 4.0])
                        .show(ui, |ui| {
                            if let Some(min) = stats.min {
                                ui.label(
                                    RichText::new("Min")
                                        .color(text_color.gamma_multiply(0.6))
                                        .small(),
                                );
                                ui.label(
                                    RichText::new(format!("{min:.2}"))
                                        .color(egui::Color32::from_rgb(59, 130, 246)),
                                );
                                ui.end_row();
                            }
                            if let Some(max) = stats.max {
                                ui.label(
                                    RichText::new("Max")
                                        .color(text_color.gamma_multiply(0.6))
                                        .small(),
                                );
                                ui.label(
                                    RichText::new(format!("{max:.2}"))
                                        .color(egui::Color32::from_rgb(239, 68, 68)),
                                );
                                ui.end_row();
                            }
                            if let Some(avg) = stats.avg {
                                ui.label(
                                    RichText::new("Avg")
                                        .color(text_color.gamma_multiply(0.6))
                                        .small(),
                                );
                                ui.label(
                                    RichText::new(format!("{avg:.2}"))
                                        .color(egui::Color32::from_rgb(34, 197, 94)),
                                );
                                ui.end_row();
                            }
                            if let Some(last) = stats.last {
                                ui.label(
                                    RichText::new("Last")
                                        .color(text_color.gamma_multiply(0.6))
                                        .small(),
                                );
                                ui.label(RichText::new(format!("{last:.2}")).color(text_color));
                                ui.end_row();
                            }
                            ui.label(
                                RichText::new("Points")
                                    .color(text_color.gamma_multiply(0.6))
                                    .small(),
                            );
                            ui.label(RichText::new(stats.count.to_string()).color(text_color));
                            ui.end_row();
                        });
                }

                // Tags section
                if !tags.is_empty() {
                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    ui.label(
                        RichText::new(format!("{} Tags", egui_phosphor::regular::TAG))
                            .color(text_color)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    for (key, values) in tags {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!("{key}:"))
                                    .color(text_color.gamma_multiply(0.7)),
                            );
                            for value in values {
                                ui.label(
                                    RichText::new(value)
                                        .color(text_color)
                                        .background_color(
                                            egui::Color32::from_rgb(59, 130, 246)
                                                .gamma_multiply(0.2),
                                        )
                                        .small(),
                                );
                            }
                        });
                    }
                }
            });
    }

    fn show_data_point_details(&self, ui: &mut egui::Ui, text_color: egui::Color32) {
        let InspectorTarget::DataPoint {
            metric_name,
            timestamp,
            value,
            tags,
        } = &self.target
        else {
            return;
        };

        // Metric name
        ui.label(
            RichText::new(format!("{} Data Point", egui_phosphor::regular::CROSSHAIR))
                .color(text_color.gamma_multiply(0.6))
                .small(),
        );
        ui.label(RichText::new(metric_name).color(text_color).strong());
        ui.add_space(12.0);

        // Timestamp
        self.show_field(ui, "Timestamp", &format!("{timestamp:.3}"), text_color);

        // Value
        ui.label(
            RichText::new("Value")
                .color(text_color.gamma_multiply(0.6))
                .small(),
        );
        ui.label(
            RichText::new(format!("{value:.4}"))
                .color(egui::Color32::from_rgb(34, 197, 94))
                .strong()
                .size(18.0),
        );
        ui.add_space(12.0);

        // Tags
        if !tags.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ui.label(
                RichText::new(format!("{} Tags", egui_phosphor::regular::TAG))
                    .color(text_color)
                    .strong(),
            );
            ui.add_space(8.0);

            egui::Grid::new("point_tags_grid")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (key, value) in tags {
                        ui.label(
                            RichText::new(format!("{key}:")).color(text_color.gamma_multiply(0.6)),
                        );
                        ui.label(RichText::new(value).color(text_color));
                        ui.end_row();
                    }
                });
        }
    }

    fn show_field(&self, ui: &mut egui::Ui, label: &str, value: &str, text_color: egui::Color32) {
        ui.label(
            RichText::new(label)
                .color(text_color.gamma_multiply(0.6))
                .small(),
        );
        ui.label(RichText::new(value).color(text_color));
        ui.add_space(8.0);
    }
}

/// Button to toggle the inspector panel
pub fn inspector_toggle_button(
    ui: &mut egui::Ui,
    is_visible: bool,
    theme: AppTheme,
) -> egui::Response {
    let text_color = text_color(theme);
    let icon = egui_phosphor::regular::SIDEBAR_SIMPLE;

    let button = if is_visible {
        egui::Button::new(RichText::new(icon).strong()).fill(ui.visuals().selection.bg_fill)
    } else {
        egui::Button::new(RichText::new(icon).color(text_color.gamma_multiply(0.7)))
    };

    ui.add(button).on_hover_text(if is_visible {
        "Hide inspector"
    } else {
        "Show inspector"
    })
}
