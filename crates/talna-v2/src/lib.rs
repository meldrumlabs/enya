//! A simple, embeddable time series database using object storage.
//!
//! It uses <https://github.com/slatedb/slatedb> as its underlying storage engine,
//! enabling storage on object storage backends (S3, GCS, local filesystem, etc.).
//!
//! The tagging and querying mechanism is modelled after Datadog's metrics service
//! (<https://www.datadoghq.com/blog/engineering/timeseries-indexing-at-scale/>).
//!
//! Data points are f32s by default, but can be switched to f64 using the `high_precision` feature flag.
//!
//! ## Key Differences from talna v1 (fjall-based)
//!
//! - Uses `SlateDB` which stores data on object storage instead of local disk
//! - Async API (requires tokio runtime)
//! - Uses key prefixes instead of column families for data organization
//! - Better suited for cloud-native deployments with bottomless storage
//!
//! ## Key Prefix Strategy
//!
//! Since `SlateDB` doesn't have column families, we use key prefixes:
//! - `d:` - Data partition (time series points)
//! - `s:` - Series mapping (series key -> series ID)
//! - `t:` - Tag index (inverted index for queries)
//! - `g:` - Tag sets (series ID -> tags)
//!
//! ## Basic usage
//!
//! ```ignore
//! use talna_v2::{Database, Duration, MetricName, tagset, timestamp};
//! use object_store::local::LocalFileSystem;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> talna_v2::Result<()> {
//!     let object_store = Arc::new(LocalFileSystem::new_with_prefix("/tmp/talna-test")?);
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
mod db;
mod db_builder;
mod duration;
mod error;
mod merge;
mod merge_operator;
mod metric_name;

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
pub use db::Database;
pub use db_builder::Builder as DatabaseBuilder;
pub use duration::Duration;
pub use error::{Error, Result};
pub use metric_name::MetricName;
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
#[cfg(feature = "high_precision")]
pub type Value = f64;

/// Value used in time series
#[cfg(not(feature = "high_precision"))]
pub type Value = f32;

/// Macro to create a list of tags.
///
/// # Examples
///
/// ```
/// use talna_v2::{tagset, TagSet};
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
