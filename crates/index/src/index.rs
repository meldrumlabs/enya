//! Codebase index for discovered metrics.
//!
//! Builds and maintains an in-memory index of all metric instrumentation
//! points discovered in a repository using registered scanners.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustc_hash::FxHashSet;
use walkdir::{DirEntry, WalkDir};

use crate::parser::ParseError;
use crate::scanner::{AlertRule, MetricInstrumentation, ScannerRegistry, YamlAlertScanner};

/// Directories to exclude from scanning.
const EXCLUDED_DIRS: [&str; 4] = ["target", ".git", "vendor", "node_modules"];

/// Discover files in a directory that match the given extensions.
///
/// Walks the directory tree, filtering for files with matching extensions
/// and excluding common build/vendor directories.
fn discover_files<'a>(
    root: &Path,
    extensions: &'a FxHashSet<&str>,
) -> impl Iterator<Item = DirEntry> + 'a {
    let root = root.to_path_buf();
    WalkDir::new(&root)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter(move |entry| {
            // Check extension matches
            let has_matching_ext = entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(ext));

            // Check not in excluded directory
            let in_excluded_dir = entry.path().components().any(|c| {
                c.as_os_str()
                    .to_str()
                    .is_some_and(|s| EXCLUDED_DIRS.contains(&s))
            });

            has_matching_ext && !in_excluded_dir
        })
}

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
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Arc::new(AtomicUsize::new(0)),
            total: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the current progress values.
    #[must_use]
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

/// An index of all discovered metric instrumentation and alert rules in a codebase.
#[derive(Debug, Clone)]
pub struct CodebaseIndex {
    /// The git URL of the repository.
    pub repo_url: String,
    /// The local path to the repository.
    pub repo_path: PathBuf,
    /// All discovered metric instrumentation points.
    pub metrics: Vec<MetricInstrumentation>,
    /// All discovered Prometheus alert rules.
    pub alerts: Vec<AlertRule>,
    /// Unix timestamp when this index was built.
    pub last_updated: i64,
}

impl CodebaseIndex {
    /// Returns the number of unique metric names.
    #[must_use]
    pub fn unique_metric_count(&self) -> usize {
        self.metrics
            .iter()
            .map(|m| &m.name)
            .collect::<FxHashSet<_>>()
            .len()
    }

    /// Returns the number of files containing metrics.
    #[must_use]
    pub fn files_with_metrics(&self) -> usize {
        self.metrics
            .iter()
            .map(|m| &m.file)
            .collect::<FxHashSet<_>>()
            .len()
    }

    /// Searches for metrics matching the given query.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&MetricInstrumentation> {
        let query_lower = query.to_lowercase();
        self.metrics
            .iter()
            .filter(|m| m.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Finds all instrumentation points for a specific metric name.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Vec<&MetricInstrumentation> {
        self.metrics.iter().filter(|m| m.name == name).collect()
    }

    /// Returns the number of alert rules.
    #[must_use]
    pub fn alert_count(&self) -> usize {
        self.alerts.len()
    }

    /// Finds all alert rules that reference a specific metric name.
    #[must_use]
    pub fn find_alerts_by_metric(&self, metric_name: &str) -> Vec<&AlertRule> {
        self.alerts
            .iter()
            .filter(|a| a.metric_name.as_deref() == Some(metric_name))
            .collect()
    }

    /// Finds an alert rule by its name.
    #[must_use]
    pub fn find_alert_by_name(&self, alert_name: &str) -> Option<&AlertRule> {
        self.alerts.iter().find(|a| a.name == alert_name)
    }

    /// Searches for alert rules matching the given query.
    #[must_use]
    pub fn search_alerts(&self, query: &str) -> Vec<&AlertRule> {
        let query_lower = query.to_lowercase();
        self.alerts
            .iter()
            .filter(|a| {
                a.name.to_lowercase().contains(&query_lower)
                    || a.metric_name
                        .as_ref()
                        .is_some_and(|m| m.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}

/// Builds a codebase index by scanning all supported source files.
///
/// Uses the provided [`ScannerRegistry`] to determine which files to scan
/// and which scanner to use for each file type.
///
/// Updates the provided `IndexProgress` atomics as files are processed,
/// allowing the UI to show progress like "Indexing [5/42]...".
///
/// # Errors
///
/// Returns an error if scanning fails.
pub fn build_index_with_progress(
    repo_url: &str,
    repo_path: &Path,
    progress: &IndexProgress,
    registry: &ScannerRegistry,
) -> Result<CodebaseIndex, ParseError> {
    // Get all supported extensions from registered scanners
    let extensions: FxHashSet<&str> = registry.all_extensions().into_iter().collect();

    // First pass: collect all scannable files
    let source_files: Vec<_> = discover_files(repo_path, &extensions).collect();

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

    // Scan for alert rules in YAML files
    let all_alerts = scan_yaml_alerts(repo_path);

    log::info!(
        "Indexed {} metrics, {} alerts",
        all_metrics.len(),
        all_alerts.len()
    );

    Ok(CodebaseIndex {
        repo_url: repo_url.to_string(),
        repo_path: repo_path.to_path_buf(),
        metrics: all_metrics,
        alerts: all_alerts,
        last_updated: crate::now_unix_secs(),
    })
}

/// Scan YAML files for Prometheus alert rules.
fn scan_yaml_alerts(repo_path: &Path) -> Vec<AlertRule> {
    let mut alert_scanner = match YamlAlertScanner::new() {
        Ok(scanner) => scanner,
        Err(e) => {
            log::warn!("Failed to initialize YAML alert scanner: {e}");
            return Vec::new();
        }
    };
    let yaml_extensions: FxHashSet<&str> = ["yaml", "yml"].into_iter().collect();

    let mut all_alerts = Vec::new();

    for entry in discover_files(repo_path, &yaml_extensions) {
        let path = entry.path();
        match alert_scanner.scan_file(path) {
            Ok(alerts) => {
                // Convert absolute paths to relative paths from repo root
                for mut alert in alerts {
                    if let Ok(relative) = alert.file.strip_prefix(repo_path) {
                        alert.file = relative.to_path_buf();
                    }
                    all_alerts.push(alert);
                }
            }
            Err(e) => {
                // Log but don't fail on individual file errors
                log::debug!("Failed to scan YAML file {}: {}", path.display(), e);
            }
        }
    }

    // Sort alerts by file path, then line number
    all_alerts.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));

    all_alerts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::MetricKind;

    fn make_test_metric(name: &str, file: &str, line: usize) -> MetricInstrumentation {
        MetricInstrumentation {
            kind: MetricKind::Counter,
            name: name.to_string(),
            labels: vec![],
            file: PathBuf::from(file),
            line,
            column: 0,
            function_name: None,
            impl_type: None,
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
            alerts: vec![],
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
            alerts: vec![],
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
            alerts: vec![],
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
            alerts: vec![],
            last_updated: 0,
        };

        let results = index.find_by_name("http.requests");
        assert_eq!(results.len(), 2);

        let results = index.find_by_name("nonexistent");
        assert_eq!(results.len(), 0);
    }

    fn make_test_alert(name: &str, metric_name: Option<&str>) -> AlertRule {
        AlertRule {
            name: name.to_string(),
            expr: "test_expr".to_string(),
            metric_name: metric_name.map(String::from),
            severity: None,
            message: None,
            runbook_url: None,
            file: PathBuf::from("alerts.yaml"),
            line: 1,
            column: 0,
        }
    }

    #[test]
    fn test_find_alerts_by_metric() {
        let index = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![
                make_test_alert("HighErrorRate", Some("errors_total")),
                make_test_alert("HighLatency", Some("latency_seconds")),
                make_test_alert("AnotherErrorAlert", Some("errors_total")),
            ],
            last_updated: 0,
        };

        let results = index.find_alerts_by_metric("errors_total");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "HighErrorRate");
        assert_eq!(results[1].name, "AnotherErrorAlert");

        let results = index.find_alerts_by_metric("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_alerts() {
        let index = CodebaseIndex {
            repo_url: "test".to_string(),
            repo_path: PathBuf::from("/test"),
            metrics: vec![],
            alerts: vec![
                make_test_alert("HighErrorRate", Some("errors_total")),
                make_test_alert("HighLatency", Some("latency_seconds")),
            ],
            last_updated: 0,
        };

        // Search by alert name
        let results = index.search_alerts("Error");
        assert_eq!(results.len(), 1);

        // Search by metric name
        let results = index.search_alerts("latency");
        assert_eq!(results.len(), 1);

        // Case insensitive
        let results = index.search_alerts("HIGH");
        assert_eq!(results.len(), 2);
    }
}
