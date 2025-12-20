//! Query validation module - validates queries and produces diagnostics.
//!
//! Provides both syntax validation (via enya-lang/enya-promql parsers) and semantic validation
//! (checking for unknown tag keys, suggesting corrections, etc.).
//!
//! Supports dual-language mode: PromQL (default) and EnyaLang.

use rustc_hash::FxHashSet;

use crate::components::overlay::diagnostics::{Diagnostic, DiagnosticLevel, DiagnosticSource};
use crate::components::util::query_completion::QueryLanguage;

/// Result of validating a query
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// List of diagnostics found
    pub diagnostics: Vec<Diagnostic>,
    /// Whether the query is valid (has no errors)
    pub is_valid: bool,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn ok() -> Self {
        Self {
            diagnostics: Vec::new(),
            is_valid: true,
        }
    }

    /// Create a validation result with diagnostics
    pub fn with_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        let is_valid = !diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error);
        Self {
            diagnostics,
            is_valid,
        }
    }
}

/// Query validator with configurable known tags
pub struct QueryValidator {
    /// Query language mode (PromQL or EnyaLang)
    language: QueryLanguage,
    /// Known tag keys (for semantic validation)
    known_tag_keys: FxHashSet<String>,
    /// Known tag values by key
    known_tag_values: rustc_hash::FxHashMap<String, FxHashSet<String>>,
}

impl Default for QueryValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryValidator {
    /// Create a new validator with default known tags (defaults to PromQL)
    pub fn new() -> Self {
        let mut validator = Self {
            language: QueryLanguage::default(),
            known_tag_keys: FxHashSet::default(),
            known_tag_values: rustc_hash::FxHashMap::default(),
        };

        // Add common tag keys
        validator.add_known_keys(&[
            "env", "service", "region", "host", "instance", "status", "method", "endpoint",
        ]);

        validator
    }

    /// Set the query language mode
    pub fn set_language(&mut self, language: QueryLanguage) {
        self.language = language;
    }

    /// Get the current query language mode
    pub fn language(&self) -> QueryLanguage {
        self.language
    }

    /// Add known tag keys
    pub fn add_known_keys(&mut self, keys: &[&str]) {
        for key in keys {
            self.known_tag_keys.insert((*key).to_string());
        }
    }

    /// Add known values for a tag key
    pub fn add_known_values(&mut self, key: &str, values: &[&str]) {
        let value_set = self.known_tag_values.entry(key.to_string()).or_default();
        for value in values {
            value_set.insert((*value).to_string());
        }
    }

    /// Validate a query and return diagnostics (dispatches based on language mode)
    pub fn validate(&self, query: &str) -> ValidationResult {
        match self.language {
            QueryLanguage::PromQL => self.validate_promql(query),
            QueryLanguage::EnyaLang => self.validate_enya_lang(query),
        }
    }

    /// Validate a PromQL query
    fn validate_promql(&self, query: &str) -> ValidationResult {
        // Skip validation for empty queries
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return ValidationResult::ok();
        }

        // Use enya-promql for validation
        let result = enya_promql::validate(query);
        if result.is_valid {
            return ValidationResult::ok();
        }

        // Convert error to diagnostic
        let error_msg = result
            .error
            .unwrap_or_else(|| "Invalid PromQL syntax".to_string());
        let diagnostic = Diagnostic::error(error_msg).with_source(DiagnosticSource::QuerySyntax);

        ValidationResult::with_diagnostics(vec![diagnostic])
    }

    /// Validate an EnyaLang query
    fn validate_enya_lang(&self, query: &str) -> ValidationResult {
        let mut diagnostics = Vec::new();

        // Skip validation for empty queries or wildcard
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return ValidationResult::ok();
        }
        if trimmed == "*" {
            return ValidationResult::ok();
        }

        // Syntax validation using enya-lang parser
        // Use parse_query to support both filter expressions and aggregation queries
        if let Err(_e) = enya_lang::parse_query(query) {
            // Try to provide more specific error messages
            let syntax_diagnostics = self.diagnose_syntax_error(query);
            if syntax_diagnostics.is_empty() {
                diagnostics.push(
                    Diagnostic::error("Invalid query syntax")
                        .with_source(DiagnosticSource::QuerySyntax),
                );
            } else {
                diagnostics.extend(syntax_diagnostics);
            }
            return ValidationResult::with_diagnostics(diagnostics);
        }

        // Semantic validation (only if syntax is valid)
        diagnostics.extend(self.validate_semantics(query));

        ValidationResult::with_diagnostics(diagnostics)
    }

    /// Try to diagnose specific syntax errors
    fn diagnose_syntax_error(&self, query: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for unbalanced parentheses
        let open_count = query.chars().filter(|c| *c == '(').count();
        let close_count = query.chars().filter(|c| *c == ')').count();
        if open_count != close_count {
            if open_count > close_count {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "Unbalanced parentheses: {} unclosed '('",
                        open_count - close_count
                    ))
                    .with_source(DiagnosticSource::QuerySyntax)
                    .with_code("E001"),
                );
            } else {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "Unbalanced parentheses: {} extra ')'",
                        close_count - open_count
                    ))
                    .with_source(DiagnosticSource::QuerySyntax)
                    .with_code("E001"),
                );
            }
            return diagnostics;
        }

        // Check for missing colon in tag (e.g., "env prod" instead of "env:prod")
        // Skip this check for aggregation queries - they have different syntax
        if !is_aggregation_query(query) {
            let words: Vec<&str> = query.split_whitespace().collect();
            for (i, word) in words.iter().enumerate() {
                // Skip operators
                if matches!(word.to_uppercase().as_str(), "AND" | "OR" | "NOT") {
                    continue;
                }
                // Skip words that start with ! (negation)
                let check_word = word.trim_start_matches('!');
                // Skip parentheses
                let check_word = check_word.trim_start_matches('(').trim_end_matches(')');

                if check_word.is_empty() || check_word == "*" {
                    continue;
                }

                // If it doesn't contain a colon, it might be a malformed tag
                if !check_word.contains(':') {
                    // Check if next word could be a value (not an operator)
                    let next_is_value = words.get(i + 1).is_some_and(|next| {
                        !matches!(next.to_uppercase().as_str(), "AND" | "OR" | "NOT")
                            && !next.contains(':')
                    });

                    if next_is_value {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "Missing ':' - did you mean '{}:{}'?",
                                check_word,
                                words.get(i + 1).unwrap_or(&"value")
                            ))
                            .with_source(DiagnosticSource::QuerySyntax)
                            .with_code("E002"),
                        );
                    } else {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "Invalid term '{check_word}' - expected format 'key:value'"
                            ))
                            .with_source(DiagnosticSource::QuerySyntax)
                            .with_code("E002"),
                        );
                    }
                }
            }
        }

        // Check for trailing/leading operators
        let normalized = query.replace(['(', ')'], " ").trim().to_string();
        let words: Vec<&str> = normalized.split_whitespace().collect();

        if let Some(first) = words.first() {
            if matches!(first.to_uppercase().as_str(), "AND" | "OR") {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "Query cannot start with '{}'",
                        first.to_uppercase()
                    ))
                    .with_source(DiagnosticSource::QuerySyntax)
                    .with_code("E003"),
                );
            }
        }

        if let Some(last) = words.last() {
            if matches!(last.to_uppercase().as_str(), "AND" | "OR" | "NOT" | "!") {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "Query cannot end with operator '{}'",
                        last.to_uppercase()
                    ))
                    .with_source(DiagnosticSource::QuerySyntax)
                    .with_code("E003"),
                );
            }
        }

        diagnostics
    }

    /// Validate semantic aspects of the query
    fn validate_semantics(&self, query: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Extract tag key:value pairs from the query
        let tags = self.extract_tags(query);

        for (key, value) in tags {
            // Check for unknown tag keys (only warn, don't error)
            if !self.known_tag_keys.is_empty() && !self.known_tag_keys.contains(&key) {
                // Find similar keys for suggestions
                let suggestion = self.find_similar_key(&key);
                let message = if let Some(similar) = suggestion {
                    format!("Unknown tag key '{key}' - did you mean '{similar}'?")
                } else {
                    format!("Unknown tag key '{key}'")
                };
                diagnostics.push(
                    Diagnostic::warning(message)
                        .with_source(DiagnosticSource::QueryValidation)
                        .with_code("W001"),
                );
            }

            // Check for potentially misspelled values
            if let Some(known_values) = self.known_tag_values.get(&key) {
                if !known_values.is_empty() && !value.ends_with('*') {
                    // Only check exact matches (not wildcards)
                    if !known_values.contains(&value) {
                        let suggestion = self.find_similar_value(&key, &value);
                        if let Some(similar) = suggestion {
                            diagnostics.push(
                                Diagnostic::hint(format!(
                                    "Unknown value '{value}' for '{key}' - did you mean '{similar}'?"
                                ))
                                .with_source(DiagnosticSource::QueryValidation)
                                .with_code("H001"),
                            );
                        }
                    }
                }
            }
        }

        diagnostics
    }

    /// Extract tag key:value pairs from a query string
    fn extract_tags(&self, query: &str) -> Vec<(String, String)> {
        let mut tags = Vec::new();

        // Simple extraction - split by whitespace and look for key:value patterns
        for word in query.split_whitespace() {
            // Remove operators and parentheses
            let word = word
                .trim_start_matches('!')
                .trim_start_matches('(')
                .trim_end_matches(')');

            // Skip operators
            if matches!(word.to_uppercase().as_str(), "AND" | "OR" | "NOT") {
                continue;
            }

            // Extract key:value
            if let Some((key, value)) = word.split_once(':') {
                let value = value.trim_end_matches('*');
                tags.push((key.to_string(), value.to_string()));
            }
        }

        tags
    }

    /// Find a similar tag key using Levenshtein distance
    fn find_similar_key(&self, key: &str) -> Option<String> {
        self.known_tag_keys
            .iter()
            .filter(|k| levenshtein_distance(key, k) <= 2)
            .min_by_key(|k| levenshtein_distance(key, k))
            .cloned()
    }

    /// Find a similar tag value using Levenshtein distance
    fn find_similar_value(&self, key: &str, value: &str) -> Option<String> {
        self.known_tag_values
            .get(key)?
            .iter()
            .filter(|v| levenshtein_distance(value, v) <= 2)
            .min_by_key(|v| levenshtein_distance(value, v))
            .cloned()
    }
}

/// Simple Levenshtein distance implementation
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(a_len + 1) {
        row[0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

/// Check if a query starts with an aggregation function.
/// Used to skip filter-specific diagnostics for aggregation queries.
fn is_aggregation_query(query: &str) -> bool {
    const AGGREGATION_FUNCTIONS: &[&str] = &[
        "sum",
        "avg",
        "min",
        "max",
        "count",
        "rate",
        "irate",
        "increase",
        "avg_over_time",
        "sum_over_time",
        "min_over_time",
        "max_over_time",
        "count_over_time",
    ];

    let trimmed = query.trim();
    AGGREGATION_FUNCTIONS
        .iter()
        .any(|func| trimmed.starts_with(func))
}

/// Convenience function to validate a query with default settings
pub fn validate_query(query: &str) -> ValidationResult {
    QueryValidator::new().validate(query)
}

/// Convenience function to check if a query is valid
pub fn is_valid_query(query: &str) -> bool {
    validate_query(query).is_valid
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create an EnyaLang validator
    fn enya_lang_validator() -> QueryValidator {
        let mut validator = QueryValidator::new();
        validator.set_language(QueryLanguage::EnyaLang);
        validator
    }

    // ============ PromQL Tests (default language) ============

    #[test]
    fn test_promql_valid_metric() {
        let result = validate_query("http_requests_total");
        assert!(result.is_valid);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_promql_valid_selector() {
        let result = validate_query(r#"http_requests_total{method="GET"}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_promql_valid_rate() {
        let result = validate_query("rate(http_requests_total[5m])");
        assert!(result.is_valid);
    }

    #[test]
    fn test_promql_valid_aggregation() {
        let result = validate_query("sum(rate(http_requests_total[5m])) by (job)");
        assert!(result.is_valid);
    }

    #[test]
    fn test_promql_valid_complex() {
        let result = validate_query(r#"sum(rate(http_requests_total{job="api"}[5m])) by (method)"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_promql_empty_query() {
        let result = validate_query("");
        assert!(result.is_valid); // Empty is OK (user is still typing)
    }

    #[test]
    fn test_promql_invalid_unclosed_brace() {
        let result = validate_query("http_requests_total{");
        assert!(!result.is_valid);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn test_promql_invalid_missing_duration() {
        let result = validate_query("rate(http_requests_total[])");
        assert!(!result.is_valid);
    }

    #[test]
    fn test_promql_invalid_unknown_function() {
        let result = validate_query("unknown_func(x)");
        assert!(!result.is_valid);
    }

    // ============ EnyaLang Tests ============

    #[test]
    fn test_enya_valid_query() {
        let result = enya_lang_validator().validate("env:prod");
        assert!(result.is_valid);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_enya_wildcard_query() {
        let result = enya_lang_validator().validate("*");
        assert!(result.is_valid);
    }

    #[test]
    fn test_enya_invalid_syntax_missing_colon() {
        let result = enya_lang_validator().validate("env prod");
        assert!(!result.is_valid);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn test_enya_unbalanced_parens() {
        let result = enya_lang_validator().validate("(env:prod AND service:db");
        assert!(!result.is_valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("Unbalanced"))
        );
    }

    #[test]
    fn test_enya_trailing_operator() {
        let result = enya_lang_validator().validate("env:prod AND");
        assert!(!result.is_valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("cannot end with"))
        );
    }

    #[test]
    fn test_enya_unknown_tag_key_warning() {
        let result = enya_lang_validator().validate("foo:bar");
        // Should be valid but have a warning
        assert!(result.is_valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.level == DiagnosticLevel::Warning)
        );
    }

    #[test]
    fn test_enya_similar_key_suggestion() {
        let result = enya_lang_validator().validate("evn:prod"); // typo for "env"
        assert!(result.is_valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.contains("did you mean 'env'"))
        );
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("a", ""), 1);
        assert_eq!(levenshtein_distance("", "a"), 1);
        assert_eq!(levenshtein_distance("env", "evn"), 2);
        assert_eq!(levenshtein_distance("prod", "prod"), 0);
        assert_eq!(levenshtein_distance("prod", "pord"), 2);
    }

    #[test]
    fn test_enya_aggregation_query_valid() {
        let validator = enya_lang_validator();

        // Basic aggregation
        let result = validator.validate("sum(*)");
        assert!(result.is_valid);

        // Aggregation with filter
        let result = validator.validate("sum(env:prod)");
        assert!(result.is_valid);

        // Aggregation with by clause
        let result = validator.validate("sum(*) by (host)");
        assert!(result.is_valid);

        // Aggregation with without clause
        let result = validator.validate("avg(env:prod) without (instance)");
        assert!(result.is_valid);

        // Multiple labels in by clause
        let result = validator.validate("sum(*) by (host, region)");
        assert!(result.is_valid);
    }

    #[test]
    fn test_is_aggregation_query() {
        assert!(is_aggregation_query("sum(*)"));
        assert!(is_aggregation_query("avg(env:prod)"));
        assert!(is_aggregation_query("sum(*) by (host)"));
        assert!(is_aggregation_query("rate(requests)[5m]"));
        assert!(!is_aggregation_query("env:prod"));
        assert!(!is_aggregation_query("*"));
    }

    // ============ Language Switching Tests ============

    #[test]
    fn test_language_switching() {
        let mut validator = QueryValidator::new();
        assert_eq!(validator.language(), QueryLanguage::PromQL);

        validator.set_language(QueryLanguage::EnyaLang);
        assert_eq!(validator.language(), QueryLanguage::EnyaLang);

        validator.set_language(QueryLanguage::PromQL);
        assert_eq!(validator.language(), QueryLanguage::PromQL);
    }

    #[test]
    fn test_promql_default() {
        // validate_query should use PromQL by default
        let result = validate_query("rate(http_requests_total[5m])");
        assert!(result.is_valid);

        // EnyaLang "AND" syntax is invalid in PromQL mode
        let result = validate_query("env:prod AND");
        assert!(!result.is_valid);
    }
}
