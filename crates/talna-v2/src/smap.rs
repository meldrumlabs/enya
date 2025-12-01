//! Series mapping - maps series key strings to series IDs
//!
//! A series key is a unique string identifier for a time series, formatted as:
//! `{metric_name}#{key1:value1;key2:value2;...}` with tags sorted alphabetically.
//!
//! This module provides the mapping from these string keys to compact u64 series IDs
//! that are used in the data partition for efficient storage.

use crate::SeriesId;
use crate::storage::{Storage, prefix};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use std::collections::HashSet;
use std::io::Cursor;

/// Series mapping - maps series key strings to series IDs
pub struct SeriesMapping {
    storage: Storage,
}

impl SeriesMapping {
    /// Create a new series mapping
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// Get the series ID for a series key
    pub async fn get(&self, series_key: &str) -> crate::Result<Option<SeriesId>> {
        let key = Storage::smap_key(series_key);
        match self.storage.get(&key).await? {
            Some(bytes) => Ok(Some(Self::read_series_id(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Insert a new series key -> series ID mapping
    pub async fn insert(&self, series_key: &str, series_id: SeriesId) -> crate::Result<()> {
        let key = Storage::smap_key(series_key);
        self.storage
            .put(key, Bytes::from(series_id.to_be_bytes().to_vec()))
            .await
    }

    /// Get the next available series ID atomically
    ///
    /// This uses a counter stored in the database to ensure unique IDs
    /// even across restarts.
    pub async fn next_series_id(&self) -> crate::Result<SeriesId> {
        // Read current counter
        let current = match self.storage.get(prefix::NEXT_SERIES_ID).await? {
            Some(bytes) => Self::read_series_id(&bytes)?,
            None => 0,
        };

        // Increment and store
        let next = current + 1;
        self.storage
            .put(
                Bytes::from_static(prefix::NEXT_SERIES_ID),
                Bytes::from(next.to_be_bytes().to_vec()),
            )
            .await?;

        Ok(current)
    }

    /// List all series IDs in the mapping
    ///
    /// Note: This performs a full scan and may be expensive for large datasets.
    pub async fn list_all(&self) -> crate::Result<HashSet<SeriesId>> {
        let mut result = HashSet::new();

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
        reader.read_u64::<BigEndian>().map_err(crate::Error::from)
    }
}
