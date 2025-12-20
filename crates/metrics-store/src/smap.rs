//! Series mapping - maps series key strings to series IDs
//!
//! A series key is a unique string identifier for a time series, formatted as:
//! `{metric_name}#{key1:value1;key2:value2;...}` with tags sorted alphabetically.
//!
//! This module provides the mapping from these string keys to compact u32 series IDs
//! that are used in the data partition for efficient storage.
//!
//! ## Caching
//!
//! Since every write needs to check if a series exists, we use an in-memory cache
//! to avoid object storage roundtrips for known series. With a single-writer model,
//! cache invalidation is straightforward: we write-through on inserts.
//!
//! ## Series ID Counter
//!
//! The series ID counter is kept in memory for performance. With a single-writer
//! model, we don't need to read-modify-write on every new series. The counter is
//! persisted on flush/close to ensure durability across restarts.

use crate::SeriesId;
use crate::cache::SeriesCache;
use crate::storage::{Storage, prefix};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use rustc_hash::FxHashSet;
use std::io::Cursor;
use std::sync::atomic::{AtomicU32, Ordering};

/// Series mapping - maps series key strings to series IDs
pub struct SeriesMapping {
    storage: Storage,
    cache: SeriesCache,
    /// In-memory series ID counter for fast allocation
    next_id: AtomicU32,
}

impl SeriesMapping {
    /// Create a new series mapping with the given cache.
    ///
    /// The counter starts at 0 and should be initialized from storage
    /// using [`Self::load_counter`] before use.
    pub fn new(storage: Storage, cache: SeriesCache) -> Self {
        Self {
            storage,
            cache,
            next_id: AtomicU32::new(0),
        }
    }

    /// Load the series ID counter from storage.
    ///
    /// This should be called once during database initialization to restore
    /// the counter value from the previous session.
    pub async fn load_counter(&self) -> crate::Result<()> {
        let current = match self.storage.get(prefix::NEXT_SERIES_ID).await? {
            Some(bytes) => Self::read_series_id(&bytes)?,
            None => 0,
        };
        self.next_id.store(current, Ordering::SeqCst);
        log::info!("Loaded series ID counter: {current}");
        Ok(())
    }

    /// Persist the series ID counter to storage.
    ///
    /// This should be called periodically and on database close to ensure
    /// the counter survives restarts.
    pub async fn flush_counter(&self) -> crate::Result<()> {
        let current = self.next_id.load(Ordering::SeqCst);
        self.storage
            .put(
                Bytes::from_static(prefix::NEXT_SERIES_ID),
                Bytes::from(current.to_be_bytes().to_vec()),
            )
            .await?;
        log::trace!("Flushed series ID counter: {current}");
        Ok(())
    }

    /// Get the series ID for a series key.
    ///
    /// Checks the local cache first to avoid object storage roundtrips.
    pub async fn get(&self, series_key: &str) -> crate::Result<Option<SeriesId>> {
        // Check cache first
        if let Some(entry) = self.cache.get(series_key) {
            return Ok(Some(*entry.value()));
        }

        // Cache miss - fetch from storage
        let key = Storage::smap_key(series_key);
        match self.storage.get(&key).await? {
            Some(bytes) => {
                let series_id = Self::read_series_id(&bytes)?;
                // Populate cache on miss
                self.cache.insert(series_key.to_string(), series_id);
                Ok(Some(series_id))
            }
            None => Ok(None),
        }
    }

    /// Insert a new series key -> series ID mapping.
    ///
    /// Uses write-through caching: updates both cache and storage.
    pub async fn insert(&self, series_key: &str, series_id: SeriesId) -> crate::Result<()> {
        let key = Storage::smap_key(series_key);
        self.storage
            .put(key, Bytes::from(series_id.to_be_bytes().to_vec()))
            .await?;

        // Write-through: update cache after successful storage write
        self.cache.insert(series_key.to_string(), series_id);
        Ok(())
    }

    /// Get the next available series ID.
    ///
    /// This increments the counter and persists it to storage **before** returning
    /// the ID. This ensures crash safety: if we crash after persisting but before
    /// using the ID, we may have gaps in IDs but never collisions.
    ///
    /// # Errors
    ///
    /// Returns an error if persisting the counter fails.
    pub async fn next_series_id(&self) -> crate::Result<SeriesId> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // Persist the incremented counter before returning
        // This ensures crash safety: worst case we have ID gaps, never collisions
        self.flush_counter().await?;

        Ok(id)
    }

    /// List all series IDs in the mapping
    ///
    /// Note: This performs a full scan and may be expensive for large datasets.
    pub async fn list_all(&self) -> crate::Result<FxHashSet<SeriesId>> {
        let mut result = FxHashSet::default();

        // Scan all keys with SMAP prefix
        let start = Bytes::from_static(prefix::SMAP);
        let end = {
            let mut e = prefix::SMAP.to_vec();
            // Increment last byte to get exclusive upper bound
            if let Some(last) = e.last_mut() {
                *last = last.saturating_add(1);
            }
            Bytes::from(e)
        };

        let mut iter = self.storage.db().scan(start..end).await?;

        while let Some(kv) = iter.next().await? {
            let series_id = Self::read_series_id(&kv.value)?;
            result.insert(series_id);
        }

        Ok(result)
    }

    fn read_series_id(bytes: &[u8]) -> crate::Result<SeriesId> {
        let mut reader = Cursor::new(bytes);
        reader.read_u32::<BigEndian>().map_err(crate::Error::from)
    }
}
