//! Benchmarks comparing JSON vs bitcode serialization for API responses.
//!
//! Run with: cargo bench -p enya-agent --bench serialization

use bitcode::{Decode, Encode};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};

type Timestamp = u128;

// Local copies of response types for benchmarking
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
struct MetricsBucket {
    start: Timestamp,
    end: Timestamp,
    value: f64,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
struct MetricsGroup {
    group: String,
    buckets: Vec<MetricsBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
struct QueryResponse {
    metric: String,
    query: String,
    parsed_agg: Option<String>,
    parsed_filter: String,
    parsed_grouping: Option<String>,
    parsed_time_range: Option<String>,
    start: Option<Timestamp>,
    end: Option<Timestamp>,
    granularity_ns: u128,
    groups: Vec<MetricsGroup>,
}

/// Generate a synthetic QueryResponse with the specified number of groups and buckets per group.
fn generate_query_response(num_groups: usize, buckets_per_group: usize) -> QueryResponse {
    let groups = (0..num_groups)
        .map(|g| MetricsGroup {
            group: format!("group_{g}"),
            buckets: (0..buckets_per_group)
                .map(|b| {
                    let start = (b as u128) * 60_000_000_000;
                    MetricsBucket {
                        start,
                        end: start + 60_000_000_000,
                        value: (g * buckets_per_group + b) as f64 * 1.5,
                        count: g * buckets_per_group + b + 1,
                    }
                })
                .collect(),
        })
        .collect();

    QueryResponse {
        metric: "cpu.usage".to_string(),
        query: "sum(env:prod) by (host)".to_string(),
        parsed_agg: Some("sum".to_string()),
        parsed_filter: "env:prod".to_string(),
        parsed_grouping: Some("by (host)".to_string()),
        parsed_time_range: None,
        start: Some(0),
        end: Some(3_600_000_000_000),
        granularity_ns: 60_000_000_000,
        groups,
    }
}

fn query_response_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_response_serialization");

    // Test cases: (num_groups, buckets_per_group)
    let test_cases = [
        (5, 60),    // Small: 5 groups, 1 hour of 1-minute buckets
        (10, 60),   // Medium: 10 groups, 1 hour
        (20, 60),   // Large: 20 groups, 1 hour
        (10, 1440), // XL: 10 groups, 24 hours of 1-minute buckets
    ];

    for (num_groups, buckets_per_group) in test_cases {
        let response = generate_query_response(num_groups, buckets_per_group);
        let id = format!("{num_groups}g_{buckets_per_group}b");

        // Pre-serialize for deserialization benchmarks
        let json_bytes = serde_json::to_vec(&response).unwrap();
        let bitcode_bytes = bitcode::encode(&response);

        // Set throughput based on JSON size (common baseline)
        group.throughput(Throughput::Bytes(json_bytes.len() as u64));

        // Serialization benchmarks
        group.bench_with_input(BenchmarkId::new("json_ser", &id), &response, |b, resp| {
            b.iter(|| serde_json::to_vec(black_box(resp)).unwrap());
        });

        group.bench_with_input(
            BenchmarkId::new("bitcode_ser", &id),
            &response,
            |b, resp| {
                b.iter(|| bitcode::encode(black_box(resp)));
            },
        );

        // Deserialization benchmarks
        group.bench_with_input(BenchmarkId::new("json_de", &id), &json_bytes, |b, bytes| {
            b.iter(|| serde_json::from_slice::<QueryResponse>(black_box(bytes)).unwrap());
        });

        group.bench_with_input(
            BenchmarkId::new("bitcode_de", &id),
            &bitcode_bytes,
            |b, bytes| {
                b.iter(|| bitcode::decode::<QueryResponse>(black_box(bytes)).unwrap());
            },
        );

        // Print sizes for comparison
        println!(
            "QueryResponse {id}: JSON={} bytes, bitcode={} bytes, ratio={:.2}x smaller",
            json_bytes.len(),
            bitcode_bytes.len(),
            json_bytes.len() as f64 / bitcode_bytes.len() as f64
        );
    }

    group.finish();
}

criterion_group!(benches, query_response_benchmarks);
criterion_main!(benches);
