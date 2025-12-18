//! Context-aware autocomplete for PromQL.
//!
//! This module provides context analysis for PromQL queries to power
//! intelligent autocomplete suggestions.

use crate::lexer::{
    BINARY_OPS, DURATION_SUGGESTIONS, KEYWORDS, LABEL_OPS, TokenHint, all_callables,
    last_token_before, partial_at_cursor, scan_until,
};

/// Completion context representing what the user is typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// Empty query or beginning of expression.
    Empty,

    /// Expecting a metric name or expression (after `(`, binary op, etc.).
    ExpectExpr,

    /// Typing a metric or function name.
    InName(String),

    /// Inside a label selector `{}`.
    InSelector,

    /// Typing a label name inside selector.
    InLabelName(String),

    /// After label name, expecting operator (=, !=, =~, !~).
    ExpectLabelOp,

    /// Typing a label value inside quotes.
    InLabelValue {
        /// The label key being matched.
        key: String,
        /// The partial value typed so far.
        partial: String,
    },

    /// After label value, expecting comma or closing brace.
    ExpectLabelCommaOrClose,

    /// Inside duration brackets `[]`.
    InDuration(String),

    /// After aggregation/function, expecting `by`/`without` or binary op.
    ExpectModifier,

    /// After `by`/`without`, expecting `(`.
    ExpectGroupingOpen,

    /// Inside grouping label list `by (...)`.
    InGroupingLabels,

    /// Typing a label name in grouping list.
    InGroupingLabelName(String),

    /// After complete expression, expecting binary operator or end.
    ExpectBinaryOp,

    /// After `@`, expecting `start()`, `end()`, or timestamp.
    ExpectAtModifier,
}

impl Default for Context {
    fn default() -> Self {
        Self::Empty
    }
}

/// Analyze the input and cursor position to determine completion context.
///
/// # Arguments
///
/// * `input` - The full query string
/// * `cursor` - The cursor position (byte offset)
///
/// # Returns
///
/// The completion context at the cursor position.
#[must_use]
pub fn analyze(input: &str, cursor: usize) -> Context {
    if input.is_empty() || cursor == 0 {
        return Context::Empty;
    }

    let cursor = cursor.min(input.len());
    let state = scan_until(input, cursor);
    let partial = partial_at_cursor(input, cursor);
    let last_token = last_token_before(input, cursor);

    // Inside a string - check if it's a label value
    if state.in_string {
        // Find the label key
        let before = &input[..cursor];
        if let Some(eq_pos) =
            before.rfind(|c| c == '=' || before.ends_with("=~") || before.ends_with("!~"))
        {
            // Look for the label name before the operator
            let before_op = &before[..eq_pos];
            let label_start = before_op
                .rfind(|c: char| c.is_whitespace() || c == '{' || c == ',')
                .map_or(0, |i| i + 1);
            let key = before_op[label_start..].trim().to_string();
            return Context::InLabelValue {
                key,
                partial: partial.to_string(),
            };
        }
    }

    // Inside duration brackets
    if state.in_duration() {
        return Context::InDuration(partial.to_string());
    }

    // Inside label selector
    if state.in_selector() {
        return analyze_selector_context(input, cursor, partial, last_token);
    }

    // Check what the last token was
    match last_token {
        None => {
            if partial.is_empty() {
                Context::Empty
            } else {
                Context::InName(partial.to_string())
            }
        }

        Some((_, TokenHint::OpenParen)) => {
            // Check if this open paren follows a grouping keyword
            if is_grouping_context(input, cursor) {
                if partial.is_empty() {
                    Context::InGroupingLabels
                } else {
                    Context::InGroupingLabelName(partial.to_string())
                }
            } else if partial.is_empty() {
                Context::ExpectExpr
            } else {
                Context::InName(partial.to_string())
            }
        }

        Some((_, TokenHint::CloseParen | TokenHint::CloseBracket)) => {
            if partial.is_empty() {
                Context::ExpectModifier
            } else {
                // Could be typing "by", "without", or an operator
                let lower = partial.to_lowercase();
                if "by".starts_with(&lower) || "without".starts_with(&lower) {
                    Context::InName(partial.to_string())
                } else {
                    Context::ExpectBinaryOp
                }
            }
        }

        Some((_, TokenHint::CloseBrace)) => {
            if partial.is_empty() {
                // After selector close - could add range, offset, or binary op
                Context::ExpectBinaryOp
            } else {
                Context::InName(partial.to_string())
            }
        }

        Some((word, TokenHint::Keyword)) => {
            let lower = word.to_lowercase();
            if lower == "by" || lower == "without" || lower == "on" || lower == "ignoring" {
                if partial.is_empty() {
                    Context::ExpectGroupingOpen
                } else {
                    Context::InGroupingLabelName(partial.to_string())
                }
            } else if lower == "offset" {
                Context::InDuration(partial.to_string())
            } else if partial.is_empty() {
                // After and/or/unless - expect expression
                Context::ExpectExpr
            } else {
                Context::InName(partial.to_string())
            }
        }

        Some((_, TokenHint::Function)) => {
            // After function name
            if partial.is_empty() {
                Context::ExpectExpr
            } else {
                // Still typing something after function
                Context::InName(partial.to_string())
            }
        }

        Some((_, TokenHint::Identifier)) => {
            // After an identifier (metric name)
            if partial.is_empty() {
                // Could be start of selector, range, or binary op
                Context::ExpectBinaryOp
            } else {
                Context::InName(partial.to_string())
            }
        }

        Some((_, TokenHint::Operator)) => {
            // After operator - expect expression
            if partial.is_empty() {
                Context::ExpectExpr
            } else {
                Context::InName(partial.to_string())
            }
        }

        Some((_, TokenHint::Comma)) => {
            // Context depends on where we are
            if state.in_parens() {
                // In grouping list or function args
                if partial.is_empty() {
                    Context::InGroupingLabels
                } else {
                    Context::InGroupingLabelName(partial.to_string())
                }
            } else if partial.is_empty() {
                Context::ExpectExpr
            } else {
                Context::InName(partial.to_string())
            }
        }

        Some((_, TokenHint::At)) => Context::ExpectAtModifier,

        Some((_, TokenHint::Duration | TokenHint::Number)) => {
            if partial.is_empty() {
                Context::ExpectBinaryOp
            } else {
                Context::InName(partial.to_string())
            }
        }

        _ => {
            if partial.is_empty() {
                Context::ExpectExpr
            } else {
                Context::InName(partial.to_string())
            }
        }
    }
}

/// Analyze context inside a label selector.
fn analyze_selector_context(
    input: &str,
    cursor: usize,
    partial: &str,
    last_token: Option<(&str, TokenHint)>,
) -> Context {
    match last_token {
        Some((_, TokenHint::OpenBrace | TokenHint::Comma)) => {
            if partial.is_empty() {
                Context::InSelector
            } else {
                Context::InLabelName(partial.to_string())
            }
        }

        Some((_, TokenHint::Identifier)) => {
            // After label name - expect operator
            if partial.is_empty() {
                Context::ExpectLabelOp
            } else {
                Context::InLabelName(partial.to_string())
            }
        }

        Some((_, TokenHint::LabelOp)) => {
            // After label op - expect value
            // Find the label key
            let before = &input[..cursor];
            let key = find_label_key(before);
            Context::InLabelValue {
                key,
                partial: partial.to_string(),
            }
        }

        Some((_, TokenHint::String)) => {
            // After string value
            Context::ExpectLabelCommaOrClose
        }

        _ => {
            if partial.is_empty() {
                Context::InSelector
            } else {
                Context::InLabelName(partial.to_string())
            }
        }
    }
}

/// Check if the context before an open paren is a grouping context (by/without/on/ignoring).
fn is_grouping_context(input: &str, cursor: usize) -> bool {
    let before = input[..cursor].trim_end();
    // The open paren should be the last character in trimmed content
    // Look for the word before it
    let before_paren = before.strip_suffix('(').unwrap_or(before).trim_end();

    // Find the last word
    let word_start = before_paren
        .rfind(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | ','))
        .map_or(0, |i| i + 1);

    let word = before_paren[word_start..].to_lowercase();
    matches!(word.as_str(), "by" | "without" | "on" | "ignoring")
}

/// Find the label key before the current position.
fn find_label_key(before: &str) -> String {
    // Look for the label name before the last operator
    let op_pos = before.rfind(['=', '!', '~']).unwrap_or(before.len());

    let before_op = &before[..op_pos];
    let label_start = before_op
        .rfind(|c: char| c.is_whitespace() || c == '{' || c == ',')
        .map_or(0, |i| i + 1);

    before_op[label_start..].trim().to_string()
}

/// Get syntax suggestions for a given context.
///
/// Returns static syntax suggestions. Dynamic suggestions (metric names,
/// label names/values) should be added by the caller.
#[must_use]
pub fn syntax_suggestions(ctx: &Context) -> Box<dyn Iterator<Item = &'static str> + '_> {
    match ctx {
        Context::Empty | Context::ExpectExpr => {
            // Suggest aggregations and functions
            Box::new(all_callables())
        }

        Context::InName(partial) => {
            let lower = partial.to_lowercase();
            let lower2 = lower.clone();
            // Filter functions and aggregations by prefix, then keywords
            Box::new(
                all_callables()
                    .filter(move |name| name.starts_with(&lower))
                    .chain(
                        KEYWORDS
                            .iter()
                            .copied()
                            .filter(move |kw| kw.starts_with(&lower2)),
                    ),
            )
        }

        Context::InSelector => {
            // Dynamic - caller should add label names
            // Suggest __name__ for metric name matching when no partial typed
            Box::new(std::iter::once("__name__"))
        }

        Context::InLabelName(partial) => {
            // Dynamic - caller should add label names
            // Filter __name__ by partial match
            let lower = partial.to_lowercase();
            if "__name__".starts_with(&lower) {
                Box::new(std::iter::once("__name__"))
            } else {
                Box::new(std::iter::empty())
            }
        }

        Context::ExpectLabelOp => Box::new(LABEL_OPS.iter().copied()),

        Context::InLabelValue { .. } => {
            // Dynamic - caller should add label values
            Box::new(std::iter::empty())
        }

        Context::ExpectLabelCommaOrClose => Box::new([",", "}"].into_iter()),

        Context::InDuration(_) => Box::new(DURATION_SUGGESTIONS.iter().copied()),

        Context::ExpectModifier => {
            // by, without, [, offset, then binary ops
            Box::new(
                ["by", "without", "[", "offset"]
                    .into_iter()
                    .chain(BINARY_OPS.iter().copied()),
            )
        }

        Context::ExpectGroupingOpen => Box::new(std::iter::once("(")),

        Context::InGroupingLabels | Context::InGroupingLabelName(_) => {
            // Dynamic - caller should add label names
            Box::new(std::iter::once(")"))
        }

        Context::ExpectBinaryOp => {
            // binary ops, [, {, offset, @
            Box::new(BINARY_OPS.iter().copied().chain(["[", "{", "offset", "@"]))
        }

        Context::ExpectAtModifier => Box::new(["start()", "end()"].into_iter()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_context() {
        assert_eq!(analyze("", 0), Context::Empty);
    }

    #[test]
    fn test_typing_name() {
        assert_eq!(analyze("http", 4), Context::InName("http".to_string()));
        assert_eq!(analyze("sum", 3), Context::InName("sum".to_string()));
        assert_eq!(analyze("rat", 3), Context::InName("rat".to_string()));
    }

    #[test]
    fn test_after_open_paren() {
        let ctx = analyze("rate(", 5);
        assert_eq!(ctx, Context::ExpectExpr);

        let ctx = analyze("rate(http", 9);
        assert_eq!(ctx, Context::InName("http".to_string()));
    }

    #[test]
    fn test_selector_context() {
        let ctx = analyze("http_requests{", 14);
        assert_eq!(ctx, Context::InSelector);

        let ctx = analyze("http_requests{method", 20);
        assert_eq!(ctx, Context::InLabelName("method".to_string()));
    }

    #[test]
    fn test_label_op_context() {
        let ctx = analyze("http_requests{method=", 21);
        assert!(
            matches!(ctx, Context::InLabelValue { key, partial } if key == "method" && partial.is_empty())
        );
    }

    #[test]
    fn test_duration_context() {
        let ctx = analyze("rate(x[", 7);
        assert_eq!(ctx, Context::InDuration(String::new()));

        let ctx = analyze("rate(x[5", 8);
        assert_eq!(ctx, Context::InDuration("5".to_string()));
    }

    #[test]
    fn test_after_close_paren() {
        let ctx = analyze("sum(x) ", 7);
        assert_eq!(ctx, Context::ExpectModifier);
    }

    #[test]
    fn test_grouping_context() {
        let ctx = analyze("sum(x) by ", 10);
        assert_eq!(ctx, Context::ExpectGroupingOpen);

        let ctx = analyze("sum(x) by (", 11);
        assert_eq!(ctx, Context::InGroupingLabels);
    }

    #[test]
    fn test_binary_op_context() {
        let ctx = analyze("a + ", 4);
        assert_eq!(ctx, Context::ExpectExpr);
    }

    #[test]
    fn test_suggestions_empty() {
        let suggestions: Vec<_> = syntax_suggestions(&Context::Empty).collect();
        assert!(suggestions.contains(&"sum"));
        assert!(suggestions.contains(&"rate"));
    }

    #[test]
    fn test_suggestions_label_op() {
        let suggestions: Vec<_> = syntax_suggestions(&Context::ExpectLabelOp).collect();
        assert!(suggestions.contains(&"="));
        assert!(suggestions.contains(&"!="));
        assert!(suggestions.contains(&"=~"));
        assert!(suggestions.contains(&"!~"));
    }

    #[test]
    fn test_suggestions_duration() {
        let suggestions: Vec<_> = syntax_suggestions(&Context::InDuration(String::new())).collect();
        assert!(suggestions.contains(&"5m"));
        assert!(suggestions.contains(&"1h"));
    }

    #[test]
    fn test_suggestions_filtered() {
        let ctx = Context::InName("ra".to_string());
        let suggestions: Vec<_> = syntax_suggestions(&ctx).collect();
        assert!(suggestions.contains(&"rate"));
        assert!(!suggestions.contains(&"sum"));
    }
}
