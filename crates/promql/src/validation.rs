//! PromQL query validation.
//!
//! This module wraps the `promql-parser` crate to provide validation
//! with best-effort error position estimation.

/// Result of validating a PromQL query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    /// Whether the query is syntactically valid.
    pub is_valid: bool,
    /// Error message if invalid.
    pub error: Option<String>,
    /// Best-effort error position (byte offset) if determinable.
    pub error_position: Option<usize>,
}

impl ValidationResult {
    /// Create a successful validation result.
    #[must_use]
    pub const fn valid() -> Self {
        Self {
            is_valid: true,
            error: None,
            error_position: None,
        }
    }

    /// Create a failed validation result.
    #[must_use]
    pub fn invalid(error: String, position: Option<usize>) -> Self {
        Self {
            is_valid: false,
            error: Some(error),
            error_position: position,
        }
    }
}

/// Validate a PromQL query.
///
/// Uses the `promql-parser` crate for parsing and returns a validation result
/// with the error message and best-effort position estimation.
///
/// # Arguments
///
/// * `query` - The PromQL query string to validate
///
/// # Returns
///
/// A `ValidationResult` indicating whether the query is valid.
///
/// # Example
///
/// ```
/// use enya_promql::validate;
///
/// let result = validate("rate(http_requests_total[5m])");
/// assert!(result.is_valid);
///
/// let result = validate("rate(http_requests_total[])");
/// assert!(!result.is_valid);
/// assert!(result.error.is_some());
/// ```
#[must_use]
pub fn validate(query: &str) -> ValidationResult {
    if query.trim().is_empty() {
        return ValidationResult::invalid("empty query".to_string(), Some(0));
    }

    match promql_parser::parser::parse(query) {
        Ok(_) => ValidationResult::valid(),
        Err(e) => {
            let position = estimate_error_position(query, &e);
            ValidationResult::invalid(e, position)
        }
    }
}

/// Best-effort estimation of error position from error message.
///
/// The `promql-parser` crate doesn't provide position info in errors,
/// so we try to extract hints from the error message.
fn estimate_error_position(query: &str, error: &str) -> Option<usize> {
    let error_lower = error.to_lowercase();

    // Try to find position hints in error messages
    // Common patterns: "at position X", "at line Y column Z", "unexpected X at Y"

    // Pattern: "at position N"
    if let Some(idx) = error_lower.find("at position ") {
        let rest = &error[idx + 12..];
        if let Some(num_str) = rest.split_whitespace().next() {
            if let Ok(pos) = num_str.parse::<usize>() {
                return Some(pos.min(query.len()));
            }
        }
    }

    // Pattern: "column N" or "col N"
    if let Some(idx) = error_lower
        .find("column ")
        .or_else(|| error_lower.find("col "))
    {
        let start = if error_lower[idx..].starts_with("column ") {
            idx + 7
        } else {
            idx + 4
        };
        let rest = &error[start..];
        if let Some(num_str) = rest.split_whitespace().next() {
            if let Ok(col) = num_str.parse::<usize>() {
                // Column is usually 1-indexed
                return Some((col.saturating_sub(1)).min(query.len()));
            }
        }
    }

    // Pattern: Look for quoted tokens in error and find them in query
    // e.g., "unexpected token '}'" - find the position of '}'
    for quote in ['\'', '"'] {
        if let Some(start) = error.find(quote) {
            if let Some(end) = error[start + 1..].find(quote) {
                let token = &error[start + 1..start + 1 + end];
                if !token.is_empty() && token.len() < 20 {
                    // Find this token in the query (from the end, as errors usually occur late)
                    if let Some(pos) = query.rfind(token) {
                        return Some(pos);
                    }
                }
            }
        }
    }

    // If we have "unexpected end" or "EOF", point to end
    if error_lower.contains("unexpected end")
        || error_lower.contains("eof")
        || error_lower.contains("end of input")
    {
        return Some(query.len());
    }

    // If we have "expected X" without position, point to end
    if error_lower.contains("expected") {
        return Some(query.len());
    }

    // Default: no position info
    None
}

/// Validate and return the parsed AST if valid.
///
/// This is useful when you need both validation and the parsed expression.
///
/// # Arguments
///
/// * `query` - The PromQL query string to validate
///
/// # Returns
///
/// `Ok(Expr)` if valid, `Err(ValidationResult)` if invalid.
pub fn validate_and_parse(query: &str) -> Result<promql_parser::parser::Expr, ValidationResult> {
    if query.trim().is_empty() {
        return Err(ValidationResult::invalid(
            "empty query".to_string(),
            Some(0),
        ));
    }

    promql_parser::parser::parse(query).map_err(|e| {
        let position = estimate_error_position(query, &e);
        ValidationResult::invalid(e, position)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple_metric() {
        let result = validate("http_requests_total");
        assert!(result.is_valid);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_valid_selector() {
        let result = validate(r#"http_requests_total{method="GET"}"#);
        assert!(result.is_valid);
    }

    #[test]
    fn test_valid_rate() {
        let result = validate("rate(http_requests_total[5m])");
        assert!(result.is_valid);
    }

    #[test]
    fn test_valid_aggregation() {
        let result = validate("sum(rate(http_requests_total[5m])) by (job)");
        assert!(result.is_valid);
    }

    #[test]
    fn test_valid_binary_op() {
        let result = validate("http_requests_total / http_requests_total");
        assert!(result.is_valid);
    }

    #[test]
    fn test_valid_complex() {
        let result = validate(
            r#"sum(rate(http_requests_total{job="api"}[5m])) by (method) / ignoring(method) sum(rate(http_requests_total[5m]))"#,
        );
        assert!(result.is_valid);
    }

    #[test]
    fn test_invalid_empty() {
        let result = validate("");
        assert!(!result.is_valid);
        assert_eq!(result.error, Some("empty query".to_string()));
    }

    #[test]
    fn test_invalid_whitespace_only() {
        let result = validate("   ");
        assert!(!result.is_valid);
    }

    #[test]
    fn test_invalid_unclosed_brace() {
        let result = validate("http_requests_total{");
        assert!(!result.is_valid);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_invalid_unclosed_bracket() {
        let result = validate("rate(http_requests_total[5m)");
        assert!(!result.is_valid);
    }

    #[test]
    fn test_invalid_missing_duration() {
        let result = validate("rate(http_requests_total[])");
        assert!(!result.is_valid);
    }

    #[test]
    fn test_invalid_unknown_function() {
        let result = validate("unknown_func(x)");
        assert!(!result.is_valid);
    }

    #[test]
    fn test_validate_and_parse_valid() {
        let result = validate_and_parse("http_requests_total");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_and_parse_invalid() {
        let result = validate_and_parse("http_requests_total{");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_position_estimation() {
        // Test various error messages
        assert_eq!(
            estimate_error_position("test", "error at position 5"),
            Some(4)
        );
        assert_eq!(
            estimate_error_position("test query", "unexpected end"),
            Some(10)
        );
    }
}
