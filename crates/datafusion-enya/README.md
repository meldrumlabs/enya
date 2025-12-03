# datafusion-enya

DataFusion integration for Enya observability - automatic metrics collection via metrics-rs.

## Usage

```rust
use datafusion::prelude::*;
use datafusion_enya::EnyaSessionContextExt;

// Create a session context with Enya instrumentation
let ctx = SessionContext::new_with_enya();

// Execute queries normally - metrics are recorded automatically
let df = ctx.sql("SELECT * FROM my_table").await?;
let results = df.collect().await?;
```

## Metrics Collected

- `datafusion.output_rows` - Rows output by each operator
- `datafusion.output_bytes` - Bytes output by each operator
- `datafusion.elapsed_compute_ns` - CPU time spent in computation
- `datafusion.bytes_scanned` - Bytes read from data sources
- `datafusion.row_groups_pruned_statistics` - Parquet row groups pruned by statistics
- `datafusion.row_groups_pruned_bloom_filter` - Parquet row groups pruned by bloom filter
- `datafusion.spilled_bytes` - Bytes spilled to disk
- `datafusion.spill_count` - Number of spill operations

All metrics are tagged with `operator` and `partition`, and optionally with `query_id` for correlation.
