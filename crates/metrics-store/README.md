# Enya Metrics Store

A simple, embeddable time-series database backed by SlateDB for object storage.

The `MetricsStore` struct layers project-wide tags such as the Git commit hash/timestamp on top
of every ingestion. Default tags can be extended at runtime (for example to attribute all samples
originating from a specific query or environment) and are automatically merged with user provided
tags without overwriting explicit values.

The crate provides a high-performance async write path using SlateDB as the storage backend,
enabling storage on object storage (S3, GCS, local filesystem, etc.).
