//! JavaScript prom-client scanner implementation.
//!
//! Scans JavaScript source files to find Prometheus metric definitions from
//! the `prom-client` npm package.
//!
//! # Supported Patterns
//!
//! ```javascript
//! const client = require('prom-client');
//! const { Counter, Gauge, Histogram, Summary } = require('prom-client');
//!
//! // Basic metrics
//! const counter = new client.Counter({ name: 'http_requests_total', help: 'Total requests' });
//! const gauge = new client.Gauge({ name: 'temperature', help: 'Temperature' });
//! const histogram = new client.Histogram({ name: 'latency', help: 'Latency' });
//!
//! // With labels
//! const counter = new client.Counter({
//!     name: 'http_requests_total',
//!     help: 'Total requests',
//!     labelNames: ['method', 'endpoint']
//! });
//!
//! // Destructured import
//! const counter = new Counter({ name: 'http_requests_total', help: 'Total requests' });
//! ```

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use super::{MetricInstrumentation, MetricKind, Scanner};
use crate::parser::ParseError;

/// Tree-sitter query for finding prom-client metric instantiations.
///
/// This query matches:
/// - `new client.Counter({...})` - member expression form
/// - `new Counter({...})` - destructured import form
const JS_METRICS_QUERY: &str = r"
[
  (new_expression
    constructor: (member_expression
      object: (identifier) @object
      property: (property_identifier) @method)
    arguments: (arguments) @args)

  (new_expression
    constructor: (identifier) @constructor
    arguments: (arguments) @args)
]
";

/// A wrapper around tree-sitter for parsing JavaScript source code.
pub struct JavaScriptParser {
    parser: Parser,
    language: Language,
}

impl JavaScriptParser {
    /// Creates a new JavaScript parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    pub fn new() -> Result<Self, ParseError> {
        let language: Language = tree_sitter_javascript::LANGUAGE.into();

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| ParseError(format!("Failed to set JavaScript language: {e}")))?;

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
            .ok_or_else(|| ParseError("Failed to parse JavaScript source".to_string()))?;

        Ok((source, tree))
    }

    /// Creates a query for matching patterns in the syntax tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the query is invalid.
    pub fn create_query(&self, query_str: &str) -> Result<Query, ParseError> {
        Query::new(&self.language, query_str)
            .map_err(|e| ParseError(format!("Invalid JavaScript query: {e}")))
    }
}

impl Default for JavaScriptParser {
    fn default() -> Self {
        Self::new().expect("Failed to create JavaScript parser")
    }
}

/// Scanner for JavaScript files using prom-client.
///
/// Detects the following patterns:
/// - `new client.Counter({name: '...', help: '...', labelNames: [...]})`
/// - `new client.Gauge({...})`
/// - `new client.Histogram({...})`
/// - `new client.Summary({...})`
/// - `new Counter({...})` (destructured import)
pub struct JavaScriptPromClientScanner;

impl JavaScriptPromClientScanner {
    /// Creates a new JavaScript prom-client scanner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for JavaScriptPromClientScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for JavaScriptPromClientScanner {
    fn extensions(&self) -> &[&str] {
        &["js", "mjs", "cjs"]
    }

    fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError> {
        let mut parser = JavaScriptParser::new()?;
        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(JS_METRICS_QUERY)?;

        scan_tree(&source, &tree, &query, path)
    }
}

/// Parses a metric kind from a `prom-client` class name.
fn metric_kind_from_class_name(name: &str) -> Option<MetricKind> {
    match name {
        "Counter" => Some(MetricKind::Counter),
        "Gauge" => Some(MetricKind::Gauge),
        // Summary is treated as a histogram for our purposes
        "Histogram" | "Summary" => Some(MetricKind::Histogram),
        _ => None,
    }
}

/// Scans a parsed syntax tree for `prom-client` metrics.
fn scan_tree(
    source: &str,
    tree: &Tree,
    query: &Query,
    file_path: &Path,
) -> Result<Vec<MetricInstrumentation>, ParseError> {
    let mut cursor = QueryCursor::new();
    let mut results = Vec::new();

    // Get all capture indices
    let object_idx = query.capture_index_for_name("object");
    let method_idx = query.capture_index_for_name("method");
    let constructor_idx = query.capture_index_for_name("constructor");
    let args_idx = query
        .capture_index_for_name("args")
        .ok_or_else(|| ParseError("Query missing args capture".to_string()))?;

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(match_) = matches.next() {
        let mut object_node = None;
        let mut method_node = None;
        let mut constructor_node = None;
        let mut args_node = None;

        for capture in match_.captures {
            if Some(capture.index) == object_idx {
                object_node = Some(capture.node);
            } else if Some(capture.index) == method_idx {
                method_node = Some(capture.node);
            } else if Some(capture.index) == constructor_idx {
                constructor_node = Some(capture.node);
            } else if capture.index == args_idx {
                args_node = Some(capture.node);
            }
        }

        let Some(args) = args_node else {
            continue;
        };

        // Determine the metric type from either member expression or direct constructor
        let (kind, start_node) = if let (Some(method_n), Some(obj_n)) = (method_node, object_node) {
            // Member expression: client.Counter
            let method_name = method_n.utf8_text(source.as_bytes()).unwrap_or_default();
            if let Some(k) = metric_kind_from_class_name(method_name) {
                (k, obj_n)
            } else {
                continue;
            }
        } else if let Some(ctor_n) = constructor_node {
            // Direct constructor: Counter
            let ctor_name = ctor_n.utf8_text(source.as_bytes()).unwrap_or_default();
            if let Some(k) = metric_kind_from_class_name(ctor_name) {
                (k, ctor_n)
            } else {
                continue;
            }
        } else {
            continue;
        };

        // Extract metric name and labels from the argument list
        let (name, labels) = extract_metric_info(&args, source);

        if !name.is_empty() {
            let start = start_node.start_position();

            // Find the containing function and class
            let (function_name, impl_type) = find_function_context(start_node, source);

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

/// Finds the containing function and class for a node by walking up the AST.
///
/// Returns `(function_name, class_name)` where either may be `None`.
fn find_function_context(node: Node<'_>, source: &str) -> (Option<String>, Option<String>) {
    let mut current = node;
    let mut function_name = None;
    let mut class_name = None;

    while let Some(parent) = current.parent() {
        match parent.kind() {
            "function_declaration" => {
                if function_name.is_none() {
                    function_name = parent
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(String::from);
                }
            }
            "method_definition" => {
                if function_name.is_none() {
                    function_name = parent
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(String::from);
                }
            }
            "arrow_function" | "function_expression" | "function" => {
                // For anonymous functions, try to find the variable name
                if function_name.is_none() {
                    if let Some(grandparent) = parent.parent() {
                        if grandparent.kind() == "variable_declarator" {
                            function_name = grandparent
                                .child_by_field_name("name")
                                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                                .map(String::from);
                        }
                    }
                }
            }
            "class_declaration" | "class" => {
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
/// - `({ name: 'metric_name', help: 'help' })` -> `name="metric_name"`, `labels=[]`
/// - `({ name: 'metric_name', labelNames: ['key1', 'key2'] })` -> labels
fn extract_metric_info(args_node: &Node<'_>, source: &str) -> (String, Vec<String>) {
    let mut name = String::new();
    let mut labels = Vec::new();

    // Find the first object argument
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        if child.kind() == "object" {
            let (n, l) = extract_from_object(&child, source);
            if !n.is_empty() {
                name = n;
                labels = l;
                break;
            }
        }
    }

    (name, labels)
}

/// Extracts name and labelNames from an object literal.
fn extract_from_object(node: &Node<'_>, source: &str) -> (String, Vec<String>) {
    let mut name = String::new();
    let mut labels = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "pair" {
            let key = child.child_by_field_name("key");
            let value = child.child_by_field_name("value");

            if let (Some(k), Some(v)) = (key, value) {
                let key_text = k.utf8_text(source.as_bytes()).unwrap_or_default();

                match key_text {
                    "name" => {
                        name = extract_string_value(&v, source);
                    }
                    "labelNames" => {
                        labels = extract_array_strings(&v, source);
                    }
                    _ => {}
                }
            }
        }
    }

    (name, labels)
}

/// Extracts a string value from a node (handles both string types).
fn extract_string_value(node: &Node<'_>, source: &str) -> String {
    let text = node.utf8_text(source.as_bytes()).unwrap_or_default();
    // Remove quotes
    text.trim_matches('"').trim_matches('\'').to_string()
}

/// Extracts string values from an array node.
fn extract_array_strings(node: &Node<'_>, source: &str) -> Vec<String> {
    let mut strings = Vec::new();

    if node.kind() == "array" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                let content = extract_string_value(&child, source);
                if !content.is_empty() {
                    strings.push(content);
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
        let mut parser = JavaScriptParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(JS_METRICS_QUERY)
            .expect("Failed to create query");
        scan_tree(source, &tree, &query, Path::new("test.js")).expect("Failed to scan")
    }

    #[test]
    fn test_simple_counter() {
        let source = r#"
const client = require('prom-client');
const counter = new client.Counter({ name: 'http_requests_total', help: 'Total HTTP requests' });
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
const client = require('prom-client');
const counter = new client.Counter({
    name: 'http_requests_total',
    help: 'Total HTTP requests',
    labelNames: ['method', 'endpoint']
});
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
const client = require('prom-client');

const counter = new client.Counter({ name: 'requests_total', help: 'Total requests' });
const gauge = new client.Gauge({ name: 'temperature', help: 'Temperature' });
const histogram = new client.Histogram({ name: 'latency', help: 'Latency' });
const summary = new client.Summary({ name: 'response_size', help: 'Response size' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 4);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[1].kind, MetricKind::Gauge);
        assert_eq!(metrics[2].kind, MetricKind::Histogram);
        assert_eq!(metrics[3].kind, MetricKind::Histogram); // Summary treated as Histogram
    }

    #[test]
    fn test_destructured_import() {
        let source = r#"
const { Counter, Gauge } = require('prom-client');

const counter = new Counter({ name: 'requests_total', help: 'Total requests' });
const gauge = new Gauge({ name: 'temperature', help: 'Temperature' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].name, "requests_total");
        assert_eq!(metrics[1].name, "temperature");
    }

    #[test]
    fn test_ignores_other_calls() {
        let source = r#"
const obj = new Object();
const arr = new Array(10);
const counter = new client.Counter({ name: 'real_metric', help: 'Help' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "real_metric");
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"const client = require('prom-client');
const counter = new client.Counter({ name: 'line_two', help: 'Help' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].line, 2);
    }

    #[test]
    fn test_function_context_simple() {
        let source = r#"
const client = require('prom-client');

function handleRequest() {
    const counter = new client.Counter({ name: 'http_requests', help: 'Help' });
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("handleRequest".to_string()));
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_class_method_context() {
        let source = r#"
const client = require('prom-client');

class Handler {
    process() {
        const counter = new client.Counter({ name: 'http_requests', help: 'Help' });
    }
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("process".to_string()));
        assert_eq!(metrics[0].impl_type, Some("Handler".to_string()));
    }

    #[test]
    fn test_module_level_metric() {
        let source = r#"
const client = require('prom-client');
const counter = new client.Counter({ name: 'startup_count', help: 'Help' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, None);
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_arrow_function_context() {
        let source = r#"
const client = require('prom-client');

const handler = () => {
    const counter = new client.Counter({ name: 'http_requests', help: 'Help' });
};
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("handler".to_string()));
    }

    #[test]
    fn test_single_quoted_strings() {
        let source = r#"
const client = require('prom-client');
const counter = new client.Counter({ name: 'http_requests', help: 'Help' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "http_requests");
    }

    #[test]
    fn test_double_quoted_strings() {
        let source = r#"
const client = require('prom-client');
const counter = new client.Counter({ name: "http_requests", help: "Help" });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "http_requests");
    }
}
