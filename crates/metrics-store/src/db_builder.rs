//! Database builder for configuring and opening the metrics database

use crate::cache::{CacheConfig, LocalCache};
use crate::db::Database;
use crate::merge_operator::MetricsMergeOperator;
use slatedb::Db;
use slatedb::object_store::ObjectStore;
use slatedb::object_store::path::Path;
use std::sync::Arc;

#[cfg(feature = "lz4")]
use slatedb::config::{CompressionCodec, Settings};

/// Builder for creating a [`Database`] instance.
pub struct Builder {
    cache_config: CacheConfig,
    #[cfg(feature = "lz4")]
    compression: Option<CompressionCodec>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// Creates a new database builder with default options.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache_config: CacheConfig::default(),
            #[cfg(feature = "lz4")]
            compression: None,
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

    /// Enables LZ4 compression for `SSTable` blocks.
    ///
    /// LZ4 provides fast compression and decompression with moderate
    /// compression ratios, making it suitable for time series data.
    ///
    /// This option requires the `lz4` feature to be enabled.
    #[cfg(feature = "lz4")]
    #[must_use]
    pub fn with_lz4_compression(mut self) -> Self {
        self.compression = Some(CompressionCodec::Lz4);
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

        #[allow(unused_mut)]
        let mut builder =
            Db::builder(path, object_store).with_merge_operator(Arc::new(MetricsMergeOperator));

        #[cfg(feature = "lz4")]
        if let Some(codec) = self.compression {
            let settings = Settings {
                compression_codec: Some(codec),
                ..Settings::default()
            };
            builder = builder.with_settings(settings);
            log::info!("SSTable compression enabled: LZ4");
        }

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
}
