//! Diff computation utilities for comparing query results.
//!
//! Uses the `similar` crate for word-level diff highlighting,
//! matching the style of the git diff viewer.

use enya_datafusion::arrow::array::RecordBatch;
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::format_array_value;
use rustc_hash::FxHashMap;
use similar::{ChangeTag, TextDiff};

use super::types::DiffStats;

/// Status of a row in the diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowDiffStatus {
    /// Row exists in both sides and is identical.
    Matching,
    /// Row only exists in the left (source) side.
    LeftOnly,
    /// Row only exists in the right (target) side.
    RightOnly,
}

/// A row with its diff status and values.
#[derive(Debug, Clone)]
pub(super) struct DiffRow {
    /// The row values as strings.
    pub values: Vec<String>,
    /// The diff status of this row.
    pub status: RowDiffStatus,
    /// Word-level highlights for each cell (start, end byte indices).
    /// Only populated for rows that have a corresponding row on the other side.
    #[allow(dead_code)] // Prepared for word-level highlighting feature
    pub cell_highlights: Vec<Vec<(usize, usize)>>,
}

/// Paired rows for side-by-side diff display.
#[derive(Debug, Clone)]
pub(super) struct DiffRowPair {
    /// Left side row (None if right-only).
    pub left: Option<DiffRow>,
    /// Right side row (None if left-only).
    pub right: Option<DiffRow>,
}

/// Complete diff result with paired rows for rendering.
#[derive(Debug, Clone)]
pub(super) struct TableDiff {
    /// Column names from the schema.
    pub columns: Vec<String>,
    /// Paired rows for side-by-side display.
    pub rows: Vec<DiffRowPair>,
    /// Statistics about the diff.
    pub stats: DiffStats,
}

/// Compute a detailed table diff with row pairing and cell-level highlighting.
pub(super) fn compute_detailed_diff(
    left_schema: Option<&SchemaRef>,
    left_batches: &[RecordBatch],
    right_schema: Option<&SchemaRef>,
    right_batches: &[RecordBatch],
) -> TableDiff {
    // Get column names
    let columns: Vec<String> = left_schema
        .or(right_schema)
        .map(|s| s.fields().iter().map(|f| f.name().clone()).collect())
        .unwrap_or_default();

    // Extract all rows as string vectors
    let left_rows: Vec<Vec<String>> = extract_all_rows(left_batches);
    let right_rows: Vec<Vec<String>> = extract_all_rows(right_batches);

    // Build hash map of left rows for matching
    let mut left_row_map: FxHashMap<u64, Vec<usize>> = FxHashMap::default();
    for (idx, row) in left_rows.iter().enumerate() {
        let hash = hash_row_values(row);
        left_row_map.entry(hash).or_default().push(idx);
    }

    // Track which left rows have been matched
    let mut left_matched: Vec<bool> = vec![false; left_rows.len()];
    let mut paired_rows: Vec<DiffRowPair> = Vec::new();

    // Stats
    let mut matching = 0usize;
    let mut right_only_count = 0usize;

    // Process right rows and find matches
    for right_row in &right_rows {
        let hash = hash_row_values(right_row);

        let matched_left_idx = left_row_map
            .get(&hash)
            .and_then(|indices| indices.iter().find(|&&idx| !left_matched[idx]).copied());

        if let Some(left_idx) = matched_left_idx {
            // Found a match
            left_matched[left_idx] = true;
            matching += 1;

            let left_row = &left_rows[left_idx];

            // Check if rows are actually identical or just have same hash
            if left_row == right_row {
                // Identical rows
                paired_rows.push(DiffRowPair {
                    left: Some(DiffRow {
                        values: left_row.clone(),
                        status: RowDiffStatus::Matching,
                        cell_highlights: vec![],
                    }),
                    right: Some(DiffRow {
                        values: right_row.clone(),
                        status: RowDiffStatus::Matching,
                        cell_highlights: vec![],
                    }),
                });
            } else {
                // Hash collision - treat as different (shouldn't happen often)
                paired_rows.push(DiffRowPair {
                    left: Some(DiffRow {
                        values: left_row.clone(),
                        status: RowDiffStatus::LeftOnly,
                        cell_highlights: vec![],
                    }),
                    right: None,
                });
                paired_rows.push(DiffRowPair {
                    left: None,
                    right: Some(DiffRow {
                        values: right_row.clone(),
                        status: RowDiffStatus::RightOnly,
                        cell_highlights: vec![],
                    }),
                });
                right_only_count += 1;
            }
        } else {
            // No match - right only
            right_only_count += 1;
            paired_rows.push(DiffRowPair {
                left: None,
                right: Some(DiffRow {
                    values: right_row.clone(),
                    status: RowDiffStatus::RightOnly,
                    cell_highlights: vec![],
                }),
            });
        }
    }

    // Add unmatched left rows
    let mut left_only_count = 0usize;
    for (idx, matched) in left_matched.iter().enumerate() {
        if !matched {
            left_only_count += 1;
            paired_rows.push(DiffRowPair {
                left: Some(DiffRow {
                    values: left_rows[idx].clone(),
                    status: RowDiffStatus::LeftOnly,
                    cell_highlights: vec![],
                }),
                right: None,
            });
        }
    }

    // Sort rows: matching first, then left-only, then right-only
    paired_rows.sort_by_key(|pair| match (&pair.left, &pair.right) {
        (Some(l), Some(_)) if l.status == RowDiffStatus::Matching => 0,
        (Some(_), None) => 1,
        (None, Some(_)) => 2,
        _ => 3,
    });

    TableDiff {
        columns,
        rows: paired_rows,
        stats: DiffStats {
            matching,
            left_only: left_only_count,
            right_only: right_only_count,
            different: 0,
        },
    }
}

/// Extract all rows from batches as string vectors.
fn extract_all_rows(batches: &[RecordBatch]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for batch in batches {
        for row_idx in 0..batch.num_rows() {
            rows.push(get_row_values(batch, row_idx));
        }
    }
    rows
}

/// Hash a row's values for quick comparison.
fn hash_row_values(values: &[String]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    for v in values {
        v.hash(&mut hasher);
    }
    hasher.finish()
}

/// Compute word-level highlights between two strings.
/// Returns highlight ranges for both the old and new strings.
#[allow(dead_code, clippy::type_complexity)] // Prepared for cell-level word highlighting feature
pub(super) fn compute_word_highlights(
    old: &str,
    new: &str,
) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    let diff = TextDiff::from_words(old, new);

    let mut old_highlights: Vec<(usize, usize)> = Vec::new();
    let mut new_highlights: Vec<(usize, usize)> = Vec::new();

    let mut old_pos = 0;
    let mut new_pos = 0;

    for change in diff.iter_all_changes() {
        let text = change.value();
        let len = text.len();

        match change.tag() {
            ChangeTag::Delete => {
                old_highlights.push((old_pos, old_pos + len));
                old_pos += len;
            }
            ChangeTag::Insert => {
                new_highlights.push((new_pos, new_pos + len));
                new_pos += len;
            }
            ChangeTag::Equal => {
                old_pos += len;
                new_pos += len;
            }
        }
    }

    // Merge adjacent highlights
    (
        merge_adjacent_highlights(old_highlights),
        merge_adjacent_highlights(new_highlights),
    )
}

/// Merge adjacent or overlapping highlight ranges.
#[allow(dead_code)] // Used by compute_word_highlights
fn merge_adjacent_highlights(mut highlights: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if highlights.is_empty() {
        return highlights;
    }

    highlights.sort_by_key(|&(start, _)| start);

    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut current = highlights[0];

    for &(start, end) in &highlights[1..] {
        if start <= current.1 {
            current.1 = current.1.max(end);
        } else {
            merged.push(current);
            current = (start, end);
        }
    }
    merged.push(current);

    merged
}

/// Check if two schemas are compatible for comparison.
/// Schemas are compatible if they have the same column names (in any order).
pub(super) fn schemas_compatible(left: &SchemaRef, right: &SchemaRef) -> bool {
    if left.fields().len() != right.fields().len() {
        return false;
    }

    let left_names: rustc_hash::FxHashSet<_> =
        left.fields().iter().map(|f| f.name().as_str()).collect();

    right
        .fields()
        .iter()
        .all(|f| left_names.contains(f.name().as_str()))
}

/// Compute a hash for a row in a record batch.
/// Used for quick comparison of rows across result sets.
fn hash_row(batch: &RecordBatch, row_idx: usize) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = rustc_hash::FxHasher::default();

    for col_idx in 0..batch.num_columns() {
        let col = batch.column(col_idx);
        // Hash the string representation of the value
        // This is simple but works for comparison purposes
        let value_str = format_array_value(col.as_ref(), row_idx);
        value_str.hash(&mut hasher);
    }

    hasher.finish()
}

/// Compute table diff statistics between two result sets.
/// Returns stats about matching rows, differences, and rows unique to each side.
pub(super) fn compute_table_diff(
    left_batches: &[RecordBatch],
    right_batches: &[RecordBatch],
) -> DiffStats {
    // Build hash map of left rows
    let mut left_hashes: FxHashMap<u64, usize> = FxHashMap::default();
    for batch in left_batches {
        for row_idx in 0..batch.num_rows() {
            let hash = hash_row(batch, row_idx);
            *left_hashes.entry(hash).or_insert(0) += 1;
        }
    }

    // Count right rows and find matches
    let mut matching = 0usize;
    let mut right_only = 0usize;

    for batch in right_batches {
        for row_idx in 0..batch.num_rows() {
            let hash = hash_row(batch, row_idx);
            if let Some(count) = left_hashes.get_mut(&hash) {
                if *count > 0 {
                    *count -= 1;
                    matching += 1;
                } else {
                    right_only += 1;
                }
            } else {
                right_only += 1;
            }
        }
    }

    // Remaining left rows are left-only
    let left_only: usize = left_hashes.values().sum();

    // Note: "different" would require row-by-row comparison with a key
    // For now, we just track left_only vs right_only
    // A more sophisticated diff would use primary key columns to identify "same" rows with different values
    DiffStats {
        left_only,
        right_only,
        different: 0, // Would need key-based comparison
        matching,
    }
}

/// Get row values as strings for a specific row index.
pub(super) fn get_row_values(batch: &RecordBatch, row_idx: usize) -> Vec<String> {
    (0..batch.num_columns())
        .map(|col_idx| format_array_value(batch.column(col_idx).as_ref(), row_idx))
        .collect()
}

/// Compute schema diff between two column lists.
pub(super) fn compute_schema_diff(
    table_name: &str,
    left_columns: &[enya_datafusion::ColumnInfo],
    right_columns: &[enya_datafusion::ColumnInfo],
) -> super::types::SchemaDiffResult {
    use super::types::{ColumnDiffStatus, SchemaDiffColumn, SchemaDiffResult};

    // Build maps by column name for efficient lookup
    let left_map: FxHashMap<&str, &enya_datafusion::ColumnInfo> =
        left_columns.iter().map(|c| (c.name.as_str(), c)).collect();
    let right_map: FxHashMap<&str, &enya_datafusion::ColumnInfo> =
        right_columns.iter().map(|c| (c.name.as_str(), c)).collect();

    let mut columns = Vec::new();
    let mut matching = 0usize;
    let mut left_only = 0usize;
    let mut right_only = 0usize;
    let mut changed = 0usize;

    // Process left columns
    for left_col in left_columns {
        if let Some(right_col) = right_map.get(left_col.name.as_str()) {
            // Column exists in both - check if definition matches
            let types_match = left_col.data_type == right_col.data_type;
            let nullable_match = left_col.nullable == right_col.nullable;

            if types_match && nullable_match {
                matching += 1;
                columns.push(SchemaDiffColumn {
                    name: left_col.name.clone(),
                    left_type: Some(left_col.data_type.clone()),
                    left_nullable: Some(left_col.nullable),
                    right_type: Some(right_col.data_type.clone()),
                    right_nullable: Some(right_col.nullable),
                    status: ColumnDiffStatus::Matching,
                });
            } else {
                changed += 1;
                columns.push(SchemaDiffColumn {
                    name: left_col.name.clone(),
                    left_type: Some(left_col.data_type.clone()),
                    left_nullable: Some(left_col.nullable),
                    right_type: Some(right_col.data_type.clone()),
                    right_nullable: Some(right_col.nullable),
                    status: ColumnDiffStatus::Changed,
                });
            }
        } else {
            // Column only in left
            left_only += 1;
            columns.push(SchemaDiffColumn {
                name: left_col.name.clone(),
                left_type: Some(left_col.data_type.clone()),
                left_nullable: Some(left_col.nullable),
                right_type: None,
                right_nullable: None,
                status: ColumnDiffStatus::LeftOnly,
            });
        }
    }

    // Process right-only columns
    for right_col in right_columns {
        if !left_map.contains_key(right_col.name.as_str()) {
            right_only += 1;
            columns.push(SchemaDiffColumn {
                name: right_col.name.clone(),
                left_type: None,
                left_nullable: None,
                right_type: Some(right_col.data_type.clone()),
                right_nullable: Some(right_col.nullable),
                status: ColumnDiffStatus::RightOnly,
            });
        }
    }

    // Sort: matching first, then changed, then left-only, then right-only
    columns.sort_by_key(|c| match c.status {
        ColumnDiffStatus::Matching => 0,
        ColumnDiffStatus::Changed => 1,
        ColumnDiffStatus::LeftOnly => 2,
        ColumnDiffStatus::RightOnly => 3,
    });

    SchemaDiffResult {
        table_name: table_name.to_string(),
        columns,
        matching,
        left_only,
        right_only,
        changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use enya_datafusion::arrow::array::{Int32Array, StringArray};
    use enya_datafusion::arrow::datatypes::{DataType, Field, Schema};
    use std::sync::Arc;

    fn create_test_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("name", DataType::Utf8, true),
        ]))
    }

    fn create_test_batch(ids: &[i32], names: &[&str]) -> RecordBatch {
        let schema = create_test_schema();
        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(ids.to_vec())),
                Arc::new(StringArray::from(names.to_vec())),
            ],
        )
        .unwrap()
    }

    #[test]
    fn test_schemas_compatible() {
        let schema1 = create_test_schema();
        let schema2 = Arc::new(Schema::new(vec![
            Field::new("name", DataType::Utf8, true),
            Field::new("id", DataType::Int32, false),
        ]));
        let schema3 = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("other", DataType::Utf8, true),
        ]));

        // Same columns, same order
        assert!(schemas_compatible(&schema1, &schema1));

        // Same columns, different order
        assert!(schemas_compatible(&schema1, &schema2));

        // Different columns
        assert!(!schemas_compatible(&schema1, &schema3));
    }

    #[test]
    fn test_compute_table_diff_matching() {
        let batch1 = create_test_batch(&[1, 2, 3], &["a", "b", "c"]);
        let batch2 = create_test_batch(&[1, 2, 3], &["a", "b", "c"]);

        let stats = compute_table_diff(&[batch1], &[batch2]);

        assert_eq!(stats.matching, 3);
        assert_eq!(stats.left_only, 0);
        assert_eq!(stats.right_only, 0);
    }

    #[test]
    fn test_compute_table_diff_different() {
        let batch1 = create_test_batch(&[1, 2], &["a", "b"]);
        let batch2 = create_test_batch(&[2, 3], &["b", "c"]);

        let stats = compute_table_diff(&[batch1], &[batch2]);

        // Row (2, "b") matches
        assert_eq!(stats.matching, 1);
        // Row (1, "a") is left-only
        assert_eq!(stats.left_only, 1);
        // Row (3, "c") is right-only
        assert_eq!(stats.right_only, 1);
    }

    #[test]
    fn test_compute_table_diff_empty() {
        let batch1 = create_test_batch(&[], &[]);
        let batch2 = create_test_batch(&[], &[]);

        let stats = compute_table_diff(&[batch1], &[batch2]);

        assert_eq!(stats.matching, 0);
        assert_eq!(stats.left_only, 0);
        assert_eq!(stats.right_only, 0);
    }
}
