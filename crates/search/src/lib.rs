//! Full-text search for Enya codebase indexing.
//!
//! This crate provides Tantivy-based full-text search for metrics, alerts,
//! and git commits discovered by the `enya-analyzer` crate.
//!
//! # Features
//!
//! - **BM25 relevance ranking** - Results are sorted by search relevance
//! - **Multi-document types** - Search metrics, alerts, and commits in one query
//! - **Persistent index** - Index is stored on disk for fast startup
//! - **Incremental updates** - Rebuild only when the codebase changes
//!
//! # Example
//!
//! ```ignore
//! use enya_search::{TantivyCodebaseIndex, SearchFilter};
//! use std::path::Path;
//!
//! // Open or create an index for a repository
//! let mut index = TantivyCodebaseIndex::open_or_create(Path::new("/path/to/repo"))?;
//!
//! // Rebuild from a CodebaseIndex
//! index.rebuild(&codebase)?;
//!
//! // Search for metrics
//! let results = index.search("http_requests", SearchFilter::Metrics, 10);
//! for result in results {
//!     println!("{}: {}:{}", result.name, result.file.display(), result.line);
//! }
//! ```

mod schema;
mod tantivy_index;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use enya_analyzer::{AlertRule, MetricInstrumentation, MetricKind};
use parking_lot::RwLock;

pub use schema::{DocType, SchemaFields};
pub use tantivy_index::{IndexError, TantivyCodebaseIndex};

/// Progress tracking for Tantivy indexing operations.
///
/// This is designed to be cloned and shared with background threads,
/// using atomic operations for lock-free progress updates.
#[derive(Debug, Clone)]
pub struct TantivyProgress {
    /// Current item being processed (0-indexed, updated atomically)
    current: Arc<AtomicUsize>,
    /// Total number of items to process
    total: Arc<AtomicUsize>,
    /// Current phase of indexing
    phase: Arc<RwLock<TantivyPhase>>,
    /// Current item name (for display)
    current_item: Arc<RwLock<Option<String>>>,
}

/// Current phase of Tantivy indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TantivyPhase {
    /// Fetching commits from git history
    #[default]
    FetchingCommits,
    /// Indexing metrics
    IndexingMetrics,
    /// Indexing alerts
    IndexingAlerts,
    /// Indexing commits
    IndexingCommits,
    /// Finalizing the index
    Finalizing,
}

impl TantivyPhase {
    /// Returns a human-readable label for the phase.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::FetchingCommits => "Loading commits",
            Self::IndexingMetrics => "Indexing metrics",
            Self::IndexingAlerts => "Indexing alerts",
            Self::IndexingCommits => "Indexing commits",
            Self::Finalizing => "Finalizing index",
        }
    }
}

impl Default for TantivyProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl TantivyProgress {
    /// Create a new progress tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Arc::new(AtomicUsize::new(0)),
            total: Arc::new(AtomicUsize::new(0)),
            phase: Arc::new(RwLock::new(TantivyPhase::default())),
            current_item: Arc::new(RwLock::new(None)),
        }
    }

    /// Get current progress (current, total).
    #[must_use]
    pub fn get(&self) -> (usize, usize) {
        (
            self.current.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    /// Get the current phase.
    #[must_use]
    pub fn phase(&self) -> TantivyPhase {
        *self.phase.read()
    }

    /// Get the current item name being processed.
    #[must_use]
    pub fn current_item(&self) -> Option<String> {
        self.current_item.read().clone()
    }

    /// Set the current phase.
    pub fn set_phase(&self, phase: TantivyPhase) {
        *self.phase.write() = phase;
        // Reset progress when changing phases
        self.current.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        *self.current_item.write() = None;
    }

    /// Set the total count for the current phase.
    pub fn set_total(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
    }

    /// Increment current and optionally set the item name.
    pub fn increment(&self, item_name: Option<String>) {
        self.current.fetch_add(1, Ordering::Relaxed);
        *self.current_item.write() = item_name;
    }

    /// Set the current item name directly.
    pub fn set_current_item(&self, item_name: Option<String>) {
        *self.current_item.write() = item_name;
    }
}

/// The type of a search result.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchResultKind {
    /// A metric instrumentation point.
    Metric(MetricKind),
    /// An alert rule.
    Alert {
        /// Alert severity (critical, warning, info).
        severity: Option<String>,
    },
    /// A git commit.
    Commit {
        /// Commit SHA.
        hash: String,
        /// Unix timestamp.
        timestamp: i64,
        /// Full diff content for viewing.
        diff: String,
    },
}

/// A search result from the codebase index.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The type of result.
    pub kind: SearchResultKind,
    /// The name (metric name, alert name, or commit message).
    pub name: String,
    /// File path (relative to repo root).
    pub file: PathBuf,
    /// Line number (1-indexed).
    pub line: usize,
    /// Relevance score (higher is better).
    pub score: f32,
    /// Optional snippet or additional context.
    pub snippet: Option<String>,
}

impl SearchResult {
    /// Creates a search result from a metric instrumentation.
    #[must_use]
    pub fn from_metric(metric: &MetricInstrumentation, score: f32) -> Self {
        Self {
            kind: SearchResultKind::Metric(metric.kind),
            name: metric.name.clone(),
            file: metric.file.clone(),
            line: metric.line,
            score,
            snippet: metric.function_name.clone(),
        }
    }

    /// Creates a search result from an alert rule.
    #[must_use]
    pub fn from_alert(alert: &AlertRule, score: f32) -> Self {
        Self {
            kind: SearchResultKind::Alert {
                severity: alert.severity.clone(),
            },
            name: alert.name.clone(),
            file: alert.file.clone(),
            line: alert.line,
            score,
            snippet: Some(alert.expr.clone()),
        }
    }
}

/// Filter for search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchFilter {
    /// Search all document types.
    #[default]
    All,
    /// Search only metrics.
    Metrics,
    /// Search only alerts.
    Alerts,
    /// Search only commits.
    Commits,
}
