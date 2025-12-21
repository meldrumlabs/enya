//! Codebase index for discovered metrics.
//!
//! Builds and maintains an in-memory index of all metric instrumentation
//! points discovered in a repository using registered scanners.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use rustc_hash::FxHashSet;
use walkdir::WalkDir;

use super::IndexProgress;
use super::parser::ParseError;
use super::scanner::{MetricInstrumentation, ScannerRegistry};
use crate::util::now_unix_secs;

/// An index of all discovered metric instrumentation in a codebase.
#[derive(Debug, Clone)]
pub struct CodebaseIndex {
    /// The git URL of the repository.
    pub repo_url: String,
    /// The local path to the repository.
    pub repo_path: PathBuf,
    /// All discovered metric instrumentation points.
    pub metrics: Vec<MetricInstrumentation>,
    /// Unix timestamp when this index was built.
    pub last_updated: i64,
}

impl CodebaseIndex {
    /// Returns the number of unique metric names.
    pub fn unique_metric_count(&self) -> usize {
        let mut names: Vec<_> = self.metrics.iter().map(|m| &m.name).collect();
        names.sort();
        names.dedup();
        names.len()
    }

    /// Returns the number of files containing metrics.
    pub fn files_with_metrics(&self) -> usize {
        let mut files: Vec<_> = self.metrics.iter().map(|m| &m.file).collect();
        files.sort();
        files.dedup();
        files.len()
    }

    /// Searches for metrics matching the given query.
    pub fn search(&self, query: &str) -> Vec<&MetricInstrumentation> {
        let query_lower = query.to_lowercase();
        self.metrics
            .iter()
            .filter(|m| m.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Finds all instrumentation points for a specific metric name.
    pub fn find_by_name(&self, name: &str) -> Vec<&MetricInstrumentation> {
        self.metrics.iter().filter(|m| m.name == name).collect()
    }
}

/// Builds a codebase index by scanning all supported source files.
///
/// Uses the provided [`ScannerRegistry`] to determine which files to scan
/// and which scanner to use for each file type.
///
/// Updates the provided `IndexProgress` atomics as files are processed,
/// allowing the UI to show progress like "Indexing [5/42]...".
pub fn build_index_with_progress(
    repo_url: &str,
    repo_path: &Path,
    progress: &IndexProgress,
    registry: &ScannerRegistry,
) -> Result<CodebaseIndex, ParseError> {
    // Get all supported extensions from registered scanners
    let extensions: FxHashSet<&str> = registry.all_extensions().into_iter().collect();

    // First pass: collect all scannable files
    let source_files: Vec<_> = WalkDir::new(repo_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(ext))
        })
        .filter(|e| {
            !e.path().components().any(|c| {
                matches!(
                    c.as_os_str().to_str(),
                    Some("target" | ".git" | "vendor" | "node_modules")
                )
            })
        })
        .collect();

    // Set total count
    progress.total.store(source_files.len(), Ordering::SeqCst);

    let mut all_metrics = Vec::new();

    // Second pass: scan files with progress updates
    for (i, entry) in source_files.iter().enumerate() {
        // Update current progress (1-indexed for display)
        progress.current.store(i + 1, Ordering::SeqCst);

        let path = entry.path();

        // Find the appropriate scanner for this file
        let Some(scanner) = registry.scanner_for(path) else {
            continue;
        };

        // Scan the file for metrics
        match scanner.scan_file(path) {
            Ok(metrics) => {
                // Convert absolute paths to relative paths from repo root
                for mut metric in metrics {
                    if let Ok(relative) = metric.file.strip_prefix(repo_path) {
                        metric.file = relative.to_path_buf();
                    }
                    all_metrics.push(metric);
                }
            }
            Err(e) => {
                // Log but don't fail on individual file errors
                log::warn!("Failed to scan {}: {}", path.display(), e);
            }
        }
    }

    // Sort by file path, then line number for consistent ordering
    all_metrics.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

    Ok(CodebaseIndex {
        repo_url: repo_url.to_string(),
        repo_path: repo_path.to_path_buf(),
        metrics: all_metrics,
        last_updated: now_unix_secs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codebase::scanner::MetricKind;

    fn make_test_metric(name: &str, file: &str, line: usize) -> MetricInstrumentation {
        MetricInstrumentation {
            kind: MetricKind::Counter,
            name: name.to_string(),
            labels: vec![],
            file: PathBuf::from(file),
            line,
            column: 0,
        }
    }

    #[test]
    fn test_unique_metric_count() {
        let index = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("http.requests", "a.rs", 1),
                make_test_metric("http.requests", "b.rs", 1), // Same name, different file
                make_test_metric("db.queries", "c.rs", 1),
            ],
            last_updated: 0,
        };

        assert_eq!(index.unique_metric_count(), 2);
    }

    #[test]
    fn test_files_with_metrics() {
        let index = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("metric1", "a.rs", 1),
                make_test_metric("metric2", "a.rs", 2), // Same file
                make_test_metric("metric3", "b.rs", 1),
            ],
            last_updated: 0,
        };

        assert_eq!(index.files_with_metrics(), 2);
    }

    #[test]
    fn test_search() {
        let index = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("http.requests", "a.rs", 1),
                make_test_metric("http.errors", "a.rs", 2),
                make_test_metric("db.queries", "b.rs", 1),
            ],
            last_updated: 0,
        };

        let results = index.search("http");
        assert_eq!(results.len(), 2);

        let results = index.search("HTTP"); // Case insensitive
        assert_eq!(results.len(), 2);

        let results = index.search("db");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_find_by_name() {
        let index = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![
                make_test_metric("http.requests", "a.rs", 1),
                make_test_metric("http.requests", "b.rs", 5),
                make_test_metric("other.metric", "c.rs", 1),
            ],
            last_updated: 0,
        };

        let results = index.find_by_name("http.requests");
        assert_eq!(results.len(), 2);

        let results = index.find_by_name("nonexistent");
        assert_eq!(results.len(), 0);
    }
}
