//! Codebase integration for the Enya editor.
//!
//! This module provides functionality to connect the editor to a git repository,
//! parse source files using tree-sitter, and discover metric instrumentation
//! points across multiple languages.
//!
//! # Architecture
//!
//! - [`CodebaseManager`]: Main entry point, manages git clone/fetch and indexing
//! - [`scanner`]: Language-agnostic scanner framework with trait-based extensibility
//! - [`repo`]: Git operations (clone, fetch, update)
//! - [`parser`]: Tree-sitter parsing utilities
//! - [`index`]: In-memory index of discovered instrumentation

mod index;
mod parser;
mod repo;
pub mod scanner;

pub use index::CodebaseIndex;
pub use scanner::{MetricInstrumentation, MetricKind, Scanner, ScannerRegistry};

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

/// Progress tracking for indexing operations.
#[derive(Debug, Clone)]
pub struct IndexProgress {
    /// Current file being processed (1-indexed)
    pub current: Arc<AtomicUsize>,
    /// Total number of files to process
    pub total: Arc<AtomicUsize>,
}

impl IndexProgress {
    /// Create a new progress tracker.
    pub fn new() -> Self {
        Self {
            current: Arc::new(AtomicUsize::new(0)),
            total: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the current progress values.
    pub fn get(&self) -> (usize, usize) {
        (
            self.current.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }
}

impl Default for IndexProgress {
    fn default() -> Self {
        Self::new()
    }
}

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
    },
    /// Codebase is ready and indexed.
    Ready { url: String },
    /// An error occurred.
    Error { url: String, message: String },
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
            | Self::Ready { url }
            | Self::Error { url, .. } => Some(url),
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
    IndexComplete { url: String, index: CodebaseIndex },
    /// An error occurred.
    Error { url: String, message: String },
}

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
        }
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
            let result = match repo::clone_repo(&url_clone) {
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

        let url = index.repo_url.clone();
        let path = index.repo_path.clone();
        self.status = CodebaseStatus::Fetching { url: url.clone() };

        let pending = Arc::clone(&self.pending_result);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let result = match repo::fetch_updates(&path) {
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
    fn start_indexing(&mut self, url: String, path: std::path::PathBuf, ctx: &egui::Context) {
        // Create shared progress tracker
        let progress = IndexProgress::new();
        self.indexing_progress = Some(progress.clone());

        self.status = CodebaseStatus::Indexing {
            url: url.clone(),
            current: 0,
            total: 0,
        };

        let pending = Arc::clone(&self.pending_result);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            // Create scanner registry for the indexing thread
            let registry = ScannerRegistry::default();

            let result = match index::build_index_with_progress(&url, &path, &progress, &registry) {
                Ok(idx) => CodebaseResult::IndexComplete { url, index: idx },
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
        // Update indexing progress from the shared atomics
        if let Some(ref progress) = self.indexing_progress {
            let (current, total) = progress.get();
            if let CodebaseStatus::Indexing { url, .. } = &self.status {
                self.status = CodebaseStatus::Indexing {
                    url: url.clone(),
                    current,
                    total,
                };
                // Request repaint to show updated progress
                if current > 0 {
                    ctx.request_repaint();
                }
            }
        }

        let result = self.pending_result.lock().take();

        let Some(result) = result else {
            return;
        };

        match result {
            CodebaseResult::CloneComplete { url, path } => {
                // Clone complete, start indexing
                self.start_indexing(url, path, ctx);
            }
            CodebaseResult::FetchComplete {
                url,
                path,
                has_changes,
            } => {
                if has_changes {
                    // Re-index if there were changes
                    self.start_indexing(url, path, ctx);
                } else {
                    // No changes, we're done
                    self.status = CodebaseStatus::Ready { url };
                }
            }
            CodebaseResult::IndexComplete { url, index } => {
                self.index = Some(index);
                self.status = CodebaseStatus::Ready { url };
                self.indexing_progress = None; // Clear progress tracker
            }
            CodebaseResult::Error { url, message } => {
                self.status = CodebaseStatus::Error { url, message };
                self.indexing_progress = None; // Clear progress tracker
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
}
