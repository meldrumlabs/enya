//! TypeScript `prom-client` scanner implementation.
//!
//! Scans TypeScript source files to find Prometheus metric definitions from
//! the `prom-client` npm package. This scanner handles both `.ts` and `.tsx` files.
//!
//! # Supported Patterns
//!
//! ```typescript
//! import client from 'prom-client';
//! import { Counter, Gauge, Histogram, Summary } from 'prom-client';
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
//!
//! // With type annotations
//! const counter: Counter<string> = new Counter({ name: 'typed_metric', help: 'Help' });
//! ```

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Tree};

use super::{MetricInstrumentation, MetricKind, MetricUsage, Scanner, UsageKind};
use crate::parser::ParseError;

/// Tree-sitter query for finding `prom-client` metric instantiations in TypeScript.
///
/// This query matches:
/// - `new client.Counter({...})` - member expression form
/// - `new Counter({...})` - destructured import form
const TS_METRICS_QUERY: &str = r"
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

/// Tree-sitter query for finding metric usage (method calls like `counter.inc()`).
const TS_USAGE_QUERY: &str = r"
(call_expression
  function: (member_expression
    object: (_) @object
    property: (property_identifier) @method)
  arguments: (arguments) @args)
";

/// A wrapper around tree-sitter for parsing TypeScript source code.
pub struct TypeScriptParser {
    parser: Parser,
    language: Language,
}

impl TypeScriptParser {
    /// Creates a new TypeScript parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    pub fn new() -> Result<Self, ParseError> {
        let language: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| ParseError(format!("Failed to set TypeScript language: {e}")))?;

        Ok(Self { parser, language })
    }

    /// Creates a new TSX parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    pub fn new_tsx() -> Result<Self, ParseError> {
        let language: Language = tree_sitter_typescript::LANGUAGE_TSX.into();

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| ParseError(format!("Failed to set TSX language: {e}")))?;

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
            .ok_or_else(|| ParseError("Failed to parse TypeScript source".to_string()))?;

        Ok((source, tree))
    }

    /// Creates a query for matching patterns in the syntax tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the query is invalid.
    pub fn create_query(&self, query_str: &str) -> Result<Query, ParseError> {
        Query::new(&self.language, query_str)
            .map_err(|e| ParseError(format!("Invalid TypeScript query: {e}")))
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new().expect("Failed to create TypeScript parser")
    }
}

/// Scanner for TypeScript files using `prom-client`.
///
/// Detects the following patterns:
/// - `new client.Counter({name: '...', help: '...', labelNames: [...]})`
/// - `new client.Gauge({...})`
/// - `new client.Histogram({...})`
/// - `new client.Summary({...})`
/// - `new Counter({...})` (destructured import)
pub struct TypeScriptPromClientScanner;

impl TypeScriptPromClientScanner {
    /// Creates a new TypeScript `prom-client` scanner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TypeScriptPromClientScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for TypeScriptPromClientScanner {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx"]
    }

    fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError> {
        // Use TSX parser for .tsx files, regular TypeScript for .ts
        let is_tsx = path.extension().is_some_and(|ext| ext == "tsx");

        let mut parser = if is_tsx {
            TypeScriptParser::new_tsx()?
        } else {
            TypeScriptParser::new()?
        };

        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(TS_METRICS_QUERY)?;

        scan_tree(&source, &tree, &query, path)
    }

    fn scan_usages(&self, path: &Path) -> Result<Vec<MetricUsage>, ParseError> {
        let is_tsx = path.extension().is_some_and(|ext| ext == "tsx");

        let mut parser = if is_tsx {
            TypeScriptParser::new_tsx()?
        } else {
            TypeScriptParser::new()?
        };

        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(TS_USAGE_QUERY)?;

        scan_usages_tree(&source, &tree, &query, path)
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

/// Extracts name and `labelNames` from an object literal.
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

/// Maps TypeScript/prom-client method names to usage kinds.
fn usage_kind_from_method_name(name: &str) -> Option<UsageKind> {
    match name {
        "inc" => Some(UsageKind::Increment),
        "dec" => Some(UsageKind::Sub),
        "set" => Some(UsageKind::Set),
        "observe" => Some(UsageKind::Observe),
        "startTimer" => Some(UsageKind::Time),
        "setToCurrentTime" => Some(UsageKind::SetToCurrentTime),
        _ => None,
    }
}

/// Scans a parsed syntax tree for metric usages.
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
    let args_idx = query
        .capture_index_for_name("args")
        .ok_or_else(|| ParseError("Query missing args capture".to_string()))?;

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(match_) = matches.next() {
        let mut object_node = None;
        let mut method_node = None;
        let mut args_node = None;

        for capture in match_.captures {
            if capture.index == object_idx {
                object_node = Some(capture.node);
            } else if capture.index == method_idx {
                method_node = Some(capture.node);
            } else if capture.index == args_idx {
                args_node = Some(capture.node);
            }
        }

        let (Some(obj), Some(method), Some(args)) = (object_node, method_node, args_node) else {
            continue;
        };

        let method_name = method.utf8_text(source.as_bytes()).unwrap_or_default();
        let Some(usage_kind) = usage_kind_from_method_name(method_name) else {
            continue;
        };

        // Extract the root variable name from potentially chained calls
        let variable_name = extract_ts_root_variable(&obj, source);
        if variable_name.is_empty() {
            continue;
        }

        // Extract label values from arguments (for labeled metrics)
        let label_values = extract_ts_label_values(&args, source);

        let start = obj.start_position();
        let (function_name, impl_type) = find_function_context(obj, source);

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

    Ok(results)
}

/// Extracts the root variable name from a potentially chained member expression.
///
/// For `counter.labels({...}).inc()`, this returns "counter".
/// For `this.metrics.counter.inc()`, this returns "counter".
fn extract_ts_root_variable(node: &Node<'_>, source: &str) -> String {
    match node.kind() {
        "identifier" => node
            .utf8_text(source.as_bytes())
            .unwrap_or_default()
            .to_string(),
        "member_expression" => {
            // For chained calls like counter.labels(...).inc(), we want the base object
            if let Some(property) = node.child_by_field_name("property") {
                let prop_text = property.utf8_text(source.as_bytes()).unwrap_or_default();
                // If the property is "labels", get the parent object
                if prop_text == "labels" {
                    if let Some(obj) = node.child_by_field_name("object") {
                        return extract_ts_root_variable(&obj, source);
                    }
                }
            }
            // Otherwise, get the deepest property name
            if let Some(property) = node.child_by_field_name("property") {
                property
                    .utf8_text(source.as_bytes())
                    .unwrap_or_default()
                    .to_string()
            } else {
                String::new()
            }
        }
        "call_expression" => {
            // For counter.labels({...}) where the result is used
            if let Some(func) = node.child_by_field_name("function") {
                if func.kind() == "member_expression" {
                    if let Some(obj) = func.child_by_field_name("object") {
                        return extract_ts_root_variable(&obj, source);
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// Extracts label values from arguments for labeled metric calls.
///
/// For `counter.labels({ method: 'GET', status: '200' }).inc()`, extracts `["GET", "200"]`.
fn extract_ts_label_values(args: &Node<'_>, source: &str) -> Vec<String> {
    let mut values = Vec::new();

    let mut cursor = args.walk();
    for child in args.children(&mut cursor) {
        match child.kind() {
            "object" => {
                // Extract values from object like { method: 'GET', status: '200' }
                let mut obj_cursor = child.walk();
                for pair in child.children(&mut obj_cursor) {
                    if pair.kind() == "pair" {
                        if let Some(value) = pair.child_by_field_name("value") {
                            let val = extract_string_value(&value, source);
                            if !val.is_empty() {
                                values.push(val);
                            }
                        }
                    }
                }
            }
            "string" => {
                // Direct string argument
                let val = extract_string_value(&child, source);
                if !val.is_empty() {
                    values.push(val);
                }
            }
            _ => {}
        }
    }

    values
}

#[cfg(test)]
#[allow(clippy::needless_raw_string_hashes)]
mod tests {
    use super::*;

    fn parse_and_scan_usages(source: &str) -> Vec<MetricUsage> {
        let mut parser = TypeScriptParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(TS_USAGE_QUERY)
            .expect("Failed to create query");
        scan_usages_tree(source, &tree, &query, Path::new("test.ts")).expect("Failed to scan")
    }

    fn parse_and_scan_usages_tsx(source: &str) -> Vec<MetricUsage> {
        let mut parser = TypeScriptParser::new_tsx().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(TS_USAGE_QUERY)
            .expect("Failed to create query");
        scan_usages_tree(source, &tree, &query, Path::new("test.tsx")).expect("Failed to scan")
    }

    fn parse_and_scan(source: &str) -> Vec<MetricInstrumentation> {
        let mut parser = TypeScriptParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(TS_METRICS_QUERY)
            .expect("Failed to create query");
        scan_tree(source, &tree, &query, Path::new("test.ts")).expect("Failed to scan")
    }

    fn parse_and_scan_tsx(source: &str) -> Vec<MetricInstrumentation> {
        let mut parser = TypeScriptParser::new_tsx().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(TS_METRICS_QUERY)
            .expect("Failed to create query");
        scan_tree(source, &tree, &query, Path::new("test.tsx")).expect("Failed to scan")
    }

    #[test]
    fn test_simple_counter() {
        let source = r#"
import client from 'prom-client';
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
import client from 'prom-client';
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
import client from 'prom-client';

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
import { Counter, Gauge } from 'prom-client';

const counter = new Counter({ name: 'requests_total', help: 'Total requests' });
const gauge = new Gauge({ name: 'temperature', help: 'Temperature' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].name, "requests_total");
        assert_eq!(metrics[1].name, "temperature");
    }

    #[test]
    fn test_typed_metric() {
        let source = r#"
import { Counter } from 'prom-client';

const counter: Counter<string> = new Counter({ name: 'typed_metric', help: 'A typed metric' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "typed_metric");
    }

    #[test]
    fn test_ignores_other_calls() {
        let source = r#"
const obj = new Object();
const arr = new Array<string>(10);
const counter = new client.Counter({ name: 'real_metric', help: 'Help' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "real_metric");
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"import client from 'prom-client';
const counter = new client.Counter({ name: 'line_two', help: 'Help' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].line, 2);
    }

    #[test]
    fn test_function_context_simple() {
        let source = r#"
import client from 'prom-client';

function handleRequest(): void {
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
import client from 'prom-client';

class Handler {
    process(): void {
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
import client from 'prom-client';
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
import client from 'prom-client';

const handler = (): void => {
    const counter = new client.Counter({ name: 'http_requests', help: 'Help' });
};
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("handler".to_string()));
    }

    #[test]
    fn test_async_function() {
        let source = r#"
import client from 'prom-client';

async function fetchData(): Promise<void> {
    const counter = new client.Counter({ name: 'fetch_count', help: 'Help' });
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("fetchData".to_string()));
    }

    #[test]
    fn test_tsx_component() {
        let source = r#"
import client from 'prom-client';
import React from 'react';

function MetricsComponent(): JSX.Element {
    const counter = new client.Counter({ name: 'component_render', help: 'Help' });
    return <div>Metrics</div>;
}
"#;
        let metrics = parse_and_scan_tsx(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "component_render");
        assert_eq!(
            metrics[0].function_name,
            Some("MetricsComponent".to_string())
        );
    }

    #[test]
    fn test_interface_and_types() {
        let source = r#"
import client, { Counter } from 'prom-client';

interface MetricConfig {
    name: string;
    help: string;
}

type MetricType = 'counter' | 'gauge';

const counter = new client.Counter({ name: 'typed_counter', help: 'Help' });
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "typed_counter");
    }

    // Usage tracking tests

    #[test]
    fn test_counter_inc() {
        let source = r#"
counter.inc();
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_gauge_set() {
        let source = r#"
gauge.set(42);
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Set);
        assert_eq!(usages[0].variable_name, "gauge");
    }

    #[test]
    fn test_gauge_dec() {
        let source = r#"
gauge.dec();
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Sub);
        assert_eq!(usages[0].variable_name, "gauge");
    }

    #[test]
    fn test_histogram_observe() {
        let source = r#"
histogram.observe(0.5);
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Observe);
        assert_eq!(usages[0].variable_name, "histogram");
    }

    #[test]
    fn test_start_timer() {
        let source = r#"
const end = histogram.startTimer();
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Time);
        assert_eq!(usages[0].variable_name, "histogram");
    }

    #[test]
    fn test_set_to_current_time() {
        let source = r#"
gauge.setToCurrentTime();
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::SetToCurrentTime);
        assert_eq!(usages[0].variable_name, "gauge");
    }

    #[test]
    fn test_usage_with_labels() {
        let source = r#"
counter.labels({ method: 'GET', status: '200' }).inc();
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[0].variable_name, "counter");
    }

    #[test]
    fn test_multiple_usages() {
        let source = r#"
counter.inc();
gauge.set(10);
histogram.observe(0.1);
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 3);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(usages[1].usage_kind, UsageKind::Set);
        assert_eq!(usages[2].usage_kind, UsageKind::Observe);
    }

    #[test]
    fn test_usage_in_function() {
        let source = r#"
function handleRequest(): void {
    counter.inc();
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].function_name, Some("handleRequest".to_string()));
    }

    #[test]
    fn test_usage_in_class_method() {
        let source = r#"
class Handler {
    process(): void {
        counter.inc();
    }
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].function_name, Some("process".to_string()));
        assert_eq!(usages[0].impl_type, Some("Handler".to_string()));
    }

    #[test]
    fn test_usage_in_async_function() {
        let source = r#"
async function fetchData(): Promise<void> {
    counter.inc();
}
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].function_name, Some("fetchData".to_string()));
    }

    #[test]
    fn test_ignores_non_metric_methods() {
        let source = r#"
console.log('hello');
array.push(1);
object.toString();
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 0);
    }

    #[test]
    fn test_usage_line_numbers() {
        let source = r#"counter.inc();
gauge.set(10);
"#;
        let usages = parse_and_scan_usages(source);
        assert_eq!(usages.len(), 2);
        assert_eq!(usages[0].line, 1);
        assert_eq!(usages[1].line, 2);
    }

    #[test]
    fn test_tsx_usage() {
        let source = r#"
function MetricsComponent(): JSX.Element {
    counter.inc();
    return <div>Metrics</div>;
}
"#;
        let usages = parse_and_scan_usages_tsx(source);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].usage_kind, UsageKind::Increment);
        assert_eq!(
            usages[0].function_name,
            Some("MetricsComponent".to_string())
        );
    }
}
