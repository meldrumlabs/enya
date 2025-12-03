//! Tag sets storage - maps series IDs to their tag sets
//!
//! Stores the complete set of tags for each series, enabling:
//! - Grouping by tag values during aggregation queries
//! - NOT queries (finding series without specific tags)
//!
//! ## Storage Format
//!
//! Tags are stored as a semicolon-separated string of `key:value` pairs,
//! sorted alphabetically by key.
//!
//! Example: `env:prod;host:h-1;service:db`
//!
//! ## Caching
//!
//! Tag sets are loaded on every query for aggregation grouping. We cache them
//! in memory to avoid repeated object storage roundtrips. With a single-writer
//! model, we use write-through caching on inserts.

use crate::SeriesId;
use crate::cache::TagSetsCache;
use crate::storage::Storage;
use bytes::Bytes;
use std::io;

/// `HashMap` type alias using `FxHash` for performance
pub type OwnedTagSets = crate::HashMap<String, String>;

/// Maps Series IDs to their tag sets
pub struct TagSets {
    storage: Storage,
    cache: TagSetsCache,
}

impl TagSets {
    /// Create a new tag sets store with the given cache
    pub fn new(storage: Storage, cache: TagSetsCache) -> Self {
        Self { storage, cache }
    }

    /// Store tags for a series.
    ///
    /// Uses write-through caching: updates both cache and storage.
    pub async fn insert(&self, series_id: SeriesId, tags: &str) -> crate::Result<()> {
        let key = Storage::tag_sets_key(series_id);
        self.storage
            .put(key, Bytes::from(tags.as_bytes().to_vec()))
            .await?;

        // Write-through: parse and cache the tag set
        let parsed = parse_key_value_pairs(tags)?;
        self.cache.insert(series_id, parsed);
        Ok(())
    }

    /// Get tags for a series.
    ///
    /// Checks the local cache first to avoid object storage roundtrips.
    pub async fn get(&self, series_id: SeriesId) -> crate::Result<OwnedTagSets> {
        // Check cache first
        if let Some(entry) = self.cache.get(&series_id) {
            return Ok(entry.value().clone());
        }

        // Cache miss - fetch from storage
        let key = Storage::tag_sets_key(series_id);

        match self.storage.get(&key).await? {
            Some(bytes) if !bytes.is_empty() => {
                let s = std::str::from_utf8(&bytes).map_err(|err| {
                    crate::Error::Io(io::Error::new(io::ErrorKind::InvalidData, err))
                })?;
                let parsed = parse_key_value_pairs(s)?;
                // Populate cache on miss
                self.cache.insert(series_id, parsed.clone());
                Ok(parsed)
            }
            _ => Ok(OwnedTagSets::default()),
        }
    }
}

fn parse_key_value_pairs(input: &str) -> crate::Result<OwnedTagSets> {
    input
        .split(';')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let mut split = pair.splitn(2, ':');

            if let (Some(key), Some(value)) = (split.next(), split.next()) {
                Ok((key.to_string(), value.to_string()))
            } else {
                Err(crate::Error::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid serialized tag",
                )))
            }
        })
        .collect()
}
