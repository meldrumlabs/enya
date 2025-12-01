//! Merge operators for atomic updates in `SlateDB`
//!
//! This module provides merge operators that enable atomic updates without
//! read-modify-write cycles, preventing race conditions during concurrent writes.
//!
//! ## Postings List Merge
//!
//! The postings list merge operator appends series IDs atomically:
//! - Each merge operand is a single u64 series ID (8 bytes, big-endian)
//! - The merge appends the ID to the existing list
//!
//! ## Storage Format
//!
//! Postings lists are stored as: `[len:u64][id1:u64][id2:u64]...`

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use slatedb::{MergeOperator, MergeOperatorError};
use std::io::Cursor;

/// Merge operator for talna-v2 that handles postings lists
///
/// This operator appends series IDs to postings lists atomically.
/// Keys with prefix `t:` (tag index) use postings list append semantics.
pub struct TalnaMergeOperator;

impl MergeOperator for TalnaMergeOperator {
    fn merge(
        &self,
        key: &Bytes,
        existing_value: Option<Bytes>,
        operand: Bytes,
    ) -> Result<Bytes, MergeOperatorError> {
        // Only tag index keys use merge
        if key.starts_with(b"t:") {
            Ok(Self::merge_postings_list_single(existing_value, &operand))
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
            Ok(Self::merge_postings_list_batch(existing_value, operands))
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

#[allow(
    clippy::expect_used,
    clippy::cast_possible_truncation,
    clippy::option_if_let_else,
    clippy::single_match_else
)]
impl TalnaMergeOperator {
    /// Parse an operand which can be either:
    /// - A raw series ID: 8 bytes `[series_id:u64]`
    /// - A previously merged postings list: 16+ bytes `[len:u64][id1:u64]...`
    ///
    /// Returns the list of series IDs contained in the operand.
    fn parse_operand(operand: &Bytes) -> Vec<u64> {
        if operand.len() == 8 {
            // Raw series ID
            let mut cursor = Cursor::new(&operand[..]);
            let id = cursor
                .read_u64::<BigEndian>()
                .expect("failed to read series ID from operand");
            vec![id]
        } else if operand.len() >= 16 && operand.len() % 8 == 0 {
            // Previously merged postings list: [len:u64][id1:u64][id2:u64]...
            let mut cursor = Cursor::new(&operand[..]);
            let len = cursor
                .read_u64::<BigEndian>()
                .expect("failed to read postings list length");

            let mut ids = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let id = cursor
                    .read_u64::<BigEndian>()
                    .expect("failed to read ID from operand");
                ids.push(id);
            }
            ids
        } else {
            panic!(
                "invalid postings list operand: expected 8 bytes (raw ID) or 16+ bytes (postings list), got {}",
                operand.len()
            );
        }
    }

    /// Merge a single series ID into the postings list
    ///
    /// Format:
    /// - Existing value: `[len:u64][id1:u64][id2:u64]...`
    /// - Operand: single `[series_id:u64]` (8 bytes) or postings list (16+ bytes)
    /// - Result: `[new_len:u64][id1:u64]...[new_id:u64]`
    fn merge_postings_list_single(existing: Option<Bytes>, operand: &Bytes) -> Bytes {
        let new_ids = Self::parse_operand(operand);

        // If only one ID, use the optimized single append path
        if let [single_id] = new_ids.as_slice() {
            return Self::append_id_to_postings(existing, *single_id);
        }

        // Otherwise, merge as a batch
        Self::merge_ids_into_postings(existing, &new_ids)
    }

    /// Merge multiple series IDs into the postings list efficiently
    fn merge_postings_list_batch(existing: Option<Bytes>, operands: &[Bytes]) -> Bytes {
        // Parse all operands - each can be a raw ID or a postings list
        let mut new_ids = Vec::with_capacity(operands.len());
        for operand in operands {
            new_ids.extend(Self::parse_operand(operand));
        }

        Self::merge_ids_into_postings(existing, &new_ids)
    }

    /// Merge a list of IDs into an existing postings list
    fn merge_ids_into_postings(existing: Option<Bytes>, new_ids: &[u64]) -> Bytes {
        // Parse existing postings list if any
        let (existing_len, existing_ids) = match existing {
            Some(bytes) => {
                let mut cursor = Cursor::new(&bytes[..]);
                let len = cursor
                    .read_u64::<BigEndian>()
                    .expect("failed to read postings list length");

                let mut ids = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    let id = cursor
                        .read_u64::<BigEndian>()
                        .expect("failed to read existing ID");
                    ids.push(id);
                }
                (len, ids)
            }
            None => (0, Vec::new()),
        };

        // Create new buffer with space for all IDs
        let new_len = existing_len + new_ids.len() as u64;
        let mut result = Vec::with_capacity((new_len as usize + 1) * 8);
        result
            .write_u64::<BigEndian>(new_len)
            .expect("failed to write length");

        // Write existing IDs
        for id in existing_ids {
            result
                .write_u64::<BigEndian>(id)
                .expect("failed to write ID");
        }

        // Write new IDs
        for id in new_ids {
            result
                .write_u64::<BigEndian>(*id)
                .expect("failed to write ID");
        }

        Bytes::from(result)
    }

    /// Append a single ID to the postings list
    fn append_id_to_postings(existing: Option<Bytes>, new_id: u64) -> Bytes {
        match existing {
            Some(bytes) => {
                // Parse existing postings list
                let mut cursor = Cursor::new(&bytes[..]);
                let len = cursor
                    .read_u64::<BigEndian>()
                    .expect("failed to read postings list length");

                // Create new buffer with space for one more ID
                let new_len = len + 1;
                let mut result = Vec::with_capacity((new_len as usize + 1) * 8);
                result
                    .write_u64::<BigEndian>(new_len)
                    .expect("failed to write length");

                // Copy existing IDs
                for _ in 0..len {
                    let id = cursor
                        .read_u64::<BigEndian>()
                        .expect("failed to read existing ID");
                    result
                        .write_u64::<BigEndian>(id)
                        .expect("failed to write ID");
                }

                // Append new ID
                result
                    .write_u64::<BigEndian>(new_id)
                    .expect("failed to write new ID");

                Bytes::from(result)
            }
            None => {
                // Create new postings list with single entry
                let mut result = Vec::with_capacity(16);
                result
                    .write_u64::<BigEndian>(1)
                    .expect("failed to write length");
                result
                    .write_u64::<BigEndian>(new_id)
                    .expect("failed to write ID");
                Bytes::from(result)
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_postings_list_merge_empty() {
        let op = TalnaMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total#host:h-1");

        // First merge creates new list
        let mut operand = Vec::new();
        operand.write_u64::<BigEndian>(42).unwrap();

        let result = op.merge(&key, None, Bytes::from(operand)).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 1); // length
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 42); // series ID
    }

    #[test]
    fn test_postings_list_merge_append() {
        let op = TalnaMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total#host:h-1");

        // Create existing list with one entry
        let mut existing = Vec::new();
        existing.write_u64::<BigEndian>(1).unwrap(); // length
        existing.write_u64::<BigEndian>(42).unwrap(); // ID

        // Append new ID
        let mut operand = Vec::new();
        operand.write_u64::<BigEndian>(99).unwrap();

        let result = op
            .merge(&key, Some(Bytes::from(existing)), Bytes::from(operand))
            .unwrap();

        let mut cursor = Cursor::new(&result[..]);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 2); // new length
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 42); // first ID
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 99); // second ID
    }

    #[test]
    fn test_postings_list_merge_batch() {
        let op = TalnaMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Create operands
        let mut op1 = Vec::new();
        op1.write_u64::<BigEndian>(1).unwrap();
        let mut op2 = Vec::new();
        op2.write_u64::<BigEndian>(2).unwrap();
        let mut op3 = Vec::new();
        op3.write_u64::<BigEndian>(3).unwrap();

        let operands = vec![Bytes::from(op1), Bytes::from(op2), Bytes::from(op3)];

        let result = op.merge_batch(&key, None, &operands).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 3); // length
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 1); // first ID
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 2); // second ID
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 3); // third ID
    }

    #[test]
    fn test_postings_list_merge_batch_with_existing() {
        let op = TalnaMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Existing list
        let mut existing = Vec::new();
        existing.write_u64::<BigEndian>(2).unwrap(); // length
        existing.write_u64::<BigEndian>(10).unwrap();
        existing.write_u64::<BigEndian>(20).unwrap();

        // New operands
        let mut op1 = Vec::new();
        op1.write_u64::<BigEndian>(30).unwrap();
        let mut op2 = Vec::new();
        op2.write_u64::<BigEndian>(40).unwrap();

        let operands = vec![Bytes::from(op1), Bytes::from(op2)];

        let result = op
            .merge_batch(&key, Some(Bytes::from(existing)), &operands)
            .unwrap();

        let mut cursor = Cursor::new(&result[..]);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 4); // length
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 10);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 20);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 30);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 40);
    }

    #[test]
    fn test_postings_list_merge_multiple() {
        let op = TalnaMergeOperator;
        let key = Bytes::from_static(b"t:cpu.total");

        // Build list by merging multiple IDs
        let mut operand1 = Vec::new();
        operand1.write_u64::<BigEndian>(1).unwrap();
        let result = op.merge(&key, None, Bytes::from(operand1)).unwrap();

        let mut operand2 = Vec::new();
        operand2.write_u64::<BigEndian>(2).unwrap();
        let result = op.merge(&key, Some(result), Bytes::from(operand2)).unwrap();

        let mut operand3 = Vec::new();
        operand3.write_u64::<BigEndian>(3).unwrap();
        let result = op.merge(&key, Some(result), Bytes::from(operand3)).unwrap();

        let mut cursor = Cursor::new(&result[..]);
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 3); // length
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 1); // first ID
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 2); // second ID
        assert_eq!(cursor.read_u64::<BigEndian>().unwrap(), 3); // third ID
    }

    #[test]
    fn test_non_tag_key_passthrough() {
        let op = TalnaMergeOperator;
        let key = Bytes::from_static(b"s:some_series_key");

        let operand = Bytes::from_static(b"some_value");
        let result = op.merge(&key, None, operand.clone()).unwrap();

        // Non-tag keys should just return the operand
        assert_eq!(result, operand);
    }
}
