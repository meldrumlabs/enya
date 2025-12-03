//! A simple, embeddable time series database using object storage.
//!
//! It uses <https://github.com/slatedb/slatedb> as its underlying storage engine,
//! enabling storage on object storage backends (S3, GCS, local filesystem, etc.).
//!
//! The tagging and querying mechanism is modelled after Datadog's metrics service
//! (<https://www.datadoghq.com/blog/engineering/timeseries-indexing-at-scale/>).
//!
//! Data points are stored as f64s.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                            MetricsStore                                 │
//! │                     (adds default tags to writes)                       │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                              Database                                   │
//! │                        (main entry point)                               │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │   Write Path                    │   Read Path                           │
//! │   ────────────                  │   ─────────                           │
//! │   write(metric, value, tags) ──►│◄── start_query(metric, filter, range) │
//! │                                 │                                       │
//! │   1. Format series key          │   1. Parse filter expression          │
//! │   2. Lookup/create series ID    │   2. Evaluate against tag index       │
//! │   3. Index tags (if new)        │   3. Get matching series IDs          │
//! │   4. Store data point           │   4. Scan data for each series        │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!          ┌──────────────────────────┼──────────────────────────┐
//!          ▼                          ▼                          ▼
//! ┌─────────────────┐     ┌─────────────────────┐     ┌─────────────────────┐
//! │  SeriesMapping  │     │      TagIndex       │     │      TagSets        │
//! │     (smap)      │     │   (inverted index)  │     │  (series → tags)    │
//! ├─────────────────┤     ├─────────────────────┤     ├─────────────────────┤
//! │ s:{series_key}  │     │ t:{metric}#{k}:{v}  │     │ g:{series_id}       │
//! │      ↓          │     │      ↓              │     │      ↓              │
//! │   series_id     │     │  [postings list]    │     │  "k1:v1;k2:v2"      │
//! └─────────────────┘     └─────────────────────┘     └─────────────────────┘
//!                                     │
//!                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                             Storage                                     │
//! │                      (SlateDB wrapper)                                  │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │   Key Format: d:{series_id:8}{!timestamp:16} → {value}                  │
//! │                                                                         │
//! │   • Timestamps inverted (!ts) for reverse chronological order           │
//! │   • Merge operator for atomic postings list appends                     │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                     │
//!                                     ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         Object Storage                                  │
//! │                    (S3, GCS, local filesystem)                          │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! ## Core Database
//!
//! - **[`Database`]**: Main entry point for writes and queries
//! - **[`MetricsStore`]**: Wrapper that adds default tags to every write
//! - **[`DatabaseBuilder`]**: Builder for configuring and opening a database
//!
//! ## Indexing
//!
//! - **`SeriesMapping`** (`s:` prefix): Maps series key strings to compact u64 IDs
//! - **`TagIndex`** (`t:` prefix): Inverted index mapping tag terms to series IDs
//! - **`TagSets`** (`g:` prefix): Maps series ID back to its complete tag set
//!
//! ## Aggregation
//!
//! - **[`index::WheelIndex`]**: Pre-computed aggregates using µWheel for fast
//!   time-range queries on recent data. Supports counters (`U64SumAggregator`)
//!   and histograms (`DDSketchAggregator` for percentiles).
//!
//! ## Query System
//!
//! Filter expressions support:
//! - Exact match: `env:prod`
//! - Wildcard: `service:db.*`
//! - Boolean: `env:prod AND service:db`
//! - Negation: `!env:staging`
//! - Grouping: `(env:prod OR env:staging) AND service:db`
//!
//! # Key Prefix Strategy
//!
//! Since `SlateDB` doesn't have column families, we use key prefixes:
//!
//! | Prefix | Purpose                        | Format                                 |
//! |--------|--------------------------------|----------------------------------------|
//! | `d:`   | Data points (time series)      | `d:{series_id:8}{!ts:16}` → `{value}` |
//! | `s:`   | Series mapping                 | `s:{series_key}` → `{series_id:8}`    |
//! | `t:`   | Tag index (inverted)           | `t:{metric}#{k}:{v}` → `[postings]`   |
//! | `g:`   | Tag sets                       | `g:{series_id:8}` → `"k1:v1;k2:v2"`   |
//! | `c:`   | Counter                        | `c:next_series_id` → `{next_id:8}`    |
//!
//! # Basic usage
//!
//! ```ignore
//! use enya_metrics_store::{Database, Duration, MetricName, tagset, timestamp};
//! use object_store::local::LocalFileSystem;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> enya_metrics_store::Result<()> {
//!     let object_store = Arc::new(LocalFileSystem::new_with_prefix("/tmp/metrics-test")?);
//!     let db = Database::builder().open(object_store, "/db").await?;
//!
//!     let metric_name = MetricName::try_from("cpu.total").unwrap();
//!
//!     db.write(
//!         metric_name,
//!         25.42,
//!         tagset!(
//!             "env" => "prod",
//!             "service" => "db",
//!             "host" => "h-1",
//!         ),
//!     ).await?;
//!
//!     db.close().await?;
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::all, missing_docs, clippy::cargo)]
#![deny(clippy::unwrap_used)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::pedantic, clippy::nursery)]
#![warn(clippy::expect_used)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::multiple_crate_versions)]
#![warn(clippy::result_unit_err)]
#![warn(clippy::needless_lifetimes)]

mod agg;
mod cache;
mod db;
mod db_builder;
mod duration;
mod error;
pub mod index;
mod merge;
mod merge_operator;
mod metric_name;
mod metrics_store;

#[doc(hidden)]
pub mod query;

mod series_key;
mod smap;
mod storage;
mod tag_index;
mod tag_sets;
mod time;

type SeriesId = u64;
type HashMap<K, V> = std::collections::HashMap<K, V, rustc_hash::FxBuildHasher>;

pub use agg::{Bucket, GroupedAggregation};
pub use cache::CacheConfig;
pub use db::Database;
pub use db_builder::Builder as DatabaseBuilder;
pub use duration::Duration;
pub use error::{Error, Result};
pub use metric_name::MetricName;
pub use metrics_store::{GIT_TIMESTAMP_TAG_KEY, GIT_VERSION_TAG_KEY, MetricConfig, MetricsStore};
pub use time::timestamp;

/// Re-export `object_store` for convenience
pub use slatedb::object_store;

/// A list of tags.
pub type TagSet<'a> = [(&'a str, &'a str)];

#[doc(hidden)]
pub use series_key::SeriesKey;

/// Nanosecond timestamp
pub type Timestamp = u128;

/// Value used in time series
pub type Value = f64;

/// Macro to create a list of tags.
///
/// # Examples
///
/// ```
/// use enya_metrics_store::{tagset, TagSet};
///
/// let tags: &TagSet = tagset!(
///   "service" => "db",
///   "env" => "production",
/// );
/// ```
#[macro_export]
macro_rules! tagset {
  ($($k:expr => $v:expr),* $(,)?) => {{
      &[$(($k.into(), $v.into()),)*]
  }}
}
