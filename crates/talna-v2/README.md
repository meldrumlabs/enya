# talna-v2

A simple, embeddable time series database using object storage (SlateDB).

## Overview

talna-v2 is a rewrite of [talna](../talna) using [SlateDB](https://github.com/slatedb/slatedb) as the storage backend instead of fjall. This enables storage on object storage backends like S3, GCS, or local filesystem with bottomless capacity.

## Key Differences from talna v1

| Feature | talna v1 (fjall) | talna-v2 (SlateDB) |
|---------|-----------------|-------------------|
| Storage | Local LSM tree | Object storage LSM |
| API | Synchronous | Async (tokio) |
| Partitioning | Column families | Key prefixes |
| Capacity | Limited by disk | Bottomless |

## Key Prefix Strategy

Since SlateDB doesn't have column families, we use key prefixes:

- `d:` - Data partition (time series points)
- `s:` - Series mapping (series key -> series ID)
- `t:` - Tag index (inverted index for queries)
- `g:` - Tag sets (series ID -> tags)
- `c:` - Counters (series ID generator)

## Usage

```rust
use talna_v2::{Database, MetricName, tagset, object_store};
use object_store::memory::InMemory;
use std::sync::Arc;

#[tokio::main]
async fn main() -> talna_v2::Result<()> {
    // Use in-memory object store for testing
    let object_store = Arc::new(InMemory::new());
    let db = Database::builder().open(object_store, "test-db").await?;

    let metric = MetricName::try_from("cpu.total").unwrap();

    // Write data points
    db.write(
        metric,
        25.42,
        tagset!(
            "env" => "prod",
            "host" => "h-1",
        ),
    ).await?;

    // Query with aggregation
    let results = db
        .avg(metric, "host")
        .filter("env:prod")
        .build()
        .await?
        .collect()?;

    db.close().await?;
    Ok(())
}
```

## Features

- **Async API** - Built on tokio for async I/O
- **Object Storage** - Store data on S3, GCS, Azure Blob, or local filesystem
- **Tag-based Queries** - Datadog-style tag filtering (AND, OR, NOT, wildcards)
- **Aggregations** - Sum, Count, Average, Min, Max with time bucketing
- **Group By** - Group time series by tag values

## License

MIT OR Apache-2.0
