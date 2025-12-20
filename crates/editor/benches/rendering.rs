//! Benchmarks for egui component rendering using headless mode.
//!
//! These benchmarks measure the CPU-side rendering performance of UI components
//! without GPU interaction. This helps identify expensive layout/paint operations.

use std::collections::HashMap;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use egui::{CentralPanel, Context, RawInput};
use enya_editor::components::{
    DataPoint, Notification, NotificationLevel, NotificationManager, Series, Sparkline, StatusLine,
    StatusMode, TimeSeriesChart,
};

/// Run an egui frame with the given closure
fn run_egui_frame<F>(ctx: &Context, mut f: F)
where
    F: FnMut(&mut egui::Ui),
{
    let _ = ctx.run(RawInput::default(), |ctx| {
        CentralPanel::default().show(ctx, |ui| f(ui));
    });
}

/// Generate test time series data
fn generate_series(point_count: usize, series_count: usize) -> Vec<Series> {
    let now = 1700000000.0; // Fixed timestamp for reproducibility
    let step = 60.0; // 1 minute intervals

    (0..series_count)
        .map(|s| {
            let points: Vec<DataPoint> = (0..point_count)
                .map(|i| {
                    let timestamp = now - (point_count - i) as f64 * step;
                    // Generate some realistic-looking data with variation per series
                    let base = 50.0 + (s as f64 * 10.0);
                    let noise = ((i as f64 * 0.1 + s as f64).sin() * 20.0)
                        + ((i as f64 * 0.05).cos() * 10.0);
                    DataPoint {
                        timestamp,
                        value: base + noise,
                    }
                })
                .collect();

            let mut tags = HashMap::new();
            tags.insert("host".to_string(), format!("server-{s}"));
            tags.insert("env".to_string(), "prod".to_string());

            Series::new(format!("http_requests_total_{s}"))
                .with_points(points)
                .with_tags_map(tags)
        })
        .collect()
}

fn bench_status_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("status_line");
    let ctx = Context::default();

    // Minimal status line
    group.bench_function("minimal", |b| {
        let status_line = StatusLine::new();
        b.iter(|| {
            run_egui_frame(&ctx, |ui| {
                status_line.show(ui);
            });
        });
    });

    // Fully populated status line
    group.bench_function("full", |b| {
        let mut status_line = StatusLine::new();
        status_line.set_mode(StatusMode::Normal);
        status_line.set_connected(true);
        status_line.set_open_tabs(5);
        status_line.set_selected_metric(Some("http_requests_total".to_string()));
        status_line.set_branch_info(Some("main".to_string()));
        status_line.set_viewport_info(Some("2x2 grid".to_string()));
        status_line.set_extra_status(Some("3 files modified".to_string()));
        status_line.set_diagnostics_count(2, 5, 10);
        status_line.mark_refresh();

        b.iter(|| {
            run_egui_frame(&ctx, |ui| {
                status_line.show(ui);
            });
        });
    });

    // Status line with sparkline (most expensive case)
    group.bench_function("with_sparkline", |b| {
        let mut status_line = StatusLine::new();
        status_line.set_mode(StatusMode::Normal);
        status_line.set_connected(true);

        let mut sparkline = Sparkline::new("fps").with_unit("").with_bounds(0.0, 100.0);
        for i in 0..15 {
            sparkline.push(50.0 + (i as f64 * 3.0).sin() * 20.0);
        }
        status_line.set_sparkline(Some(sparkline));

        b.iter(|| {
            run_egui_frame(&ctx, |ui| {
                status_line.show(ui);
            });
        });
    });

    // Different modes
    for mode in [
        StatusMode::Normal,
        StatusMode::Command,
        StatusMode::Search,
        StatusMode::Zen,
    ] {
        group.bench_with_input(BenchmarkId::new("mode", mode.label()), &mode, |b, &mode| {
            let mut status_line = StatusLine::new();
            status_line.set_mode(mode);
            status_line.set_connected(true);
            status_line.set_open_tabs(3);

            b.iter(|| {
                run_egui_frame(&ctx, |ui| {
                    status_line.show(ui);
                });
            });
        });
    }

    group.finish();
}

fn bench_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("notifications");
    let ctx = Context::default();

    // Empty notification manager (fast path)
    group.bench_function("empty", |b| {
        let mut manager = NotificationManager::new();
        b.iter(|| {
            manager.show(&ctx);
        });
    });

    // Single notification
    group.bench_function("single", |b| {
        b.iter_batched(
            || {
                let mut manager = NotificationManager::new();
                manager.info("Test notification");
                manager
            },
            |mut manager| {
                manager.show(&ctx);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Multiple notifications (typical case)
    for count in [1, 3, 5] {
        group.bench_with_input(BenchmarkId::new("count", count), &count, |b, &count| {
            b.iter_batched(
                || {
                    let mut manager = NotificationManager::new();
                    for i in 0..count {
                        let level = match i % 4 {
                            0 => NotificationLevel::Info,
                            1 => NotificationLevel::Success,
                            2 => NotificationLevel::Warn,
                            _ => NotificationLevel::Error,
                        };
                        manager.notify(
                            Notification::new(format!("Notification {i}"), level)
                                .with_message("This is a test message body"),
                        );
                    }
                    manager
                },
                |mut manager| {
                    manager.show(&ctx);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_time_series_chart(c: &mut Criterion) {
    let mut group = c.benchmark_group("time_series_chart");
    let ctx = Context::default();

    // Chart rendering with different data sizes
    for (points, series_count) in [(60, 1), (300, 1), (300, 5), (1000, 10)] {
        let label = format!("{points}pts_{series_count}series");
        let data = generate_series(points, series_count);

        group.bench_with_input(BenchmarkId::new("render", &label), &data, |b, data| {
            let mut chart = TimeSeriesChart::new("benchmark_metric");
            for series in data.clone() {
                chart.add_series(series);
            }

            b.iter(|| {
                run_egui_frame(&ctx, |ui| {
                    // Allocate a reasonable chart area
                    ui.set_min_size(egui::vec2(800.0, 400.0));
                    chart.show(ui);
                });
            });
        });
    }

    // Chart with demo data (includes commit markers)
    group.bench_function("with_demo_data", |b| {
        let mut chart = TimeSeriesChart::with_demo_data("benchmark_metric");
        chart.set_show_commits(true);

        b.iter(|| {
            run_egui_frame(&ctx, |ui| {
                ui.set_min_size(egui::vec2(800.0, 400.0));
                chart.show(ui);
            });
        });
    });

    group.finish();
}

fn bench_sparkline_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparkline");
    let ctx = Context::default();

    // Sparkline string rendering (pure CPU)
    for point_count in [5, 10, 15] {
        group.bench_with_input(
            BenchmarkId::new("render_string", point_count),
            &point_count,
            |b, &count| {
                let mut sparkline = Sparkline::new("test").with_bounds(0.0, 100.0);
                for i in 0..count {
                    sparkline.push(50.0 + (i as f64 * 0.5).sin() * 30.0);
                }

                b.iter(|| sparkline.render());
            },
        );
    }

    // Sparkline in status line context
    group.bench_function("in_status_line", |b| {
        let mut status_line = StatusLine::new();
        let mut sparkline = Sparkline::new("fps").with_unit("").with_bounds(0.0, 100.0);
        for i in 0..15 {
            sparkline.push(60.0 + (i as f64 * 0.3).sin() * 10.0);
        }
        status_line.set_sparkline(Some(sparkline));

        b.iter(|| {
            run_egui_frame(&ctx, |ui| {
                status_line.show(ui);
            });
        });
    });

    group.finish();
}

fn bench_egui_context_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("egui_overhead");
    let ctx = Context::default();

    // Baseline: empty frame
    group.bench_function("empty_frame", |b| {
        b.iter(|| {
            let _ = ctx.run(RawInput::default(), |_ctx| {
                // Do nothing
            });
        });
    });

    // Single panel
    group.bench_function("single_panel", |b| {
        b.iter(|| {
            run_egui_frame(&ctx, |_ui| {
                // Empty panel
            });
        });
    });

    // Panel with label
    group.bench_function("panel_with_label", |b| {
        b.iter(|| {
            run_egui_frame(&ctx, |ui| {
                ui.label("Hello, World!");
            });
        });
    });

    // Panel with horizontal layout
    group.bench_function("horizontal_layout", |b| {
        b.iter(|| {
            run_egui_frame(&ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Left");
                    ui.label("Center");
                    ui.label("Right");
                });
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_status_line,
    bench_notifications,
    bench_time_series_chart,
    bench_sparkline_rendering,
    bench_egui_context_overhead,
);
criterion_main!(benches);
