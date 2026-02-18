//! Codebase integration for the Enya editor.
//!
//! Provides egui integration for the enya-analyzer crate.

use std::sync::Arc;

use parking_lot::Mutex;
use rustc_hash::FxHashMap;

// Re-export from enya-analyzer
pub use enya_analyzer::{
    AlertRule, CodebaseIndex, CommitInfo, IndexProgress, MetricInstrumentation, MetricKind,
    Scanner, ScannerRegistry, build_index_with_progress, fetch_commit_history,
    fetch_recent_commits, get_head_commit, get_head_commit_message,
};

// Full-text search module (native only)
#[cfg(not(target_arch = "wasm32"))]
pub mod search;

#[cfg(not(target_arch = "wasm32"))]
pub use search::{IndexError, SearchFilter, SearchResult, SearchResultKind, TantivyCodebaseIndex};

// Re-export CommitMarker from the chart module
pub use crate::components::pane::time_series_chart::CommitMarker;

/// Status of codebase operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodebaseStatus {
    /// No codebase configured.
    None,
    /// Currently cloning the repository.
    Cloning { url: String },
    /// Currently fetching updates.
    Fetching { url: String },
    /// Currently indexing the codebase.
    Indexing {
        url: String,
        /// Current file being indexed (1-indexed)
        current: usize,
        /// Total files to index
        total: usize,
        /// Name of the current file being indexed
        current_file: Option<String>,
        /// Language being scanned (for icon display)
        language: Option<String>,
    },
    /// Codebase is ready and indexed.
    Ready {
        url: String,
        /// Repository name extracted from URL
        repo_name: String,
        /// Number of metrics discovered
        metrics_count: usize,
        /// Language that was scanned
        language: Option<String>,
        /// HEAD commit message (subject line)
        head_commit_msg: Option<String>,
        /// HEAD commit hash (short form)
        head_commit_hash: Option<String>,
    },
    /// An error occurred.
    Error { url: String, message: String },
}

/// Extract repository name from a git URL.
///
/// Examples:
/// - `git@github.com:org/repo.git` → `repo`
/// - `https://github.com/org/repo.git` → `repo`
/// - `https://github.com/org/repo` → `repo`
fn extract_repo_name(url: &str) -> String {
    // Handle both HTTPS (/) and SSH (:) separators
    let name = url.rsplit(['/', ':']).next().unwrap_or(url);
    name.trim_end_matches(".git").to_string()
}

impl CodebaseStatus {
    /// Returns true if the codebase is ready for queries.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    /// Returns true if an operation is in progress.
    pub fn is_loading(&self) -> bool {
        matches!(
            self,
            Self::Cloning { .. } | Self::Fetching { .. } | Self::Indexing { .. }
        )
    }

    /// Returns the URL if one is configured.
    pub fn url(&self) -> Option<&str> {
        match self {
            Self::None => None,
            Self::Cloning { url }
            | Self::Fetching { url }
            | Self::Indexing { url, .. }
            | Self::Ready { url, .. }
            | Self::Error { url, .. } => Some(url),
        }
    }

    /// Returns the language if one is configured.
    pub fn language(&self) -> Option<&str> {
        match self {
            Self::Indexing { language, .. } | Self::Ready { language, .. } => language.as_deref(),
            _ => None,
        }
    }

    /// Returns the repository name if ready.
    pub fn repo_name(&self) -> Option<&str> {
        match self {
            Self::Ready { repo_name, .. } => Some(repo_name),
            _ => None,
        }
    }

    /// Returns the metrics count if ready.
    pub fn metrics_count(&self) -> Option<usize> {
        match self {
            Self::Ready { metrics_count, .. } => Some(*metrics_count),
            _ => None,
        }
    }

    /// Returns the HEAD commit message if ready.
    pub fn head_commit_msg(&self) -> Option<&str> {
        match self {
            Self::Ready {
                head_commit_msg, ..
            } => head_commit_msg.as_deref(),
            _ => None,
        }
    }

    /// Returns the HEAD commit hash if ready.
    pub fn head_commit_hash(&self) -> Option<&str> {
        match self {
            Self::Ready {
                head_commit_hash, ..
            } => head_commit_hash.as_deref(),
            _ => None,
        }
    }
}

/// Result from an async codebase operation.
#[derive(Debug)]
pub enum CodebaseResult {
    /// Clone completed successfully.
    CloneComplete {
        url: String,
        path: std::path::PathBuf,
    },
    /// Fetch completed, indicates if there were changes.
    FetchComplete {
        url: String,
        path: std::path::PathBuf,
        has_changes: bool,
    },
    /// Indexing completed.
    IndexComplete {
        url: String,
        index: CodebaseIndex,
        language: Option<String>,
    },
    /// Commit history fetch completed.
    HistoryComplete {
        start_secs: i64,
        end_secs: i64,
        commits: Vec<CommitInfo>,
    },
    /// An error occurred.
    Error { url: String, message: String },
}

/// Result from building a Tantivy index (native only).
#[cfg(not(target_arch = "wasm32"))]
type TantivyResult = Result<TantivyCodebaseIndex, IndexError>;

/// Manages codebase integration for the editor.
///
/// Handles git operations (clone/fetch) and source code indexing using
/// a polling pattern compatible with egui's frame-based update loop.
pub struct CodebaseManager {
    status: CodebaseStatus,
    pending_result: Arc<Mutex<Option<CodebaseResult>>>,
    index: Option<CodebaseIndex>,
    /// Progress tracking for indexing (shared with background thread)
    indexing_progress: Option<IndexProgress>,
    /// Registry of available scanners for different languages
    scanner_registry: ScannerRegistry,
    /// Configured language for scanning (e.g., "rust", "go", "python")
    /// If empty, all language scanners are used.
    language: String,
    /// Cached commit history keyed by (start_secs, end_secs)
    commit_cache: FxHashMap<(i64, i64), Vec<CommitMarker>>,
    /// Time range currently being fetched (to avoid duplicate requests)
    pending_history_range: Option<(i64, i64)>,
    /// Flag indicating new commits arrived this frame
    commits_updated: bool,
    /// Tantivy full-text search index (native only)
    #[cfg(not(target_arch = "wasm32"))]
    tantivy_index: Option<TantivyCodebaseIndex>,
    /// Pending Tantivy index result from background thread
    #[cfg(not(target_arch = "wasm32"))]
    pending_tantivy: Arc<Mutex<Option<TantivyResult>>>,
    /// Progress tracking for Tantivy indexing (native only)
    #[cfg(not(target_arch = "wasm32"))]
    tantivy_progress: Option<search::TantivyProgress>,
    /// Interval in seconds between automatic git fetches (0 = disabled)
    git_sync_interval_secs: u64,
    /// Last time an automatic or manual git sync was performed
    last_git_sync: Option<crate::util::Instant>,
}

impl Default for CodebaseManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CodebaseManager {
    /// Creates a new codebase manager with no repository configured.
    pub fn new() -> Self {
        Self {
            status: CodebaseStatus::None,
            pending_result: Arc::new(Mutex::new(None)),
            index: None,
            indexing_progress: None,
            scanner_registry: ScannerRegistry::default(),
            language: String::new(),
            commit_cache: FxHashMap::default(),
            pending_history_range: None,
            commits_updated: false,
            #[cfg(not(target_arch = "wasm32"))]
            tantivy_index: None,
            #[cfg(not(target_arch = "wasm32"))]
            pending_tantivy: Arc::new(Mutex::new(None)),
            #[cfg(not(target_arch = "wasm32"))]
            tantivy_progress: None,
            git_sync_interval_secs: 300,
            last_git_sync: None,
        }
    }

    /// Sets the language for metric scanning.
    ///
    /// When set, only scanners for the specified language will be used during indexing.
    /// Supported values: "rust", "go", "python", "javascript", "typescript"
    pub fn set_language(&mut self, language: impl Into<String>) {
        self.language = language.into();
    }

    /// Returns a reference to the scanner registry.
    pub fn scanner_registry(&self) -> &ScannerRegistry {
        &self.scanner_registry
    }

    /// Returns the current status.
    pub fn status(&self) -> &CodebaseStatus {
        &self.status
    }

    /// Returns the codebase index if available.
    pub fn index(&self) -> Option<&CodebaseIndex> {
        self.index.as_ref()
    }

    /// Initiates cloning a repository from the given URL.
    ///
    /// The clone happens in a background thread. Call [`poll`](Self::poll) each
    /// frame to check for completion.
    pub fn clone_repo(&mut self, url: &str, ctx: &egui::Context) {
        let url = url.to_string();
        self.status = CodebaseStatus::Cloning { url: url.clone() };

        let pending = Arc::clone(&self.pending_result);
        let ctx = ctx.clone();
        let url_clone = url.clone();

        std::thread::spawn(move || {
            let result = match enya_analyzer::repo::clone_repo(&url_clone) {
                Ok(path) => CodebaseResult::CloneComplete {
                    url: url_clone,
                    path,
                },
                Err(e) => CodebaseResult::Error {
                    url: url_clone,
                    message: e.to_string(),
                },
            };

            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Fetches updates for the currently configured repository.
    pub fn fetch_updates(&mut self, ctx: &egui::Context) {
        let Some(index) = &self.index else {
            return;
        };

        // Reset auto-sync timer so we don't double-fetch after a manual sync
        self.last_git_sync = Some(crate::util::Instant::now());

        let url = index.repo_url.clone();
        let path = index.repo_path.clone();
        self.status = CodebaseStatus::Fetching { url: url.clone() };

        let pending = Arc::clone(&self.pending_result);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let result = match enya_analyzer::repo::fetch_updates(&path) {
                Ok(has_changes) => CodebaseResult::FetchComplete {
                    url,
                    path,
                    has_changes,
                },
                Err(e) => CodebaseResult::Error {
                    url,
                    message: e.to_string(),
                },
            };

            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Builds the codebase index from the given repository path.
    ///
    /// If `language` is provided, only scanners for that language will be used.
    /// Otherwise, all language scanners are used.
    fn start_indexing(
        &mut self,
        url: String,
        path: std::path::PathBuf,
        language: Option<String>,
        ctx: &egui::Context,
    ) {
        // Create shared progress tracker
        let progress = IndexProgress::new();
        self.indexing_progress = Some(progress.clone());

        self.status = CodebaseStatus::Indexing {
            url: url.clone(),
            current: 0,
            total: 0,
            current_file: None,
            language: language.clone(),
        };

        let pending = Arc::clone(&self.pending_result);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            // Create scanner registry for the indexing thread
            // Use language-specific registry if configured, otherwise use all scanners
            let registry = match language {
                Some(ref lang) if !lang.is_empty() => ScannerRegistry::for_language(lang),
                _ => ScannerRegistry::default(),
            };

            let result = match build_index_with_progress(&url, &path, &progress, &registry) {
                Ok(idx) => CodebaseResult::IndexComplete {
                    url,
                    index: idx,
                    language,
                },
                Err(e) => CodebaseResult::Error {
                    url,
                    message: e.to_string(),
                },
            };

            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Polls for completion of async operations.
    ///
    /// Call this each frame to check if clone/fetch/index operations have completed.
    pub fn poll(&mut self, ctx: &egui::Context) {
        // Reset per-frame flags
        self.commits_updated = false;

        // Request continuous repaints during fetching (so UI stays responsive)
        if matches!(self.status, CodebaseStatus::Fetching { .. }) {
            ctx.request_repaint();
        }

        // Update indexing progress from the shared atomics
        if let Some(ref progress) = self.indexing_progress {
            let (current, total) = progress.get();
            let current_file = progress.current_file();
            if let CodebaseStatus::Indexing { url, language, .. } = &self.status {
                self.status = CodebaseStatus::Indexing {
                    url: url.clone(),
                    current,
                    total,
                    current_file,
                    language: language.clone(),
                };
                // Request repaint to show updated progress
                if current > 0 {
                    ctx.request_repaint();
                }
            }
        }

        // Request continuous repaints during Tantivy indexing
        #[cfg(not(target_arch = "wasm32"))]
        if self.tantivy_progress.is_some() {
            ctx.request_repaint();
        }

        // Check for completed Tantivy index build (native only)
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(result) = self.pending_tantivy.lock().take() {
            match result {
                Ok(tantivy_index) => {
                    log::info!(
                        "Tantivy index ready: {} metrics, {} alerts, {} commits",
                        tantivy_index.metric_count(),
                        tantivy_index.alert_count(),
                        tantivy_index.commit_count()
                    );
                    self.tantivy_index = Some(tantivy_index);
                }
                Err(e) => {
                    log::warn!("Failed to build Tantivy index: {e}");
                    // Continue without Tantivy - fallback to in-memory search
                }
            }
            // Clear progress tracker when done
            self.tantivy_progress = None;
        }

        let result = self.pending_result.lock().take();

        let Some(result) = result else {
            return;
        };

        match result {
            CodebaseResult::CloneComplete { url, path } => {
                // Clone complete, start indexing
                let language = if self.language.is_empty() {
                    None
                } else {
                    Some(self.language.clone())
                };
                self.start_indexing(url, path, language, ctx);
            }
            CodebaseResult::FetchComplete {
                url,
                path,
                has_changes,
            } => {
                if has_changes {
                    // Re-index if there were changes
                    let language = if self.language.is_empty() {
                        None
                    } else {
                        Some(self.language.clone())
                    };
                    self.start_indexing(url, path, language, ctx);
                } else {
                    // No changes, preserve existing ready state info
                    if let CodebaseStatus::Ready {
                        repo_name,
                        metrics_count,
                        language,
                        head_commit_msg,
                        head_commit_hash,
                        ..
                    } = &self.status
                    {
                        self.status = CodebaseStatus::Ready {
                            url,
                            repo_name: repo_name.clone(),
                            metrics_count: *metrics_count,
                            language: language.clone(),
                            head_commit_msg: head_commit_msg.clone(),
                            head_commit_hash: head_commit_hash.clone(),
                        };
                    } else {
                        // Fallback if we somehow weren't ready before
                        let language = if self.language.is_empty() {
                            None
                        } else {
                            Some(self.language.clone())
                        };
                        // Get the head commit message and hash
                        let head_commit_msg = self
                            .index
                            .as_ref()
                            .and_then(|idx| get_head_commit_message(&idx.repo_path).ok());
                        let head_commit_hash = self
                            .index
                            .as_ref()
                            .and_then(|idx| get_head_commit(&idx.repo_path).ok());
                        self.status = CodebaseStatus::Ready {
                            repo_name: extract_repo_name(&url),
                            metrics_count: self.index.as_ref().map_or(0, |i| i.metrics.len()),
                            language,
                            url,
                            head_commit_msg,
                            head_commit_hash,
                        };
                    }
                }
            }
            CodebaseResult::IndexComplete {
                url,
                index,
                language,
            } => {
                let metrics_count = index.metrics.len();
                let repo_name = extract_repo_name(&url);

                // Spawn background task to build Tantivy index (native only)
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let repo_path = index.repo_path.clone();
                    let index_clone = index.clone();
                    let pending_tantivy = Arc::clone(&self.pending_tantivy);
                    let ctx_clone = ctx.clone();

                    // Create progress tracker for Tantivy indexing
                    let progress = search::TantivyProgress::new();
                    self.tantivy_progress = Some(progress.clone());

                    std::thread::spawn(move || {
                        progress.set_phase(search::TantivyPhase::FetchingCommits);

                        // Check for existing index to enable incremental updates
                        let existing_index = TantivyCodebaseIndex::open_or_create(&repo_path).ok();
                        let last_indexed_commit = existing_index
                            .as_ref()
                            .and_then(|idx| idx.indexed_commit().map(ToString::to_string));

                        let is_incremental = last_indexed_commit.is_some();
                        if let Some(ref commit) = last_indexed_commit {
                            progress.set_current_item(Some(format!(
                                "Checking for new commits since {}...",
                                &commit[..7.min(commit.len())]
                            )));
                        } else {
                            progress.set_current_item(Some(
                                "Loading full commit history...".to_string(),
                            ));
                        }

                        // Create progress callback for diff fetching
                        let progress_clone = progress.clone();
                        let ctx_for_progress = ctx_clone.clone();
                        let last_repaint =
                            std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                        let last_repaint_clone = Arc::clone(&last_repaint);

                        let progress_callback: enya_analyzer::ProgressCallback =
                            Box::new(move |current, total, item| {
                                progress_clone.set_total(total);
                                progress_clone
                                    .set_progress_atomic(current, item.map(ToString::to_string));

                                let last =
                                    last_repaint_clone.load(std::sync::atomic::Ordering::Relaxed);
                                if (current >= last + 100 || current == total)
                                    && last_repaint_clone
                                        .compare_exchange(
                                            last,
                                            current,
                                            std::sync::atomic::Ordering::Relaxed,
                                            std::sync::atomic::Ordering::Relaxed,
                                        )
                                        .is_ok()
                                {
                                    ctx_for_progress.request_repaint();
                                }
                            });

                        // Fetch commits with batch mode (single git command for all diffs)
                        // If we have a last indexed commit, only fetch new commits (incremental)
                        let commits = enya_analyzer::fetch_all_commits_with_diffs_batch(
                            &repo_path,
                            last_indexed_commit.as_deref(),
                            Some(&progress_callback),
                        )
                        .unwrap_or_else(|e| {
                            log::warn!("Failed to fetch commits for indexing: {e}");
                            Vec::new()
                        });

                        ctx_clone.request_repaint();

                        // Build or update the Tantivy index
                        let result = if is_incremental && !commits.is_empty() {
                            // Incremental update: add new commits to existing index
                            log::info!(
                                "Incremental update: adding {} new commits to index",
                                commits.len()
                            );
                            existing_index.ok_or(IndexError::NotInitialized).and_then(
                                |mut tantivy_index| {
                                    tantivy_index.add_commits(&commits, Some(&progress))?;
                                    Ok(tantivy_index)
                                },
                            )
                        } else if is_incremental && commits.is_empty() {
                            // No new commits - just return existing index
                            log::info!("No new commits to index");
                            existing_index.ok_or(IndexError::NotInitialized)
                        } else {
                            // Full rebuild
                            log::info!("Full index rebuild with {} commits", commits.len());
                            TantivyCodebaseIndex::open_or_create(&repo_path).and_then(
                                |mut tantivy_index| {
                                    tantivy_index.rebuild_with_progress(
                                        &index_clone,
                                        &commits,
                                        Some(&progress),
                                    )?;
                                    Ok(tantivy_index)
                                },
                            )
                        };

                        *pending_tantivy.lock() = Some(result);
                        ctx_clone.request_repaint();
                    });
                }

                // Get the head commit message and hash
                let head_commit_msg = get_head_commit_message(&index.repo_path).ok();
                let head_commit_hash = get_head_commit(&index.repo_path).ok();

                self.index = Some(index);
                self.status = CodebaseStatus::Ready {
                    url,
                    repo_name,
                    metrics_count,
                    language,
                    head_commit_msg,
                    head_commit_hash,
                };
                self.indexing_progress = None; // Clear progress tracker
            }
            CodebaseResult::HistoryComplete {
                start_secs,
                end_secs,
                commits,
            } => {
                // Convert CommitInfo to CommitMarker and cache
                let markers: Vec<CommitMarker> = commits
                    .into_iter()
                    .map(|c| CommitMarker::new(c.hash, c.timestamp as f64, c.message))
                    .collect();
                self.commit_cache.insert((start_secs, end_secs), markers);
                self.pending_history_range = None;
                self.commits_updated = true;
            }
            CodebaseResult::Error { url, message } => {
                self.status = CodebaseStatus::Error { url, message };
                self.indexing_progress = None; // Clear progress tracker
                self.pending_history_range = None;
            }
        }
    }

    /// Searches for metrics matching the given query.
    pub fn search_metrics(&self, query: &str) -> Vec<&MetricInstrumentation> {
        let Some(index) = &self.index else {
            return Vec::new();
        };

        let query_lower = query.to_lowercase();
        index
            .metrics
            .iter()
            .filter(|m| m.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Returns all discovered metrics.
    pub fn all_metrics(&self) -> &[MetricInstrumentation] {
        self.index
            .as_ref()
            .map(|i| i.metrics.as_slice())
            .unwrap_or(&[])
    }

    /// Initiates fetching commit history for a time range.
    ///
    /// The fetch happens in a background thread. Call [`poll`](Self::poll) each
    /// frame to check for completion. Results are cached.
    pub fn fetch_history(&mut self, start_secs: f64, end_secs: f64, ctx: &egui::Context) {
        let Some(index) = &self.index else {
            return;
        };

        let start = start_secs as i64;
        let end = end_secs as i64;

        // Already cached?
        if self.commit_cache.contains_key(&(start, end)) {
            return;
        }

        // Already fetching this range?
        if self.pending_history_range == Some((start, end)) {
            return;
        }

        let repo_path = index.repo_path.clone();
        self.pending_history_range = Some((start, end));

        let pending = Arc::clone(&self.pending_result);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let result = match fetch_commit_history(&repo_path, start, end) {
                Ok(commits) => CodebaseResult::HistoryComplete {
                    start_secs: start,
                    end_secs: end,
                    commits,
                },
                Err(e) => CodebaseResult::Error {
                    url: repo_path.display().to_string(),
                    message: format!("Failed to fetch history: {e}"),
                },
            };

            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Returns cached commits for the given time range, if available.
    ///
    /// Returns `None` if commits haven't been fetched yet for this range.
    /// Call [`fetch_history`](Self::fetch_history) to initiate a fetch.
    #[must_use]
    pub fn get_commits(&self, start_secs: f64, end_secs: f64) -> Option<&[CommitMarker]> {
        let key = (start_secs as i64, end_secs as i64);
        self.commit_cache.get(&key).map(Vec::as_slice)
    }

    /// Returns true if new commits arrived during the last [`poll`](Self::poll) call.
    #[must_use]
    pub fn commits_updated(&self) -> bool {
        self.commits_updated
    }

    /// Clears the commit cache.
    ///
    /// Call this when the codebase is updated to ensure fresh history.
    pub fn clear_commit_cache(&mut self) {
        self.commit_cache.clear();
    }

    /// Returns true if Tantivy full-text search is available.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn has_tantivy_index(&self) -> bool {
        self.tantivy_index.is_some()
    }

    /// Returns true if Tantivy index is currently being built in the background.
    ///
    /// This is true when we're in Ready state (tree-sitter done) but Tantivy
    /// hasn't finished yet.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn is_tantivy_indexing(&self) -> bool {
        // We're building Tantivy if we have a progress tracker active
        self.tantivy_progress.is_some()
    }

    /// Returns the Tantivy indexing progress if currently building.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn tantivy_progress(&self) -> Option<&search::TantivyProgress> {
        self.tantivy_progress.as_ref()
    }

    /// Returns a reference to the Tantivy index if available.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn tantivy_index(&self) -> Option<&TantivyCodebaseIndex> {
        self.tantivy_index.as_ref()
    }

    /// Searches using Tantivy full-text search.
    ///
    /// Returns ranked search results. Falls back to in-memory substring
    /// search if Tantivy is not available.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn search_ranked(
        &self,
        query: &str,
        filter: SearchFilter,
        limit: usize,
    ) -> Vec<SearchResult> {
        if let Some(tantivy) = &self.tantivy_index {
            tantivy.search(query, filter, limit)
        } else {
            // Fallback to in-memory search
            self.fallback_search(query, filter, limit)
        }
    }

    /// Fallback in-memory search when Tantivy is not available.
    #[cfg(not(target_arch = "wasm32"))]
    fn fallback_search(
        &self,
        query: &str,
        filter: SearchFilter,
        limit: usize,
    ) -> Vec<SearchResult> {
        let Some(index) = &self.index else {
            return Vec::new();
        };

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        // Search metrics
        if matches!(filter, SearchFilter::All | SearchFilter::Metrics) {
            for metric in &index.metrics {
                if metric.name.to_lowercase().contains(&query_lower) {
                    results.push(SearchResult::from_metric(metric, 1.0));
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        }

        // Search alerts
        if matches!(filter, SearchFilter::All | SearchFilter::Alerts) {
            for alert in &index.alerts {
                if alert.name.to_lowercase().contains(&query_lower)
                    || alert
                        .metric_name
                        .as_ref()
                        .is_some_and(|m| m.to_lowercase().contains(&query_lower))
                {
                    results.push(SearchResult::from_alert(alert, 1.0));
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        }

        results
    }

    /// Sets the interval for automatic git fetch in seconds.
    ///
    /// Set to 0 to disable automatic fetching.
    pub fn set_git_sync_interval(&mut self, seconds: u64) {
        self.git_sync_interval_secs = seconds;
    }

    /// Checks if an automatic git fetch is due and triggers it if so.
    ///
    /// This mirrors the `check_auto_refresh` pattern used for query refresh.
    /// Only fetches when the codebase is in `Ready` state to avoid interfering
    /// with ongoing clone/fetch/index operations.
    pub fn check_auto_git_sync(&mut self, ctx: &egui::Context) {
        if self.git_sync_interval_secs == 0 {
            return;
        }
        if !self.status.is_ready() {
            return;
        }
        if self.index.is_none() {
            return;
        }

        let now = crate::util::Instant::now();
        let should_sync = match self.last_git_sync {
            Some(last) => now.duration_since(last).as_secs() >= self.git_sync_interval_secs,
            None => true,
        };

        if should_sync {
            log::info!(
                "Auto git sync triggered (interval: {}s)",
                self.git_sync_interval_secs
            );
            self.fetch_updates(ctx);
        }
    }
}
