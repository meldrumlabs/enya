//! Tantivy schema definitions for the codebase index.

use tantivy::schema::{
    FAST, Field, INDEXED, NumericOptions, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
};

/// Document types stored in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum DocType {
    /// A metric instrumentation point.
    Metric = 1,
    /// A Prometheus alert rule.
    Alert = 2,
    /// A git commit.
    Commit = 3,
}

impl DocType {
    /// Convert from u64 value.
    #[must_use]
    pub fn from_u64(value: u64) -> Option<Self> {
        match value {
            1 => Some(Self::Metric),
            2 => Some(Self::Alert),
            3 => Some(Self::Commit),
            _ => None,
        }
    }
}

/// Collection of field handles for the schema.
#[derive(Debug, Clone)]
pub struct SchemaFields {
    // Common fields
    pub doc_type: Field,
    pub doc_id: Field,
    pub file_path: Field,
    pub line: Field,
    pub column: Field,

    // Metric fields
    pub metric_name: Field,
    pub metric_kind: Field,
    pub labels: Field,
    pub function_name: Field,
    pub impl_type: Field,

    // Alert fields
    pub alert_name: Field,
    pub alert_expr: Field,
    pub severity: Field,
    pub message: Field,
    pub runbook_url: Field,
    pub metric_refs: Field,

    // Commit fields
    pub commit_hash: Field,
    pub commit_message: Field,
    pub commit_timestamp: Field,
    pub metrics_touched: Field,
    pub files_changed: Field,
    /// Raw diff content (stored but not fully indexed for size reasons)
    pub diff_content: Field,
    /// Functions added in the commit (space-separated for search)
    pub functions_added: Field,
    /// Functions removed in the commit
    pub functions_removed: Field,
    /// Functions modified in the commit
    pub functions_modified: Field,
    /// Metrics added in the commit
    pub commit_metrics_added: Field,
    /// Metrics removed in the commit
    pub commit_metrics_removed: Field,
}

/// Build the Tantivy schema for the codebase index.
///
/// Returns both the schema and field handles for easy access.
#[must_use]
pub fn build_schema() -> (Schema, SchemaFields) {
    let mut builder = Schema::builder();

    // Text options for full-text searchable fields
    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("default")
                .set_index_option(tantivy::schema::IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();

    // Options for exact-match string fields (not tokenized)
    let string_options = STRING | STORED;

    // Numeric options for u64 fields
    let u64_options = NumericOptions::default()
        .set_indexed()
        .set_stored()
        .set_fast();

    // === Common fields ===
    let doc_type = builder.add_u64_field("doc_type", INDEXED | STORED | FAST);
    let doc_id = builder.add_text_field("doc_id", string_options.clone());
    let file_path = builder.add_text_field("file_path", text_options.clone());
    let line = builder.add_u64_field("line", u64_options.clone());
    let column = builder.add_u64_field("column", STORED);

    // === Metric fields ===
    let metric_name = builder.add_text_field("metric_name", text_options.clone());
    let metric_kind = builder.add_text_field("metric_kind", string_options.clone());
    let labels = builder.add_text_field("labels", text_options.clone());
    let function_name = builder.add_text_field("function_name", text_options.clone());
    let impl_type = builder.add_text_field("impl_type", text_options.clone());

    // === Alert fields ===
    let alert_name = builder.add_text_field("alert_name", text_options.clone());
    let alert_expr = builder.add_text_field("alert_expr", text_options.clone());
    let severity = builder.add_text_field("severity", string_options.clone());
    let message = builder.add_text_field("message", text_options.clone());
    let runbook_url = builder.add_text_field("runbook_url", string_options.clone());
    let metric_refs = builder.add_text_field("metric_refs", text_options.clone());

    // === Commit fields ===
    let commit_hash = builder.add_text_field("commit_hash", string_options);
    let commit_message = builder.add_text_field("commit_message", text_options.clone());
    let commit_timestamp = builder.add_i64_field("commit_timestamp", INDEXED | STORED | FAST);
    // Use same text_options as other searchable fields to support all query types
    let metrics_touched = builder.add_text_field("metrics_touched", text_options.clone());
    // Files changed in the commit - searchable by filename (e.g., "executor.rs")
    let files_changed = builder.add_text_field("files_changed", text_options.clone());
    // Raw diff content - STORED for preview but not indexed (too large)
    let diff_content = builder.add_text_field("diff_content", STORED);
    // Semantic fields - indexed for search
    let functions_added = builder.add_text_field("functions_added", text_options.clone());
    let functions_removed = builder.add_text_field("functions_removed", text_options.clone());
    let functions_modified = builder.add_text_field("functions_modified", text_options.clone());
    let commit_metrics_added = builder.add_text_field("commit_metrics_added", text_options.clone());
    let commit_metrics_removed = builder.add_text_field("commit_metrics_removed", text_options);

    let schema = builder.build();

    let fields = SchemaFields {
        doc_type,
        doc_id,
        file_path,
        line,
        column,
        metric_name,
        metric_kind,
        labels,
        function_name,
        impl_type,
        alert_name,
        alert_expr,
        severity,
        message,
        runbook_url,
        metric_refs,
        commit_hash,
        commit_message,
        commit_timestamp,
        metrics_touched,
        files_changed,
        diff_content,
        functions_added,
        functions_removed,
        functions_modified,
        commit_metrics_added,
        commit_metrics_removed,
    };

    (schema, fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_schema() {
        let (schema, fields) = build_schema();

        // Verify all fields exist
        assert!(schema.get_field("doc_type").is_ok());
        assert!(schema.get_field("metric_name").is_ok());
        assert!(schema.get_field("alert_name").is_ok());
        assert!(schema.get_field("commit_hash").is_ok());

        // Verify field handles match
        assert_eq!(schema.get_field("doc_type").unwrap(), fields.doc_type);
        assert_eq!(schema.get_field("metric_name").unwrap(), fields.metric_name);
    }

    #[test]
    fn test_doc_type_conversion() {
        assert_eq!(DocType::from_u64(1), Some(DocType::Metric));
        assert_eq!(DocType::from_u64(2), Some(DocType::Alert));
        assert_eq!(DocType::from_u64(3), Some(DocType::Commit));
        assert_eq!(DocType::from_u64(0), None);
        assert_eq!(DocType::from_u64(99), None);
    }
}
