//! Go `prometheus/client_golang` scanner implementation.
//!
//! Scans Go source files to find Prometheus metric definitions from
//! `github.com/prometheus/client_golang/prometheus` and
//! `github.com/prometheus/client_golang/prometheus/promauto`.
//!
//! # Supported Definition Patterns
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
//!
//! # Supported Usage Patterns
//!
//! ```go
//! // Counter usage
//! counter.Inc()
//! counter.Add(5)
//!
//! // Gauge usage
//! gauge.Set(42)
//! gauge.Inc()
//! gauge.Dec()
//! gauge.Add(10)
//! gauge.Sub(5)
//! gauge.SetToCurrentTime()
//!
//! // Histogram/Summary usage
//! histogram.Observe(0.5)
//!
//! // Vector usage
//! counterVec.WithLabelValues("GET").Inc()
//! ```

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use super::{MetricInstrumentation, MetricKind, MetricUsage, Scanner, UsageKind};
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

/// Tree-sitter query for finding metric usage patterns.
///
/// This query matches method calls like:
/// - `counter.Inc()` / `counter.Add(5)`
/// - `gauge.Set(42)` / `gauge.Inc()` / `gauge.Dec()`
/// - `histogram.Observe(0.5)`
/// - `counterVec.WithLabelValues("GET").Inc()`
const GO_USAGE_QUERY: &str = r"
(call_expression
  function: (selector_expression
    operand: (_) @object
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

    fn scan_usages(&self, path: &Path) -> Result<Vec<MetricUsage>, ParseError> {
        let mut parser = GoParser::new()?;
        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(GO_USAGE_QUERY)?;

        scan_usages_tree(&source, &tree, &query, path)
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

/// Parses a usage kind from a Go Prometheus method name.
fn usage_kind_from_method_name(name: &str) -> Option<UsageKind> {
    match name {
        "Inc" => Some(UsageKind::Increment),
        "Dec" | "Sub" => Some(UsageKind::Sub),
        "Add" => Some(UsageKind::Add),
        "Set" => Some(UsageKind::Set),
        "Observe" => Some(UsageKind::Observe),
        "SetToCurrentTime" => Some(UsageKind::SetToCurrentTime),
        _ => None,
    }
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

/// Scans a parsed syntax tree for Prometheus metric usages.
fn scan_usages_tree(
    source: &str,
    tree: &Tree,
    query: &Query,
    file_path: &Path,
) -> Result<Vec<MetricUsage>, ParseError> {
    let mut cursor = QueryCursor::new();
    let mut results = Vec::new();

    let object_idx = query
        .capture_index_for_name("object")
        .ok_or_else(|| ParseError("Query missing object capture".to_string()))?;
    let method_idx = query
        .capture_index_for_name("method")
        .ok_or_else(|| ParseError("Query missing method capture".to_string()))?;

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(match_) = matches.next() {
        let mut object_node = None;
        let mut method_node = None;

        for capture in match_.captures {
            if capture.index == object_idx {
                object_node = Some(capture.node);
            } else if capture.index == method_idx {
                method_node = Some(capture.node);
            }
        }

        let (Some(obj_node), Some(meth_node)) = (object_node, method_node) else {
            continue;
        };

        let method_name = meth_node.utf8_text(source.as_bytes()).unwrap_or_default();

        // Check if this is a metric usage method
        let Some(usage_kind) = usage_kind_from_method_name(method_name) else {
            continue;
        };

        // Extract the variable name (handles chained calls)
        let variable_name = extract_go_root_variable(&obj_node, source);

        if !variable_name.is_empty() {
            let start = obj_node.start_position();

            // Find the containing function and type
            let (function_name, impl_type) = find_function_context(obj_node, source);

            // Try to extract label values from WithLabelValues() calls
            let label_values = extract_go_label_values(&obj_node, source);

            results.push(MetricUsage {
                usage_kind,
                variable_name,
                label_values,
                file: file_path.to_path_buf(),
                line: start.row + 1,
                column: start.column,
                function_name,
                impl_type,
            });
        }
    }

    Ok(results)
}

/// Extracts the root variable name from a potentially chained Go expression.
///
/// For `counterVec.WithLabelValues("GET").Inc()`, this returns `"counterVec"`.
/// For `s.counter.Inc()`, this returns `"s.counter"`.
fn extract_go_root_variable(node: &Node<'_>, source: &str) -> String {
    match node.kind() {
        "identifier" => node
            .utf8_text(source.as_bytes())
            .unwrap_or_default()
            .to_string(),
        "selector_expression" => {
            // For s.counter, return the full expression
            node.utf8_text(source.as_bytes())
                .unwrap_or_default()
                .to_string()
        }
        "call_expression" => {
            // This is a chained call like counterVec.WithLabelValues(...).Inc()
            if let Some(func) = node.child_by_field_name("function") {
                if func.kind() == "selector_expression" {
                    if let Some(obj) = func.child_by_field_name("operand") {
                        return extract_go_root_variable(&obj, source);
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// Extracts label values from a `WithLabelValues(...)` call in a method chain.
///
/// For `counterVec.WithLabelValues("GET", "200").Inc()`, this returns `["GET", "200"]`.
fn extract_go_label_values(node: &Node<'_>, source: &str) -> Vec<String> {
    // Check if this is a call expression with WithLabelValues
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "selector_expression" {
                if let Some(field) = func.child_by_field_name("field") {
                    let field_name = field.utf8_text(source.as_bytes()).unwrap_or_default();
                    if field_name == "WithLabelValues" || field_name == "With" {
                        // Extract string arguments
                        if let Some(args) = node.child_by_field_name("arguments") {
                            return extract_go_string_args(&args, source);
                        }
                    }
                }
            }
        }
    }
    Vec::new()
}

/// Extracts string values from argument list.
fn extract_go_string_args(args_node: &Node<'_>, source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = args_node.walk();

    for child in args_node.children(&mut cursor) {
        if child.kind() == "interpreted_string_literal" || child.kind() == "raw_string_literal" {
            let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
            let content = text.trim_matches('"').trim_matches('`').to_string();
            if !content.is_empty() {
                values.push(content);
            }
        }
    }

    values
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

    // Usage tracking tests

    fn parse_and_scan_usages(source: &str) -> Vec<MetricUsage> {
        let mut parser = GoParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(GO_USAGE_QUERY)
            .expect("Failed to create query");
        scan_usages_tree(source, &tree, &query, Path::new("test.go")).expect("Failed to scan")
    }

    #[test]
    fn test_counter_inc() {
        let source = r#"
package main

func main() {
    counter.Inc()
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_counter_add() {
        let source = r#"
package main

func main() {
    counter.Add(5)
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Add);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_gauge_set() {
        let source = r#"
package main

func main() {
    gauge.Set(42.0)
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Set);
        assert_eq!(usages[0].variable_name, "gauge");
    }

    #[test]
    fn test_gauge_dec() {
        let source = r#"
package main

func main() {
    gauge.Dec()
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Sub);
        assert_eq!(usages[0].variable_name, "gauge");
    }

    #[test]
    fn test_histogram_observe() {
        let source = r#"
package main

func main() {
    histogram.Observe(0.5)
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Observe);
        assert_eq!(usages[0].variable_name, "histogram");
    }

    #[test]
    fn test_usage_in_function() {
        let source = r#"
package main

func handleRequest() {
    counter.Inc()
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].function_name, Some("handleRequest".to_string()));
    }

    #[test]
    fn test_usage_in_method() {
        let source = r#"
package main

type Handler struct{}

func (h *Handler) Process() {
    h.counter.Inc()
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].function_name, Some("Process".to_string()));
        assert_eq!(usages[0].impl_type, Some("Handler".to_string()));
        assert_eq!(usages[0].variable_name, "h.counter");
    }

    #[test]
    fn test_multiple_usages() {
        let source = r#"
package main

func main() {
    counter.Inc()
    gauge.Set(10)
    histogram.Observe(0.5)
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 3);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[1].usage_kind, UsageKind::Set);
        assert_eq!(usages[2].usage_kind, UsageKind::Observe);
    }

    #[test]
    fn test_ignores_non_metric_methods() {
        let source = r#"
package main

func main() {
    obj.OtherMethod()
    something.DoStuff()
    counter.Inc()
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_set_to_current_time() {
        let source = r#"
package main

func main() {
    gauge.SetToCurrentTime()
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::SetToCurrentTime);
    }
}
