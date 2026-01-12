//! Benchmarks for LogQL completion.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use enya_logql::{analyze, scan_until, syntax_suggestions, validate};

fn bench_scan_until(c: &mut Criterion) {
    let query = r#"{app="nginx", env="prod"} |= "error" | json | level="error""#;

    c.bench_function("scan_until", |b| {
        b.iter(|| scan_until(black_box(query), query.len()));
    });
}

fn bench_analyze(c: &mut Criterion) {
    let query = r#"{app="nginx", env="prod"} |= "error" | json | level="error""#;

    c.bench_function("analyze", |b| {
        b.iter(|| analyze(black_box(query), query.len()));
    });
}

fn bench_suggestions(c: &mut Criterion) {
    c.bench_function("syntax_suggestions", |b| {
        let ctx = analyze("{app=\"nginx\"} | ", 16);
        b.iter(|| {
            let _: Vec<_> = syntax_suggestions(black_box(&ctx)).collect();
        });
    });
}

fn bench_validate(c: &mut Criterion) {
    let query = r#"sum(rate({app="nginx", env="prod"} |= "error" [5m])) by (level)"#;

    c.bench_function("validate", |b| {
        b.iter(|| validate(black_box(query)));
    });
}

criterion_group!(
    benches,
    bench_scan_until,
    bench_analyze,
    bench_suggestions,
    bench_validate
);
criterion_main!(benches);
