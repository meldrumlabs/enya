//! Go `prometheus/client_golang` scanner implementation.
//!
//! Scans Go source files to find Prometheus metric definitions from
//! `github.com/prometheus/client_golang/prometheus` and
//! `github.com/prometheus/client_golang/prometheus/promauto`.
//!
//! # Supported Patterns
//!
//! ```go
//! import "github.com/prometheus/client_golang/prometheus"
//! import "github.com/prometheus/client_golang/prometheus/promauto"
//!
//! // Basic metrics
//! counter := prometheus.NewCounter(prometheus.CounterOpts{Name: "requests_total"})
//! gauge := prometheus.NewGauge(prometheus.GaugeOpts{Name: "temperature"})
//! histogram := prometheus.NewHistogram(prometheus.HistogramOpts{Name: "latency"})
//!
//! // Vector metrics (with labels)
//! counterVec := prometheus.NewCounterVec(prometheus.CounterOpts{Name: "requests_total"}, []string{"method"})
//!
//! // Promauto (auto-registration)
//! counter := promauto.NewCounter(prometheus.CounterOpts{Name: "requests_total"})
//! ```

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use super::{MetricInstrumentation, MetricKind, Scanner};
use crate::parser::ParseError;

/// Tree-sitter query for finding prometheus metric constructor calls.
///
/// This query matches call expressions like:
/// - `prometheus.NewCounter(...)` / `prometheus.NewCounterVec(...)`
/// - `promauto.NewCounter(...)` / `promauto.NewCounterVec(...)`
const GO_METRICS_QUERY: &str = r"
(call_expression
  function: (selector_expression
    operand: (identifier) @package
    field: (field_identifier) @method)
  arguments: (argument_list) @args)
";

/// A wrapper around tree-sitter for parsing Go source code.
pub struct GoParser {
    parser: Parser,
    language: Language,
}

impl GoParser {
    /// Creates a new Go parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    pub fn new() -> Result<Self, ParseError> {
        let language: Language = tree_sitter_go::LANGUAGE.into();

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| ParseError(format!("Failed to set Go language: {e}")))?;

        Ok(Self { parser, language })
    }

    /// Parses the given source code into a syntax tree.
    #[must_use]
    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }

    /// Parses a file from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn parse_file(&mut self, path: &Path) -> Result<(String, Tree), ParseError> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| ParseError(format!("Failed to read file {}: {e}", path.display())))?;

        let tree = self
            .parse(&source)
            .ok_or_else(|| ParseError("Failed to parse Go source".to_string()))?;

        Ok((source, tree))
    }

    /// Creates a query for matching patterns in the syntax tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the query is invalid.
    pub fn create_query(&self, query_str: &str) -> Result<Query, ParseError> {
        Query::new(&self.language, query_str)
            .map_err(|e| ParseError(format!("Invalid Go query: {e}")))
    }
}

impl Default for GoParser {
    fn default() -> Self {
        Self::new().expect("Failed to create Go parser")
    }
}

/// Scanner for Go files using `prometheus/client_golang`.
///
/// Detects the following patterns:
/// - `prometheus.NewCounter(CounterOpts{...})`
/// - `prometheus.NewCounterVec(CounterOpts{...}, []string{...})`
/// - `prometheus.NewGauge(GaugeOpts{...})`
/// - `prometheus.NewGaugeVec(GaugeOpts{...}, []string{...})`
/// - `prometheus.NewHistogram(HistogramOpts{...})`
/// - `prometheus.NewHistogramVec(HistogramOpts{...}, []string{...})`
/// - `promauto.NewCounter(...)` and similar
pub struct GoPrometheusScanner;

impl GoPrometheusScanner {
    /// Creates a new Go prometheus scanner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoPrometheusScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for GoPrometheusScanner {
    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError> {
        let mut parser = GoParser::new()?;
        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(GO_METRICS_QUERY)?;

        scan_tree(&source, &tree, &query, path)
    }
}

/// Parses a metric kind from a Go Prometheus method name.
fn metric_kind_from_method_name(name: &str) -> Option<MetricKind> {
    match name {
        "NewCounter" | "NewCounterVec" | "NewCounterFunc" => Some(MetricKind::Counter),
        "NewGauge" | "NewGaugeVec" | "NewGaugeFunc" => Some(MetricKind::Gauge),
        // Summary is treated as a histogram for our purposes
        "NewHistogram" | "NewHistogramVec" | "NewSummary" | "NewSummaryVec" => {
            Some(MetricKind::Histogram)
        }
        _ => None,
    }
}

/// Checks if the package name is a Prometheus-related package.
fn is_prometheus_package(name: &str) -> bool {
    matches!(name, "prometheus" | "promauto")
}

/// Scans a parsed syntax tree for Prometheus metrics.
fn scan_tree(
    source: &str,
    tree: &Tree,
    query: &Query,
    file_path: &Path,
) -> Result<Vec<MetricInstrumentation>, ParseError> {
    let mut cursor = QueryCursor::new();
    let mut results = Vec::new();

    let package_idx = query
        .capture_index_for_name("package")
        .ok_or_else(|| ParseError("Query missing package capture".to_string()))?;
    let method_idx = query
        .capture_index_for_name("method")
        .ok_or_else(|| ParseError("Query missing method capture".to_string()))?;
    let args_idx = query
        .capture_index_for_name("args")
        .ok_or_else(|| ParseError("Query missing args capture".to_string()))?;

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(match_) = matches.next() {
        let mut package_node = None;
        let mut method_node = None;
        let mut args_node = None;

        for capture in match_.captures {
            if capture.index == package_idx {
                package_node = Some(capture.node);
            } else if capture.index == method_idx {
                method_node = Some(capture.node);
            } else if capture.index == args_idx {
                args_node = Some(capture.node);
            }
        }

        let (Some(pkg_node), Some(method_n), Some(args)) = (package_node, method_node, args_node)
        else {
            continue;
        };

        let package_name = pkg_node.utf8_text(source.as_bytes()).unwrap_or_default();
        let method_name = method_n.utf8_text(source.as_bytes()).unwrap_or_default();

        // Only process prometheus/promauto package calls
        if !is_prometheus_package(package_name) {
            continue;
        }

        let Some(kind) = metric_kind_from_method_name(method_name) else {
            continue;
        };

        // Extract metric name and labels from the argument list
        let (name, labels) = extract_metric_info(&args, source);

        if !name.is_empty() {
            let start = pkg_node.start_position();

            // Find the containing function and type
            let (function_name, impl_type) = find_function_context(pkg_node, source);

            results.push(MetricInstrumentation {
                kind,
                name,
                labels,
                file: file_path.to_path_buf(),
                line: start.row + 1, // Convert to 1-indexed
                column: start.column,
                function_name,
                impl_type,
            });
        }
    }

    Ok(results)
}

/// Finds the containing function and type for a node by walking up the AST.
///
/// Returns `(function_name, receiver_type)` where either may be `None`.
fn find_function_context(node: Node<'_>, source: &str) -> (Option<String>, Option<String>) {
    let mut current = node;
    let mut function_name = None;
    let mut receiver_type = None;

    while let Some(parent) = current.parent() {
        if parent.kind() == "function_declaration" || parent.kind() == "method_declaration" {
            if function_name.is_none() {
                function_name = parent
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(String::from);
            }

            // For method declarations, look for the receiver type
            if receiver_type.is_none() && parent.kind() == "method_declaration" {
                if let Some(receiver) = parent.child_by_field_name("receiver") {
                    receiver_type = extract_receiver_type(&receiver, source);
                }
            }
        }
        current = parent;
    }

    (function_name, receiver_type)
}

/// Extracts the receiver type from a method receiver node.
fn extract_receiver_type(receiver: &Node<'_>, source: &str) -> Option<String> {
    let mut cursor = receiver.walk();
    for child in receiver.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            // Look for the type in the parameter declaration
            if let Some(type_node) = child.child_by_field_name("type") {
                let type_text = type_node.utf8_text(source.as_bytes()).ok()?;
                // Remove pointer prefix if present
                let type_name = type_text.trim_start_matches('*');
                return Some(type_name.to_string());
            }
        }
    }
    None
}

/// Extracts the metric name and label keys from an argument list.
///
/// Parses patterns like:
/// - `(prometheus.CounterOpts{Name: "metric_name"})` -> `name="metric_name"`, `labels=[]`
/// - `(prometheus.CounterOpts{Name: "metric_name"}, []string{"label1", "label2"})` -> labels
fn extract_metric_info(args_node: &Node<'_>, source: &str) -> (String, Vec<String>) {
    let mut name = String::new();
    let mut labels = Vec::new();

    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        if child.kind() == "composite_literal" {
            let child_text = child.utf8_text(source.as_bytes()).unwrap_or_default();

            // Check if this is a []string{...} for labels
            if child_text.contains("[]string") {
                if labels.is_empty() {
                    labels = extract_string_slice(&child, source);
                }
            } else if name.is_empty() {
                // This is the Opts struct
                name = extract_name_from_opts(&child, source);
            }
        }
    }

    (name, labels)
}

/// Extracts the Name field from an Opts struct.
fn extract_name_from_opts(node: &Node<'_>, source: &str) -> String {
    // Find the literal_value (struct body)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "literal_value" {
            return extract_name_field(&child, source);
        }
    }
    String::new()
}

/// Extracts the Name field value from a struct literal body.
fn extract_name_field(node: &Node<'_>, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "keyed_element" {
            // Get key and value from keyed_element
            let key_node = child.child_by_field_name("key");
            let value_node = child.child_by_field_name("value");

            if let (Some(key), Some(value)) = (key_node, value_node) {
                // The key might be wrapped in literal_element
                let key_text = extract_identifier_text(&key, source);
                if key_text == "Name" {
                    return extract_string_from_value(&value, source);
                }
            }
        }
    }
    String::new()
}

/// Extracts identifier text from a node (handles `literal_element` wrapper).
fn extract_identifier_text(node: &Node<'_>, source: &str) -> String {
    if node.kind() == "identifier" || node.kind() == "field_identifier" {
        return node
            .utf8_text(source.as_bytes())
            .unwrap_or_default()
            .to_string();
    }

    // Check children for identifier
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "field_identifier" {
            return child
                .utf8_text(source.as_bytes())
                .unwrap_or_default()
                .to_string();
        }
    }
    String::new()
}

/// Extracts string value from a value node (handles `literal_element` wrapper).
fn extract_string_from_value(node: &Node<'_>, source: &str) -> String {
    if node.kind() == "interpreted_string_literal" || node.kind() == "raw_string_literal" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or_default();
        return text.trim_matches('"').trim_matches('`').to_string();
    }

    // Check children for string literal (handles literal_element wrapper)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "interpreted_string_literal" || child.kind() == "raw_string_literal" {
            let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
            return text.trim_matches('"').trim_matches('`').to_string();
        }
        // Recurse into literal_element
        if child.kind() == "literal_element" {
            let result = extract_string_from_value(&child, source);
            if !result.is_empty() {
                return result;
            }
        }
    }
    String::new()
}

/// Extracts strings from a []string{...} composite literal.
fn extract_string_slice(node: &Node<'_>, source: &str) -> Vec<String> {
    let mut strings = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "literal_value" {
            let mut val_cursor = child.walk();
            for val_child in child.children(&mut val_cursor) {
                // Handle direct string literals
                if val_child.kind() == "interpreted_string_literal"
                    || val_child.kind() == "raw_string_literal"
                {
                    let text = val_child.utf8_text(source.as_bytes()).unwrap_or_default();
                    let content = text.trim_matches('"').trim_matches('`').to_string();
                    if !content.is_empty() {
                        strings.push(content);
                    }
                }
                // Handle literal_element wrapper
                else if val_child.kind() == "literal_element" {
                    let content = extract_string_from_value(&val_child, source);
                    if !content.is_empty() {
                        strings.push(content);
                    }
                }
            }
        }
    }

    strings
}

#[cfg(test)]
#[allow(clippy::needless_raw_string_hashes)]
mod tests {
    use super::*;

    fn parse_and_scan(source: &str) -> Vec<MetricInstrumentation> {
        let mut parser = GoParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(GO_METRICS_QUERY)
            .expect("Failed to create query");
        scan_tree(source, &tree, &query, Path::new("test.go")).expect("Failed to scan")
    }

    #[test]
    fn test_simple_counter() {
        let source = r#"
package main

import "github.com/prometheus/client_golang/prometheus"

func init() {
    counter := prometheus.NewCounter(prometheus.CounterOpts{
        Name: "http_requests_total",
        Help: "Total HTTP requests",
    })
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[0].name, "http_requests_total");
        assert!(metrics[0].labels.is_empty());
    }

    #[test]
    fn test_counter_vec_with_labels() {
        let source = r#"
package main

import "github.com/prometheus/client_golang/prometheus"

var counter = prometheus.NewCounterVec(
    prometheus.CounterOpts{
        Name: "http_requests_total",
        Help: "Total HTTP requests",
    },
    []string{"method", "endpoint"},
)
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[0].name, "http_requests_total");
        assert_eq!(metrics[0].labels, vec!["method", "endpoint"]);
    }

    #[test]
    fn test_all_metric_types() {
        let source = r#"
package main

import "github.com/prometheus/client_golang/prometheus"

var (
    counter = prometheus.NewCounter(prometheus.CounterOpts{Name: "requests_total"})
    gauge = prometheus.NewGauge(prometheus.GaugeOpts{Name: "temperature"})
    histogram = prometheus.NewHistogram(prometheus.HistogramOpts{Name: "latency"})
    summary = prometheus.NewSummary(prometheus.SummaryOpts{Name: "response_size"})
)
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 4);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[1].kind, MetricKind::Gauge);
        assert_eq!(metrics[2].kind, MetricKind::Histogram);
        assert_eq!(metrics[3].kind, MetricKind::Histogram); // Summary treated as Histogram
    }

    #[test]
    fn test_promauto_package() {
        let source = r#"
package main

import "github.com/prometheus/client_golang/prometheus/promauto"

var counter = promauto.NewCounter(prometheus.CounterOpts{
    Name: "auto_requests_total",
})
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "auto_requests_total");
    }

    #[test]
    fn test_ignores_other_calls() {
        let source = r#"
package main

import "fmt"
import "github.com/prometheus/client_golang/prometheus"

func main() {
    fmt.Println("Hello")
    counter := prometheus.NewCounter(prometheus.CounterOpts{Name: "real_metric"})
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "real_metric");
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"package main
var c = prometheus.NewCounter(prometheus.CounterOpts{Name: "line_two"})
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].line, 2);
    }

    #[test]
    fn test_function_context_simple() {
        let source = r#"
package main

import "github.com/prometheus/client_golang/prometheus"

func handleRequest() {
    counter := prometheus.NewCounter(prometheus.CounterOpts{Name: "http_requests"})
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("handleRequest".to_string()));
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_method_context() {
        let source = r#"
package main

import "github.com/prometheus/client_golang/prometheus"

type Handler struct{}

func (h *Handler) Process() {
    counter := prometheus.NewCounter(prometheus.CounterOpts{Name: "http_requests"})
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("Process".to_string()));
        assert_eq!(metrics[0].impl_type, Some("Handler".to_string()));
    }

    #[test]
    fn test_package_level_metric() {
        let source = r#"
package main

import "github.com/prometheus/client_golang/prometheus"

var counter = prometheus.NewCounter(prometheus.CounterOpts{Name: "startup_count"})
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, None);
        assert_eq!(metrics[0].impl_type, None);
    }
}
