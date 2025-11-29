use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use enya_common::aggregators::{DDSketchAggregator, DDSketchPartial};
use uwheel::aggregator::Aggregator;

/// Benchmark inserting values into a DDSketch.
fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("ddsketch_insert");

    for count in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("sequential", count), &count, |b, &n| {
            b.iter(|| {
                let mut sketch = DDSketchAggregator::lift(0.0);
                for i in 1..n {
                    DDSketchAggregator::combine_mutable(&mut sketch, i as f64);
                }
                black_box(sketch)
            });
        });

        group.bench_with_input(BenchmarkId::new("random", count), &count, |b, &n| {
            // Pre-generate "random" values using a simple LCG to avoid RNG overhead in the loop
            let values: Vec<f64> = (0..n)
                .scan(12345u64, |state, _| {
                    *state = state.wrapping_mul(1103515245).wrapping_add(12345);
                    Some((*state as f64) / (u64::MAX as f64) * 1000.0)
                })
                .collect();

            b.iter(|| {
                let mut sketch = DDSketchAggregator::lift(values[0]);
                for &v in &values[1..] {
                    DDSketchAggregator::combine_mutable(&mut sketch, v);
                }
                black_box(sketch)
            });
        });
    }

    group.finish();
}

/// Benchmark merging multiple DDSketch partials (simulates time range query aggregation).
fn bench_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("ddsketch_merge");

    for sketch_count in [2, 10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("merge_partials", sketch_count),
            &sketch_count,
            |b, &n| {
                // Pre-create partials with some data each
                let partials: Vec<DDSketchPartial> = (0..n)
                    .map(|i| {
                        let mut sketch = DDSketchAggregator::lift((i * 100) as f64);
                        for j in 1..100 {
                            DDSketchAggregator::combine_mutable(
                                &mut sketch,
                                (i * 100 + j) as f64,
                            );
                        }
                        DDSketchAggregator::freeze(sketch)
                    })
                    .collect();

                b.iter(|| {
                    let mut result = DDSketchAggregator::IDENTITY;
                    for partial in partials.clone() {
                        result = DDSketchAggregator::combine(result, partial);
                    }
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark the full pipeline: insert values, freeze, merge, and query quantile.
fn bench_time_range_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("ddsketch_time_range_query");

    // Simulate different time range scenarios:
    // - 60 sketches = 1 hour of per-minute data
    // - 1440 sketches = 1 day of per-minute data
    for (name, sketch_count, values_per_sketch) in [
        ("1h_1min_buckets", 60, 100),
        ("1d_1min_buckets", 1440, 100),
        ("1h_dense_buckets", 60, 1000),
    ] {
        // Pre-create partials representing aggregated time buckets
        let partials: Vec<DDSketchPartial> = (0..sketch_count)
            .map(|bucket| {
                let base = (bucket * values_per_sketch) as f64;
                let mut sketch = DDSketchAggregator::lift(base);
                for i in 1..values_per_sketch {
                    // Simulate latency values with some variance
                    let value = base + (i as f64 * 0.1) + ((i % 10) as f64);
                    DDSketchAggregator::combine_mutable(&mut sketch, value);
                }
                DDSketchAggregator::freeze(sketch)
            })
            .collect();

        group.bench_function(BenchmarkId::new("merge_and_query_p99", name), |b| {
            b.iter(|| {
                // Merge all partials (simulating combining all time buckets in range)
                let mut merged = DDSketchAggregator::IDENTITY;
                for partial in partials.clone() {
                    merged = DDSketchAggregator::combine(merged, partial);
                }

                // Lower to final aggregate and query quantile
                let aggregate = DDSketchAggregator::lower(merged);
                let sketch = aggregate.into_sketch();
                black_box(sketch.quantile(0.99))
            });
        });

        group.bench_function(BenchmarkId::new("merge_only", name), |b| {
            b.iter(|| {
                let mut merged = DDSketchAggregator::IDENTITY;
                for partial in partials.clone() {
                    merged = DDSketchAggregator::combine(merged, partial);
                }
                black_box(merged)
            });
        });
    }

    group.finish();
}

/// Benchmark quantile queries on a pre-merged sketch.
fn bench_quantile_query(c: &mut Criterion) {
    // Create a sketch with substantial data
    let mut sketch = DDSketchAggregator::lift(0.0);
    for i in 1..100_000 {
        DDSketchAggregator::combine_mutable(&mut sketch, i as f64);
    }
    let frozen = DDSketchAggregator::freeze(sketch);
    let aggregate = DDSketchAggregator::lower(frozen);
    let final_sketch = aggregate.into_sketch();

    let mut group = c.benchmark_group("ddsketch_quantile");

    group.bench_function("p50", |b| {
        b.iter(|| black_box(final_sketch.quantile(0.5)))
    });

    group.bench_function("p99", |b| {
        b.iter(|| black_box(final_sketch.quantile(0.99)))
    });

    group.bench_function("p999", |b| {
        b.iter(|| black_box(final_sketch.quantile(0.999)))
    });

    group.bench_function("multiple_quantiles", |b| {
        b.iter(|| {
            let p50 = final_sketch.quantile(0.5);
            let p90 = final_sketch.quantile(0.90);
            let p95 = final_sketch.quantile(0.95);
            let p99 = final_sketch.quantile(0.99);
            black_box((p50, p90, p95, p99))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_insert,
    bench_merge,
    bench_time_range_query,
    bench_quantile_query,
);
criterion_main!(benches);
