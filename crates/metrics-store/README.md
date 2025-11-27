# Enya Metrics Store

`MetricsStore` wraps the `talna::Database` API and layers project-wide tags such as the Git
commit hash/timestamp on top of every ingestion. Default tags can be extended at runtime (for
example to attribute all samples originating from a specific query or environment) and are
automatically merged with user provided tags without overwriting explicit values.

The crate is backed by the [`talna`](../talna) time-series database and re-exports the same
high-performance write path while keeping the ergonomics focused on Enya's workflows.
