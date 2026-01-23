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
/// v5: Fixed batch diff fetching (removed --name-only conflict with -p, diffs were empty)
const CURRENT_SCHEMA_VERSION: u32 = 5;

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
        // Track the newest commit (first in the list since commits are newest-first)
        self.metadata.indexed_commit = commits.first().map(|c| c.hash.clone());
        self.save_metadata()?;

        log::info!(
            "Tantivy index rebuilt: {} metrics, {} alerts, {} commits",
            self.metadata.metric_count,
            self.metadata.alert_count,
            self.metadata.commit_count
        );

        Ok(())
    }

    /// Adds new commits to the existing index (incremental update).
    ///
    /// Unlike `rebuild_with_progress`, this does NOT delete existing documents.
    /// Use this for incremental indexing when you only have new commits.
    ///
    /// # Arguments
    ///
    /// * `new_commits` - New commits to add (should be newer than existing indexed commits)
    /// * `progress` - Optional progress callback
    ///
    /// # Errors
    ///
    /// Returns an error if indexing fails.
    pub fn add_commits(
        &mut self,
        new_commits: &[CommitInfo],
        progress: Option<&TantivyProgress>,
    ) -> Result<(), IndexError> {
        if new_commits.is_empty() {
            return Ok(());
        }

        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        // Index new commits (use current count as starting index for doc_id)
        let start_index = self.metadata.commit_count;

        if let Some(p) = progress {
            p.set_phase(TantivyPhase::IndexingCommits);
            p.set_total(new_commits.len());
        }

        for (i, commit) in new_commits.iter().enumerate() {
            if let Some(p) = progress {
                let short_hash = &commit.hash[..7.min(commit.hash.len())];
                let first_line = commit.message.lines().next().unwrap_or("");
                let truncated = if first_line.len() > 40 {
                    format!("{}...", &first_line[..37])
                } else {
                    first_line.to_string()
                };
                p.increment(Some(format!("{short_hash} {truncated}")));
            }
            let doc = self.commit_to_document(commit, start_index + i);
            writer.add_document(doc)?;
        }

        if let Some(p) = progress {
            p.set_phase(TantivyPhase::Finalizing);
            p.set_current_item(Some("Committing index...".to_string()));
        }

        writer.commit()?;
        self.reader.reload()?;

        // Update metadata
        self.metadata.commit_count += new_commits.len();
        // Update indexed_commit to the newest (first in the list)
        if let Some(newest) = new_commits.first() {
            self.metadata.indexed_commit = Some(newest.hash.clone());
        }
        self.save_metadata()?;

        log::info!(
            "Added {} new commits to index (total: {})",
            new_commits.len(),
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

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_empty_query_returns_empty() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![make_test_metric("test_metric", "test.rs", 1)],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Empty string should return empty results
        let results = index.search("", SearchFilter::All, 10);
        assert!(results.is_empty(), "Empty query should return no results");

        // Whitespace-only should also be handled (becomes empty after processing)
        let results = index.search("   ", SearchFilter::All, 10);
        assert!(
            results.is_empty(),
            "Whitespace-only query should return no results"
        );
    }

    #[test]
    fn test_special_characters_in_query() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("http_requests_total", "src/api.rs", 10),
                make_test_metric("db::query::count", "src/db.rs", 20),
            ],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Colons are common in Rust module paths - should work
        let results = index.search("db", SearchFilter::Metrics, 10);
        assert!(!results.is_empty(), "Should find metric with :: in name");

        // Underscores should work
        let results = index.search("http_requests", SearchFilter::Metrics, 10);
        assert!(!results.is_empty(), "Should find metric with underscores");
    }

    #[test]
    fn test_unicode_in_metric_names() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                // Emoji in metric name (unusual but valid)
                MetricInstrumentation {
                    kind: MetricKind::Gauge,
                    name: "rocket_launches_total".to_string(),
                    labels: vec![],
                    file: PathBuf::from("src/rocket.rs"),
                    line: 20,
                    column: 0,
                    function_name: None,
                    impl_type: None,
                },
            ],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Should be able to search for ASCII metric
        let results = index.search("launches", SearchFilter::Metrics, 10);
        assert!(!results.is_empty(), "Should find metric by ASCII part");
        assert_eq!(results[0].name, "rocket_launches_total");
    }

    #[test]
    fn test_case_insensitive_search() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![make_test_metric("HTTP_Requests_Total", "src/api.rs", 10)],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Lowercase query should find uppercase metric
        let results = index.search("http_requests_total", SearchFilter::Metrics, 10);
        assert!(!results.is_empty(), "Lowercase query should find metric");
        assert_eq!(results[0].name, "HTTP_Requests_Total");

        // Uppercase query should also work
        let results = index.search("HTTP", SearchFilter::Metrics, 10);
        assert!(!results.is_empty(), "Uppercase query should find metric");

        // Mixed case query should work
        let results = index.search("Http_Requests", SearchFilter::Metrics, 10);
        assert!(!results.is_empty(), "Mixed case query should find metric");
    }

    #[test]
    fn test_very_long_metric_name() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        // Create a very long metric name
        let long_name = "a".repeat(500) + "_requests_total";
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![MetricInstrumentation {
                kind: MetricKind::Counter,
                name: long_name.clone(),
                labels: vec![],
                file: PathBuf::from("src/api.rs"),
                line: 10,
                column: 0,
                function_name: None,
                impl_type: None,
            }],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Should be able to find by partial match
        let results = index.search("requests_total", SearchFilter::Metrics, 10);
        assert!(
            !results.is_empty(),
            "Should find metric with very long name"
        );
    }

    // ==================== Alert Search Tests ====================

    #[test]
    fn test_search_alerts_by_expression() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![
                make_test_alert(
                    "HighErrorRate",
                    "rate(http_errors_total[5m]) / rate(http_requests_total[5m]) > 0.05",
                    Some("critical"),
                ),
                make_test_alert(
                    "HighLatency",
                    "histogram_quantile(0.99, rate(latency_bucket[5m])) > 1",
                    Some("warning"),
                ),
                make_test_alert(
                    "DatabaseSlowQueries",
                    "rate(db_slow_queries_total[5m]) > 10",
                    Some("warning"),
                ),
            ],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Search by expression content - histogram_quantile is unique to HighLatency
        let results = index.search_alerts("histogram_quantile", 10);
        assert!(!results.is_empty());
        assert!(
            results.iter().any(|r| r.name == "HighLatency"),
            "Should find HighLatency alert"
        );

        // Search by unique metric name in expression
        let results = index.search_alerts("http_errors_total", 10);
        assert!(!results.is_empty());
        assert!(
            results.iter().any(|r| r.name == "HighErrorRate"),
            "Should find HighErrorRate alert"
        );

        // Search for database-related alerts
        let results = index.search_alerts("db_slow", 10);
        assert!(!results.is_empty());
        assert!(
            results.iter().any(|r| r.name == "DatabaseSlowQueries"),
            "Should find DatabaseSlowQueries alert"
        );
    }

    #[test]
    fn test_alert_severity_in_results() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![
                make_test_alert("CriticalAlert", "rate(errors[5m]) > 100", Some("critical")),
                make_test_alert("WarningAlert", "rate(errors[5m]) > 10", Some("warning")),
                make_test_alert("NoSeverityAlert", "rate(errors[5m]) > 1", None),
            ],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Search for all alerts with "errors"
        let results = index.search_alerts("errors", 10);
        assert_eq!(results.len(), 3);

        // Verify severity is correctly returned
        for result in &results {
            match &result.kind {
                SearchResultKind::Alert { severity } => match result.name.as_str() {
                    "CriticalAlert" => assert_eq!(severity.as_deref(), Some("critical")),
                    "WarningAlert" => assert_eq!(severity.as_deref(), Some("warning")),
                    "NoSeverityAlert" => assert!(severity.is_none()),
                    _ => panic!("Unexpected alert: {}", result.name),
                },
                _ => panic!("Expected Alert kind"),
            }
        }
    }

    #[test]
    fn test_search_alerts_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![
                make_test_alert("cpu_usage_high", "cpu > 90", Some("warning")),
                make_test_alert("memory_usage_high", "mem > 90", Some("warning")),
                make_test_alert("disk_space_low", "disk < 10", Some("critical")),
            ],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Search for alerts with "high" in name (Tantivy tokenizes and lowercases)
        let results = index.search_alerts("high", 10);
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|r| r.name == "cpu_usage_high"));
        assert!(results.iter().any(|r| r.name == "memory_usage_high"));

        // Exact name search
        let results = index.search_alerts("disk_space_low", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "disk_space_low");
    }

    // ==================== Diff Content Tests ====================

    fn make_test_commit_with_diff(
        hash: &str,
        timestamp: i64,
        message: &str,
        diff: &str,
    ) -> CommitInfo {
        CommitInfo {
            hash: hash.to_string(),
            timestamp,
            message: message.to_string(),
            diff: diff.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_diff_content_indexed_and_retrieved() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let test_diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,7 @@
 fn main() {
-    println!("Hello");
+    println!("Hello, World!");
+    // Added a comment
 }
"#;

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![],
            last_updated: 0,
        };

        let commits = vec![make_test_commit_with_diff(
            "abc123",
            1700000000,
            "feat: update greeting",
            test_diff,
        )];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Search for the commit
        let results = index.search_commits("greeting", 10);
        assert_eq!(results.len(), 1);

        // Verify the full diff is in the result
        match &results[0].kind {
            SearchResultKind::Commit { diff, .. } => {
                assert!(diff.contains("Hello, World!"));
                assert!(diff.contains("Added a comment"));
                assert!(diff.contains("diff --git"));
            }
            _ => panic!("Expected Commit kind"),
        }
    }

    #[test]
    fn test_large_diff_in_snippet_truncated() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        // Create a diff larger than 500 chars
        let large_diff = "a".repeat(1000);

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![],
            last_updated: 0,
        };

        let commits = vec![make_test_commit_with_diff(
            "abc123",
            1700000000,
            "feat: big change",
            &large_diff,
        )];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        let results = index.search_commits("big", 10);
        assert_eq!(results.len(), 1);

        // Snippet should be truncated to ~500 chars
        assert!(results[0].snippet.is_some());
        let snippet = results[0].snippet.as_ref().unwrap();
        assert!(
            snippet.len() <= 505,
            "Snippet should be truncated: len={}",
            snippet.len()
        );
        assert!(
            snippet.ends_with("..."),
            "Truncated snippet should end with ..."
        );

        // But full diff should be in the result kind
        match &results[0].kind {
            SearchResultKind::Commit { diff, .. } => {
                assert_eq!(diff.len(), 1000, "Full diff should be preserved");
            }
            _ => panic!("Expected Commit kind"),
        }
    }

    #[test]
    fn test_empty_diff_handled() {
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

        // Commit with empty diff
        let commits = vec![make_test_commit_with_diff(
            "abc123",
            1700000000,
            "chore: empty diff",
            "",
        )];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        let results = index.search_commits("empty", 10);
        assert_eq!(results.len(), 1);

        // Empty diff should result in None snippet
        assert!(
            results[0].snippet.is_none(),
            "Empty diff should have no snippet"
        );

        match &results[0].kind {
            SearchResultKind::Commit { diff, .. } => {
                assert!(diff.is_empty(), "Diff should be empty string");
            }
            _ => panic!("Expected Commit kind"),
        }
    }

    // ==================== Schema Migration Tests ====================

    #[test]
    fn test_schema_version_stored_in_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![make_test_metric("test", "test.rs", 1)],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Drop the index
        drop(index);

        // Read metadata file directly
        let metadata_path = index_dir.join("enya_metadata.json");
        let content = std::fs::read_to_string(metadata_path).unwrap();
        let metadata: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify schema version is stored
        assert!(metadata["schema_version"].is_u64());
        assert_eq!(
            metadata["schema_version"].as_u64().unwrap(),
            super::CURRENT_SCHEMA_VERSION as u64
        );
    }

    #[test]
    fn test_index_reopening_preserves_data() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        // Create and populate
        {
            let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
            let codebase = CodebaseIndex {
                repo_url: "test".to_string(),
                repo_path: PathBuf::from("/test"),
                metrics: vec![
                    make_test_metric("unique_metric_alpha", "src/one.rs", 10),
                    make_test_metric("unique_metric_beta", "src/two.rs", 20),
                ],
                alerts: vec![make_test_alert(
                    "unique_alert_gamma",
                    "expr > 1",
                    Some("warning"),
                )],
                last_updated: 12345,
            };
            let commits = vec![make_test_commit(
                "abc123",
                1700000000,
                "unique initial xyzzy",
            )];
            index.rebuild_with_commits(&codebase, &commits).unwrap();
        }

        // Reopen and verify
        {
            let index = TantivyCodebaseIndex::open(&index_dir).unwrap();
            assert_eq!(index.metric_count(), 2);
            assert_eq!(index.alert_count(), 1);
            assert_eq!(index.commit_count(), 1);

            // Verify search still works - use exact unique terms
            let results = index.search_metrics("unique_metric_alpha", 10);
            assert!(!results.is_empty());
            assert!(results.iter().any(|r| r.name == "unique_metric_alpha"));

            let results = index.search_alerts("unique_alert_gamma", 10);
            assert!(!results.is_empty());

            let results = index.search_commits("xyzzy", 10);
            assert!(!results.is_empty());
        }
    }

    // ==================== Progress Tracking Tests ====================

    #[test]
    fn test_tantivy_progress_tracking() {
        use crate::{TantivyPhase, TantivyProgress};

        let progress = TantivyProgress::new();

        // Initial state
        assert_eq!(progress.get(), (0, 0));
        assert_eq!(progress.phase(), TantivyPhase::FetchingCommits);
        assert!(progress.current_item().is_none());

        // Set phase
        progress.set_phase(TantivyPhase::IndexingMetrics);
        assert_eq!(progress.phase(), TantivyPhase::IndexingMetrics);
        assert_eq!(progress.get(), (0, 0)); // Reset on phase change

        // Set total
        progress.set_total(100);
        assert_eq!(progress.get(), (0, 100));

        // Increment
        progress.increment(Some("metric_1".to_string()));
        assert_eq!(progress.get(), (1, 100));
        assert_eq!(progress.current_item(), Some("metric_1".to_string()));

        progress.increment(Some("metric_2".to_string()));
        assert_eq!(progress.get(), (2, 100));
        assert_eq!(progress.current_item(), Some("metric_2".to_string()));

        // Set current item directly
        progress.set_current_item(Some("custom_item".to_string()));
        assert_eq!(progress.current_item(), Some("custom_item".to_string()));
        assert_eq!(progress.get(), (2, 100)); // Count unchanged
    }

    #[test]
    fn test_tantivy_phase_labels() {
        use crate::TantivyPhase;

        assert_eq!(TantivyPhase::FetchingCommits.label(), "Fetching commits");
        assert_eq!(TantivyPhase::IndexingMetrics.label(), "Indexing metrics");
        assert_eq!(TantivyPhase::IndexingAlerts.label(), "Indexing alerts");
        assert_eq!(TantivyPhase::IndexingCommits.label(), "Indexing commits");
        assert_eq!(TantivyPhase::Finalizing.label(), "Finalizing");
    }

    #[test]
    fn test_rebuild_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let progress = TantivyProgress::new();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("metric1", "test1.rs", 1),
                make_test_metric("metric2", "test2.rs", 2),
            ],
            alerts: vec![make_test_alert("alert1", "expr", Some("warning"))],
            last_updated: 0,
        };

        let commits = vec![make_test_commit("abc", 1000, "commit 1")];

        index
            .rebuild_with_progress(&codebase, &commits, Some(&progress))
            .unwrap();

        // After rebuild, phase should be Finalizing
        assert_eq!(progress.phase(), TantivyPhase::Finalizing);
    }

    // ==================== SearchResult Helper Tests ====================

    #[test]
    fn test_search_result_from_metric() {
        use crate::SearchResult;

        let metric = MetricInstrumentation {
            kind: MetricKind::Histogram,
            name: "http_request_duration_seconds".to_string(),
            labels: vec!["method".to_string(), "path".to_string()],
            file: PathBuf::from("src/middleware.rs"),
            line: 42,
            column: 8,
            function_name: Some("request_handler".to_string()),
            impl_type: Some("HttpServer".to_string()),
        };

        let result = SearchResult::from_metric(&metric, 0.95);

        assert_eq!(result.name, "http_request_duration_seconds");
        assert_eq!(result.file, PathBuf::from("src/middleware.rs"));
        assert_eq!(result.line, 42);
        assert!((result.score - 0.95).abs() < f32::EPSILON);
        assert_eq!(result.snippet, Some("request_handler".to_string()));
        assert!(matches!(
            result.kind,
            SearchResultKind::Metric(MetricKind::Histogram)
        ));
    }

    #[test]
    fn test_search_result_from_alert() {
        use crate::SearchResult;

        let alert = AlertRule {
            name: "HighErrorRate".to_string(),
            expr: "rate(errors[5m]) > 0.1".to_string(),
            metric_name: Some("errors".to_string()),
            severity: Some("critical".to_string()),
            message: Some("Error rate is too high".to_string()),
            runbook_url: Some("https://runbook.example.com/errors".to_string()),
            file: PathBuf::from("alerts/http.yaml"),
            line: 15,
            column: 2,
        };

        let result = SearchResult::from_alert(&alert, 0.88);

        assert_eq!(result.name, "HighErrorRate");
        assert_eq!(result.file, PathBuf::from("alerts/http.yaml"));
        assert_eq!(result.line, 15);
        assert!((result.score - 0.88).abs() < f32::EPSILON);
        assert_eq!(result.snippet, Some("rate(errors[5m]) > 0.1".to_string()));
        assert!(matches!(
            result.kind,
            SearchResultKind::Alert {
                severity: Some(ref s)
            } if s == "critical"
        ));
    }

    #[test]
    fn test_search_result_from_metric_no_function() {
        use crate::SearchResult;

        let metric = MetricInstrumentation {
            kind: MetricKind::Counter,
            name: "global_counter".to_string(),
            labels: vec![],
            file: PathBuf::from("src/lib.rs"),
            line: 5,
            column: 0,
            function_name: None, // No function
            impl_type: None,
        };

        let result = SearchResult::from_metric(&metric, 1.0);
        assert!(result.snippet.is_none());
    }

    #[test]
    fn test_search_result_from_alert_no_severity() {
        use crate::SearchResult;

        let alert = AlertRule {
            name: "InfoAlert".to_string(),
            expr: "up == 0".to_string(),
            metric_name: None,
            severity: None,
            message: None,
            runbook_url: None,
            file: PathBuf::from("alerts.yaml"),
            line: 1,
            column: 0,
        };

        let result = SearchResult::from_alert(&alert, 0.5);
        assert!(matches!(
            result.kind,
            SearchResultKind::Alert { severity: None }
        ));
    }

    // ==================== Semantic Fields Tests ====================

    #[allow(clippy::too_many_arguments)]
    fn make_test_commit_with_semantics(
        hash: &str,
        timestamp: i64,
        message: &str,
        files: Vec<&str>,
        funcs_added: Vec<&str>,
        funcs_removed: Vec<&str>,
        funcs_modified: Vec<&str>,
        metrics_added: Vec<&str>,
        metrics_removed: Vec<&str>,
    ) -> CommitInfo {
        use enya_analyzer::DiffSemantics;
        CommitInfo {
            hash: hash.to_string(),
            timestamp,
            message: message.to_string(),
            files_changed: files.into_iter().map(String::from).collect(),
            semantics: DiffSemantics {
                functions_added: funcs_added.into_iter().map(String::from).collect(),
                functions_removed: funcs_removed.into_iter().map(String::from).collect(),
                functions_modified: funcs_modified.into_iter().map(String::from).collect(),
                metrics_added: metrics_added.into_iter().map(String::from).collect(),
                metrics_removed: metrics_removed.into_iter().map(String::from).collect(),
                imports_added: vec![],
                imports_removed: vec![],
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_search_by_function_added() {
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
            make_test_commit_with_semantics(
                "abc123",
                1700000000,
                "feat: add authentication",
                vec!["src/auth.rs"],
                vec!["authenticate_user", "validate_token"],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
            make_test_commit_with_semantics(
                "def456",
                1700001000,
                "feat: add caching",
                vec!["src/cache.rs"],
                vec!["cache_get", "cache_set"],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
        ];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Search for function name added
        let results = index.search("authenticate_user", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::Commit { hash, .. } if hash == "abc123"
        ));

        // Search for partial function name
        let results = index.search("cache", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::Commit { hash, .. } if hash == "def456"
        ));
    }

    #[test]
    fn test_search_by_function_modified() {
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

        let commits = vec![make_test_commit_with_semantics(
            "abc123",
            1700000000,
            "fix: update handler",
            vec!["src/handler.rs"],
            vec![],
            vec![],
            vec!["process_request", "handle_error"],
            vec![],
            vec![],
        )];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Search for modified function
        let results = index.search("process_request", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_by_metrics_added_removed() {
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
            make_test_commit_with_semantics(
                "abc123",
                1700000000,
                "feat: add observability",
                vec!["src/metrics.rs"],
                vec![],
                vec![],
                vec![],
                vec!["http_requests_total", "http_request_duration"],
                vec![],
            ),
            make_test_commit_with_semantics(
                "def456",
                1700001000,
                "chore: remove deprecated metrics",
                vec!["src/metrics.rs"],
                vec![],
                vec![],
                vec![],
                vec![],
                vec!["old_counter", "legacy_gauge"],
            ),
        ];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Search for metric added
        let results = index.search("http_requests_total", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::Commit { hash, .. } if hash == "abc123"
        ));

        // Search for metric removed
        let results = index.search("legacy_gauge", SearchFilter::Commits, 10);
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::Commit { hash, .. } if hash == "def456"
        ));
    }

    // ==================== Complex Query Tests ====================

    #[test]
    fn test_multi_word_query() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("http_requests_total", "src/api.rs", 10),
                make_test_metric("grpc_requests_total", "src/grpc.rs", 20),
                make_test_metric("http_errors_total", "src/api.rs", 30),
            ],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Multi-word query should match metrics with all words
        let results = index.search("http requests", SearchFilter::Metrics, 10);
        // Should find http_requests_total (has both) but not grpc_requests_total
        assert!(
            results.iter().any(|r| r.name == "http_requests_total"),
            "Should find metric with both words"
        );
    }

    #[test]
    fn test_quoted_phrase_query() {
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
            make_test_commit("abc123", 1700000000, "feat: add user authentication flow"),
            make_test_commit("def456", 1700001000, "fix: user profile page"),
            make_test_commit("ghi789", 1700002000, "docs: authentication guide"),
        ];

        index.rebuild_with_commits(&codebase, &commits).unwrap();

        // Quoted phrase should match exact sequence
        let results = index.search("\"user authentication\"", SearchFilter::Commits, 10);
        // Should only match the commit with "user authentication" together
        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0].kind,
            SearchResultKind::Commit { hash, .. } if hash == "abc123"
        ));
    }

    #[test]
    fn test_limit_parameter() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        // Create many metrics
        let metrics: Vec<MetricInstrumentation> = (0..50)
            .map(|i| make_test_metric(&format!("metric_{i}"), "src/metrics.rs", i))
            .collect();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics,
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Search with limit of 5
        let results = index.search("metric", SearchFilter::Metrics, 5);
        assert_eq!(results.len(), 5, "Should respect limit parameter");

        // Search with limit of 100 (more than available)
        let results = index.search("metric", SearchFilter::Metrics, 100);
        assert_eq!(results.len(), 50, "Should return all available results");
    }

    #[test]
    fn test_search_by_file_path() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("api_metric", "src/api/handler.rs", 10),
                make_test_metric("db_metric", "src/database/queries.rs", 20),
            ],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Search by file path component
        let results = index.search("handler", SearchFilter::Metrics, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "api_metric");

        // Search by directory name
        let results = index.search("database", SearchFilter::Metrics, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "db_metric");
    }

    #[test]
    fn test_search_by_labels() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                MetricInstrumentation {
                    kind: MetricKind::Counter,
                    name: "http_requests".to_string(),
                    labels: vec!["method".to_string(), "endpoint".to_string()],
                    file: PathBuf::from("src/api.rs"),
                    line: 10,
                    column: 0,
                    function_name: None,
                    impl_type: None,
                },
                MetricInstrumentation {
                    kind: MetricKind::Gauge,
                    name: "active_connections".to_string(),
                    labels: vec!["service".to_string()],
                    file: PathBuf::from("src/conn.rs"),
                    line: 20,
                    column: 0,
                    function_name: None,
                    impl_type: None,
                },
            ],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Search by label name
        let results = index.search("endpoint", SearchFilter::Metrics, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "http_requests");
    }

    // ==================== Error Handling Tests ====================

    #[test]
    fn test_index_error_display() {
        use crate::IndexError;

        let tantivy_err = IndexError::Tantivy(TantivyError::SystemError("test error".to_string()));
        assert!(tantivy_err.to_string().contains("Tantivy error"));

        let io_err = IndexError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_err.to_string().contains("I/O error"));

        let not_init = IndexError::NotInitialized;
        assert!(not_init.to_string().contains("not initialized"));

        let parse_err = IndexError::MetadataParse("invalid json".to_string());
        assert!(parse_err.to_string().contains("Metadata parse error"));
    }

    #[test]
    fn test_open_nonexistent_creates_new() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        // open_or_create on nonexistent index should create it
        let index = TantivyCodebaseIndex::open_or_create(&repo_path).unwrap();
        assert_eq!(index.metric_count(), 0);
        assert_eq!(index.alert_count(), 0);
        assert_eq!(index.commit_count(), 0);
    }

    #[test]
    fn test_delete_index() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        // Create and populate
        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![make_test_metric("test", "test.rs", 1)],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        assert!(index_dir.exists());

        // Delete
        index.delete().unwrap();

        assert!(!index_dir.exists(), "Index directory should be deleted");
    }

    // ==================== Metric Kind Tests ====================

    #[test]
    fn test_all_metric_kinds() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();
        let codebase = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                MetricInstrumentation {
                    kind: MetricKind::Counter,
                    name: "counter_metric".to_string(),
                    labels: vec![],
                    file: PathBuf::from("src/metrics.rs"),
                    line: 10,
                    column: 0,
                    function_name: None,
                    impl_type: None,
                },
                MetricInstrumentation {
                    kind: MetricKind::Gauge,
                    name: "gauge_metric".to_string(),
                    labels: vec![],
                    file: PathBuf::from("src/metrics.rs"),
                    line: 20,
                    column: 0,
                    function_name: None,
                    impl_type: None,
                },
                MetricInstrumentation {
                    kind: MetricKind::Histogram,
                    name: "histogram_metric".to_string(),
                    labels: vec![],
                    file: PathBuf::from("src/metrics.rs"),
                    line: 30,
                    column: 0,
                    function_name: None,
                    impl_type: None,
                },
            ],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase).unwrap();

        // Verify each kind is returned correctly
        let results = index.search("counter_metric", SearchFilter::Metrics, 10);
        assert!(matches!(
            results[0].kind,
            SearchResultKind::Metric(MetricKind::Counter)
        ));

        let results = index.search("gauge_metric", SearchFilter::Metrics, 10);
        assert!(matches!(
            results[0].kind,
            SearchResultKind::Metric(MetricKind::Gauge)
        ));

        let results = index.search("histogram_metric", SearchFilter::Metrics, 10);
        assert!(matches!(
            results[0].kind,
            SearchResultKind::Metric(MetricKind::Histogram)
        ));
    }

    // ==================== Index Counts Tests ====================

    #[test]
    fn test_indexed_commit_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        // Fresh index has no indexed commit
        assert!(index.indexed_commit().is_none());
    }

    #[test]
    fn test_counts_after_multiple_rebuilds() {
        let temp_dir = TempDir::new().unwrap();
        let index_dir = temp_dir.path().join("index");

        let mut index = TantivyCodebaseIndex::create(&index_dir).unwrap();

        // First rebuild with 2 metrics with unique prefix
        let codebase1 = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("old_apple_metric", "test.rs", 1),
                make_test_metric("old_banana_metric", "test.rs", 2),
            ],
            alerts: vec![],
            last_updated: 0,
        };
        index.rebuild(&codebase1).unwrap();
        assert_eq!(index.metric_count(), 2);

        // Second rebuild with 5 metrics - should replace, not add
        let codebase2 = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("fresh_cherry_metric", "test.rs", 1),
                make_test_metric("fresh_date_metric", "test.rs", 2),
                make_test_metric("fresh_elderberry_metric", "test.rs", 3),
                make_test_metric("fresh_fig_metric", "test.rs", 4),
                make_test_metric("fresh_grape_metric", "test.rs", 5),
            ],
            alerts: vec![make_test_alert("unique_rebuild_alert", "expr", None)],
            last_updated: 0,
        };
        index.rebuild(&codebase2).unwrap();

        assert_eq!(index.metric_count(), 5);
        assert_eq!(index.alert_count(), 1);

        // Old metrics should not be searchable (using unique term "apple")
        let results = index.search("apple", SearchFilter::Metrics, 10);
        assert!(
            results.is_empty(),
            "Old metrics should be cleared after rebuild"
        );

        // New metrics should be searchable (using unique term "cherry")
        let results = index.search("cherry", SearchFilter::Metrics, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fresh_cherry_metric");
    }
}
