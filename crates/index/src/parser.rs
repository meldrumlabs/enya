//! Tree-sitter integration for parsing Rust source files.
//!
//! Provides a wrapper around tree-sitter-rust for parsing Rust code
//! and querying for specific patterns like macro invocations.

use std::path::Path;

use tree_sitter::{Language, Parser, Query, Tree};

/// Error type for parsing operations.
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// A wrapper around tree-sitter for parsing Rust source code.
pub struct RustParser {
    parser: Parser,
    language: Language,
}

impl RustParser {
    /// Creates a new Rust parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the tree-sitter language cannot be set.
    pub fn new() -> Result<Self, ParseError> {
        let language: Language = tree_sitter_rust::LANGUAGE.into();

        let mut parser = Parser::new();
        parser
            .set_language(&language)
            .map_err(|e| ParseError(format!("Failed to set language: {e}")))?;

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
            .ok_or_else(|| ParseError("Failed to parse source".to_string()))?;

        Ok((source, tree))
    }

    /// Creates a query for matching patterns in the syntax tree.
    ///
    /// # Errors
    ///
    /// Returns an error if the query is invalid.
    pub fn create_query(&self, query_str: &str) -> Result<Query, ParseError> {
        Query::new(&self.language, query_str).map_err(|e| ParseError(format!("Invalid query: {e}")))
    }
}

impl Default for RustParser {
    fn default() -> Self {
        Self::new().expect("Failed to create Rust parser")
    }
}

/// Tree-sitter query for finding metrics-rs macro invocations.
///
/// This query matches:
/// - `counter!("name", ...)`
/// - `gauge!("name", ...)`
/// - `histogram!("name", ...)`
pub const METRICS_QUERY: &str = r"
(macro_invocation
  macro: (identifier) @macro_name
  (token_tree) @args)
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_function() {
        let mut parser = RustParser::new().expect("Failed to create parser");
        let source = r#"
fn main() {
    println!("Hello, world!");
}
"#;
        let tree = parser.parse(source).expect("Failed to parse");
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn test_parse_with_metrics_macro() {
        let mut parser = RustParser::new().expect("Failed to create parser");
        let source = r#"
fn handle_request() {
    counter!("http.requests", "method" => "GET").increment(1);
}
"#;
        let tree = parser.parse(source).expect("Failed to parse");
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn test_metrics_query() {
        use streaming_iterator::StreamingIterator;
        use tree_sitter::QueryCursor;

        let mut parser = RustParser::new().expect("Failed to create parser");
        let source = r#"
fn handle_request() {
    counter!("http.requests", "method" => "GET").increment(1);
    gauge!("connections.active").set(42.0);
    histogram!("request.latency_ms").record(150.0);
}
"#;
        let tree = parser.parse(source).expect("Failed to parse");
        let query = parser
            .create_query(METRICS_QUERY)
            .expect("Failed to create query");
        let mut cursor = QueryCursor::new();

        let mut count = 0;
        let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        while matches.next().is_some() {
            count += 1;
        }

        // Should find 3 macro invocations (counter, gauge, histogram)
        assert_eq!(count, 3);
    }
}
