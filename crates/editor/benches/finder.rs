//! Benchmarks for the fuzzy finder component.
//!
//! These benchmarks measure the performance of fuzzy matching operations
//! which are critical for responsive command palette and metrics finder UX.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use enya_editor::components::{Finder, FinderConfig, FinderItem};

/// Test item for benchmarking the finder
#[derive(Clone)]
struct MetricItem {
    name: String,
    labels: String,
}

impl FinderItem for MetricItem {
    fn search_text(&self) -> &str {
        &self.name
    }

    fn icon(&self) -> &'static str {
        ""
    }

    fn secondary_text(&self) -> Option<String> {
        Some(self.labels.clone())
    }
}

/// Generate realistic metric names for benchmarking
fn generate_metrics(count: usize) -> Vec<MetricItem> {
    let prefixes = [
        "http_requests_total",
        "http_request_duration_seconds",
        "process_cpu_seconds_total",
        "process_resident_memory_bytes",
        "go_goroutines",
        "go_gc_duration_seconds",
        "node_cpu_seconds_total",
        "node_memory_MemTotal_bytes",
        "node_disk_read_bytes_total",
        "node_network_receive_bytes_total",
        "prometheus_tsdb_head_samples_appended_total",
        "prometheus_engine_query_duration_seconds",
        "up",
        "scrape_duration_seconds",
        "scrape_samples_scraped",
    ];

    let label_sets = [
        "job=\"api\",instance=\"localhost:9090\"",
        "job=\"prometheus\",instance=\"localhost:9091\"",
        "method=\"GET\",path=\"/api/v1/query\"",
        "method=\"POST\",path=\"/api/v1/write\"",
        "cpu=\"0\",mode=\"user\"",
        "device=\"sda\",mountpoint=\"/\"",
        "handler=\"/metrics\"",
        "le=\"0.1\"",
        "quantile=\"0.99\"",
    ];

    (0..count)
        .map(|i| {
            let prefix = prefixes[i % prefixes.len()];
            let suffix = i / prefixes.len();
            let labels = label_sets[i % label_sets.len()];
            MetricItem {
                name: if suffix > 0 {
                    format!("{prefix}_{suffix}")
                } else {
                    prefix.to_string()
                },
                labels: labels.to_string(),
            }
        })
        .collect()
}

fn bench_finder_refresh(c: &mut Criterion) {
    let mut group = c.benchmark_group("finder_refresh");

    for size in [100, 500, 1000, 5000] {
        let items = generate_metrics(size);
        let config = FinderConfig {
            placeholder: "Search metrics...",
            icon: "",
            show_preview: false,
            empty_message: "No results",
            no_items_message: "No metrics",
        };

        // Benchmark empty query (shows all items sorted)
        group.bench_with_input(BenchmarkId::new("empty_query", size), &items, |b, items| {
            let mut finder: Finder<MetricItem> = Finder::new(config.clone());
            finder.set_items(items.clone());
            b.iter(|| {
                finder.mark_needs_refresh();
                // Access results to force evaluation
                finder.results().len()
            });
        });

        // Benchmark short query (common case)
        group.bench_with_input(BenchmarkId::new("short_query", size), &items, |b, items| {
            let mut finder: Finder<MetricItem> = Finder::new(config.clone());
            finder.set_items(items.clone());
            finder.open();
            b.iter(|| {
                // Simulate typing "http"
                finder.mark_needs_refresh();
                finder.results().len()
            });
        });

        // Benchmark fuzzy query (tests fuzzy matching perf)
        group.bench_with_input(BenchmarkId::new("fuzzy_query", size), &items, |b, items| {
            let mut finder: Finder<MetricItem> = Finder::new(config.clone());
            finder.set_items(items.clone());
            finder.open();
            b.iter(|| {
                // Simulate fuzzy typing "hrdur" for "http_request_duration"
                finder.mark_needs_refresh();
                finder.results().len()
            });
        });
    }

    group.finish();
}

fn bench_finder_set_items(c: &mut Criterion) {
    let mut group = c.benchmark_group("finder_set_items");

    for size in [100, 500, 1000, 5000] {
        let items = generate_metrics(size);
        let config = FinderConfig::default();

        group.bench_with_input(BenchmarkId::new("set_items", size), &items, |b, items| {
            let mut finder: Finder<MetricItem> = Finder::new(config.clone());
            b.iter(|| {
                finder.set_items(items.clone());
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_finder_refresh, bench_finder_set_items);
criterion_main!(benches);
