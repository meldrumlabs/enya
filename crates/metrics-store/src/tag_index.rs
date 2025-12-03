//! Inverted tag index for efficient tag-based queries
//!
//! Maps tag terms to posting lists (lists of series IDs that have that tag).
//!
//! ## Term Formats
//!
//! - Metric-level term: `{metric}` - matches all series for a metric
//! - Tag term: `{metric}#{key}:{value}` - matches series with specific tag value
//!
//! ## Posting List Format
//!
//! Posting lists are stored as serialized `RoaringBitmap`s for efficient
//! compression and set operations.

use crate::storage::{Storage, prefix};
use crate::{MetricName, SeriesId, TagSet};
use bytes::Bytes;
use roaring::RoaringBitmap;
use std::io::{self, Cursor};

/// Inverted index mapping tag terms to series IDs
pub struct TagIndex {
    storage: Storage,
}

impl TagIndex {
    /// Create a new tag index
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// Index a series with the given metric and tags
    ///
    /// Creates index entries for:
    /// - The metric itself (to query all series for a metric)
    /// - Each tag key:value pair
    pub async fn index(
        &self,
        metric: MetricName<'_>,
        tags: &TagSet<'_>,
        series_id: SeriesId,
    ) -> crate::Result<()> {
        // Index the metric-level term
        self.index_term(metric.as_ref(), series_id).await?;

        // Index each tag
        for (key, value) in tags {
            let term = Self::format_key(metric.as_str(), key, value);
            self.index_term(&term, series_id).await?;
        }

        Ok(())
    }

    async fn index_term(&self, term: &str, series_id: SeriesId) -> crate::Result<()> {
        let key = Storage::tag_index_key(term);

        // Use merge operator to atomically add series ID to postings list
        // The merge operator expects a single u32 series ID as the operand
        let operand = Bytes::from(series_id.to_be_bytes().to_vec());
        self.storage.merge(key, operand).await
    }

    /// Format a tag index key
    #[must_use]
    pub fn format_key(metric_name: &str, key: &str, value: &str) -> String {
        let mut s = String::with_capacity(metric_name.len() + 1 + key.len() + 1 + value.len());
        s.push_str(metric_name);
        s.push('#');
        s.push_str(key);
        s.push(':');
        s.push_str(value);
        s
    }

    /// Query for series IDs matching an exact term
    #[allow(clippy::option_if_let_else)]
    pub async fn query_eq(&self, term: &str) -> crate::Result<Vec<SeriesId>> {
        let key = Storage::tag_index_key(term);
        match self.storage.get(&key).await? {
            Some(bytes) => Self::deserialize_postings_list(&bytes),
            None => Ok(Vec::new()),
        }
    }

    /// Query for series IDs matching a prefix
    ///
    /// Used for wildcard queries like `service:db.*`
    pub async fn query_prefix(&self, prefix_term: &str) -> crate::Result<Vec<SeriesId>> {
        let mut combined = RoaringBitmap::new();

        // Build the key range for prefix scan
        let start_key = Storage::tag_index_key(prefix_term);
        let end_key = {
            let mut end = prefix::TAG_INDEX.to_vec();
            end.extend_from_slice(prefix_term.as_bytes());
            // Append 0xFF to get exclusive upper bound for prefix
            end.push(0xFF);
            Bytes::from(end)
        };

        let mut iter = self.storage.db().scan(start_key..end_key).await?;

        while let Some(kv) = iter.next().await? {
            let bitmap = Self::deserialize_bitmap(&kv.value)?;
            combined |= bitmap;
        }

        // RoaringBitmap iterates in sorted order, so we just collect
        Ok(combined.iter().collect())
    }

    fn deserialize_postings_list(bytes: &[u8]) -> crate::Result<Vec<SeriesId>> {
        let bitmap = Self::deserialize_bitmap(bytes)?;
        Ok(bitmap.iter().collect())
    }

    fn deserialize_bitmap(bytes: &[u8]) -> crate::Result<RoaringBitmap> {
        let mut cursor = Cursor::new(bytes);
        RoaringBitmap::deserialize_from(&mut cursor).map_err(|e| {
            crate::Error::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to deserialize RoaringBitmap: {e}"),
            ))
        })
    }
}
