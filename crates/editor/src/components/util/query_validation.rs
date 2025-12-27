//! Query validation module - validates PromQL queries and produces diagnostics.
//!
//! Provides syntax validation via the enya-promql parser.

use crate::components::overlay::diagnostics::{Diagnostic, DiagnosticLevel, DiagnosticSource};

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

/// Validate a PromQL query and return diagnostics
pub fn validate_query(query: &str) -> ValidationResult {
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

/// Convenience function to check if a query is valid
pub fn is_valid_query(query: &str) -> bool {
    validate_query(query).is_valid
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
