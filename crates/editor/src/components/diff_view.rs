use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, RichText, Stroke};
use egui_plot::{Line, Plot, PlotPoints};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

use super::time_series_chart::{DataPoint, Series};

/// Global counter for unique component IDs
static NEXT_ID: AtomicUsize = AtomicUsize::new(5000);

/// Time offset presets for quick diff comparisons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOffset {
    /// 1 hour ago
    OneHour,
    /// 6 hours ago
    SixHours,
    /// 1 day ago
    OneDay,
    /// 7 days ago
    OneWeek,
    /// 30 days ago
    OneMonth,
    /// Custom offset in seconds
    Custom(i64),
}

impl DiffOffset {
    /// Get the offset in seconds
    pub fn as_seconds(&self) -> i64 {
        match self {
            Self::OneHour => 3600,
            Self::SixHours => 6 * 3600,
            Self::OneDay => 86400,
            Self::OneWeek => 7 * 86400,
            Self::OneMonth => 30 * 86400,
            Self::Custom(s) => *s,
        }
    }

    /// Get a human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            Self::OneHour => "1h",
            Self::SixHours => "6h",
            Self::OneDay => "1d",
            Self::OneWeek => "7d",
            Self::OneMonth => "30d",
            Self::Custom(_) => "custom",
        }
    }

    /// Get available presets
    pub fn presets() -> &'static [DiffOffset] {
        &[
            Self::OneHour,
            Self::SixHours,
            Self::OneDay,
            Self::OneWeek,
            Self::OneMonth,
        ]
    }

    /// Parse from string (e.g., "-7d", "-1h", "-30d")
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('-');

        // Try preset shortcuts
        match s {
            "1h" => return Some(Self::OneHour),
            "6h" => return Some(Self::SixHours),
            "1d" => return Some(Self::OneDay),
            "7d" => return Some(Self::OneWeek),
            "30d" => return Some(Self::OneMonth),
            _ => {}
        }

        // Try parsing custom format like "2h", "3d", "12h"
        if s.len() >= 2 {
            let (num_part, unit) = s.split_at(s.len() - 1);
            if let Ok(num) = num_part.parse::<i64>() {
                let multiplier = match unit {
                    "h" => 3600,
                    "d" => 86400,
                    "w" => 7 * 86400,
                    "m" => 60,
                    "s" => 1,
                    _ => return None,
                };
                return Some(Self::Custom(num * multiplier));
            }
        }

        None
    }
}

/// Statistics for a time series
#[derive(Debug, Clone, Default)]
pub struct SeriesStats {
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub last: f64,
    pub count: usize,
}

impl SeriesStats {
    /// Calculate statistics from a series of data points
    pub fn from_points(points: &[DataPoint]) -> Self {
        if points.is_empty() {
            return Self::default();
        }

        let mut values: Vec<f64> = points.iter().map(|p| p.value).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let count = values.len();
        let sum: f64 = values.iter().sum();
        let avg = sum / count as f64;
        let min = values.first().copied().unwrap_or(0.0);
        let max = values.last().copied().unwrap_or(0.0);
        let last = points.last().map(|p| p.value).unwrap_or(0.0);

        let p50 = Self::percentile(&values, 50.0);
        let p95 = Self::percentile(&values, 95.0);
        let p99 = Self::percentile(&values, 99.0);

        Self {
            min,
            max,
            avg,
            p50,
            p95,
            p99,
            last,
            count,
        }
    }

    fn percentile(sorted_values: &[f64], p: f64) -> f64 {
        if sorted_values.is_empty() {
            return 0.0;
        }
        let idx = ((p / 100.0) * (sorted_values.len() - 1) as f64).round() as usize;
        let idx = idx.min(sorted_values.len() - 1);
        sorted_values[idx]
    }
}

/// Delta between two statistics
#[derive(Debug, Clone)]
pub struct StatsDelta {
    pub abs: f64,
    pub pct: f64,
    pub improved: bool,
}

impl StatsDelta {
    /// Calculate delta between baseline and compare values
    /// `higher_is_better`: if true, increase = improvement; if false, decrease = improvement
    pub fn new(baseline: f64, compare: f64, higher_is_better: bool) -> Self {
        let abs = compare - baseline;
        let pct = if baseline.abs() > f64::EPSILON {
            (abs / baseline) * 100.0
        } else {
            0.0
        };
        let improved = if higher_is_better {
            abs > 0.0
        } else {
            abs < 0.0
        };
        Self { abs, pct, improved }
    }

    /// Format as string with sign and percentage
    pub fn format(&self, unit: &str) -> String {
        let sign = if self.abs >= 0.0 { "+" } else { "" };
        format!("{sign}{:.2}{unit} ({sign}{:.1}%)", self.abs, self.pct)
    }
}

/// A side-by-side diff view for comparing time series data
pub struct DiffView {
    /// Unique identifier
    id: usize,
    /// Metric name being compared
    metric_name: String,
    /// Baseline series (the "before" data)
    baseline_series: Vec<Series>,
    /// Compare series (the "after" data, typically current)
    compare_series: Vec<Series>,
    /// Time offset for baseline (how far back from compare)
    offset: DiffOffset,
    /// Current theme
    theme: AppTheme,
    /// Baseline statistics
    baseline_stats: SeriesStats,
    /// Compare statistics
    compare_stats: SeriesStats,
    /// Whether lower values are better (for metrics like latency)
    lower_is_better: bool,
    /// Title for the baseline panel
    baseline_label: String,
    /// Title for the compare panel
    compare_label: String,
}

impl Default for DiffView {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl DiffView {
    /// Create a new diff view for the given metric
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            metric_name: name,
            baseline_series: Vec::new(),
            compare_series: Vec::new(),
            offset: DiffOffset::OneWeek,
            theme: AppTheme::default(),
            baseline_stats: SeriesStats::default(),
            compare_stats: SeriesStats::default(),
            lower_is_better: true, // Default: lower is better (e.g., latency)
            baseline_label: "Baseline".to_string(),
            compare_label: "Current".to_string(),
        }
    }

    /// Create a diff view with demo data
    pub fn with_demo_data(metric_name: impl Into<String>, offset: DiffOffset) -> Self {
        let name = metric_name.into();
        let mut view = Self::new(&name);
        view.offset = offset;

        // Generate demo data
        let now = 1_700_000_000.0;
        let duration = 3600.0 * 6.0; // 6 hours of data
        let num_points = 180;
        let offset_seconds = offset.as_seconds() as f64;

        // Current period (compare) - slightly better performance
        let compare_points: Vec<DataPoint> = (0..num_points)
            .map(|i| {
                let t = now - duration + (i as f64 / num_points as f64) * duration;
                let base = 38.0 + 12.0 * (t / 400.0).sin();
                let noise = (t * 17.0).sin() * 4.0;
                DataPoint {
                    timestamp: t,
                    value: (base + noise).max(0.0),
                }
            })
            .collect();

        // Baseline period - slightly worse performance
        let baseline_points: Vec<DataPoint> = (0..num_points)
            .map(|i| {
                let t = now - offset_seconds - duration + (i as f64 / num_points as f64) * duration;
                let base = 45.0 + 15.0 * (t / 350.0).sin();
                let noise = (t * 19.0).sin() * 6.0;
                DataPoint {
                    timestamp: t,
                    value: (base + noise).max(0.0),
                }
            })
            .collect();

        view.set_compare_data(vec![
            Series::new(&name)
                .with_tag("host", "all")
                .with_points(compare_points)
                .with_color(Color32::from_rgb(59, 130, 246)),
        ]);

        view.set_baseline_data(vec![
            Series::new(&name)
                .with_tag("host", "all")
                .with_points(baseline_points)
                .with_color(Color32::from_rgb(156, 163, 175)),
        ]);

        view.update_labels();
        view
    }

    /// Set the metric name
    pub fn set_metric_name(&mut self, name: impl Into<String>) {
        self.metric_name = name.into();
    }

    /// Set the diff offset
    pub fn set_offset(&mut self, offset: DiffOffset) {
        self.offset = offset;
        self.update_labels();
    }

    /// Set whether lower values are better
    pub fn set_lower_is_better(&mut self, lower_is_better: bool) {
        self.lower_is_better = lower_is_better;
    }

    /// Set baseline series data
    pub fn set_baseline_data(&mut self, series: Vec<Series>) {
        // Aggregate all points for stats
        let all_points: Vec<DataPoint> = series
            .iter()
            .flat_map(|s| s.points.iter().cloned())
            .collect();
        self.baseline_stats = SeriesStats::from_points(&all_points);
        self.baseline_series = series;
    }

    /// Set compare series data
    pub fn set_compare_data(&mut self, series: Vec<Series>) {
        let all_points: Vec<DataPoint> = series
            .iter()
            .flat_map(|s| s.points.iter().cloned())
            .collect();
        self.compare_stats = SeriesStats::from_points(&all_points);
        self.compare_series = series;
    }

    /// Swap baseline and compare
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.baseline_series, &mut self.compare_series);
        std::mem::swap(&mut self.baseline_stats, &mut self.compare_stats);
        std::mem::swap(&mut self.baseline_label, &mut self.compare_label);
    }

    /// Set theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Update labels based on offset
    fn update_labels(&mut self) {
        self.baseline_label = format!("{} ago", self.offset.label());
        self.compare_label = "Current".to_string();
    }

    /// Get color for improvement/regression indicators
    fn delta_color(&self, improved: bool) -> Color32 {
        if improved {
            Color32::from_rgb(34, 197, 94) // Green
        } else {
            Color32::from_rgb(239, 68, 68) // Red
        }
    }

    /// Render the diff view
    pub fn show(&mut self, ui: &mut egui::Ui) -> DiffViewAction {
        let text_col = text_color(self.theme);
        let mut action = DiffViewAction::None;

        // Track button clicks
        let mut close_clicked = false;
        let mut swap_clicked = false;
        let mut preset_clicked: Option<DiffOffset> = None;

        // Header bar
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} Diff Mode", egui_phosphor::regular::GIT_DIFF))
                    .color(text_col)
                    .strong(),
            );

            ui.add_space(16.0);

            ui.label(
                RichText::new(format!("base: -{}", self.offset.label()))
                    .color(text_col.gamma_multiply(0.7))
                    .size(12.0),
            );

            ui.add_space(8.0);

            ui.label(
                RichText::new("compare: now")
                    .color(text_col.gamma_multiply(0.7))
                    .size(12.0),
            );

            // Right side controls
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Close button
                if ui
                    .small_button(RichText::new(egui_phosphor::regular::X).size(14.0))
                    .on_hover_text("Exit diff mode (:diffoff, Escape, or X)")
                    .clicked()
                {
                    close_clicked = true;
                }

                ui.add_space(8.0);

                // Swap button
                if ui
                    .small_button(
                        RichText::new(egui_phosphor::regular::ARROWS_LEFT_RIGHT).size(14.0),
                    )
                    .on_hover_text("Swap base and compare (dx)")
                    .clicked()
                {
                    swap_clicked = true;
                }

                ui.add_space(8.0);

                // Offset presets
                for preset in DiffOffset::presets() {
                    let is_selected = *preset == self.offset;
                    let btn_text = RichText::new(preset.label())
                        .size(11.0)
                        .color(if is_selected {
                            Color32::WHITE
                        } else {
                            text_col.gamma_multiply(0.7)
                        });

                    let btn = egui::Button::new(btn_text).fill(if is_selected {
                        Color32::from_rgb(59, 130, 246)
                    } else {
                        Color32::TRANSPARENT
                    });

                    if ui.add(btn).clicked() {
                        preset_clicked = Some(*preset);
                    }
                }
            });
        });

        // Handle button actions outside the closure
        if close_clicked {
            action = DiffViewAction::Close;
        } else if swap_clicked {
            self.swap();
        } else if let Some(preset) = preset_clicked {
            self.offset = preset;
            action = DiffViewAction::OffsetChanged(self.offset);
            self.update_labels();
        }

        ui.add_space(8.0);

        // Side-by-side charts
        let available = ui.available_size();
        let chart_width = (available.x - 16.0) / 2.0;
        let chart_height = available.y - 80.0; // Leave room for summary bar

        ui.horizontal(|ui| {
            // Baseline chart (left)
            ui.vertical(|ui| {
                ui.set_width(chart_width);

                // Panel header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.baseline_label)
                            .color(text_col.gamma_multiply(0.8))
                            .size(13.0),
                    );
                    ui.label(
                        RichText::new("(baseline)")
                            .color(text_col.gamma_multiply(0.5))
                            .size(11.0),
                    );
                });

                // Stats row
                self.render_stats_row(ui, &self.baseline_stats, None);

                ui.add_space(4.0);

                // Chart
                ui.allocate_ui(egui::vec2(chart_width, chart_height - 50.0), |ui| {
                    self.render_chart(ui, &self.baseline_series, "baseline");
                });
            });

            ui.add_space(16.0);

            // Compare chart (right)
            ui.vertical(|ui| {
                ui.set_width(chart_width);

                // Panel header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.compare_label)
                            .color(text_col.gamma_multiply(0.8))
                            .size(13.0),
                    );
                    ui.label(
                        RichText::new("(compare)")
                            .color(text_col.gamma_multiply(0.5))
                            .size(11.0),
                    );
                });

                // Stats row with deltas
                self.render_stats_row(ui, &self.compare_stats, Some(&self.baseline_stats));

                ui.add_space(4.0);

                // Chart
                ui.allocate_ui(egui::vec2(chart_width, chart_height - 50.0), |ui| {
                    self.render_chart(ui, &self.compare_series, "compare");
                });
            });
        });

        // Summary bar at bottom
        ui.add_space(8.0);
        self.render_summary_bar(ui);

        action
    }

    /// Render statistics row for a panel
    fn render_stats_row(
        &self,
        ui: &mut egui::Ui,
        stats: &SeriesStats,
        baseline: Option<&SeriesStats>,
    ) {
        let text_col = text_color(self.theme);
        let dim_col = text_col.gamma_multiply(0.6);

        ui.horizontal(|ui| {
            // avg
            ui.label(RichText::new("avg:").color(dim_col).size(11.0));
            ui.label(
                RichText::new(format!("{:.1}ms", stats.avg))
                    .color(text_col)
                    .size(11.0),
            );

            if let Some(base) = baseline {
                let delta = StatsDelta::new(base.avg, stats.avg, !self.lower_is_better);
                ui.label(
                    RichText::new(format!("({:+.1}%)", delta.pct))
                        .color(self.delta_color(delta.improved))
                        .size(10.0),
                );
            }

            ui.add_space(12.0);

            // p99
            ui.label(RichText::new("p99:").color(dim_col).size(11.0));
            ui.label(
                RichText::new(format!("{:.1}ms", stats.p99))
                    .color(text_col)
                    .size(11.0),
            );

            if let Some(base) = baseline {
                let delta = StatsDelta::new(base.p99, stats.p99, !self.lower_is_better);
                ui.label(
                    RichText::new(format!("({:+.1}%)", delta.pct))
                        .color(self.delta_color(delta.improved))
                        .size(10.0),
                );
            }
        });
    }

    /// Render a single chart panel
    fn render_chart(&self, ui: &mut egui::Ui, series: &[Series], id_suffix: &str) {
        if series.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("No data")
                        .color(text_color(self.theme).gamma_multiply(0.5))
                        .italics(),
                );
            });
            return;
        }

        let plot = Plot::new(format!("diff_{}_{}", self.id, id_suffix))
            .legend(egui_plot::Legend::default().position(egui_plot::Corner::RightTop))
            .show_axes(true)
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .allow_scroll(true);

        plot.show(ui, |plot_ui| {
            for (i, s) in series.iter().enumerate() {
                let color = s.color.unwrap_or_else(|| self.series_color(i));

                let points: PlotPoints<'_> =
                    s.points.iter().map(|p| [p.timestamp, p.value]).collect();

                let line = Line::new(s.label(), points)
                    .color(color)
                    .stroke(Stroke::new(2.0, color));

                plot_ui.line(line);
            }
        });
    }

    /// Render the summary bar at the bottom
    fn render_summary_bar(&self, ui: &mut egui::Ui) {
        let bg_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(245, 247, 250),
            AppTheme::Dark => Color32::from_rgb(35, 38, 45),
        };

        egui::Frame::new()
            .fill(bg_color)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Calculate deltas
                    let avg_delta = StatsDelta::new(
                        self.baseline_stats.avg,
                        self.compare_stats.avg,
                        !self.lower_is_better,
                    );
                    let p99_delta = StatsDelta::new(
                        self.baseline_stats.p99,
                        self.compare_stats.p99,
                        !self.lower_is_better,
                    );

                    // Overall assessment
                    let overall_improved = avg_delta.improved && p99_delta.improved;
                    let overall_regressed = !avg_delta.improved && !p99_delta.improved;

                    let status_icon = if overall_improved {
                        egui_phosphor::regular::CHECK_CIRCLE
                    } else if overall_regressed {
                        egui_phosphor::regular::WARNING_CIRCLE
                    } else {
                        egui_phosphor::regular::MINUS_CIRCLE
                    };

                    let status_color = if overall_improved {
                        Color32::from_rgb(34, 197, 94)
                    } else if overall_regressed {
                        Color32::from_rgb(239, 68, 68)
                    } else {
                        Color32::from_rgb(234, 179, 8)
                    };

                    let status_text = if overall_improved {
                        "improved"
                    } else if overall_regressed {
                        "regressed"
                    } else {
                        "mixed"
                    };

                    // Summary stats
                    ui.label(
                        RichText::new(format!(
                            "Δ avg: {:+.1}ms ({:+.1}%)",
                            avg_delta.abs, avg_delta.pct
                        ))
                        .color(self.delta_color(avg_delta.improved))
                        .size(12.0),
                    );

                    ui.add_space(24.0);

                    ui.label(
                        RichText::new(format!(
                            "Δ p99: {:+.1}ms ({:+.1}%)",
                            p99_delta.abs, p99_delta.pct
                        ))
                        .color(self.delta_color(p99_delta.improved))
                        .size(12.0),
                    );

                    // Right side: overall status
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{status_icon} {status_text}"))
                                .color(status_color)
                                .size(13.0)
                                .strong(),
                        );
                    });
                });
            });
    }

    /// Get a default color for series index
    fn series_color(&self, index: usize) -> Color32 {
        const PALETTE: &[Color32] = &[
            Color32::from_rgb(59, 130, 246), // Blue
            Color32::from_rgb(16, 185, 129), // Green
            Color32::from_rgb(245, 158, 11), // Amber
            Color32::from_rgb(239, 68, 68),  // Red
            Color32::from_rgb(139, 92, 246), // Purple
            Color32::from_rgb(236, 72, 153), // Pink
        ];
        PALETTE[index % PALETTE.len()]
    }
}

/// Actions that can result from diff view interaction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiffViewAction {
    /// No action
    None,
    /// Close diff view
    Close,
    /// Offset was changed
    OffsetChanged(DiffOffset),
    /// Navigate to next diff (when comparing multiple metrics)
    NextDiff,
    /// Navigate to previous diff
    PrevDiff,
}

/// Implement Component trait
impl super::Component for DiffView {
    fn show(&mut self, ui: &mut egui::Ui) {
        DiffView::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        format!("Diff: {}", self.metric_name)
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn set_api_key(&mut self, _key: &str) {}

    fn set_staging_api_key(&mut self, _key: &str) {}

    fn label(&self) -> egui::RichText {
        egui::RichText::new(format!(
            "{} {} ({})",
            egui_phosphor::regular::GIT_DIFF,
            self.metric_name,
            self.offset.label()
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_offset_parse() {
        assert_eq!(DiffOffset::parse("-7d"), Some(DiffOffset::OneWeek));
        assert_eq!(DiffOffset::parse("1h"), Some(DiffOffset::OneHour));
        assert_eq!(DiffOffset::parse("30d"), Some(DiffOffset::OneMonth));

        // Custom values
        if let Some(DiffOffset::Custom(s)) = DiffOffset::parse("2h") {
            assert_eq!(s, 2 * 3600);
        } else {
            panic!("Expected Custom offset");
        }

        if let Some(DiffOffset::Custom(s)) = DiffOffset::parse("3d") {
            assert_eq!(s, 3 * 86400);
        } else {
            panic!("Expected Custom offset");
        }
    }

    #[test]
    fn test_series_stats() {
        let points = vec![
            DataPoint {
                timestamp: 0.0,
                value: 10.0,
            },
            DataPoint {
                timestamp: 1.0,
                value: 20.0,
            },
            DataPoint {
                timestamp: 2.0,
                value: 30.0,
            },
            DataPoint {
                timestamp: 3.0,
                value: 40.0,
            },
            DataPoint {
                timestamp: 4.0,
                value: 50.0,
            },
        ];

        let stats = SeriesStats::from_points(&points);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 50.0);
        assert_eq!(stats.avg, 30.0);
        assert_eq!(stats.last, 50.0);
        assert_eq!(stats.count, 5);
    }

    #[test]
    fn test_stats_delta() {
        // Lower is better (latency)
        let delta = StatsDelta::new(100.0, 80.0, false);
        assert!(delta.improved);
        assert_eq!(delta.abs, -20.0);
        assert_eq!(delta.pct, -20.0);

        // Higher is better (throughput)
        let delta = StatsDelta::new(100.0, 120.0, true);
        assert!(delta.improved);
        assert_eq!(delta.abs, 20.0);
        assert_eq!(delta.pct, 20.0);
    }
}
