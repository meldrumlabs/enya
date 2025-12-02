//! Storage abstraction layer for `SlateDB`
//!
//! This module provides the core storage primitives using `SlateDB` with key prefixes
//! to separate different data types (data points, series mapping, tag index, tag sets).
//!
//! ## Key Prefix Strategy
//!
//! Since `SlateDB` doesn't have column families like fjall, we use key prefixes:
//!
//! | Prefix | Purpose                      | Key Format                              |
//! |--------|------------------------------|-----------------------------------------|
//! | `d:`   | Time series data points      | `d:{series_id:8}{!timestamp:16}`        |
//! | `s:`   | Series mapping               | `s:{series_key_string}`                 |
//! | `t:`   | Tag index (inverted)         | `t:{metric}#{tag_key}:{tag_value}`      |
//! | `g:`   | Tag sets (series -> tags)    | `g:{series_id:8}`                       |
//! | `c:`   | Counter (series ID generator)| `c:next_series_id`                      |

use bytes::Bytes;
use slatedb::Db;
use std::sync::Arc;

/// Key prefixes for different data types
pub mod prefix {
    /// Data partition prefix - stores actual time series data points
    pub const DATA: &[u8] = b"d:";

    /// Series mapping prefix - maps series key string to series ID
    pub const SMAP: &[u8] = b"s:";

    /// Tag index prefix - inverted index for tag queries
    pub const TAG_INDEX: &[u8] = b"t:";

    /// Tag sets prefix - maps series ID to its tags
    pub const TAG_SETS: &[u8] = b"g:";

    /// Counter prefix - for atomic counters like next series ID
    #[allow(dead_code)]
    pub const COUNTER: &[u8] = b"c:";

    /// Next series ID counter key
    pub const NEXT_SERIES_ID: &[u8] = b"c:next_series_id";
}

/// Storage handle wrapping a `SlateDB` instance
#[derive(Clone)]
pub struct Storage {
    db: Arc<Db>,
}

impl Storage {
    /// Create a new storage handle from a `SlateDB` instance
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Get the underlying `SlateDB` instance
    pub fn db(&self) -> &Db {
        &self.db
    }

    /// Build a prefixed key for data partition
    #[must_use]
    pub fn data_key(series_id: u64, timestamp: u128) -> Bytes {
        let mut key = Vec::with_capacity(2 + 8 + 16);
        key.extend_from_slice(prefix::DATA);
        key.extend_from_slice(&series_id.to_be_bytes());
        // Invert timestamp for reverse chronological ordering
        key.extend_from_slice(&(!timestamp).to_be_bytes());
        Bytes::from(key)
    }

    /// Build a prefixed key for series mapping
    #[must_use]
    pub fn smap_key(series_key: &str) -> Bytes {
        let mut key = Vec::with_capacity(2 + series_key.len());
        key.extend_from_slice(prefix::SMAP);
        key.extend_from_slice(series_key.as_bytes());
        Bytes::from(key)
    }

    /// Build a prefixed key for tag index
    #[must_use]
    pub fn tag_index_key(term: &str) -> Bytes {
        let mut key = Vec::with_capacity(2 + term.len());
        key.extend_from_slice(prefix::TAG_INDEX);
        key.extend_from_slice(term.as_bytes());
        Bytes::from(key)
    }

    /// Build a prefixed key for tag sets
    #[must_use]
    pub fn tag_sets_key(series_id: u64) -> Bytes {
        let mut key = Vec::with_capacity(2 + 8);
        key.extend_from_slice(prefix::TAG_SETS);
        key.extend_from_slice(&series_id.to_be_bytes());
        Bytes::from(key)
    }

    /// Get a value by key
    pub async fn get(&self, key: &[u8]) -> crate::Result<Option<Bytes>> {
        Ok(self.db.get(key).await?)
    }

    /// Put a key-value pair
    pub async fn put(&self, key: Bytes, value: Bytes) -> crate::Result<()> {
        self.db.put(&key[..], &value[..]).await?;
        Ok(())
    }

    /// Delete a key
    pub async fn delete(&self, key: Bytes) -> crate::Result<()> {
        self.db.delete(&key[..]).await?;
        Ok(())
    }

    /// Merge a value atomically using the configured merge operator
    ///
    /// This allows atomic updates without read-modify-write cycles.
    /// The merge operator is configured when opening the database.
    pub async fn merge(&self, key: Bytes, operand: Bytes) -> crate::Result<()> {
        self.db.merge(&key[..], &operand[..]).await?;
        Ok(())
    }

    /// Close the database
    pub async fn close(&self) -> crate::Result<()> {
        self.db.close().await?;
        Ok(())
    }
}
