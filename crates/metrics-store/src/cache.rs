//! Local caching layer for performance with object storage backends.
//!
//! With `SlateDB` on object storage, every storage operation incurs network latency.
//! This module provides in-memory caching using foyer to minimize roundtrips for
//! frequently accessed metadata:
//!
//! - **Series Mapping**: Caches series key → series ID lookups (hot path on writes)
//! - **Tag Sets**: Caches series ID → tag set lookups (hot path on queries)
//!
//! Since we use a single-writer model, cache invalidation is straightforward:
//! we simply write-through on mutations.

use crate::SeriesId;
use crate::tag_sets::OwnedTagSets;
use foyer::{Cache, CacheBuilder};

/// Default capacity for series mapping cache (number of entries)
const DEFAULT_SERIES_CACHE_CAPACITY: usize = 100_000;

/// Default capacity for tag sets cache (number of entries)
const DEFAULT_TAG_SETS_CACHE_CAPACITY: usize = 100_000;

/// Cache for series key to series ID mappings.
///
/// This is on the critical write path - every write needs to check if the series exists.
pub type SeriesCache = Cache<String, SeriesId>;

/// Cache for series ID to tag set mappings.
///
/// This is on the critical query path - aggregations need tag sets for grouping.
pub type TagSetsCache = Cache<SeriesId, OwnedTagSets>;

/// Configuration for the local cache layer.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of series mappings to cache
    pub series_cache_capacity: usize,

    /// Maximum number of tag sets to cache
    pub tag_sets_cache_capacity: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            series_cache_capacity: DEFAULT_SERIES_CACHE_CAPACITY,
            tag_sets_cache_capacity: DEFAULT_TAG_SETS_CACHE_CAPACITY,
        }
    }
}

/// Local cache for metadata to reduce object storage roundtrips.
#[derive(Clone)]
pub struct LocalCache {
    /// Series key → Series ID cache
    series: SeriesCache,

    /// Series ID → Tag Set cache
    tag_sets: TagSetsCache,
}

impl LocalCache {
    /// Create a new local cache with the given configuration.
    #[must_use]
    pub fn new(config: &CacheConfig) -> Self {
        let series = CacheBuilder::new(config.series_cache_capacity).build();
        let tag_sets = CacheBuilder::new(config.tag_sets_cache_capacity).build();

        Self { series, tag_sets }
    }

    /// Get the series cache.
    #[must_use]
    pub fn series(&self) -> &SeriesCache {
        &self.series
    }

    /// Get the tag sets cache.
    #[must_use]
    pub fn tag_sets(&self) -> &TagSetsCache {
        &self.tag_sets
    }
}

impl Default for LocalCache {
    fn default() -> Self {
        Self::new(&CacheConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_series_cache() {
        let cache = LocalCache::default();

        // Insert and retrieve
        cache.series().insert("cpu.total#env:prod".to_string(), 42);
        let entry = cache.series().get("cpu.total#env:prod");
        assert!(entry.is_some());
        let entry = entry.unwrap_or_else(|| panic!("entry should exist"));
        assert_eq!(*entry.value(), 42);

        // Miss on unknown key
        assert!(cache.series().get("unknown").is_none());
    }

    #[test]
    fn test_tag_sets_cache() {
        let cache = LocalCache::default();

        let mut tags = OwnedTagSets::default();
        tags.insert("env".to_string(), "prod".to_string());
        tags.insert("host".to_string(), "h-1".to_string());

        cache.tag_sets().insert(42, tags.clone());

        let entry = cache.tag_sets().get(&42);
        assert!(entry.is_some());
        let cached = entry.unwrap_or_else(|| panic!("entry should exist"));
        assert_eq!(cached.value().get("env"), Some(&"prod".to_string()));
        assert_eq!(cached.value().get("host"), Some(&"h-1".to_string()));
    }
}
