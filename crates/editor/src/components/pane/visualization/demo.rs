//! Demo data population for visualizations.
//!
//! This module provides functions to populate visualizations with
//! demo data when not connected to a real data source.

use egui::Color32;

use crate::ui::palette;

use super::super::time_series_chart::{CommitMarker, DataPoint, Series, TimeSeriesChart};

use super::Threshold;
use super::Visualization;
use super::bar::{Bar, BarChartViz};
use super::gauge::GaugeChart;
use super::sparkline::SparklineViz;
use super::stat::StatChart;

// Import heatmap demo function from its module
use super::heatmap::populate_heatmap_demo;

/// Populate demo data for a visualization based on its type
pub fn populate_demo_data(viz: &mut Visualization, query: &str) {
    match viz {
        Visualization::TimeSeries(chart) => {
            populate_time_series_demo(chart, query);
        }
        Visualization::Stat(stat) => {
            populate_stat_demo(stat, query);
        }
        Visualization::Gauge(gauge) => {
            populate_gauge_demo(gauge, query);
        }
        Visualization::BarChart(bar) => {
            populate_bar_chart_demo(bar, query);
        }
        Visualization::Sparkline(spark) => {
            populate_sparkline_demo(spark, query);
        }
        Visualization::Heatmap(heatmap) => {
            populate_heatmap_demo(heatmap, query);
        }
    }
}

/// Populate demo data for time series
fn populate_time_series_demo(chart: &mut TimeSeriesChart, query: &str) {
    // Check if this is a "many series" demo query (e.g., by endpoint, by method, etc.)
    let query_lower = query.to_lowercase();
    if query_lower.contains("by_endpoint")
        || query_lower.contains("by endpoint")
        || query_lower.contains("by_method")
    {
        populate_many_series_demo(chart, query);
        return;
    }

    // Generate some demo data based on query hash for variety
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));
    let now = 1_700_000_000.0;
    let duration = 86400.0;
    let num_points = 240;

    // Series 1
    let base1 = 50.0 + (hash % 50) as f64;
    let freq1 = 200.0 + (hash % 100) as f64;
    let points1: Vec<DataPoint> = (0..num_points)
        .map(|i| {
            let t = now + (i as f64 / num_points as f64) * duration;
            let base = base1 + 20.0 * (t / freq1).sin();
            let noise = (t * 17.0).sin() * 5.0;
            DataPoint {
                timestamp: t,
                value: base + noise,
            }
        })
        .collect();

    chart.add_series(
        Series::new(query)
            .with_tag("host", "server1")
            .with_points(points1)
            .with_color(Color32::from_rgb(59, 130, 246)),
    );

    // Series 2
    let base2 = 70.0 + (hash % 30) as f64;
    let freq2 = 150.0 + (hash % 80) as f64;
    let points2: Vec<DataPoint> = (0..num_points)
        .map(|i| {
            let t = now + (i as f64 / num_points as f64) * duration;
            let base = base2 + 15.0 * (t / freq2).cos();
            let noise = (t * 23.0).sin() * 3.0;
            DataPoint {
                timestamp: t,
                value: base + noise,
            }
        })
        .collect();

    chart.add_series(
        Series::new(query)
            .with_tag("host", "server2")
            .with_points(points2)
            .with_color(Color32::from_rgb(16, 185, 129)),
    );

    // Add demo commit markers
    chart.add_commit(CommitMarker::new(
        "a1b2c3d",
        now + duration * 0.1,
        "Fix connection pooling",
    ));
    chart.add_commit(CommitMarker::new(
        "e4f5g6h",
        now + duration * 0.35,
        "Add retry logic",
    ));
    chart.add_commit(CommitMarker::new(
        "i7j8k9l",
        now + duration * 0.5,
        "Update dependencies",
    ));
    chart.add_commit(CommitMarker::new(
        "m0n1o2p",
        now + duration * 0.7,
        "Refactor auth module",
    ));
    chart.add_commit(CommitMarker::new(
        "q3r4s5t",
        now + duration * 0.9,
        "Performance improvements",
    ));
}

/// Populate demo data for stat visualization
fn populate_stat_demo(stat: &mut StatChart, query: &str) {
    // Generate a demo value based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Generate a reasonable value
    let base_value = 1000.0 + (hash % 50000) as f64;
    stat.set_value(base_value);

    // Set unit based on common metric patterns
    let unit = if query.contains("latency") || query.contains("duration") {
        "ms"
    } else if query.contains("rate") || query.contains("percent") {
        "%"
    } else if query.contains("bytes") || query.contains("size") {
        "bytes"
    } else {
        "" // No unit
    };
    stat.set_unit(unit);

    // Generate sparkline data (last 24 data points)
    let sparkline: Vec<f64> = (0..24)
        .map(|i| ((i as f64 * 0.3 + hash as f64 * 0.01).sin() * 0.2 + 1.0) * base_value)
        .collect();
    stat.set_sparkline_data(sparkline);

    // Set change indicator
    let change = ((hash % 200) as f64 - 100.0) / 10.0; // -10% to +10%
    stat.set_change(change, "vs last hour");

    // Add some thresholds for visual interest
    stat.add_threshold(Threshold::new(base_value * 0.8, palette::semantic::WARNING));
    stat.add_threshold(Threshold::new(base_value * 1.2, palette::semantic::ERROR));
}

/// Populate demo data for gauge visualization
fn populate_gauge_demo(gauge: &mut GaugeChart, query: &str) {
    // Generate a demo value based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Determine if this looks like a percentage metric
    let is_percentage = query.contains("percent")
        || query.contains("utilization")
        || query.contains("usage")
        || query.contains("cpu")
        || query.contains("memory");

    if is_percentage {
        // Percentage gauge (0-100%)
        gauge.set_range(0.0, 100.0);
        gauge.set_unit("%");
        let value = (hash % 85) as f64 + 15.0; // 15-100%
        gauge.set_value(value);

        // Traffic light thresholds for utilization
        gauge.add_threshold(Threshold::new(70.0, palette::semantic::WARNING));
        gauge.add_threshold(Threshold::new(90.0, palette::semantic::ERROR));
    } else {
        // Generic gauge with custom range
        let max_val = 1000.0 + (hash % 9000) as f64;
        gauge.set_range(0.0, max_val);

        // Set unit based on metric patterns
        let unit = if query.contains("latency") || query.contains("duration") {
            "ms"
        } else if query.contains("bytes") || query.contains("size") {
            "MB"
        } else if query.contains("rate") || query.contains("rps") {
            "req/s"
        } else {
            ""
        };
        gauge.set_unit(unit);

        // Value somewhere in the range
        let value = (hash % (max_val as u64)) as f64;
        gauge.set_value(value);

        // Thresholds at 70% and 90% of max
        gauge.add_threshold(Threshold::new(max_val * 0.7, palette::semantic::WARNING));
        gauge.add_threshold(Threshold::new(max_val * 0.9, palette::semantic::ERROR));
    }
}

/// Populate demo data for bar chart visualization
fn populate_bar_chart_demo(bar: &mut BarChartViz, query: &str) {
    // Generate demo data based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Generate category names based on query content
    let categories: Vec<&str> = if query.contains("region") || query.contains("location") {
        vec![
            "us-east-1",
            "us-west-2",
            "eu-west-1",
            "ap-south-1",
            "ap-northeast-1",
        ]
    } else if query.contains("service") || query.contains("app") {
        vec![
            "api-gateway",
            "auth-service",
            "db-primary",
            "cache",
            "worker",
        ]
    } else if query.contains("host") || query.contains("server") {
        vec![
            "server-01",
            "server-02",
            "server-03",
            "server-04",
            "server-05",
        ]
    } else if query.contains("status") || query.contains("code") {
        vec![
            "200 OK",
            "201 Created",
            "400 Bad Request",
            "404 Not Found",
            "500 Error",
        ]
    } else {
        vec![
            "Category A",
            "Category B",
            "Category C",
            "Category D",
            "Category E",
        ]
    };

    // Generate values with some variation
    let bars: Vec<Bar> = categories
        .iter()
        .enumerate()
        .map(|(i, &label)| {
            let base = 100.0 + (hash % 900) as f64;
            let variation = ((hash.wrapping_add(i as u64 * 17)) % 100) as f64 / 100.0;
            let value = base * (0.3 + variation * 0.7);
            Bar::new(label, value)
        })
        .collect();

    bar.set_bars(bars);
}

/// Populate demo data for sparkline visualization
fn populate_sparkline_demo(spark: &mut SparklineViz, query: &str) {
    // Generate demo data based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    // Generate 50 data points with some variation
    let base_value = 100.0 + (hash % 500) as f64;
    let data: Vec<f64> = (0..50)
        .map(|i| {
            let trend = (i as f64 * 0.02).sin() * 0.15; // Slight trend
            let noise = ((hash.wrapping_add(i as u64) % 100) as f64 - 50.0) / 200.0; // Random noise
            let seasonal = ((i as f64 * 0.2 + hash as f64 * 0.01).sin()) * 0.1; // Seasonal pattern
            base_value * (1.0 + trend + noise + seasonal)
        })
        .collect();

    spark.set_data(data);
}

/// Populate demo data with many series (12 API endpoints) for testing legend overflow
fn populate_many_series_demo(chart: &mut TimeSeriesChart, query: &str) {
    let now = 1_700_000_000.0;
    let duration = 86400.0;
    let num_points = 120;

    // API endpoints for the demo
    let endpoints = [
        "/api/users",
        "/api/orders",
        "/api/products",
        "/api/auth/login",
        "/api/auth/logout",
        "/api/cart",
        "/api/checkout",
        "/api/search",
        "/api/inventory",
        "/api/payments",
        "/api/webhooks",
        "/api/notifications",
    ];

    // Colors for each series
    let colors = [
        Color32::from_rgb(99, 179, 237),  // Sky blue
        Color32::from_rgb(129, 140, 248), // Indigo
        Color32::from_rgb(94, 234, 212),  // Teal
        Color32::from_rgb(192, 132, 252), // Purple
        Color32::from_rgb(251, 191, 36),  // Amber
        Color32::from_rgb(244, 114, 182), // Pink
        Color32::from_rgb(52, 211, 153),  // Emerald
        Color32::from_rgb(248, 113, 113), // Coral
        Color32::from_rgb(163, 230, 53),  // Lime
        Color32::from_rgb(251, 146, 60),  // Orange
        Color32::from_rgb(147, 197, 253), // Light blue
        Color32::from_rgb(196, 181, 253), // Light purple
    ];

    for (i, endpoint) in endpoints.iter().enumerate() {
        let hash = endpoint
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        let base = 20.0 + (hash % 80) as f64;
        let freq = 100.0 + (hash % 200) as f64;
        let phase = (i as f64) * 0.5;

        let points: Vec<DataPoint> = (0..num_points)
            .map(|j| {
                let t = now + (j as f64 / num_points as f64) * duration;
                let value = base + 15.0 * ((t / freq) + phase).sin() + (t * 13.0).sin() * 3.0;
                DataPoint {
                    timestamp: t,
                    value: value.max(0.0),
                }
            })
            .collect();

        chart.add_series(
            Series::new(query)
                .with_tag("endpoint", *endpoint)
                .with_points(points)
                .with_color(colors[i % colors.len()]),
        );
    }
}
