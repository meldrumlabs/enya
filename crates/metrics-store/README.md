# Enya Metrics Store

A simple, embeddable time-series database backed by SlateDB for object storage.

The `MetricsStore` struct layers project-wide tags such as the Git commit hash/timestamp on top
of every ingestion. Default tags can be extended at runtime (for example to attribute all samples
originating from a specific query or environment) and are automatically merged with user provided
tags without overwriting explicit values.

The crate provides a high-performance async write path using SlateDB as the storage backend,
enabling storage on object storage (S3, GCS, local filesystem, etc.).

## Future Improvements

- **Dictionary encoding for tags**: Replace repeated tag strings with compact integer IDs to reduce storage overhead. Tags like `env:production` or `host:web-01` repeat across many series; a dictionary mapping (`k:{tag_string}` → `{tag_id:u32}`) would store each unique tag once and reference it by ID in tag sets. This trades write-path complexity for significant storage savings when tags have low cardinality.
