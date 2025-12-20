//! Benchmarks for workspace encoding/decoding.
//!
//! These benchmarks measure the performance of workspace serialization
//! which is critical for URL sharing and workspace persistence.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use enya_editor::workspace::{
    LayoutConfig, LayoutContainer, LayoutNode, LayoutType, PaneConfig, WorkspaceConfig,
};

/// Generate a workspace with the given number of panes
fn generate_workspace(pane_count: usize, with_layout: bool) -> WorkspaceConfig {
    let mut ws = WorkspaceConfig::new("benchmark-workspace");
    ws.view.theme = "dark".to_string();
    ws.time.preset = "1h".to_string();

    for i in 0..pane_count {
        let pane = PaneConfig::new(format!(
            "rate(http_requests_total{{service=\"api-{i}\",env=\"prod\"}}[5m])"
        ))
        .with_name(format!("API Service {i}"))
        .with_tag(if i % 3 == 0 { "Critical" } else { "" });

        ws.panes.push(pane);
    }

    if with_layout && pane_count >= 2 {
        // Create a horizontal split layout
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: (0..pane_count).map(LayoutNode::Pane).collect(),
            shares: Vec::new(),
        });
    }

    ws
}

/// Generate a workspace with nested layout
fn generate_nested_layout_workspace(depth: usize) -> WorkspaceConfig {
    let mut ws = WorkspaceConfig::new("nested-benchmark");
    ws.view.theme = "dark".to_string();

    // Add enough panes for the nested structure
    let pane_count = 2_usize.pow(depth as u32);
    for i in 0..pane_count {
        ws.panes.push(PaneConfig::new(format!("metric_{i}")));
    }

    // Build nested layout
    fn build_nested(start: usize, count: usize, depth: usize) -> LayoutNode {
        if depth == 0 || count == 1 {
            LayoutNode::Pane(start)
        } else {
            let half = count / 2;
            let layout_type = if depth % 2 == 0 {
                LayoutType::Horizontal
            } else {
                LayoutType::Vertical
            };
            LayoutNode::Container(LayoutContainer {
                layout_type,
                children: vec![
                    build_nested(start, half, depth - 1),
                    build_nested(start + half, count - half, depth - 1),
                ],
                shares: Vec::new(),
            })
        }
    }

    if let LayoutNode::Container(container) = build_nested(0, pane_count, depth) {
        ws.layout = Some(LayoutConfig {
            layout_type: container.layout_type,
            children: container.children,
            shares: container.shares,
        });
    }

    ws
}

fn bench_workspace_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("workspace_encode");

    for pane_count in [1, 3, 5, 10] {
        let ws = generate_workspace(pane_count, false);

        group.bench_with_input(BenchmarkId::new("to_base64", pane_count), &ws, |b, ws| {
            b.iter(|| ws.to_base64().unwrap());
        });
    }

    // Benchmark with layout
    for pane_count in [2, 4, 8] {
        let ws = generate_workspace(pane_count, true);

        group.bench_with_input(
            BenchmarkId::new("to_base64_with_layout", pane_count),
            &ws,
            |b, ws| {
                b.iter(|| ws.to_base64().unwrap());
            },
        );
    }

    group.finish();
}

fn bench_workspace_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("workspace_decode");

    for pane_count in [1, 3, 5, 10] {
        let ws = generate_workspace(pane_count, false);
        let encoded = ws.to_base64().unwrap();

        group.bench_with_input(
            BenchmarkId::new("from_base64", pane_count),
            &encoded,
            |b, encoded| {
                b.iter(|| WorkspaceConfig::from_base64(encoded).unwrap());
            },
        );
    }

    // Benchmark with layout
    for pane_count in [2, 4, 8] {
        let ws = generate_workspace(pane_count, true);
        let encoded = ws.to_base64().unwrap();

        group.bench_with_input(
            BenchmarkId::new("from_base64_with_layout", pane_count),
            &encoded,
            |b, encoded| {
                b.iter(|| WorkspaceConfig::from_base64(encoded).unwrap());
            },
        );
    }

    group.finish();
}

fn bench_toml_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("toml_serialization");

    for pane_count in [1, 5, 10] {
        let ws = generate_workspace(pane_count, false);

        group.bench_with_input(BenchmarkId::new("to_toml", pane_count), &ws, |b, ws| {
            b.iter(|| ws.to_toml().unwrap());
        });

        let toml = ws.to_toml().unwrap();
        group.bench_with_input(
            BenchmarkId::new("from_toml", pane_count),
            &toml,
            |b, toml| {
                b.iter(|| WorkspaceConfig::from_toml(toml).unwrap());
            },
        );
    }

    group.finish();
}

fn bench_nested_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_layout");

    for depth in [2, 3, 4] {
        let ws = generate_nested_layout_workspace(depth);

        group.bench_with_input(BenchmarkId::new("encode_depth", depth), &ws, |b, ws| {
            b.iter(|| ws.to_base64().unwrap());
        });

        let encoded = ws.to_base64().unwrap();
        group.bench_with_input(
            BenchmarkId::new("decode_depth", depth),
            &encoded,
            |b, encoded| {
                b.iter(|| WorkspaceConfig::from_base64(encoded).unwrap());
            },
        );
    }

    group.finish();
}

fn bench_single_pane_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_pane");

    // Create a workspace with varying query lengths
    let query_lengths = [50, 100, 200];

    for len in query_lengths {
        let query = format!(
            "rate(http_requests_total{{{}}}[5m])",
            (0..len / 20)
                .map(|i| format!("label{i}=\"value{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        );

        let mut ws = WorkspaceConfig::new("test");
        ws.panes
            .push(PaneConfig::new(&query).with_name("Test Pane"));

        group.bench_with_input(BenchmarkId::new("pane_to_base64", len), &ws, |b, ws| {
            b.iter(|| ws.pane_to_base64(0).unwrap());
        });
    }

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    for pane_count in [1, 5, 10] {
        let ws = generate_workspace(pane_count, true);

        group.bench_with_input(
            BenchmarkId::new("encode_decode", pane_count),
            &ws,
            |b, ws| {
                b.iter(|| {
                    let encoded = ws.to_base64().unwrap();
                    WorkspaceConfig::from_base64(&encoded).unwrap()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_workspace_encode,
    bench_workspace_decode,
    bench_toml_serialization,
    bench_nested_layout,
    bench_single_pane_encode,
    bench_roundtrip,
);
criterion_main!(benches);
