//! Database builder for configuring and opening the metrics database

use crate::cache::{CacheConfig, LocalCache};
use crate::db::Database;
use crate::merge_operator::MetricsMergeOperator;
use slatedb::Db;
use slatedb::object_store::ObjectStore;
use slatedb::object_store::path::Path;
use std::sync::Arc;

/// Builder for creating a [`Database`] instance.
pub struct Builder {
    cache_config: CacheConfig,
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

        let db = Db::builder(path, object_store)
            .with_merge_operator(Arc::new(MetricsMergeOperator))
            .build()
            .await?;
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
