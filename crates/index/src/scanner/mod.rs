//! Language-agnostic scanner framework.
//!
//! Provides a trait-based architecture for scanning source files to discover
//! metric instrumentation points and alert rule definitions. Each language/library
//! combination can implement the [`Scanner`] trait to support different ecosystems.
//!
//! # Architecture
//!
//! - [`Scanner`] - Trait for language-specific scanners
//! - [`ScannerRegistry`] - Collection of registered scanners
//! - [`MetricInstrumentation`] - Language-agnostic metric definition
//! - [`MetricKind`] - Counter, Gauge, or Histogram
//! - [`AlertRule`] - Prometheus alert rule definition

mod go;
mod javascript;
mod python;
mod rust;
mod typescript;
mod yaml;

pub use go::GoPrometheusScanner;
pub use javascript::JavaScriptPromClientScanner;
pub use python::PythonPrometheusScanner;
pub use rust::RustMetricsScanner;
pub use typescript::TypeScriptPromClientScanner;
pub use yaml::YamlAlertScanner;

use std::path::{Path, PathBuf};

use crate::parser::ParseError;

/// The kind of metric instrumentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
}

impl MetricKind {
    /// Returns the display name for this kind.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gauge => "gauge",
            Self::Histogram => "histogram",
        }
    }
}

impl std::fmt::Display for MetricKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A discovered metric instrumentation point in source code.
///
/// This is a language-agnostic representation of where a metric is defined
/// or recorded in a codebase. Different [`Scanner`] implementations produce
/// these from language-specific patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricInstrumentation {
    /// The kind of metric (counter, gauge, histogram).
    pub kind: MetricKind,
    /// The metric name (e.g., `http_requests_total`).
    pub name: String,
    /// Label keys used with this metric (e.g., `["method", "status"]`).
    pub labels: Vec<String>,
    /// The file path where this metric is defined.
    pub file: PathBuf,
    /// Line number (1-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub column: usize,
    /// The function containing this metric (e.g., `handle_request`).
    pub function_name: Option<String>,
    /// The impl type if inside an impl block (e.g., `Handler`).
    pub impl_type: Option<String>,
}

/// A discovered Prometheus alert rule.
///
/// Represents an alert rule found in YAML files that references a metric
/// via its `PromQL` expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertRule {
    /// The alert name (e.g., `HighErrorRate`).
    pub name: String,
    /// The `PromQL` expression for this alert.
    pub expr: String,
    /// The primary metric name extracted from the expression.
    pub metric_name: Option<String>,
    /// Alert severity (if specified in labels).
    pub severity: Option<String>,
    /// Alert message (from annotations).
    pub message: Option<String>,
    /// Runbook URL (from annotations).
    pub runbook_url: Option<String>,
    /// The file path where this alert is defined.
    pub file: PathBuf,
    /// Line number (1-indexed) where the alert starts.
    pub line: usize,
    /// Column number (0-indexed).
    pub column: usize,
}

/// Trait for language-specific metric scanners.
///
/// Implement this trait to add support for a new language or metrics library.
/// The scanner is responsible for:
/// 1. Declaring which file extensions it handles
/// 2. Parsing source files and finding metric instrumentation points
///
/// # Example
///
/// ```ignore
/// pub struct GoPrometheusScanner;
///
/// impl Scanner for GoPrometheusScanner {
///     fn extensions(&self) -> &[&str] {
///         &["go"]
///     }
///
///     fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError> {
///         // Use tree-sitter-go to find prometheus.NewCounter(), etc.
///     }
/// }
/// ```
pub trait Scanner: Send + Sync {
    /// File extensions this scanner handles (e.g., `["rs"]` for Rust).
    fn extensions(&self) -> &[&str];

    /// Scan a source file for metric instrumentation points.
    ///
    /// Returns all metrics found in the file, or an error if parsing fails.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the file cannot be read or parsed.
    fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError>;
}

/// Registry of available scanners.
///
/// Maintains a collection of [`Scanner`] implementations and routes files
/// to the appropriate scanner based on extension.
pub struct ScannerRegistry {
    scanners: Vec<Box<dyn Scanner>>,
}

impl ScannerRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scanners: Vec::new(),
        }
    }

    /// Registers a scanner with this registry.
    pub fn register(&mut self, scanner: Box<dyn Scanner>) {
        self.scanners.push(scanner);
    }

    /// Finds a scanner that can handle the given file path.
    ///
    /// Returns `None` if no registered scanner handles this file type.
    #[must_use]
    pub fn scanner_for(&self, path: &Path) -> Option<&dyn Scanner> {
        let ext = path.extension()?.to_str()?;
        self.scanners
            .iter()
            .find(|s| s.extensions().contains(&ext))
            .map(AsRef::as_ref)
    }

    /// Returns all file extensions supported by registered scanners.
    #[must_use]
    pub fn all_extensions(&self) -> Vec<&str> {
        self.scanners
            .iter()
            .flat_map(|s| s.extensions().iter().copied())
            .collect()
    }
}

impl Default for ScannerRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(RustMetricsScanner::new()));
        registry.register(Box::new(PythonPrometheusScanner::new()));
        registry.register(Box::new(GoPrometheusScanner::new()));
        registry.register(Box::new(JavaScriptPromClientScanner::new()));
        registry.register(Box::new(TypeScriptPromClientScanner::new()));
        registry
    }
}
