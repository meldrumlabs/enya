# Meldrum

## Metrics Store

The metrics store is built on top of talna, a LSM-based storage engine for time-series. It also integrates uwheel for efficient
aggregation and filtering queries.

Crates:

- talna
- uwheel

```rust
use talna::{Database, Duration, MetricName, tagset, timestamp};

let db = Database::builder().open(path)?;

let metric_name = MetricName::try_from("query.latency").unwrap();

db.write(
    metric_name,
    25.42, 
    tagset!(
        "env" => "prod",
        "service" => "db",
        "host" => "h-1",
        "git_ver" => "9ch32iu1h312hiioj"
        "git_timestamp" => "2025-10-01 23:00:00"
    ),
)?;

```

With this we can allow both external tags which users set through metrics-rs but also add internal ones such as git version and git timestamps to allow track performance over commits.

### Prometheus Integration

Just have to find a way to integrate with regular Prometheus setups.

1. Assume Prometheus server exists that can be scraped

- May not contain all data needed?

2. Install manual recorder for metrics (metrics-rs)
3. Other?

```rust
while let Some(metric) = metrics_stream.next().await {
  // 1. Consume 
  // 2. Possibly parse into Tagsets and so on.
  // 3. Write to storage 
  metrics_store.write(metric);
}
```

### Query Interface

TBD - Possibly query lang like PromQL that makes it easy to map over queries to Meldrum

## DataFusion

Meldrum component for tracking and storing DF metrics, schema, table stats into
storage over time.

## Tokio

Meldrum component for tracking tokio runtimes and visualizing tasks on them.

## Memory

rust-jemalloc-pprof to periodically store dumps to storage that can be retrieved as flamegraphs and stored over time.
