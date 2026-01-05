//! Diff parsing and semantic extraction.
//!
//! This module parses unified diffs and extracts semantic information
//! about what changed (functions, metrics, imports).

use std::path::Path;

use crate::repo::DiffSemantics;

/// Parses a unified diff and extracts semantic information.
///
/// This function analyzes the diff to identify:
/// - Functions that were added, removed, or modified
/// - Metric instrumentation changes
/// - Import statement changes
#[must_use]
pub fn extract_semantics(diff: &str) -> DiffSemantics {
    let mut semantics = DiffSemantics::default();

    // Parse the diff into hunks
    let hunks = parse_diff_hunks(diff);

    for hunk in hunks {
        // Extract function changes from this hunk
        extract_function_changes(&hunk, &mut semantics);

        // Extract metric changes
        extract_metric_changes(&hunk, &mut semantics);

        // Extract import changes
        extract_import_changes(&hunk, &mut semantics);
    }

    // Deduplicate
    semantics.functions_added.sort();
    semantics.functions_added.dedup();
    semantics.functions_removed.sort();
    semantics.functions_removed.dedup();
    semantics.functions_modified.sort();
    semantics.functions_modified.dedup();
    semantics.metrics_added.sort();
    semantics.metrics_added.dedup();
    semantics.metrics_removed.sort();
    semantics.metrics_removed.dedup();
    semantics.imports_added.sort();
    semantics.imports_added.dedup();
    semantics.imports_removed.sort();
    semantics.imports_removed.dedup();

    semantics
}

/// A parsed diff hunk with added and removed lines.
#[derive(Debug, Default)]
struct DiffHunk {
    /// File path this hunk belongs to
    file_path: String,
    /// Lines that were added (without the + prefix)
    added_lines: Vec<String>,
    /// Lines that were removed (without the - prefix)
    removed_lines: Vec<String>,
    /// The function context from @@ header (if available)
    function_context: Option<String>,
}

/// Language type detected from file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    Rust,
    Go,
    Python,
    JavaScript,
    Unknown,
}

impl Language {
    fn from_path(path: &str) -> Self {
        let path = Path::new(path);
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("rs") => Self::Rust,
            Some(ext) if ext.eq_ignore_ascii_case("go") => Self::Go,
            Some(ext) if ext.eq_ignore_ascii_case("py") => Self::Python,
            Some(ext)
                if ext.eq_ignore_ascii_case("js")
                    || ext.eq_ignore_ascii_case("ts")
                    || ext.eq_ignore_ascii_case("jsx")
                    || ext.eq_ignore_ascii_case("tsx") =>
            {
                Self::JavaScript
            }
            _ => Self::Unknown,
        }
    }
}

/// Parse a unified diff into individual hunks.
fn parse_diff_hunks(diff: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current_file = String::new();
    let mut current_hunk: Option<DiffHunk> = None;

    for line in diff.lines() {
        // New file header: +++ b/path/to/file.rs
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file = path.to_string();
            continue;
        }
        if let Some(path) = line.strip_prefix("+++ ") {
            // Handle +++ a/path format
            current_file = path.strip_prefix("a/").unwrap_or(path).to_string();
            continue;
        }

        // Hunk header: @@ -start,count +start,count @@ optional function context
        if line.starts_with("@@") {
            // Save previous hunk if any
            if let Some(hunk) = current_hunk.take() {
                if !hunk.added_lines.is_empty() || !hunk.removed_lines.is_empty() {
                    hunks.push(hunk);
                }
            }

            // Extract function context from @@ header
            let function_context = extract_function_from_hunk_header(line);

            current_hunk = Some(DiffHunk {
                file_path: current_file.clone(),
                function_context,
                ..Default::default()
            });
            continue;
        }

        // Skip diff metadata lines
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("Binary files")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
        {
            continue;
        }

        // Collect added/removed lines
        if let Some(hunk) = &mut current_hunk {
            if let Some(added) = line.strip_prefix('+') {
                hunk.added_lines.push(added.to_string());
            } else if let Some(removed) = line.strip_prefix('-') {
                hunk.removed_lines.push(removed.to_string());
            }
            // Context lines (starting with space) are ignored
        }
    }

    // Don't forget the last hunk
    if let Some(hunk) = current_hunk {
        if !hunk.added_lines.is_empty() || !hunk.removed_lines.is_empty() {
            hunks.push(hunk);
        }
    }

    hunks
}

/// Extract function name from @@ hunk header.
///
/// Format: `@@ -start,count +start,count @@ fn function_name(...)`
fn extract_function_from_hunk_header(line: &str) -> Option<String> {
    // Find the second @@ and get everything after it
    let parts: Vec<&str> = line.splitn(3, "@@").collect();
    if parts.len() < 3 {
        return None;
    }

    let context = parts[2].trim();
    if context.is_empty() {
        return None;
    }

    // Try to extract function name from various patterns
    // Rust: fn function_name, pub fn function_name, async fn function_name
    // Go: func FunctionName, func (r *Receiver) MethodName
    // Python: def function_name
    // JavaScript/TypeScript: function functionName

    // Rust patterns
    if let Some(idx) = context.find("fn ") {
        let after_fn = &context[idx + 3..];
        if let Some(name) = extract_identifier(after_fn) {
            return Some(name);
        }
    }

    // Go patterns
    if let Some(idx) = context.find("func ") {
        let after_func = &context[idx + 5..];
        // Skip receiver: func (r *Receiver)
        let name_part = if after_func.starts_with('(') {
            // Find closing paren and get the name after
            if let Some(close) = after_func.find(')') {
                after_func[close + 1..].trim()
            } else {
                after_func
            }
        } else {
            after_func
        };
        if let Some(name) = extract_identifier(name_part) {
            return Some(name);
        }
    }

    // Python patterns
    if let Some(idx) = context.find("def ") {
        let after_def = &context[idx + 4..];
        if let Some(name) = extract_identifier(after_def) {
            return Some(name);
        }
    }

    // JavaScript/TypeScript function keyword
    if let Some(idx) = context.find("function ") {
        let after_function = &context[idx + 9..];
        if let Some(name) = extract_identifier(after_function) {
            return Some(name);
        }
    }

    None
}

/// Extract an identifier (function name) from the start of a string.
fn extract_identifier(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let mut chars = s.chars().peekable();

    // First char must be alphabetic or underscore
    let first = chars.next()?;
    if !first.is_alphabetic() && first != '_' {
        return None;
    }

    let mut name = String::new();
    name.push(first);

    // Rest can be alphanumeric or underscore
    for c in chars {
        if c.is_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }

    if name.is_empty() { None } else { Some(name) }
}

/// Extract function changes from a hunk using pattern matching.
fn extract_function_changes(hunk: &DiffHunk, semantics: &mut DiffSemantics) {
    // If we have a function context from the @@ header, that function was modified
    if let Some(ref func_name) = hunk.function_context {
        // Only add to modified if we have actual content changes (not just context)
        if !hunk.added_lines.is_empty() || !hunk.removed_lines.is_empty() {
            semantics.functions_modified.push(func_name.clone());
        }
    }

    let lang = Language::from_path(&hunk.file_path);

    // Look for function definitions in added lines
    for line in &hunk.added_lines {
        if let Some(name) = extract_function_definition(line, lang) {
            semantics.functions_added.push(name);
        }
    }

    // Look for function definitions in removed lines
    for line in &hunk.removed_lines {
        if let Some(name) = extract_function_definition(line, lang) {
            semantics.functions_removed.push(name);
        }
    }
}

/// Extract a function definition from a single line of code.
fn extract_function_definition(line: &str, lang: Language) -> Option<String> {
    let line = line.trim();

    match lang {
        Language::Rust => {
            // Rust: fn name, pub fn name, async fn name, pub async fn name
            if let Some(idx) = line.find("fn ") {
                // Make sure 'fn' is at word boundary (not part of another word)
                let is_word_boundary =
                    idx == 0 || !line.chars().nth(idx - 1).is_some_and(char::is_alphanumeric);
                if is_word_boundary {
                    let after_fn = &line[idx + 3..];
                    return extract_identifier(after_fn);
                }
            }
        }
        Language::Go => {
            // Go: func Name or func (r *Receiver) Name
            if let Some(rest) = line.strip_prefix("func ") {
                // Skip receiver if present
                let name_part = if rest.starts_with('(') {
                    if let Some(close) = rest.find(')') {
                        rest[close + 1..].trim()
                    } else {
                        rest
                    }
                } else {
                    rest
                };
                return extract_identifier(name_part);
            }
        }
        Language::Python => {
            // Python: def name or async def name
            if let Some(idx) = line.find("def ") {
                let after_def = &line[idx + 4..];
                return extract_identifier(after_def);
            }
        }
        Language::JavaScript => {
            // JavaScript/TypeScript: function name, async function name
            if let Some(idx) = line.find("function ") {
                let after_function = &line[idx + 9..];
                return extract_identifier(after_function);
            }
            // Arrow function: const name = (...) => or let name = function
            if line.contains(" = (") || line.contains(" = function") {
                if let Some(rest) = line
                    .strip_prefix("const ")
                    .or_else(|| line.strip_prefix("let "))
                    .or_else(|| line.strip_prefix("var "))
                {
                    return extract_identifier(rest);
                }
            }
        }
        Language::Unknown => {}
    }

    None
}

/// Extract metric instrumentation changes from a hunk.
fn extract_metric_changes(hunk: &DiffHunk, semantics: &mut DiffSemantics) {
    // Patterns that indicate metric instrumentation
    let metric_patterns = [
        // Prometheus/metrics-rs patterns
        ".inc()",
        ".inc_by(",
        ".dec()",
        ".dec_by(",
        ".set(",
        ".observe(",
        ".record(",
        ".add(",
        // Counter/Gauge/Histogram constructors
        "Counter::new(",
        "Gauge::new(",
        "Histogram::new(",
        "IntCounter::new(",
        "IntGauge::new(",
        "register_counter!",
        "register_gauge!",
        "register_histogram!",
        // Go prometheus patterns
        "prometheus.NewCounter(",
        "prometheus.NewGauge(",
        "prometheus.NewHistogram(",
        "promauto.NewCounter(",
        "promauto.NewGauge(",
        "promauto.NewHistogram(",
        ".WithLabelValues(",
        ".With(",
        // Python prometheus patterns
        "Counter(",
        "Gauge(",
        "Histogram(",
        "Summary(",
    ];

    for line in &hunk.added_lines {
        for pattern in &metric_patterns {
            if line.contains(pattern) {
                // Try to extract the metric name
                if let Some(name) = extract_metric_name(line) {
                    semantics.metrics_added.push(name);
                }
                break;
            }
        }
    }

    for line in &hunk.removed_lines {
        for pattern in &metric_patterns {
            if line.contains(pattern) {
                if let Some(name) = extract_metric_name(line) {
                    semantics.metrics_removed.push(name);
                }
                break;
            }
        }
    }
}

/// Try to extract a metric name from a line of code.
fn extract_metric_name(line: &str) -> Option<String> {
    // Look for quoted strings that look like metric names
    // e.g., "http_requests_total", 'grpc_latency_seconds'

    let mut in_quote = false;
    let mut quote_char = '"';
    let mut current_string = String::new();
    let mut found_strings = Vec::new();

    for c in line.chars() {
        if !in_quote && (c == '"' || c == '\'') {
            in_quote = true;
            quote_char = c;
            current_string.clear();
        } else if in_quote && c == quote_char {
            in_quote = false;
            if looks_like_metric_name(&current_string) {
                found_strings.push(current_string.clone());
            }
        } else if in_quote {
            current_string.push(c);
        }
    }

    // Return the first string that looks like a metric name
    found_strings.into_iter().next()
}

/// Check if a string looks like a Prometheus metric name.
fn looks_like_metric_name(s: &str) -> bool {
    // Metric names typically:
    // - Contain underscores
    // - Are lowercase
    // - End with _total, _count, _sum, _bucket, _seconds, _bytes, etc.
    // - Or contain common metric words

    if s.len() < 3 || !s.contains('_') {
        return false;
    }

    let s_lower = s.to_lowercase();

    // Common metric suffixes
    let suffixes = [
        "_total", "_count", "_sum", "_bucket", "_seconds", "_bytes", "_info", "_created", "_gauge",
        "_counter",
    ];

    // Common metric words
    let keywords = [
        "request",
        "response",
        "error",
        "latency",
        "duration",
        "http",
        "grpc",
        "queue",
        "cache",
        "connection",
        "active",
        "pending",
    ];

    // Check for suffix match
    for suffix in &suffixes {
        if s_lower.ends_with(suffix) {
            return true;
        }
    }

    // Check for keyword match
    for keyword in &keywords {
        if s_lower.contains(keyword) {
            return true;
        }
    }

    false
}

/// Extract import statement changes from a hunk.
fn extract_import_changes(hunk: &DiffHunk, semantics: &mut DiffSemantics) {
    let lang = Language::from_path(&hunk.file_path);

    for line in &hunk.added_lines {
        if let Some(import) = extract_import(line, lang) {
            semantics.imports_added.push(import);
        }
    }

    for line in &hunk.removed_lines {
        if let Some(import) = extract_import(line, lang) {
            semantics.imports_removed.push(import);
        }
    }
}

/// Extract an import statement from a line.
fn extract_import(line: &str, lang: Language) -> Option<String> {
    let line = line.trim();

    match lang {
        Language::Rust => {
            if line.starts_with("use ") {
                // Rust: use foo::bar;
                let import = line
                    .strip_prefix("use ")?
                    .trim_end_matches(';')
                    .trim()
                    .to_string();
                return Some(import);
            }
        }
        Language::Go => {
            if line.starts_with("import ") {
                // Go: import "path/to/package" or import name "path"
                if let Some(start) = line.find('"') {
                    if let Some(end) = line[start + 1..].find('"') {
                        return Some(line[start + 1..start + 1 + end].to_string());
                    }
                }
            }
        }
        Language::Python => {
            if line.starts_with("import ") {
                // Python: import foo
                return Some(line.strip_prefix("import ")?.trim().to_string());
            }
            if line.starts_with("from ") {
                // Python: from foo import bar
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    return Some(parts[1].to_string());
                }
            }
        }
        Language::JavaScript => {
            if line.starts_with("import ") {
                // JavaScript/TypeScript: import { foo } from 'bar'
                if let Some(from_idx) = line.find(" from ") {
                    let after_from = &line[from_idx + 6..];
                    let path = after_from
                        .trim()
                        .trim_matches(|c| c == '\'' || c == '"' || c == ';');
                    return Some(path.to_string());
                }
            }
            if line.starts_with("require(") || line.contains("require(") {
                // CommonJS: require('foo')
                if let Some(start) = line.find("require(") {
                    let after = &line[start + 8..];
                    if let Some(quote_start) = after.find(['\'', '"']) {
                        let quote_char = after.chars().nth(quote_start)?;
                        let path_start = quote_start + 1;
                        if let Some(end) = after[path_start..].find(quote_char) {
                            return Some(after[path_start..path_start + end].to_string());
                        }
                    }
                }
            }
        }
        Language::Unknown => {}
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_diff_hunks() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,8 @@ fn main() {
     let x = 1;
+    let y = 2;
+    let z = 3;
     println!("Hello");
-    let old = 5;
 }
"#;

        let hunks = parse_diff_hunks(diff);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].file_path, "src/main.rs");
        assert_eq!(hunks[0].added_lines.len(), 2);
        assert_eq!(hunks[0].removed_lines.len(), 1);
        assert_eq!(hunks[0].function_context, Some("main".to_string()));
    }

    #[test]
    fn test_extract_function_definition_rust() {
        assert_eq!(
            extract_function_definition("fn foo() {", Language::Rust),
            Some("foo".to_string())
        );
        assert_eq!(
            extract_function_definition("pub fn bar(x: i32) -> i32 {", Language::Rust),
            Some("bar".to_string())
        );
        assert_eq!(
            extract_function_definition("async fn baz() {", Language::Rust),
            Some("baz".to_string())
        );
        assert_eq!(
            extract_function_definition("    pub async fn qux() {", Language::Rust),
            Some("qux".to_string())
        );
    }

    #[test]
    fn test_extract_function_definition_go() {
        assert_eq!(
            extract_function_definition("func Foo() {", Language::Go),
            Some("Foo".to_string())
        );
        assert_eq!(
            extract_function_definition("func (s *Server) Handle() {", Language::Go),
            Some("Handle".to_string())
        );
    }

    #[test]
    fn test_extract_function_definition_python() {
        assert_eq!(
            extract_function_definition("def foo():", Language::Python),
            Some("foo".to_string())
        );
        assert_eq!(
            extract_function_definition("async def bar():", Language::Python),
            Some("bar".to_string())
        );
    }

    #[test]
    fn test_extract_metric_name() {
        assert_eq!(
            extract_metric_name(r#"counter.with_label_values(&["http_requests_total"]).inc()"#),
            Some("http_requests_total".to_string())
        );
        assert_eq!(
            extract_metric_name(r#"histogram.observe("grpc_latency_seconds", 0.5)"#),
            Some("grpc_latency_seconds".to_string())
        );
    }

    #[test]
    fn test_looks_like_metric_name() {
        assert!(looks_like_metric_name("http_requests_total"));
        assert!(looks_like_metric_name("grpc_latency_seconds"));
        assert!(looks_like_metric_name("cache_hits_count"));
        assert!(!looks_like_metric_name("foo")); // No underscore
        assert!(!looks_like_metric_name("ab")); // Too short
    }

    #[test]
    fn test_extract_semantics_function_added() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,7 @@
 fn existing() {}
+
+fn new_function() {
+    println!("hello");
+}
"#;

        let semantics = extract_semantics(diff);
        assert!(
            semantics
                .functions_added
                .contains(&"new_function".to_string())
        );
    }

    #[test]
    fn test_extract_semantics_metric_change() {
        let diff = r#"diff --git a/src/server.rs b/src/server.rs
--- a/src/server.rs
+++ b/src/server.rs
@@ -10,6 +10,7 @@ fn handle_request() {
     process();
+    counter.with_label_values(&["http_requests_total"]).inc();
 }
"#;

        let semantics = extract_semantics(diff);
        assert!(
            semantics
                .metrics_added
                .contains(&"http_requests_total".to_string())
        );
    }

    #[test]
    fn test_extract_import_rust() {
        assert_eq!(
            extract_import("use std::collections::HashMap;", Language::Rust),
            Some("std::collections::HashMap".to_string())
        );
    }

    #[test]
    fn test_extract_import_python() {
        assert_eq!(
            extract_import("import prometheus_client", Language::Python),
            Some("prometheus_client".to_string())
        );
        assert_eq!(
            extract_import("from prometheus_client import Counter", Language::Python),
            Some("prometheus_client".to_string())
        );
    }

    #[test]
    fn test_extract_import_javascript() {
        assert_eq!(
            extract_import(
                "import { Counter } from 'prom-client';",
                Language::JavaScript
            ),
            Some("prom-client".to_string())
        );
    }
}
