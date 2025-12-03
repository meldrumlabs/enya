//! Merge operators for atomic updates in `SlateDB`
//!
//! This module provides merge operators that enable atomic updates without
//! read-modify-write cycles, preventing race conditions during concurrent writes.
//!
//! ## Postings List Merge
//!
//! The postings list merge operator adds series IDs atomically using `RoaringBitmap`:
//! - Each merge operand is a single u32 series ID (4 bytes, big-endian)
//! - The merge adds the ID to the existing bitmap
//!
//! ## Storage Format
//!
//! Postings lists are stored as serialized `RoaringBitmap`s.

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use roaring::RoaringBitmap;
use slatedb::{MergeOperator, MergeOperatorError};
use std::io::Cursor;

/// Merge operator for metrics-store that handles postings lists
///
/// This operator adds series IDs to postings lists atomically using `RoaringBitmap`.
/// Keys with prefix `t:` (tag index) use bitmap merge semantics.
pub struct MetricsMergeOperator;

impl MergeOperator for MetricsMergeOperator {
    fn merge(
        &self,
        key: &Bytes,
        existing_value: Option<Bytes>,
        operand: Bytes,
    ) -> Result<Bytes, MergeOperatorError> {
        // Only tag index keys use merge
        if key.starts_with(b"t:") {
            Ok(Self::merge_bitmap_single(existing_value, &operand))
        } else {
            // For unknown prefixes, just use the new value
            Ok(operand)
        }
    }

    fn merge_batch(
        &self,
        key: &Bytes,
        existing_value: Option<Bytes>,
        operands: &[Bytes],
    ) -> Result<Bytes, MergeOperatorError> {
        if key.starts_with(b"t:") {
            Ok(Self::merge_bitmap_batch(existing_value.as_ref(), operands))
        } else {
            // For unknown prefixes, use the last operand
            operands
                .last()
                .cloned()
                .or(existing_value)
                .ok_or(MergeOperatorError::EmptyBatch)
        }
    }
}

impl MetricsMergeOperator {
    /// Parse an operand which can be either:
    /// - A raw series ID: 4 bytes `[series_id:u32]`
    /// - A previously merged bitmap (variable length serialized `RoaringBitmap`)
    ///
    /// Returns the `RoaringBitmap` containing the IDs, or None on parse error.
    fn parse_operand(operand: &Bytes) -> Option<RoaringBitmap> {
        let mut cursor = Cursor::new(&operand[..]);
        if operand.len() == 4 {
            // Raw series ID (4 bytes)
            let id = cursor.read_u32::<BigEndian>().ok()?;
            let mut bitmap = RoaringBitmap::new();
            bitmap.insert(id);
            Some(bitmap)
        } else {
            // Previously merged bitmap
            RoaringBitmap::deserialize_from(&mut cursor).ok()
        }
    }

    /// Deserialize an existing bitmap or create a new one.
    /// Returns None on deserialization error.
    fn deserialize_existing(existing: Option<&Bytes>) -> Option<RoaringBitmap> {
        existing.map_or_else(
            || Some(RoaringBitmap::new()),
            |bytes| {
                let mut cursor = Cursor::new(&bytes[..]);
                RoaringBitmap::deserialize_from(&mut cursor).ok()
            },
        )
    }

    /// Merge a single series ID into the postings list.
    ///
    /// On corrupted data, logs a warning and recovers gracefully:
    /// - Corrupted existing value: starts fresh with just the new operand
    /// - Corrupted operand: keeps the existing value unchanged
    fn merge_bitmap_single(existing: Option<Bytes>, operand: &Bytes) -> Bytes {
        // Try to parse the operand
        let Some(new_bitmap) = Self::parse_operand(operand) else {
            log::warn!(
                "Failed to parse merge operand ({} bytes), skipping",
                operand.len()
            );
            // Return existing value if available, otherwise empty bitmap
            return existing.unwrap_or_else(|| Self::serialize_bitmap(&RoaringBitmap::new()));
        };

        // Try to deserialize existing, start fresh if corrupted
        let mut result = Self::deserialize_existing(existing.as_ref()).unwrap_or_else(|| {
            log::warn!("Failed to deserialize existing bitmap, starting fresh");
            RoaringBitmap::new()
        });

        result |= new_bitmap;
        Self::serialize_bitmap(&result)
    }

    /// Merge multiple series IDs into the postings list efficiently.
    ///
    /// On corrupted data, logs warnings and recovers gracefully:
    /// - Corrupted existing value: starts fresh
    /// - Corrupted operands: skips them and continues with valid ones
    fn merge_bitmap_batch(existing: Option<&Bytes>, operands: &[Bytes]) -> Bytes {
        // Try to deserialize existing, start fresh if corrupted
        let mut result = Self::deserialize_existing(existing).unwrap_or_else(|| {
            if existing.is_some() {
                log::warn!("Failed to deserialize existing bitmap in batch, starting fresh");
            }
            RoaringBitmap::new()
        });

        for operand in operands {
            if let Some(bitmap) = Self::parse_operand(operand) {
                result |= bitmap;
            } else {
                log::warn!(
                    "Failed to parse merge operand ({} bytes) in batch, skipping",
                    operand.len()
                );
            }
        }

        Self::serialize_bitmap(&result)
    }

    /// Serialize a `RoaringBitmap` to bytes.
    ///
    /// Serialization to a `Vec` should never fail in practice.
    fn serialize_bitmap(bitmap: &RoaringBitmap) -> Bytes {
        let mut buf = Vec::with_capacity(bitmap.serialized_size());
        if let Err(e) = bitmap.serialize_into(&mut buf) {
            // This should never happen with Vec, but log if it does
            log::error!("Failed to serialize bitmap: {e}");
        }
        Bytes::from(buf)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    #[test]
    fn test_postings_list_merge_empty() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total#host:h-1");

        // First merge creates new bitmap
        let mut operand = Vec::new();
        operand.write_u32::<BigEndian>(42).unwrap();

        let result = op.merge(&key, None, Bytes::from(operand)).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 1);
        assert!(bitmap.contains(42));
    }

    #[test]
    fn test_postings_list_merge_append() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total#host:h-1");

        // Create existing bitmap with one entry
        let mut existing_bitmap = RoaringBitmap::new();
        existing_bitmap.insert(42);
        let mut existing = Vec::new();
        existing_bitmap.serialize_into(&mut existing).unwrap();

        // Append new ID
        let mut operand = Vec::new();
        operand.write_u32::<BigEndian>(99).unwrap();

        let result = op
            .merge(&key, Some(Bytes::from(existing)), Bytes::from(operand))
            .unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 2);
        assert!(bitmap.contains(42));
        assert!(bitmap.contains(99));
    }

    #[test]
    fn test_postings_list_merge_batch() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Create operands
        let mut op1 = Vec::new();
        op1.write_u32::<BigEndian>(1).unwrap();
        let mut op2 = Vec::new();
        op2.write_u32::<BigEndian>(2).unwrap();
        let mut op3 = Vec::new();
        op3.write_u32::<BigEndian>(3).unwrap();

        let operands = vec![Bytes::from(op1), Bytes::from(op2), Bytes::from(op3)];

        let result = op.merge_batch(&key, None, &operands).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 3);
        assert!(bitmap.contains(1));
        assert!(bitmap.contains(2));
        assert!(bitmap.contains(3));
    }

    #[test]
    fn test_postings_list_merge_batch_with_existing() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Existing bitmap
        let mut existing_bitmap = RoaringBitmap::new();
        existing_bitmap.insert(10);
        existing_bitmap.insert(20);
        let mut existing = Vec::new();
        existing_bitmap.serialize_into(&mut existing).unwrap();

        // New operands
        let mut op1 = Vec::new();
        op1.write_u32::<BigEndian>(30).unwrap();
        let mut op2 = Vec::new();
        op2.write_u32::<BigEndian>(40).unwrap();

        let operands = vec![Bytes::from(op1), Bytes::from(op2)];

        let result = op
            .merge_batch(&key, Some(Bytes::from(existing)), &operands)
            .unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 4);
        assert!(bitmap.contains(10));
        assert!(bitmap.contains(20));
        assert!(bitmap.contains(30));
        assert!(bitmap.contains(40));
    }

    #[test]
    fn test_postings_list_merge_multiple() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Build bitmap by merging multiple IDs
        let mut operand1 = Vec::new();
        operand1.write_u32::<BigEndian>(1).unwrap();
        let result = op.merge(&key, None, Bytes::from(operand1)).unwrap();

        let mut operand2 = Vec::new();
        operand2.write_u32::<BigEndian>(2).unwrap();
        let result = op.merge(&key, Some(result), Bytes::from(operand2)).unwrap();

        let mut operand3 = Vec::new();
        operand3.write_u32::<BigEndian>(3).unwrap();
        let result = op.merge(&key, Some(result), Bytes::from(operand3)).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 3);
        assert!(bitmap.contains(1));
        assert!(bitmap.contains(2));
        assert!(bitmap.contains(3));
    }

    #[test]
    fn test_postings_list_merge_duplicates() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Merge same ID multiple times - bitmap should deduplicate
        let mut operand = Vec::new();
        operand.write_u32::<BigEndian>(42).unwrap();

        let result = op.merge(&key, None, Bytes::from(operand.clone())).unwrap();
        let result = op
            .merge(&key, Some(result), Bytes::from(operand.clone()))
            .unwrap();
        let result = op.merge(&key, Some(result), Bytes::from(operand)).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 1); // Still only 1 entry
        assert!(bitmap.contains(42));
    }

    #[test]
    fn test_non_tag_key_passthrough() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"s:some_series_key");

        let operand = Bytes::from_static(b"some_value");
        let result = op.merge(&key, None, operand.clone()).unwrap();

        // Non-tag keys should just return the operand
        assert_eq!(result, operand);
    }

    #[test]
    fn test_corrupted_operand_skipped_gracefully() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Invalid operand (5 bytes - not a valid u32 or bitmap)
        let operand = Bytes::from_static(b"12345");

        // Should not panic, returns empty bitmap
        let result = op.merge(&key, None, operand).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert!(bitmap.is_empty());
    }

    #[test]
    fn test_corrupted_existing_starts_fresh() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Invalid existing value
        let existing = Bytes::from_static(b"not a valid bitmap");

        let mut operand = Vec::new();
        operand.write_u32::<BigEndian>(42).unwrap();

        // Should not panic, starts fresh with just the new operand
        let result = op
            .merge(&key, Some(existing), Bytes::from(operand))
            .unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 1);
        assert!(bitmap.contains(42));
    }

    #[test]
    fn test_batch_with_some_corrupted_operands() {
        let op = MetricsMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Mix of valid and invalid operands
        let mut op1 = Vec::new();
        op1.write_u32::<BigEndian>(1).unwrap();
        let op2 = Bytes::from_static(b"invalid");
        let mut op3 = Vec::new();
        op3.write_u32::<BigEndian>(3).unwrap();

        let operands = vec![Bytes::from(op1), op2, Bytes::from(op3)];

        // Should skip the invalid operand and merge the valid ones
        let result = op.merge_batch(&key, None, &operands).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        let bitmap = RoaringBitmap::deserialize_from(&mut cursor).unwrap();
        assert_eq!(bitmap.len(), 2);
        assert!(bitmap.contains(1));
        assert!(bitmap.contains(3));
    }
}
