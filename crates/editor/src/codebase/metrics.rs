//! Metrics-rs instrumentation discovery.
//!
//! Scans Rust source files to find `counter!`, `gauge!`, and `histogram!` macro
//! invocations and extracts metric names and labels.

use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Query, QueryCursor};

use super::parser::{METRICS_QUERY, ParseError, RustParser};

/// The kind of metric instrumentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
}

impl MetricKind {
    /// Parses a metric kind from a macro name.
    pub fn from_macro_name(name: &str) -> Option<Self> {
        match name {
            "counter" => Some(Self::Counter),
            "gauge" => Some(Self::Gauge),
            "histogram" => Some(Self::Histogram),
            _ => None,
        }
    }

    /// Returns the macro name for this kind.
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

/// A discovered metric instrumentation point in the source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricInstrumentation {
    /// The kind of metric (counter, gauge, histogram).
    pub kind: MetricKind,
    /// The metric name (e.g., "http.requests").
    pub name: String,
    /// Label keys used with this metric (e.g., ["method", "endpoint"]).
    pub labels: Vec<String>,
    /// The file path where this metric is defined.
    pub file: std::path::PathBuf,
    /// Line number (1-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub column: usize,
}

/// Scans a Rust source file for metrics-rs macro invocations.
pub fn scan_file(path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError> {
    let mut parser = RustParser::new()?;
    let (source, tree) = parser.parse_file(path)?;
    let query = parser.create_query(METRICS_QUERY)?;

    scan_tree(&source, &tree, &query, path)
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
            results.push(MetricInstrumentation {
                kind,
                name,
                labels,
                file: file_path.to_path_buf(),
                line: start.row + 1, // Convert to 1-indexed
                column: start.column,
            });
        }
    }

    Ok(results)
}

/// Extracts the metric name and label keys from a macro's token tree.
///
/// Parses patterns like:
/// - `("metric.name")` -> name="metric.name", labels=[]
/// - `("metric.name", "key" => value)` -> name="metric.name", labels=["key"]
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

            if !found_name {
                // First string is the metric name
                name = string_content;
                found_name = true;
            } else {
                // Check if this is followed by =>
                let rest: String = chars[i + 1..].iter().collect();
                let rest_trimmed = rest.trim_start();
                if rest_trimmed.starts_with("=>") {
                    labels.push(string_content);
                }
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
        let mut parser = RustParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let query = parser.create_query(METRICS_QUERY).unwrap();
        scan_tree(source, &tree, &query, Path::new("test.rs")).unwrap()
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
}
