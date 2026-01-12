//! Basic LogQL query validation.
//!
//! Provides lightweight validation for LogQL queries without requiring
//! a full parser. Validates structure, balanced brackets, and basic syntax.

use crate::lexer::{is_callable, scan_until};

/// Result of validating a LogQL query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Query is valid (or at least has valid structure).
    Valid,
    /// Query has validation errors.
    Invalid(Vec<ValidationError>),
}

impl ValidationResult {
    /// Check if the result is valid.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Get the errors if invalid.
    #[must_use]
    pub fn errors(&self) -> &[ValidationError] {
        match self {
            Self::Valid => &[],
            Self::Invalid(errors) => errors,
        }
    }
}

/// A validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The error message.
    pub message: String,
    /// Optional position in the query where the error occurred.
    pub position: Option<usize>,
}

impl ValidationError {
    /// Create a new validation error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: None,
        }
    }

    /// Create a validation error with position.
    #[must_use]
    pub fn at_position(message: impl Into<String>, position: usize) -> Self {
        Self {
            message: message.into(),
            position: Some(position),
        }
    }
}

/// Validate a LogQL query.
///
/// This performs basic structural validation:
/// - Balanced parentheses, braces, and brackets
/// - Stream selector present (for log queries)
/// - String literals properly closed
/// - Basic function usage
///
/// Note: This is not a full parser and may accept some invalid queries.
#[must_use]
pub fn validate(input: &str) -> ValidationResult {
    let mut errors = Vec::new();

    // Empty query is technically valid (just won't return results)
    if input.trim().is_empty() {
        return ValidationResult::Valid;
    }

    // Scan the full input to check structural balance
    let state = scan_until(input, input.len());

    // Check for unclosed string
    if state.in_string {
        errors.push(ValidationError::new(format!(
            "Unclosed string literal (expected closing {})",
            state.string_delim
        )));
    }

    // Check for unbalanced brackets
    if state.paren_depth > 0 {
        errors.push(ValidationError::new(format!(
            "Unclosed parenthesis ({} unclosed)",
            state.paren_depth
        )));
    } else if state.paren_depth < 0 {
        errors.push(ValidationError::new("Extra closing parenthesis"));
    }

    if state.brace_depth > 0 {
        errors.push(ValidationError::new(format!(
            "Unclosed brace ({} unclosed)",
            state.brace_depth
        )));
    } else if state.brace_depth < 0 {
        errors.push(ValidationError::new("Extra closing brace"));
    }

    if state.bracket_depth > 0 {
        errors.push(ValidationError::new(format!(
            "Unclosed bracket ({} unclosed)",
            state.bracket_depth
        )));
    } else if state.bracket_depth < 0 {
        errors.push(ValidationError::new("Extra closing bracket"));
    }

    // Check for stream selector (log queries need one)
    if !input.contains('{') && !has_metric_reference(input) {
        errors.push(ValidationError::new(
            "Query should start with a stream selector {labels} or a metric aggregation",
        ));
    }

    // Check for empty stream selector
    if input.contains("{}") {
        errors.push(ValidationError::new(
            "Empty stream selector {} - add at least one label matcher",
        ));
    }

    // Validate function calls have proper structure
    validate_function_calls(input, &mut errors);

    if errors.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Invalid(errors)
    }
}

/// Check if the query starts with a metric-style reference (function call).
fn has_metric_reference(input: &str) -> bool {
    let trimmed = input.trim();

    // Find the first word
    let word_end = trimmed
        .find(|c: char| c.is_whitespace() || c == '(' || c == '{')
        .unwrap_or(trimmed.len());

    let first_word = &trimmed[..word_end];
    is_callable(first_word)
}

/// Validate function call structure.
fn validate_function_calls(input: &str, errors: &mut Vec<ValidationError>) {
    let mut chars = input.chars().peekable();
    let mut word = String::new();
    let mut pos = 0;

    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            if c == '(' && is_callable(&word) {
                // Function call - check it has content
                let func_name = word.clone();
                let paren_pos = pos;

                // Scan to find matching close paren
                let mut depth = 1;
                let mut has_content = false;
                let mut inner_pos = 0;

                for inner_c in chars.by_ref() {
                    inner_pos += 1;
                    match inner_c {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        c if !c.is_whitespace() => has_content = true,
                        _ => {}
                    }
                }

                if !has_content && depth == 0 {
                    errors.push(ValidationError::at_position(
                        format!("Function `{func_name}()` requires an argument"),
                        paren_pos,
                    ));
                }

                pos += inner_pos;
            }
            word.clear();
        }
        pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_queries() {
        assert!(validate("{app=\"nginx\"}").is_valid());
        assert!(validate("{app=\"nginx\"} |= \"error\"").is_valid());
        assert!(validate("{app=\"nginx\"} | json").is_valid());
        assert!(validate("rate({app=\"nginx\"}[5m])").is_valid());
        assert!(validate("sum(rate({app=\"nginx\"}[5m])) by (env)").is_valid());
    }

    #[test]
    fn test_empty_query() {
        assert!(validate("").is_valid());
        assert!(validate("   ").is_valid());
    }

    #[test]
    fn test_unclosed_string() {
        let result = validate("{app=\"nginx}");
        assert!(!result.is_valid());
        assert!(result.errors()[0].message.contains("Unclosed string"));
    }

    #[test]
    fn test_unbalanced_parens() {
        let result = validate("sum(rate({app=\"nginx\"}[5m])");
        assert!(!result.is_valid());
        assert!(result.errors()[0].message.contains("Unclosed parenthesis"));
    }

    #[test]
    fn test_unbalanced_braces() {
        let result = validate("{app=\"nginx\"");
        assert!(!result.is_valid());
        assert!(result.errors()[0].message.contains("Unclosed brace"));
    }

    #[test]
    fn test_empty_selector() {
        let result = validate("{}");
        assert!(!result.is_valid());
        assert!(result.errors()[0].message.contains("Empty stream selector"));
    }

    #[test]
    fn test_no_selector() {
        let result = validate("hello world");
        assert!(!result.is_valid());
        assert!(result.errors()[0].message.contains("stream selector"));
    }

    #[test]
    fn test_function_starts_query() {
        // This is valid - metric-style query starting with function
        assert!(validate("sum(rate({app=\"nginx\"}[5m]))").is_valid());
    }

    #[test]
    fn test_empty_function_call() {
        let result = validate("sum()");
        assert!(!result.is_valid());
        assert!(result.errors()[0].message.contains("requires an argument"));
    }
}
