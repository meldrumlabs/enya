//! Rust metrics-rs scanner implementation.
//!
//! Scans Rust source files to find `counter!`, `gauge!`, and `histogram!` macro
//! invocations from the metrics-rs crate and extracts metric names and labels.

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

use super::{MetricInstrumentation, MetricKind, Scanner};
use crate::parser::{METRICS_QUERY, ParseError, RustParser};

impl MetricKind {
    /// Parses a metric kind from a metrics-rs macro name.
    #[must_use]
    pub fn from_macro_name(name: &str) -> Option<Self> {
        match name {
            "counter" => Some(Self::Counter),
            "gauge" => Some(Self::Gauge),
            "histogram" => Some(Self::Histogram),
            _ => None,
        }
    }
}

/// Scanner for Rust files using metrics-rs macros.
///
/// Detects the following patterns:
/// - `counter!("metric.name")` / `counter!("metric.name", "label" => value)`
/// - `gauge!("metric.name")` / `gauge!("metric.name", "label" => value)`
/// - `histogram!("metric.name")` / `histogram!("metric.name", "label" => value)`
pub struct RustMetricsScanner;

impl RustMetricsScanner {
    /// Creates a new Rust metrics-rs scanner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for RustMetricsScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for RustMetricsScanner {
    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError> {
        let mut parser = RustParser::new()?;
        let (source, tree) = parser.parse_file(path)?;
        let query = parser.create_query(METRICS_QUERY)?;

        scan_tree(&source, &tree, &query, path)
    }
}

/// Scans a parsed syntax tree for metrics macros.
fn scan_tree(
    source: &str,
    tree: &tree_sitter::Tree,
    query: &Query,
    file_path: &Path,
) -> Result<Vec<MetricInstrumentation>, ParseError> {
    let mut cursor = QueryCursor::new();
    let mut results = Vec::new();

    let macro_name_idx = query
        .capture_index_for_name("macro_name")
        .ok_or_else(|| ParseError("Query missing macro_name capture".to_string()))?;
    let args_idx = query
        .capture_index_for_name("args")
        .ok_or_else(|| ParseError("Query missing args capture".to_string()))?;

    let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
    while let Some(match_) = matches.next() {
        let mut macro_name_node = None;
        let mut args_node = None;

        for capture in match_.captures {
            if capture.index == macro_name_idx {
                macro_name_node = Some(capture.node);
            } else if capture.index == args_idx {
                args_node = Some(capture.node);
            }
        }

        let (Some(macro_node), Some(args)) = (macro_name_node, args_node) else {
            continue;
        };

        let macro_name = macro_node.utf8_text(source.as_bytes()).unwrap_or_default();

        let Some(kind) = MetricKind::from_macro_name(macro_name) else {
            continue;
        };

        // Extract metric name and labels from the token tree
        let (name, labels) = extract_metric_info(&args, source);

        if !name.is_empty() {
            let start = macro_node.start_position();

            // Find the containing function and impl type
            let (function_name, impl_type) = find_function_context(macro_node, source);

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

/// Finds the containing function and impl type for a node by walking up the AST.
///
/// Returns `(function_name, impl_type)` where either may be `None` if the metric
/// is not inside a function or impl block.
fn find_function_context(node: Node<'_>, source: &str) -> (Option<String>, Option<String>) {
    let mut current = node;
    let mut function_name = None;
    let mut impl_type = None;

    while let Some(parent) = current.parent() {
        match parent.kind() {
            "function_item" => {
                if function_name.is_none() {
                    function_name = parent
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(String::from);
                }
            }
            "impl_item" => {
                if impl_type.is_none() {
                    impl_type = parent
                        .child_by_field_name("type")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .map(String::from);
                }
            }
            _ => {}
        }
        current = parent;
    }

    (function_name, impl_type)
}

/// Extracts the metric name and label keys from a macro's token tree.
///
/// Parses patterns like:
/// - `("metric.name")` -> `name="metric.name"`, `labels=[]`
/// - `("metric.name", "key" => value)` -> `name="metric.name"`, `labels=["key"]`
fn extract_metric_info(args_node: &Node<'_>, source: &str) -> (String, Vec<String>) {
    let mut name = String::new();
    let mut labels = Vec::new();

    let args_text = args_node.utf8_text(source.as_bytes()).unwrap_or_default();

    // Simple parsing: find the first string literal for the name
    // and any "key" => patterns for labels
    let mut in_string = false;
    let mut string_start = 0;
    let mut found_name = false;

    let chars: Vec<char> = args_text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '"' && !in_string {
            in_string = true;
            string_start = i + 1;
        } else if c == '"' && in_string {
            let string_content: String = chars[string_start..i].iter().collect();

            if found_name {
                // Check if this is followed by =>
                let rest: String = chars[i + 1..].iter().collect();
                let rest_trimmed = rest.trim_start();
                if rest_trimmed.starts_with("=>") {
                    labels.push(string_content);
                }
            } else {
                // First string is the metric name
                name = string_content;
                found_name = true;
            }
            in_string = false;
        }

        i += 1;
    }

    (name, labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_scan(source: &str) -> Vec<MetricInstrumentation> {
        let mut parser = RustParser::new().expect("Failed to create parser");
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(METRICS_QUERY)
            .expect("Failed to create query");
        scan_tree(source, &tree, &query, Path::new("test.rs")).expect("Failed to scan")
    }

    #[test]
    fn test_simple_counter() {
        let source = r#"
fn main() {
    counter!("http.requests").increment(1);
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[0].name, "http.requests");
        assert!(metrics[0].labels.is_empty());
    }

    #[test]
    fn test_counter_with_labels() {
        let source = r#"
fn handle(method: &str) {
    counter!("http.requests", "method" => method, "status" => "200").increment(1);
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[0].name, "http.requests");
        assert_eq!(metrics[0].labels, vec!["method", "status"]);
    }

    #[test]
    fn test_all_metric_types() {
        let source = r#"
fn metrics_example() {
    counter!("requests.total").increment(1);
    gauge!("connections.active").set(42.0);
    histogram!("request.latency_ms").record(150.0);
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 3);
        assert_eq!(metrics[0].kind, MetricKind::Counter);
        assert_eq!(metrics[1].kind, MetricKind::Gauge);
        assert_eq!(metrics[2].kind, MetricKind::Histogram);
    }

    #[test]
    fn test_ignores_other_macros() {
        let source = r#"
fn main() {
    println!("Hello, world!");
    debug!("Debugging");
    counter!("real.metric").increment(1);
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "real.metric");
    }

    #[test]
    fn test_line_numbers() {
        let source = r#"fn main() {
    counter!("line.two").increment(1);
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].line, 2);
    }

    #[test]
    fn test_function_context_simple() {
        let source = r#"
fn handle_request() {
    counter!("http.requests").increment(1);
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("handle_request".to_string()));
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_function_context_in_impl() {
        let source = r#"
impl Handler {
    fn process(&self) {
        counter!("http.requests").increment(1);
    }
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("process".to_string()));
        assert_eq!(metrics[0].impl_type, Some("Handler".to_string()));
    }

    #[test]
    fn test_function_context_in_closure() {
        let source = r#"
fn setup() {
    let handler = || {
        counter!("http.requests").increment(1);
    };
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        // Closures should still find the outer function
        assert_eq!(metrics[0].function_name, Some("setup".to_string()));
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_function_context_at_module_level() {
        let source = r#"
counter!("startup.count").increment(1);
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, None);
        assert_eq!(metrics[0].impl_type, None);
    }

    #[test]
    fn test_function_context_trait_impl() {
        let source = r#"
impl Service for MyHandler {
    fn handle(&self) {
        counter!("service.requests").increment(1);
    }
}
"#;
        let metrics = parse_and_scan(source);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].function_name, Some("handle".to_string()));
        // The impl type captures the type, not the trait
        assert_eq!(metrics[0].impl_type, Some("MyHandler".to_string()));
    }
}
