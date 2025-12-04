//! Database builder for configuring and opening the metrics database

use crate::cache::{CacheConfig, LocalCache};
use crate::db::Database;
use crate::merge_operator::MetricsMergeOperator;
use slatedb::Db;
use slatedb::config::Settings;
use slatedb::object_store::ObjectStore;
use slatedb::object_store::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "lz4")]
use slatedb::config::CompressionCodec;

/// Default flush interval (1 second) - relaxed from `SlateDB`'s 100ms default
/// for lightweight embedded use where losing ~1s of data on crash is acceptable.
const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_secs(1);

/// Default L0 SST size (16 MiB) - smaller than `SlateDB`'s 64 MiB default
/// to reduce memory pressure for embedded deployments.
const DEFAULT_L0_SST_SIZE_BYTES: usize = 16 * 1024 * 1024;

/// Default max unflushed bytes (256 MiB) - smaller than `SlateDB`'s 1 GiB default
/// to limit memory usage in embedded scenarios.
const DEFAULT_MAX_UNFLUSHED_BYTES: usize = 256 * 1024 * 1024;

/// Builder for creating a [`Database`] instance.
///
/// The builder provides sensible defaults optimized for lightweight embedded
/// observability use cases where:
/// - Losing a few seconds of data on crash is acceptable
/// - Memory footprint should be minimized
/// - Object storage I/O should be reduced
///
/// # Example
///
/// ```ignore
/// use enya_metrics_store::Builder;
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// let db = Builder::new()
///     .with_flush_interval(Duration::from_secs(5))
///     .with_l0_sst_size_bytes(32 * 1024 * 1024)
///     .open(object_store, "metrics")
///     .await?;
/// ```
pub struct Builder {
    cache_config: CacheConfig,
    flush_interval: Duration,
    l0_sst_size_bytes: usize,
    max_unflushed_bytes: usize,
    #[cfg(feature = "lz4")]
    compression: Option<CompressionCodec>,
    /// Default TTL for data points in seconds
    default_ttl: Option<u64>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// Creates a new database builder with default options.
    ///
    /// Default settings are optimized for lightweight embedded use:
    /// - `flush_interval`: 1 second (vs `SlateDB`'s 100ms)
    /// - `l0_sst_size_bytes`: 16 MiB (vs `SlateDB`'s 64 MiB)
    /// - `max_unflushed_bytes`: 256 MiB (vs `SlateDB`'s 1 GiB)
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache_config: CacheConfig::default(),
            flush_interval: DEFAULT_FLUSH_INTERVAL,
            l0_sst_size_bytes: DEFAULT_L0_SST_SIZE_BYTES,
            max_unflushed_bytes: DEFAULT_MAX_UNFLUSHED_BYTES,
            #[cfg(feature = "lz4")]
            compression: None,
            default_ttl: None,
        }
    }

    /// Sets the cache configuration.
    ///
    /// The local cache reduces object storage roundtrips for frequently
    /// accessed metadata like series mappings and tag sets.
    #[must_use]
    pub fn with_cache_config(mut self, config: CacheConfig) -> Self {
        self.cache_config = config;
        self
    }

    /// Sets how frequently to flush data to object storage.
    ///
    /// Lower values reduce data loss on crash but increase object storage I/O.
    /// Higher values reduce I/O costs but increase the crash data loss window.
    ///
    /// Default: 1 second (relaxed from `SlateDB`'s 100ms for embedded use)
    ///
    /// # Guidelines
    ///
    /// - **100ms**: Near-realtime durability, high I/O cost
    /// - **1s**: Good balance for most observability use cases (default)
    /// - **5s**: Lower I/O, acceptable for dashboards/debugging
    #[must_use]
    pub fn with_flush_interval(mut self, interval: Duration) -> Self {
        self.flush_interval = interval;
        self
    }

    /// Sets the target size for L0 SST files.
    ///
    /// Smaller values reduce memory usage but may increase the number of
    /// L0 files, which can impact read performance. Larger values use more
    /// memory but produce fewer, larger files.
    ///
    /// Default: 16 MiB (reduced from `SlateDB`'s 64 MiB for embedded use)
    #[must_use]
    pub fn with_l0_sst_size_bytes(mut self, size: usize) -> Self {
        self.l0_sst_size_bytes = size;
        self
    }

    /// Sets the maximum bytes of unflushed data before applying backpressure.
    ///
    /// This limits memory usage by blocking writes when too much data is
    /// pending flush. Lower values bound memory usage more tightly but may
    /// cause write stalls under high load.
    ///
    /// Default: 256 MiB (reduced from `SlateDB`'s 1 GiB for embedded use)
    #[must_use]
    pub fn with_max_unflushed_bytes(mut self, size: usize) -> Self {
        self.max_unflushed_bytes = size;
        self
    }

    /// Enables LZ4 compression for `SSTable` blocks.
    ///
    /// LZ4 provides fast compression and decompression with moderate
    /// compression ratios (typically 2-4x for time series data), making it
    /// suitable for reducing storage costs with minimal CPU overhead (~5%).
    ///
    /// This option requires the `lz4` feature to be enabled.
    #[cfg(feature = "lz4")]
    #[must_use]
    pub fn with_lz4_compression(mut self) -> Self {
        self.compression = Some(CompressionCodec::Lz4);
        self
    }

    /// Sets the default time-to-live (TTL) for data points.
    ///
    /// Data points older than the TTL will be automatically removed during
    /// compaction. This helps bound storage growth for long-running agents.
    ///
    /// Note that TTL applies to data points only. Series metadata (mappings,
    /// tag indices) are not affected and will persist indefinitely.
    ///
    /// Default: no TTL (data points persist until explicitly deleted)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    ///
    /// let db = Builder::new()
    ///     .with_state_ttl(Duration::from_secs(7 * 24 * 60 * 60)) // 7 days
    ///     .open(object_store, "metrics")
    ///     .await?;
    /// ```
    #[must_use]
    pub fn with_state_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl.as_secs());
        self
    }

    /// Opens or creates a database at the specified path in the object store.
    ///
    /// # Arguments
    ///
    /// * `object_store` - The object store to use (S3, GCS, local filesystem, etc.)
    /// * `path` - The path prefix within the object store for this database
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub async fn open(
        self,
        object_store: Arc<dyn ObjectStore>,
        path: impl Into<Path>,
    ) -> crate::Result<Database> {
        let path = path.into();
        log::info!("Opening metrics database at {path}");

        let settings = self.build_settings();
        log::info!(
            "SlateDB settings: flush_interval={:?}, l0_sst_size={}MiB, max_unflushed={}MiB, default_ttl={:?}",
            settings.flush_interval,
            settings.l0_sst_size_bytes / (1024 * 1024),
            settings.max_unflushed_bytes / (1024 * 1024),
            settings.default_ttl.map(Duration::from_secs),
        );

        let builder = Db::builder(path, object_store)
            .with_merge_operator(Arc::new(MetricsMergeOperator))
            .with_settings(settings);

        let db = builder.build().await?;
        let db = Arc::new(db);

        let cache = LocalCache::new(&self.cache_config);
        log::info!(
            "Initialized local cache (series: {}, tag_sets: {})",
            self.cache_config.series_cache_capacity,
            self.cache_config.tag_sets_cache_capacity
        );

        Database::from_db(db, &cache).await
    }

    /// Builds the `SlateDB` settings from the builder configuration.
    fn build_settings(&self) -> Settings {
        #[allow(unused_mut)]
        let mut settings = Settings {
            flush_interval: Some(self.flush_interval),
            l0_sst_size_bytes: self.l0_sst_size_bytes,
            max_unflushed_bytes: self.max_unflushed_bytes,
            default_ttl: self.default_ttl,
            ..Settings::default()
        };

        #[cfg(feature = "lz4")]
        if let Some(codec) = self.compression {
            settings.compression_codec = Some(codec);
            log::info!("SSTable compression enabled: LZ4");
        }

        settings
    }
}
