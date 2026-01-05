//! Tantivy-based full-text search index for the codebase.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use enya_analyzer::{AlertRule, CodebaseIndex, CommitInfo, MetricInstrumentation, MetricKind};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, TantivyError, doc};

use crate::schema::{DocType, SchemaFields, build_schema};
use crate::{SearchFilter, SearchResult, SearchResultKind, TantivyPhase, TantivyProgress};

/// Error type for Tantivy index operations.
#[derive(Debug)]
pub enum IndexError {
    /// Tantivy error.
    Tantivy(TantivyError),
    /// I/O error.
    Io(io::Error),
    /// Index is not initialized.
    NotInitialized,
    /// Failed to parse metadata.
    MetadataParse(String),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tantivy(e) => write!(f, "Tantivy error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::NotInitialized => write!(f, "Index not initialized"),
            Self::MetadataParse(msg) => write!(f, "Metadata parse error: {msg}"),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<TantivyError> for IndexError {
    fn from(e: TantivyError) -> Self {
        Self::Tantivy(e)
    }
}

impl From<io::Error> for IndexError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Current schema version. Bump this when schema changes require a full rebuild.
/// v1: Initial schema
/// v2: Added files_changed field for commit file tracking
/// v3: Fixed metrics_touched field to use WithFreqsAndPositions for proper query support
/// v4: Added diff_content and semantic fields (functions_added/removed/modified, metrics_added/removed)
const CURRENT_SCHEMA_VERSION: u32 = 4;

/// Metadata about the indexed state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IndexMetadata {
    /// Git commit SHA when the index was built.
    indexed_commit: Option<String>,
    /// Unix timestamp when the index was built.
    indexed_at: i64,
    /// Number of metrics indexed.
    metric_count: usize,
    /// Number of alerts indexed.
    alert_count: usize,
    /// Number of commits indexed.
    commit_count: usize,
    /// Schema version for compatibility checking.
    schema_version: u32,
}

impl Default for IndexMetadata {
    fn default() -> Self {
        Self {
            indexed_commit: None,
            indexed_at: 0,
            metric_count: 0,
            alert_count: 0,
            commit_count: 0,
            schema_version: CURRENT_SCHEMA_VERSION,
        }
    }
}

/// Tantivy-based full-text search index for metrics, alerts, and commits.
pub struct TantivyCodebaseIndex {
    index: Index,
    reader: IndexReader,
    fields: SchemaFields,
    index_dir: PathBuf,
    metadata: IndexMetadata,
}

impl TantivyCodebaseIndex {
    /// Opens an existing index or creates a new one for the given repository.
    ///
    /// The index is stored in `{repo_path}/.enya/tantivy/`.
    /// If the existing index has an incompatible schema version, it will be
    /// deleted and recreated.
    ///
    /// # Errors
    ///
    /// Returns an error if the index cannot be created or opened.
    pub fn open_or_create(repo_path: &Path) -> Result<Self, IndexError> {
        let index_dir = Self::index_dir_for_repo(repo_path)?;

        if index_dir.exists() {
            // Check schema version before opening
            let metadata = Self::load_metadata(&index_dir).unwrap_or_default();
            if metadata.schema_version != CURRENT_SCHEMA_VERSION {
                log::info!(
                    "Index schema version {} is outdated (current: {}), recreating index",
                    metadata.schema_version,
                    CURRENT_SCHEMA_VERSION
                );
                // Delete the old index directory
                if let Err(e) = fs::remove_dir_all(&index_dir) {
                    log::warn!("Failed to remove old index directory: {e}");
                }
                // Create fresh index
                return Self::create(&index_dir);
            }

            // Try to open existing index
            Self::open(&index_dir)
        } else {
            // Create new index
            Self::create(&index_dir)
        }
    }

    /// Creates a new index at the given directory.
    fn create(index_dir: &Path) -> Result<Self, IndexError> {
        fs::create_dir_all(index_dir)?;

        let (schema, fields) = build_schema();
        let index = Index::create_in_dir(index_dir, schema)?;
        // Use Manual reload policy so we control when the reader sees new commits
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;

        Ok(Self {
            index,
            reader,
            fields,
            index_dir: index_dir.to_path_buf(),
            metadata: IndexMetadata::default(),
        })
    }

    /// Opens an existing index.
    fn open(index_dir: &Path) -> Result<Self, IndexError> {
        let index = Index::open_in_dir(index_dir)?;
        // Use Manual reload policy so we control when the reader sees new commits
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;

        // Rebuild field handles from the loaded schema
        let (_, fields) = build_schema();

        // Load metadata if it exists
        let metadata = Self::load_metadata(index_dir).unwrap_or_default();

        Ok(Self {
            index,
            reader,
            fields,
            index_dir: index_dir.to_path_buf(),
            metadata,
        })
    }

    /// Returns the index directory for a repository.
    ///
    /// The index is stored in `{repo_path}/.enya/tantivy/`.
    fn index_dir_for_repo(repo_path: &Path) -> Result<PathBuf, IndexError> {
        Ok(repo_path.join(".enya").join("tantivy"))
    }

    /// Loads metadata from the index directory.
    fn load_metadata(index_dir: &Path) -> Result<IndexMetadata, IndexError> {
        let metadata_path = index_dir.join("enya_metadata.json");
        if !metadata_path.exists() {
            return Ok(IndexMetadata::default());
        }

        let content = fs::read_to_string(&metadata_path)?;
        serde_json::from_str(&content).map_err(|e| IndexError::MetadataParse(e.to_string()))
    }

    /// Saves metadata to the index directory.
    fn save_metadata(&self) -> Result<(), IndexError> {
        let metadata_path = self.index_dir.join("enya_metadata.json");
        let content = serde_json::to_string_pretty(&self.metadata)
            .map_err(|e| IndexError::MetadataParse(e.to_string()))?;
        fs::write(metadata_path, content)?;
        Ok(())
    }

    /// Returns the commit SHA that was indexed, if known.
    #[must_use]
    pub fn indexed_commit(&self) -> Option<&str> {
        self.metadata.indexed_commit.as_deref()
    }

    /// Returns the number of metrics in the index.
    #[must_use]
    pub fn metric_count(&self) -> usize {
        self.metadata.metric_count
    }

    /// Returns the number of alerts in the index.
    #[must_use]
    pub fn alert_count(&self) -> usize {
        self.metadata.alert_count
    }

    /// Returns the number of commits in the index.
    #[must_use]
    pub fn commit_count(&self) -> usize {
        self.metadata.commit_count
    }

    /// Rebuilds the index from a `CodebaseIndex`.
    ///
    /// This clears the existing index and re-indexes all metrics and alerts.
    /// Use `rebuild_with_commits` to also index commit history.
    ///
    /// # Errors
    ///
    /// Returns an error if indexing fails.
    pub fn rebuild(&mut self, codebase: &CodebaseIndex) -> Result<(), IndexError> {
        self.rebuild_with_commits(codebase, &[])
    }

    /// Rebuilds the index from a `CodebaseIndex` and commit history.
    ///
    /// This clears the existing index and re-indexes all metrics, alerts, and commits.
    ///
    /// # Errors
    ///
    /// Returns an error if indexing fails.
    pub fn rebuild_with_commits(
        &mut self,
        codebase: &CodebaseIndex,
        commits: &[CommitInfo],
    ) -> Result<(), IndexError> {
        self.rebuild_with_progress(codebase, commits, None)
    }

    /// Rebuilds the index with progress tracking.
    ///
    /// Same as `rebuild_with_commits` but reports progress via `TantivyProgress`.
    ///
    /// # Errors
    ///
    /// Returns an error if indexing fails.
    pub fn rebuild_with_progress(
        &mut self,
        codebase: &CodebaseIndex,
        commits: &[CommitInfo],
        progress: Option<&TantivyProgress>,
    ) -> Result<(), IndexError> {
        // Create a new writer with a reasonable heap size (50MB)
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        // Clear all existing documents
        writer.delete_all_documents()?;

        // Index all metrics
        if let Some(p) = progress {
            p.set_phase(TantivyPhase::IndexingMetrics);
            p.set_total(codebase.metrics.len());
        }
        for (i, metric) in codebase.metrics.iter().enumerate() {
            if let Some(p) = progress {
                p.increment(Some(metric.name.clone()));
            }
            let doc = self.metric_to_document(metric, i);
            writer.add_document(doc)?;
        }

        // Index all alerts
        if let Some(p) = progress {
            p.set_phase(TantivyPhase::IndexingAlerts);
            p.set_total(codebase.alerts.len());
        }
        for (i, alert) in codebase.alerts.iter().enumerate() {
            if let Some(p) = progress {
                p.increment(Some(alert.name.clone()));
            }
            let doc = self.alert_to_document(alert, i);
            writer.add_document(doc)?;
        }

        // Index all commits
        if let Some(p) = progress {
            p.set_phase(TantivyPhase::IndexingCommits);
            p.set_total(commits.len());
        }
        for (i, commit) in commits.iter().enumerate() {
            if let Some(p) = progress {
                // Show short commit hash + first line of message
                let short_hash = &commit.hash[..7.min(commit.hash.len())];
                let first_line = commit.message.lines().next().unwrap_or("");
                let truncated = if first_line.len() > 40 {
                    format!("{}...", &first_line[..37])
                } else {
                    first_line.to_string()
                };
                p.increment(Some(format!("{short_hash} {truncated}")));
            }
            let doc = self.commit_to_document(commit, i);
            writer.add_document(doc)?;
        }

        // Finalize
        if let Some(p) = progress {
            p.set_phase(TantivyPhase::Finalizing);
            p.set_current_item(Some("Committing index...".to_string()));
        }

        writer.commit()?;

        // Reload the reader to see new documents
        self.reader.reload()?;

        // Update metadata
        self.metadata.metric_count = codebase.metrics.len();
        self.metadata.alert_count = codebase.alerts.len();
        self.metadata.commit_count = commits.len();
        self.metadata.indexed_at = codebase.last_updated;
        self.save_metadata()?;

        log::info!(
            "Tantivy index rebuilt: {} metrics, {} alerts, {} commits",
            self.metadata.metric_count,
            self.metadata.alert_count,
            self.metadata.commit_count
        );

        Ok(())
    }

    /// Converts a metric to a Tantivy document.
    fn metric_to_document(&self, metric: &MetricInstrumentation, index: usize) -> TantivyDocument {
        let kind_str = match metric.kind {
            MetricKind::Counter => "counter",
            MetricKind::Gauge => "gauge",
            MetricKind::Histogram => "histogram",
        };

        let labels_str = metric.labels.join(" ");
        let doc_id = format!("metric:{index}");

        doc!(
            self.fields.doc_type => DocType::Metric as u64,
            self.fields.doc_id => doc_id,
            self.fields.metric_name => metric.name.clone(),
            self.fields.metric_kind => kind_str.to_string(),
            self.fields.labels => labels_str,
            self.fields.file_path => metric.file.display().to_string(),
            self.fields.line => metric.line as u64,
            self.fields.column => metric.column as u64,
            self.fields.function_name => metric.function_name.clone().unwrap_or_default(),
            self.fields.impl_type => metric.impl_type.clone().unwrap_or_default()
        )
    }

    /// Converts an alert to a Tantivy document.
    fn alert_to_document(&self, alert: &AlertRule, index: usize) -> TantivyDocument {
        let doc_id = format!("alert:{index}");

        doc!(
            self.fields.doc_type => DocType::Alert as u64,
            self.fields.doc_id => doc_id,
            self.fields.alert_name => alert.name.clone(),
            self.fields.alert_expr => alert.expr.clone(),
            self.fields.severity => alert.severity.clone().unwrap_or_default(),
            self.fields.message => alert.message.clone().unwrap_or_default(),
            self.fields.runbook_url => alert.runbook_url.clone().unwrap_or_default(),
            self.fields.metric_refs => alert.metric_name.clone().unwrap_or_default(),
            self.fields.file_path => alert.file.display().to_string(),
            self.fields.line => alert.line as u64,
            self.fields.column => alert.column as u64
        )
    }

    /// Converts a commit to a Tantivy document.
    fn commit_to_document(&self, commit: &CommitInfo, index: usize) -> TantivyDocument {
        let doc_id = format!("commit:{index}");

        // Join files changed as space-separated for full-text search
        // This allows searching by filename (e.g., "executor.rs")
        let files_str = commit.files_changed.join(" ");

        // Join semantic fields as space-separated for search
        let functions_added = commit.semantics.functions_added.join(" ");
        let functions_removed = commit.semantics.functions_removed.join(" ");
        let functions_modified = commit.semantics.functions_modified.join(" ");
        let metrics_added = commit.semantics.metrics_added.join(" ");
        let metrics_removed = commit.semantics.metrics_removed.join(" ");

        doc!(
            self.fields.doc_type => DocType::Commit as u64,
            self.fields.doc_id => doc_id,
            self.fields.commit_hash => commit.hash.clone(),
            self.fields.commit_message => commit.message.clone(),
            self.fields.commit_timestamp => commit.timestamp,
            self.fields.files_changed => files_str,
            self.fields.diff_content => commit.diff.clone(),
            self.fields.functions_added => functions_added,
            self.fields.functions_removed => functions_removed,
            self.fields.functions_modified => functions_modified,
            self.fields.commit_metrics_added => metrics_added,
            self.fields.commit_metrics_removed => metrics_removed,
            // file_path is empty for commits (they're repo-level)
            self.fields.file_path => String::new(),
            self.fields.line => 0u64
        )
    }

    /// Searches the index with the given query.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string.
    /// * `filter` - Filter to limit results by document type.
    /// * `limit` - Maximum number of results to return.
    ///
    /// # Returns
    ///
    /// A vector of search results, sorted by relevance score.
    #[must_use]
    pub fn search(&self, query: &str, filter: SearchFilter, limit: usize) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        let searcher = self.reader.searcher();

        // Build search fields based on filter
        let search_fields = match filter {
            SearchFilter::All => vec![
                self.fields.metric_name,
                self.fields.alert_name,
                self.fields.commit_message,
                self.fields.file_path,
                self.fields.labels,
                self.fields.function_name,
                self.fields.files_changed,
                self.fields.functions_added,
                self.fields.functions_modified,
            ],
            SearchFilter::Metrics => vec![
                self.fields.metric_name,
                self.fields.labels,
                self.fields.function_name,
                self.fields.file_path,
            ],
            SearchFilter::Alerts => vec![
                self.fields.alert_name,
                self.fields.alert_expr,
                self.fields.message,
                self.fields.file_path,
            ],
            SearchFilter::Commits => vec![
                self.fields.commit_message,
                self.fields.metrics_touched,
                self.fields.files_changed,
                self.fields.functions_added,
                self.fields.functions_removed,
                self.fields.functions_modified,
                self.fields.commit_metrics_added,
                self.fields.commit_metrics_removed,
            ],
        };

        let query_parser = QueryParser::for_index(&self.index, search_fields.clone());

        // For interactive search, we want prefix matching behavior:
        // - "grpc" should match "grpc_requests_total" (exact token match)
        // - "grpc_r" should match "grpc_requests_total" (prefix on second token)
        // - "req" should match "grpc_requests_total" (prefix on any token)
        //
        // Strategy: Always try wildcard first for better interactive experience,
        // then fall back to exact if no results (handles special query syntax)
        if !query.contains('*') && !query.contains('"') && !query.contains(':') {
            // Build a wildcard query: split on underscores/spaces, add * to each term
            let terms: Vec<&str> = query
                .split(|c: char| c == '_' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .collect();

            let wildcard_query = terms
                .iter()
                .map(|term| format!("{term}*"))
                .collect::<Vec<_>>()
                .join(" ");

            if !wildcard_query.is_empty() {
                let mut results = self.execute_search(
                    &searcher,
                    &query_parser,
                    &wildcard_query,
                    filter,
                    limit * 2,
                );

                if !results.is_empty() {
                    // Re-rank results to prefer those where the metric name contains
                    // the full original query (with underscores) as a substring
                    let query_lower = query.to_lowercase();
                    results.sort_by(|a, b| {
                        let a_name_lower = a.name.to_lowercase();
                        let b_name_lower = b.name.to_lowercase();

                        // Exact match gets highest priority
                        let a_exact = a_name_lower == query_lower;
                        let b_exact = b_name_lower == query_lower;
                        if a_exact != b_exact {
                            return b_exact.cmp(&a_exact);
                        }

                        // Prefix match (name starts with query) gets next priority
                        let a_prefix = a_name_lower.starts_with(&query_lower);
                        let b_prefix = b_name_lower.starts_with(&query_lower);
                        if a_prefix != b_prefix {
                            return b_prefix.cmp(&a_prefix);
                        }

                        // Substring match gets next priority
                        let a_contains = a_name_lower.contains(&query_lower);
                        let b_contains = b_name_lower.contains(&query_lower);
                        if a_contains != b_contains {
                            return b_contains.cmp(&a_contains);
                        }

                        // Fall back to Tantivy's BM25 score
                        b.score
                            .partial_cmp(&a.score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });

                    results.truncate(limit);
                    return results;
                }
            }
        }

        // Fall back to exact query (handles quoted strings, field queries, etc.)
        self.execute_search(&searcher, &query_parser, query, filter, limit)
    }

    /// Execute a search with the given query string.
    fn execute_search(
        &self,
        searcher: &tantivy::Searcher,
        query_parser: &QueryParser,
        query: &str,
        filter: SearchFilter,
        limit: usize,
    ) -> Vec<SearchResult> {
        let tantivy_query = match query_parser.parse_query(query) {
            Ok(q) => q,
            Err(e) => {
                log::warn!("Failed to parse query '{query}': {e}");
                return Vec::new();
            }
        };

        let top_docs = match searcher.search(&tantivy_query, &TopDocs::with_limit(limit)) {
            Ok(docs) => docs,
            Err(e) => {
                log::warn!("Search failed: {e}");
                return Vec::new();
            }
        };

        let mut results = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) else {
                continue;
            };

            if let Some(result) = self.document_to_result(&doc, score, filter) {
                results.push(result);
            }
        }

        results
    }

    /// Converts a Tantivy document back to a `SearchResult`.
    fn document_to_result(
        &self,
        doc: &TantivyDocument,
        score: f32,
        filter: SearchFilter,
    ) -> Option<SearchResult> {
        // Get document type
        let doc_type = doc.get_first(self.fields.doc_type)?.as_u64()?;

        let doc_type = DocType::from_u64(doc_type)?;

        // Apply filter
        match (filter, doc_type) {
            (SearchFilter::Metrics, DocType::Alert | DocType::Commit) => return None,
            (SearchFilter::Alerts, DocType::Metric | DocType::Commit) => return None,
            (SearchFilter::Commits, DocType::Metric | DocType::Alert) => return None,
            _ => {}
        }

        // Extract common fields
        let file_path = doc.get_first(self.fields.file_path)?.as_str()?;

        let line = doc.get_first(self.fields.line)?.as_u64()? as usize;

        match doc_type {
            DocType::Metric => {
                let name = doc
                    .get_first(self.fields.metric_name)?
                    .as_str()?
                    .to_string();

                let kind_str = doc.get_first(self.fields.metric_kind)?.as_str()?;

                let kind = match kind_str {
                    "counter" => MetricKind::Counter,
                    "gauge" => MetricKind::Gauge,
                    "histogram" => MetricKind::Histogram,
                    _ => MetricKind::Counter,
                };

                let function_name = doc
                    .get_first(self.fields.function_name)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);

                Some(SearchResult {
                    kind: SearchResultKind::Metric(kind),
                    name,
                    file: PathBuf::from(file_path),
                    line,
                    score,
                    snippet: function_name,
                })
            }
            DocType::Alert => {
                let name = doc.get_first(self.fields.alert_name)?.as_str()?.to_string();

                let severity = doc
                    .get_first(self.fields.severity)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);

                let expr = doc
                    .get_first(self.fields.alert_expr)
                    .and_then(|v| v.as_str())
                    .map(String::from);

                Some(SearchResult {
                    kind: SearchResultKind::Alert { severity },
                    name,
                    file: PathBuf::from(file_path),
                    line,
                    score,
                    snippet: expr,
                })
            }
            DocType::Commit => {
                let message = doc
                    .get_first(self.fields.commit_message)?
                    .as_str()?
                    .to_string();

                let hash = doc
                    .get_first(self.fields.commit_hash)?
                    .as_str()?
                    .to_string();

                let timestamp = doc.get_first(self.fields.commit_timestamp)?.as_i64()?;

                // Get full diff content for diff viewer
                let diff = doc
                    .get_first(self.fields.diff_content)
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_default();

                // Create a snippet for preview (first ~500 chars)
                let diff_snippet = if diff.len() > 500 {
                    Some(format!("{}...", &diff[..500]))
                } else if !diff.is_empty() {
                    Some(diff.clone())
                } else {
                    None
                };

                Some(SearchResult {
                    kind: SearchResultKind::Commit {
                        hash,
                        timestamp,
                        diff,
                    },
                    name: message,
                    file: PathBuf::new(),
                    line: 0,
                    score,
                    snippet: diff_snippet,
                })
            }
        }
    }

    /// Searches for metrics only.
    #[must_use]
    pub fn search_metrics(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        self.search(query, SearchFilter::Metrics, limit)
    }

    /// Searches for alerts only.
    #[must_use]
    pub fn search_alerts(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        self.search(query, SearchFilter::Alerts, limit)
    }

    /// Searches for commits only.
    #[must_use]
    pub fn search_commits(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        self.search(query, SearchFilter::Commits, limit)
    }

    /// Deletes the index directory.
    ///
    /// Call this to force a full rebuild on next open.
    pub fn delete(self) -> Result<(), IndexError> {
        // Drop self first to release file handles
        drop(self.index);
        drop(self.reader);

        fs::remove_dir_all(&self.index_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_metric(name: &str, file: &str, line: usize) -> MetricInstrumentation {
        MetricInstrumentation {
            kind: MetricKind::Counter,
            name: name.to_string(),
            labels: vec!["method".to_string(), "status".to_string()],
            file: PathBuf::from(file),
            line,
            column: 0,
            function_name: Some("handle_request".to_string()),
            impl_type: None,
        }
    }

    fn make_test_alert(name: &str, expr: &str, severity: Option<&str>) -> AlertRule {
        AlertRule {
            name: name.to_string(),
            expr: expr.to_string(),
            metric_name: Some("http_requests_total".to_string()),
            severity: severity.map(String::from),
            message: Some("Alert message".to_string()),
            runbook_url: None,
            file: PathBuf::from("alerts.yaml"),
            line: 1,
            column: 0,
        }
    }

    #[test]
    fn test_create_and_search() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("http_requests_total", "src/api.rs", 10),
                make_test_metric("db_queries_total", "src/db.rs", 20),
            ],
            alerts: vec![make_test_alert(
                "HighErrorRate",
                "rate(errors_total[5m]) > 0.1",
                Some("critical"),
            )],
            last_updated: 1234567890,
        };

        index.rebuild(&codebase).unwrap();

        // Search for metrics - best match should be first
        let results = index.search("http_requests_total", SearchFilter::All, 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "http_requests_total");
        assert!(results[0].score > 0.0);

        // Search for alerts by name
        let results = index.search_alerts("HighErrorRate", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "HighErrorRate");
    }

    #[test]
    fn test_filter_by_type() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        // Use names that share a common token after tokenization
        // The default tokenizer lowercases and splits on non-alphanumeric
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![make_test_metric("api_latency", "src/api.rs", 10)],
            alerts: vec![make_test_alert(
                "api_slow",
                "rate(api_requests[5m]) > 100",
                None,
            )],
            last_updated: 0,
        };

        index.rebuild(&codebase).unwrap();

        // Search all - both contain "api" token
        let results = index.search("api", SearchFilter::All, 10);
        assert_eq!(results.len(), 2);

        // Search metrics only
        let results = index.search("api", SearchFilter::Metrics, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].kind, SearchResultKind::Metric(_)));

        // Search alerts only
        let results = index.search("api", SearchFilter::Alerts, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].kind, SearchResultKind::Alert { .. }));
    }

    #[test]
    fn test_metadata_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        // Create and populate index
        {
            let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
            let codebase = CodebaseIndex {
                repo_url: "test".to_string(),
                repo_path: PathBuf::from("/test"),
                metrics: vec![make_test_metric("test_metric", "test.rs", 1)],
                alerts: vec![],
                last_updated: 9999,
            };
            index.rebuild(&codebase).unwrap();
            assert_eq!(index.metric_count(), 1);
        }

        // Reopen and verify metadata
        {
            let index = TantivyCodebaseIndex::open(&index_dir).unwrap();
            assert_eq!(index.metric_count(), 1);
            assert_eq!(index.alert_count(), 0);
        }
    }

    fn make_test_commit(hash: &str, timestamp: i64, message: &str) -> CommitInfo {
        CommitInfo {
            hash: hash.to_string(),
            timestamp,
            message: message.to_string(),
            ..Default::default()
        }
    }

    fn make_test_commit_with_files(
        hash: &str,
        timestamp: i64,
        message: &str,
        files: Vec<&str>,
    ) -> CommitInfo {
        CommitInfo {
            hash: hash.to_string(),
            timestamp,
            message: message.to_string(),
            files_changed: files.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn test_commit_indexing() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![make_test_metric("http_requests_total", "src/api.rs", 10)],
            alerts: vec![],
            last_updated: 1234567890,
        };

        let commits = vec![
            make_test_commit("abc123", 1700000000, "feat: add http metrics endpoint"),
            make_test_commit("def456", 1700001000, "fix: database connection pool"),
            make_test_commit("ghi789", 1700002000, "refactor: cleanup api handlers"),
        ];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Verify counts
        assert_eq!(index.metric_count(), 1);
        assert_eq!(index.commit_count(), 3);

        // Search for commits by message content
        let results = index.search_commits("http", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "feat: add http metrics endpoint");
        assert!(matches!(
            results[0].kind,
            SearchResultKind::Commit { ref hash, .. } if hash == "abc123"
        ));

        // Search for "database" commit
        let results = index.search_commits("database", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fix: database connection pool");

        // Search all - should find metric and commit containing "api"
        let results = index.search("api", SearchFilter::All, 10);
        assert_eq!(results.len(), 2); // metric file path + commit message
    }

    #[test]
    fn test_commit_filter() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![make_test_metric("feature_flag", "src/feature.rs", 10)],
            alerts: vec![],
            last_updated: 0,
        };

        let commits = vec![make_test_commit(
            "abc123",
            1700000000,
            "feat: add feature flag support",
        )];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Search all - both contain "feature"
        let results = index.search("feature", SearchFilter::All, 10);
        assert_eq!(results.len(), 2);

        // Search commits only
        let results = index.search("feature", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].kind, SearchResultKind::Commit { .. }));

        // Search metrics only
        let results = index.search("feature", SearchFilter::Metrics, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].kind, SearchResultKind::Metric(_)));
    }

    #[test]
    fn test_prefix_search() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("grpc_requests_total", "src/grpc.rs", 10),
                make_test_metric("grpc_errors_total", "src/grpc.rs", 20),
                make_test_metric("http_requests_total", "src/http.rs", 30),
            ],
            alerts: vec![],
            last_updated: 0,
        };

        index.rebuild(&codebase).unwrap();

        // Full token match: "grpc" should match grpc metrics (wildcard expands to grpc*)
        let results = index.search("grpc", SearchFilter::Metrics, 10);
        assert_eq!(results.len(), 2, "grpc should match both grpc metrics");

        // Partial prefix with underscore: "grpc_r" should match via wildcard
        // The query becomes "grpc* r*" which matches both grpc metrics
        // (grpc matches "grpc", r* matches "requests" in one)
        let results = index.search("grpc_r", SearchFilter::Metrics, 10);
        assert!(
            !results.is_empty(),
            "grpc_r should match grpc metrics via prefix search"
        );
        // The grpc_requests_total should be in results
        assert!(
            results.iter().any(|r| r.name == "grpc_requests_total"),
            "grpc_requests_total should be in results"
        );

        // Testing with a full word that exists in the index
        let results = index.search("requests", SearchFilter::Metrics, 10);
        assert!(
            !results.is_empty(),
            "requests should match metrics containing 'requests'"
        );
    }

    #[test]
    fn test_ranking_prefers_substring_match() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("grpc_requests_in_flight", "src/grpc.rs", 10),
                make_test_metric("grpc_requests_total", "src/grpc.rs", 20),
                make_test_metric("poly_api_http_requests_in_flight", "src/poly.rs", 30),
            ],
            alerts: vec![],
            last_updated: 0,
        };

        index.rebuild(&codebase).unwrap();

        // When searching for "grpc_requests_in_flight", the exact match should be first
        let results = index.search("grpc_requests_in_flight", SearchFilter::Metrics, 10);
        assert!(!results.is_empty());
        assert_eq!(
            results[0].name, "grpc_requests_in_flight",
            "Exact match should be ranked first"
        );

        // When searching for "grpc_requests", prefix matches should come before substring matches
        let results = index.search("grpc_requests", SearchFilter::Metrics, 10);
        assert!(!results.is_empty());
        // Both grpc_requests_* should come before poly_api_http_requests_in_flight
        let grpc_positions: Vec<usize> = results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.name.starts_with("grpc_requests"))
            .map(|(i, _)| i)
            .collect();
        let poly_position = results.iter().position(|r| r.name.contains("poly"));

        if let Some(poly_pos) = poly_position {
            for grpc_pos in &grpc_positions {
                assert!(
                    *grpc_pos < poly_pos,
                    "grpc_requests_* metrics should rank before poly_* metric"
                );
            }
        }
    }

    #[test]
    fn test_search_commits_by_file() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![],
            last_updated: 0,
        };

        let commits = vec![
            make_test_commit_with_files(
                "abc123",
                1700000000,
                "feat: add executor",
                vec!["src/executor.rs", "src/lib.rs"],
            ),
            make_test_commit_with_files(
                "def456",
                1700001000,
                "fix: scheduler bug",
                vec!["src/scheduler.rs"],
            ),
            make_test_commit_with_files(
                "ghi789",
                1700002000,
                "refactor: main entrypoint",
                vec!["src/main.rs", "src/executor.rs"],
            ),
        ];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Search by filename (without extension) should find commits that modified that file
        // Note: Tantivy tokenizes "executor.rs" into ["executor", "rs"] tokens
        let results = index.search("executor", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 2, "Two commits modified executor.rs");

        // Verify we got the right commits
        let hashes: Vec<&str> = results
            .iter()
            .filter_map(|r| match &r.kind {
                SearchResultKind::Commit { hash, .. } => Some(hash.as_str()),
                _ => None,
            })
            .collect();
        assert!(hashes.contains(&"abc123"));
        assert!(hashes.contains(&"ghi789"));

        // Search for scheduler
        let results = index.search("scheduler", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::Commit { hash, .. } if hash == "def456"
        ));

        // Search for "main" - should find commit that modified main.rs
        let results = index.search("main", SearchFilter::Commits, 10);
        assert!(
            results.iter().any(|r| matches!(
                &r.kind,
                SearchResultKind::Commit { hash, .. } if hash == "ghi789"
            )),
            "Should find commit that modified main.rs"
        );
    }
}
