//! Database builder for configuring and opening a talna-v2 database

use crate::db::Database;
use crate::merge_operator::TalnaMergeOperator;
use slatedb::Db;
use slatedb::object_store::ObjectStore;
use slatedb::object_store::path::Path;
use std::sync::Arc;

/// Builder for creating a [`Database`] instance.
pub struct Builder {
    // Future: add configuration options here
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
        Self {}
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
        log::info!("Opening talna-v2 database at {path}");

        let db = Db::builder(path, object_store)
            .with_merge_operator(Arc::new(TalnaMergeOperator))
            .build()
            .await?;
        let db = Arc::new(db);

        Ok(Database::from_db(db))
    }
}
