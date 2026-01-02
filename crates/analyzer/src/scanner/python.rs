//! Python `prometheus_client` scanner implementation.
//!
//! Scans Python source files to find `Counter`, `Gauge`, `Histogram`, and `Summary`
//! instantiations from the `prometheus_client` library and extracts metric names and labels.
//!
//! # Supported Definition Patterns
//!
//! ```python
//! from prometheus_client import Counter, Gauge, Histogram, Summary
//!
//! # Basic metrics
//! counter = Counter('http_requests_total', 'Total HTTP requests')
//! gauge = Gauge('temperature_celsius', 'Current temperature')
//! histogram = Histogram('request_latency_seconds', 'Request latency')
//!
//! # Metrics with labels
//! counter = Counter('http_requests_total', 'Help', ['method', 'endpoint'])
//! ```
//!
//! # Supported Usage Patterns
//!
//! ```python
//! # Counter usage
//! counter.inc()
//! counter.inc(5)
//! counter.labels(method='GET').inc()
//!
//! # Gauge usage
//! gauge.set(42)
//! gauge.inc()
//! gauge.dec()
//! gauge.set_to_current_time()
//!
//! # Histogram/Summary usage
//! histogram.observe(0.5)
//! histogram.time()  # context manager
//! ```

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use super::{MetricInstrumentation, MetricKind, MetricUsage, Scanner, UsageKind};
use crate::parser::ParseError;

/// Tree-sitter query for finding `prometheus_client` metric instantiations.
///
/// This query matches:
/// - `Counter('name', 'help', ...)` - function calls with Counter/Gauge/Histogram/Summary
const PYTHON_METRICS_QUERY: &str = r"
(call
  function: (identifier) @func_name
  arguments: (argument_list) @args)
";

/// Tree-sitter query for finding metric usage patterns.
///
/// This query matches method calls like:
/// - `counter.inc()` / `counter.inc(5)`
/// - `gauge.set(42)` / `gauge.inc()` / `gauge.dec()`
/// - `histogram.observe(0.5)`
/// - `counter.labels(...).inc()`
const PYTHON_USAGE_QUERY: &str = r"
(call
  function: (attribute
    object: (_) @object
    attribute: (identifier) @method)
  arguments: (argument_list) @args)
";

/// A wrapper around tree-sitter for parsing Python source code.
pub struct PythonParser {
    parser: Parser,
    language: Language,
}

impl PythonParser {
    /// Creates a new Python parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    pub fn new() -> Result<Self, ParseError> {
        let language: Language = tree_sitter_python::LANGUAGE.into();

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| ParseError(format!("Failed to set Python language: {e}")))?;

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
            .ok_or_else(|| ParseError("Failed to parse Python source".to_string()))?;

        Ok((source, tree))
    }

    /// Creates a query for matching patterns in the syntax tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the query is invalid.
    pub fn create_query(&self, query_str: &str) -> Result<Query, ParseError> {
        Query::new(&self.language, query_str)
            .map_err(|e| ParseError(format!("Invalid Python query: {e}")))
    }
}

impl Default for PythonParser {
    fn default() -> Self {
        Self::new().expect("Failed to create Python parser")
    }
}

/// Scanner for Python files using `prometheus_client`.
///
/// Detects the following patterns:
/// - `Counter('name', 'help')` / `Counter('name', 'help', ['label1', 'label2'])`
/// - `Gauge('name', 'help')` / `Gauge('name', 'help', ['label1', 'label2'])`
/// - `Histogram('name', 'help')` / `Histogram('name', 'help', ['label1', 'label2'])`
/// - `Summary('name', 'help')` / `Summary('name', 'help', ['label1', 'label2'])`
pub struct PythonPrometheusScanner;

impl PythonPrometheusScanner {
    /// Creates a new Python `prometheus_client` scanner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for PythonPrometheusScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for PythonPrometheusScanner {
    fn extensions(&self) -> &[&str] {
        &["py"]
    }

    fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError> {
        let mut parser = PythonParser::new()?;
        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(PYTHON_METRICS_QUERY)?;

        scan_tree(&source, &tree, &query, path)
    }

    fn scan_usages(&self, path: &Path) -> Result<Vec<MetricUsage>, ParseError> {
        let mut parser = PythonParser::new()?;
        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(PYTHON_USAGE_QUERY)?;

        scan_usages_tree(&source, &tree, &query, path)
    }
}

/// Parses a metric kind from a `prometheus_client` class name.
fn metric_kind_from_class_name(name: &str) -> Option<MetricKind> {
    match name {
        "Counter" => Some(MetricKind::Counter),
        "Gauge" => Some(MetricKind::Gauge),
        // Summary is treated as a histogram for our purposes
        "Histogram" | "Summary" => Some(MetricKind::Histogram),
        _ => None,
    }
}

/// Parses a usage kind from a `prometheus_client` method name.
fn usage_kind_from_method_name(name: &str) -> Option<UsageKind> {
    match name {
        "inc" => Some(UsageKind::Increment),
        "dec" => Some(UsageKind::Sub),
        "set" => Some(UsageKind::Set),
        "observe" => Some(UsageKind::Observe),
        "time" => Some(UsageKind::Time),
        "set_to_current_time" => Some(UsageKind::SetToCurrentTime),
        "track_inprogress" => Some(UsageKind::TrackInProgress),
        _ => None,
    }
}

/// Scans a parsed syntax tree for `prometheus_client` metrics.
fn scan_tree(
    source: &str,
    tree: &Tree,
    query: &Query,
    file_path: &Path,
) -> Result<Vec<MetricInstrumentation>, ParseError> {
    let mut cursor = QueryCursor::new();
    let mut results = Vec::new();

    let func_name_idx = query
        .capture_index_for_name("func_name")
        .ok_or_else(|| ParseError("Query missing func_name capture".to_string()))?;
    let args_idx = query
        .capture_index_for_name("args")
        .ok_or_else(|| ParseError("Query missing args capture".to_string()))?;

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(match_) = matches.next() {
        let mut func_name_node = None;
        let mut args_node = None;

        for capture in match_.captures {
            if capture.index == func_name_idx {
                func_name_node = Some(capture.node);
            } else if capture.index == args_idx {
                args_node = Some(capture.node);
            }
        }

        let (Some(func_node), Some(args)) = (func_name_node, args_node) else {
            continue;
        };

        let func_name = func_node.utf8_text(source.as_bytes()).unwrap_or_default();

        let Some(kind) = metric_kind_from_class_name(func_name) else {
            continue;
        };

        // Extract metric name and labels from the argument list
        let (name, labels) = extract_metric_info(&args, source);

        if !name.is_empty() {
            let start = func_node.start_position();

            // Find the containing function and class
            let (function_name, impl_type) = find_function_context(func_node, source);

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

/// Scans a parsed syntax tree for `prometheus_client` metric usages.
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

        // Extract the variable name (handles chained calls like counter.labels(...).inc())
        let variable_name = extract_root_variable(&obj_node, source);

        if !variable_name.is_empty() {
            let start = obj_node.start_position();

            // Find the containing function and class
            let (function_name, impl_type) = find_function_context(obj_node, source);

            // Try to extract label values from .labels() calls
            let label_values = extract_label_values_from_chain(&obj_node, source);

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

/// Extracts the root variable name from a potentially chained expression.
///
/// For `counter.labels(...).inc()`, this returns `"counter"`.
/// For `self.counter.inc()`, this returns `"self.counter"`.
fn extract_root_variable(node: &Node<'_>, source: &str) -> String {
    match node.kind() {
        "identifier" => node
            .utf8_text(source.as_bytes())
            .unwrap_or_default()
            .to_string(),
        "attribute" => {
            // For self.counter, return the full expression
            node.utf8_text(source.as_bytes())
                .unwrap_or_default()
                .to_string()
        }
        "call" => {
            // This is a chained call like counter.labels(...).inc()
            // Get the function being called
            if let Some(func) = node.child_by_field_name("function") {
                if func.kind() == "attribute" {
                    // Get the object of the attribute (counter.labels -> counter)
                    if let Some(obj) = func.child_by_field_name("object") {
                        return extract_root_variable(&obj, source);
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// Extracts label values from a `.labels(...)` call in a method chain.
///
/// For `counter.labels(method='GET', status='200').inc()`, this returns
/// `["GET", "200"]` (the values, not the keys).
fn extract_label_values_from_chain(node: &Node<'_>, source: &str) -> Vec<String> {
    // Check if this is a call node (e.g., counter.labels(...))
    if node.kind() == "call" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.kind() == "attribute" {
                if let Some(attr) = func.child_by_field_name("attribute") {
                    let attr_name = attr.utf8_text(source.as_bytes()).unwrap_or_default();
                    if attr_name == "labels" {
                        // Extract values from the arguments
                        if let Some(args) = node.child_by_field_name("arguments") {
                            return extract_keyword_values(&args, source);
                        }
                    }
                }
            }
        }
    }
    Vec::new()
}

/// Extracts values from keyword arguments in a `.labels(key='value')` call.
fn extract_keyword_values(args_node: &Node<'_>, source: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = args_node.walk();

    for child in args_node.children(&mut cursor) {
        if child.kind() == "keyword_argument" {
            // Get the value part of key=value
            if let Some(value) = child.child_by_field_name("value") {
                if value.kind() == "string" {
                    let content = extract_string_content(&value, source);
                    if !content.is_empty() {
                        values.push(content);
                    }
                }
            }
        }
    }

    values
}

/// Finds the containing function and class for a node by walking up the AST.
///
/// Returns `(function_name, class_name)` where either may be `None` if the metric
/// is not inside a function or class.
fn find_function_context(node: Node<'_>, source: &str) -> (Option<String>, Option<String>) {
    let mut current = node;
    let mut function_name = None;
    let mut class_name = None;

    while let Some(parent) = current.parent() {
        match parent.kind() {
            "function_definition" => {
                if function_name.is_none() {
                    function_name = parent
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(String::from);
                }
            }
            "class_definition" => {
                if class_name.is_none() {
                    class_name = parent
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(String::from);
                }
            }
            _ => {}
        }
        current = parent;
    }

    (function_name, class_name)
}

/// Extracts the metric name and label keys from an argument list.
///
/// Parses patterns like:
/// - `('metric_name', 'help')` -> `name="metric_name"`, `labels=[]`
/// - `('metric_name', 'help', ['key1', 'key2'])` -> `name="metric_name"`, `labels=["key1", "key2"]`
fn extract_metric_info(args_node: &Node<'_>, source: &str) -> (String, Vec<String>) {
    let mut name = String::new();
    let mut labels = Vec::new();

    // Find the first string argument (metric name)
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        match child.kind() {
            "string" => {
                if name.is_empty() {
                    name = extract_string_content(&child, source);
                }
            }
            "list" => {
                // This is the labels list
                labels = extract_list_strings(&child, source);
            }
            _ => {}
        }
    }

    (name, labels)
}

/// Extracts the content from a string node, removing quotes.
fn extract_string_content(node: &Node<'_>, source: &str) -> String {
    let text = node.utf8_text(source.as_bytes()).unwrap_or_default();
    // Remove quotes (single, double, or triple)
    let text = text.trim();
    if text.starts_with("\"\"\"") || text.starts_with("'''") {
        text[3..text.len().saturating_sub(3)].to_string()
    } else if text.starts_with('"') || text.starts_with('\'') {
        text[1..text.len().saturating_sub(1)].to_string()
    } else {
        text.to_string()
    }
}

/// Extracts string values from a list node.
fn extract_list_strings(node: &Node<'_>, source: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "string" {
            let content = extract_string_content(&child, source);
            if !content.is_empty() {
                strings.push(content);
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
        let mut parser = PythonParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(PYTHON_METRICS_QUERY)
            .expect("Failed to create query");
        scan_tree(source, &tree, &query, Path::new("test.py")).expect("Failed to scan")
    }

    #[test]
    fn test_simple_counter() {
        let source = r#"
from prometheus_client import Counter
counter = Counter('http_requests_total', 'Total HTTP requests')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[0].name, "http_requests_total");
        assert!(metrics[0].labels.is_empty());
    }

    #[test]
    fn test_counter_with_labels() {
        let source = r#"
from prometheus_client import Counter
counter = Counter('http_requests_total', 'Total HTTP requests', ['method', 'endpoint'])
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
from prometheus_client import Counter, Gauge, Histogram, Summary

counter = Counter('requests_total', 'Total requests')
gauge = Gauge('temperature', 'Current temperature')
histogram = Histogram('latency_seconds', 'Request latency')
summary = Summary('response_size', 'Response size')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 4);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[1].kind, MetricKind::Gauge);
        assert_eq!(metrics[2].kind, MetricKind::Histogram);
        assert_eq!(metrics[3].kind, MetricKind::Histogram); // Summary treated as Histogram
    }

    #[test]
    fn test_ignores_other_calls() {
        let source = r#"
print("Hello, world!")
result = some_function("argument")
counter = Counter('real_metric', 'Help text')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "real_metric");
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"from prometheus_client import Counter
counter = Counter('line_two', 'Help')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].line, 2);
    }

    #[test]
    fn test_function_context_simple() {
        let source = r#"
from prometheus_client import Counter

def handle_request():
    counter = Counter('http_requests', 'Help')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("handle_request".to_string()));
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_function_context_in_class() {
        let source = r#"
from prometheus_client import Counter

class Handler:
    def process(self):
        counter = Counter('http_requests', 'Help')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("process".to_string()));
        assert_eq!(metrics[0].impl_type, Some("Handler".to_string()));
    }

    #[test]
    fn test_module_level_metric() {
        let source = r#"
from prometheus_client import Counter
counter = Counter('startup_count', 'Help')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, None);
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_double_quoted_strings() {
        let source = r#"
counter = Counter("http_requests", "Help text")
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "http_requests");
    }

    #[test]
    fn test_single_quoted_strings() {
        let source = r#"
counter = Counter('http_requests', 'Help text')
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "http_requests");
    }

    // Usage tracking tests

    fn parse_and_scan_usages(source: &str) -> Vec<MetricUsage> {
        let mut parser = PythonParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(PYTHON_USAGE_QUERY)
            .expect("Failed to create query");
        scan_usages_tree(source, &tree, &query, Path::new("test.py")).expect("Failed to scan")
    }

    #[test]
    fn test_counter_inc() {
        let source = r#"
counter.inc()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_counter_inc_with_value() {
        let source = r#"
counter.inc(5)
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_gauge_set() {
        let source = r#"
gauge.set(42.0)
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Set);
        assert_eq!(usages[0].variable_name, "gauge");
    }

    #[test]
    fn test_gauge_dec() {
        let source = r#"
gauge.dec()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Sub);
        assert_eq!(usages[0].variable_name, "gauge");
    }

    #[test]
    fn test_histogram_observe() {
        let source = r#"
histogram.observe(0.5)
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Observe);
        assert_eq!(usages[0].variable_name, "histogram");
    }

    #[test]
    fn test_self_attribute_usage() {
        let source = r#"
self.counter.inc()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[0].variable_name, "self.counter");
    }

    #[test]
    fn test_usage_in_function() {
        let source = r#"
def handle_request():
    counter.inc()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].function_name, Some("handle_request".to_string()));
    }

    #[test]
    fn test_usage_in_class_method() {
        let source = r#"
class Handler:
    def process(self):
        self.counter.inc()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].function_name, Some("process".to_string()));
        assert_eq!(usages[0].impl_type, Some("Handler".to_string()));
    }

    #[test]
    fn test_multiple_usages() {
        let source = r#"
counter.inc()
gauge.set(10)
histogram.observe(0.5)
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
obj.other_method()
something.do_stuff()
counter.inc()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_set_to_current_time() {
        let source = r#"
gauge.set_to_current_time()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::SetToCurrentTime);
    }

    #[test]
    fn test_time_context_manager() {
        let source = r#"
histogram.time()
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Time);
    }
}
