//! Benchmarks for enya-promql crate.
//!
//! Run with: `cargo bench -p enya-promql`

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use enya_promql::{analyze, scan_until, syntax_suggestions, validate};

/// Sample queries of varying complexity for benchmarking.
mod queries {
    /// Simple metric name
    pub const SIMPLE: &str = "http_requests_total";

    /// Metric with label selector
    pub const SELECTOR: &str = r#"http_requests_total{method="GET", status="200"}"#;

    /// Rate function with duration
    pub const RATE: &str = "rate(http_requests_total[5m])";

    /// Aggregation with grouping
    pub const AGGREGATION: &str = "sum(rate(http_requests_total[5m])) by (method)";

    /// Complex query with multiple functions and labels
    pub const COMPLEX: &str = r#"sum(rate(http_requests_total{job="api", method=~"GET|POST"}[5m])) by (method, status) / ignoring(status) group_left sum(rate(http_requests_total[5m])) by (method)"#;

    /// Very long query for stress testing
    pub const LONG: &str = r#"histogram_quantile(0.99, sum(rate(http_request_duration_seconds_bucket{job="api-server", instance=~"api-.*", method="GET", handler="/api/v1/users", status="200"}[5m])) by (le, method, handler)) / histogram_quantile(0.50, sum(rate(http_request_duration_seconds_bucket{job="api-server", instance=~"api-.*", method="GET", handler="/api/v1/users", status="200"}[5m])) by (le, method, handler))"#;
}

/// Benchmark the lexer's scan_until function.
fn bench_scan_until(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer/scan_until");

    group.bench_function("simple", |b| {
        b.iter(|| scan_until(black_box(queries::SIMPLE), queries::SIMPLE.len()));
    });

    group.bench_function("selector", |b| {
        b.iter(|| scan_until(black_box(queries::SELECTOR), queries::SELECTOR.len()));
    });

    group.bench_function("rate", |b| {
        b.iter(|| scan_until(black_box(queries::RATE), queries::RATE.len()));
    });

    group.bench_function("aggregation", |b| {
        b.iter(|| scan_until(black_box(queries::AGGREGATION), queries::AGGREGATION.len()));
    });

    group.bench_function("complex", |b| {
        b.iter(|| scan_until(black_box(queries::COMPLEX), queries::COMPLEX.len()));
    });

    group.bench_function("long", |b| {
        b.iter(|| scan_until(black_box(queries::LONG), queries::LONG.len()));
    });

    group.finish();
}

/// Benchmark completion context analysis.
fn bench_analyze(c: &mut Criterion) {
    let mut group = c.benchmark_group("completion/analyze");

    // Empty input (start of query)
    group.bench_function("empty", |b| {
        b.iter(|| analyze(black_box(""), 0));
    });

    // Typing a function name
    group.bench_function("typing_function", |b| {
        b.iter(|| analyze(black_box("rat"), 3));
    });

    // Inside a selector
    group.bench_function("in_selector", |b| {
        let query = "http_requests{method=";
        b.iter(|| analyze(black_box(query), query.len()));
    });

    // Inside a label value
    group.bench_function("in_label_value", |b| {
        let query = r#"http_requests{method="GE"#;
        b.iter(|| analyze(black_box(query), query.len()));
    });

    // Inside a duration
    group.bench_function("in_duration", |b| {
        let query = "rate(x[5";
        b.iter(|| analyze(black_box(query), query.len()));
    });

    // After aggregation, expecting modifier
    group.bench_function("expect_modifier", |b| {
        let query = "sum(rate(x[5m])) ";
        b.iter(|| analyze(black_box(query), query.len()));
    });

    // In grouping labels
    group.bench_function("in_grouping", |b| {
        let query = "sum(x) by (meth";
        b.iter(|| analyze(black_box(query), query.len()));
    });

    // Complex query at various cursor positions
    group.bench_function("complex_middle", |b| {
        let query = queries::COMPLEX;
        let cursor = query.len() / 2;
        b.iter(|| analyze(black_box(query), cursor));
    });

    group.bench_function("complex_end", |b| {
        b.iter(|| analyze(black_box(queries::COMPLEX), queries::COMPLEX.len()));
    });

    group.finish();
}

/// Benchmark syntax suggestion generation.
fn bench_syntax_suggestions(c: &mut Criterion) {
    use enya_promql::Context;

    let mut group = c.benchmark_group("completion/syntax_suggestions");

    // Empty context - returns all callables
    group.bench_function("empty", |b| {
        let ctx = Context::Empty;
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            // Consume the iterator to measure full cost
            iter.count()
        });
    });

    // InName with filtering
    group.bench_function("in_name_short", |b| {
        let ctx = Context::InName("r".to_string());
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            iter.count()
        });
    });

    group.bench_function("in_name_medium", |b| {
        let ctx = Context::InName("rat".to_string());
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            iter.count()
        });
    });

    group.bench_function("in_name_long", |b| {
        let ctx = Context::InName("histogram_qu".to_string());
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            iter.count()
        });
    });

    // ExpectLabelOp
    group.bench_function("expect_label_op", |b| {
        let ctx = Context::ExpectLabelOp;
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            iter.count()
        });
    });

    // InDuration
    group.bench_function("in_duration", |b| {
        let ctx = Context::InDuration("5".to_string());
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            iter.count()
        });
    });

    // ExpectModifier
    group.bench_function("expect_modifier", |b| {
        let ctx = Context::ExpectModifier;
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            iter.count()
        });
    });

    // ExpectBinaryOp
    group.bench_function("expect_binary_op", |b| {
        let ctx = Context::ExpectBinaryOp;
        b.iter(|| {
            let iter = syntax_suggestions(black_box(&ctx));
            iter.count()
        });
    });

    group.finish();
}

/// Benchmark query validation (uses promql-parser).
fn bench_validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation/validate");

    group.bench_function("simple", |b| {
        b.iter(|| validate(black_box(queries::SIMPLE)));
    });

    group.bench_function("selector", |b| {
        b.iter(|| validate(black_box(queries::SELECTOR)));
    });

    group.bench_function("rate", |b| {
        b.iter(|| validate(black_box(queries::RATE)));
    });

    group.bench_function("aggregation", |b| {
        b.iter(|| validate(black_box(queries::AGGREGATION)));
    });

    group.bench_function("complex", |b| {
        b.iter(|| validate(black_box(queries::COMPLEX)));
    });

    group.bench_function("long", |b| {
        b.iter(|| validate(black_box(queries::LONG)));
    });

    // Invalid queries
    group.bench_function("invalid_unclosed_brace", |b| {
        b.iter(|| validate(black_box("http_requests{")));
    });

    group.bench_function("invalid_unknown_func", |b| {
        b.iter(|| validate(black_box("unknown_func(x)")));
    });

    group.finish();
}

/// Benchmark the full autocomplete workflow (analyze + suggestions).
fn bench_autocomplete_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow/autocomplete");

    // Simulate typing "rate(" and getting completions
    group.bench_function("type_rate_open_paren", |b| {
        let query = "rate(";
        b.iter(|| {
            let ctx = analyze(black_box(query), query.len());
            let suggestions = syntax_suggestions(&ctx);
            suggestions.count()
        });
    });

    // Simulate typing in a selector and getting label suggestions
    group.bench_function("in_selector", |b| {
        let query = "http_requests{met";
        b.iter(|| {
            let ctx = analyze(black_box(query), query.len());
            let suggestions = syntax_suggestions(&ctx);
            suggestions.count()
        });
    });

    // Simulate typing after aggregation
    group.bench_function("after_aggregation", |b| {
        let query = "sum(rate(x[5m])) b";
        b.iter(|| {
            let ctx = analyze(black_box(query), query.len());
            let suggestions = syntax_suggestions(&ctx);
            suggestions.count()
        });
    });

    group.finish();
}

/// Benchmark partial_at_cursor function.
fn bench_partial_at_cursor(c: &mut Criterion) {
    use enya_promql::partial_at_cursor;

    let mut group = c.benchmark_group("lexer/partial_at_cursor");

    group.bench_function("simple", |b| {
        let query = "sum(http";
        b.iter(|| partial_at_cursor(black_box(query), query.len()));
    });

    group.bench_function("after_delimiter", |b| {
        let query = "sum(";
        b.iter(|| partial_at_cursor(black_box(query), query.len()));
    });

    group.bench_function("long_partial", |b| {
        let query = "histogram_quantile_long_function_name";
        b.iter(|| partial_at_cursor(black_box(query), query.len()));
    });

    group.bench_function("complex_query_middle", |b| {
        let query = queries::COMPLEX;
        let cursor = query.len() / 2;
        b.iter(|| partial_at_cursor(black_box(query), cursor));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scan_until,
    bench_analyze,
    bench_syntax_suggestions,
    bench_validate,
    bench_autocomplete_workflow,
    bench_partial_at_cursor,
);

criterion_main!(benches);
