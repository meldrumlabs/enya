//! Completion support for the query language.
//!
//! Provides context-aware completion suggestions based on cursor position.
//!
//! # Example
//!
//! ```
//! use enya_lang::completion::{analyze, Context, syntax_suggestions};
//!
//! let input = "env:prod AND ";
//! let ctx = analyze(input, input.len());
//! assert!(matches!(ctx, Context::ExpectExpr));
//!
//! let suggestions = syntax_suggestions(&ctx);
//! assert!(suggestions.contains(&"!"));
//! assert!(suggestions.contains(&"("));
//! ```

use crate::lexer::{Token, tokenize_filter_query};
use smallvec::{SmallVec, smallvec};

/// All aggregation function names.
pub const AGGREGATION_FUNCTIONS: &[&str] = &["sum", "avg", "min", "max", "count"];

/// All time-aware aggregation function names (require time range).
pub const TIME_AGGREGATION_FUNCTIONS: &[&str] = &[
    "rate",
    "irate",
    "increase",
    "avg_over_time",
    "sum_over_time",
    "min_over_time",
    "max_over_time",
    "count_over_time",
];

/// All function names (both regular and time-aware).
pub const ALL_FUNCTIONS: &[&str] = &[
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

/// Common duration suggestions for time ranges.
pub const DURATION_SUGGESTIONS: &[&str] =
    &["1m", "5m", "15m", "30m", "1h", "6h", "12h", "1d", "7d"];

/// Completion context indicating what type of input is expected at the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// Expecting the start of a query: aggregation function, identifier, `!`, `(`, or `*`.
    /// Occurs at start of input.
    ExpectQueryStart,

    /// Expecting an expression: identifier, `!`, `(`, or `*`.
    /// Occurs after `(`, after `AND`/`OR`, or after `!`.
    ExpectExpr,

    /// Expecting a binary operator (`AND`, `OR`) or `)` or `}`.
    /// Occurs after a complete identifier or after `)`.
    ExpectOperator,

    /// Expecting opening delimiter after aggregation function.
    /// Contains the function name.
    ExpectAggregationOpen(String),

    /// Expecting time range or grouping clause after closing aggregation.
    /// Contains the function name to determine if time range is required.
    ExpectTimeRangeOrGrouping(String),

    /// Expecting grouping clause (`by`, `without`) or end of query.
    /// Occurs after closing aggregation delimiter or after time range.
    ExpectGroupingOrEnd,

    /// Expecting opening paren after `by` or `without`.
    ExpectGroupingOpen,

    /// Inside a label list in a `by`/`without` clause.
    /// Expecting a label name.
    InLabelList,

    /// Expecting comma or closing paren in label list.
    ExpectLabelListContinue,

    /// Currently typing an aggregation function name.
    InAggregationFunc(String),

    /// Currently typing a label name in a by/without clause.
    InLabelName(String),

    /// Currently typing a tag key (before the `:`).
    /// Contains the partial key typed so far.
    InTagKey(String),

    /// Currently typing a tag value (after the `:`).
    /// Contains the key and the partial value typed so far.
    InTagValue {
        /// The complete tag key.
        key: String,
        /// The partial value typed so far.
        partial_value: String,
    },

    /// Expecting a duration value inside brackets.
    ExpectDuration,

    /// Currently typing a duration value inside brackets.
    InDuration(String),
}

/// Returns syntax-based completion suggestions for the given context.
///
/// These are the keywords and operators valid at the current position.
/// Tag keys and values should be provided separately from domain knowledge.
#[must_use]
pub fn syntax_suggestions(ctx: &Context) -> SmallVec<[&'static str; 16]> {
    match ctx {
        Context::ExpectQueryStart => {
            let mut suggestions: SmallVec<[&'static str; 16]> = smallvec!["!", "(", "*"];
            suggestions.extend(ALL_FUNCTIONS.iter().copied());
            suggestions
        }
        Context::ExpectExpr => smallvec!["!", "(", "{", "*"],
        Context::ExpectOperator => smallvec!["AND", "OR", ")", "}"],
        Context::ExpectAggregationOpen(_) => smallvec!["(", "{"],
        Context::ExpectTimeRangeOrGrouping(func) => {
            // For time-aware functions, suggest time range first
            if TIME_AGGREGATION_FUNCTIONS.contains(&func.as_str()) {
                smallvec!["["]
            } else {
                smallvec!["[", "by", "without"]
            }
        }
        Context::ExpectGroupingOrEnd => smallvec!["by", "without"],
        Context::ExpectGroupingOpen => smallvec!["("],
        Context::ExpectLabelListContinue => smallvec![",", ")"],
        Context::InAggregationFunc(partial) => ALL_FUNCTIONS
            .iter()
            .copied()
            .filter(|f| f.starts_with(partial.as_str()))
            .collect(),
        Context::ExpectDuration | Context::InDuration(_) => {
            DURATION_SUGGESTIONS.iter().copied().collect()
        }
        Context::InLabelList
        | Context::InLabelName(_)
        | Context::InTagKey(_)
        | Context::InTagValue { .. } => smallvec![],
    }
}

/// Analyzes the input up to the cursor position and determines the completion context.
///
/// # Arguments
///
/// * `input` - The full input string
/// * `cursor` - The cursor position (byte offset, must be `<= input.len()`)
///
/// # Returns
///
/// The [`Context`] indicating what type of completion is appropriate.
#[must_use]
pub fn analyze(input: &str, cursor: usize) -> Context {
    let cursor = cursor.min(input.len());
    let before_cursor = &input[..cursor];
    let after_cursor = &input[cursor..];

    // Check if we're in the middle of typing something
    if let Some(ctx) = check_partial_input(before_cursor, after_cursor) {
        return ctx;
    }

    // Otherwise, determine context from the token sequence
    context_from_tokens(before_cursor)
}

/// Checks if the cursor is in the middle of typing a partial identifier.
///
/// Returns `Some(Context)` if the cursor appears to be mid-word (typing),
/// or `None` if the cursor is at a token boundary.
fn check_partial_input(before_cursor: &str, after_cursor: &str) -> Option<Context> {
    // If there's trailing whitespace before cursor, we've finished the previous token
    if before_cursor.ends_with(char::is_whitespace) {
        return None;
    }

    let trimmed = before_cursor.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    // Check if we're inside a duration bracket
    if let Some(bracket_pos) = trimmed.rfind('[') {
        // Check if there's a closing bracket after the opening one
        let after_bracket = &trimmed[bracket_pos + 1..];
        if !after_bracket.contains(']') {
            // We're inside an unclosed bracket - duration context
            let partial = after_bracket.trim();
            if partial.is_empty() {
                return Some(Context::ExpectDuration);
            }
            return Some(Context::InDuration(partial.to_string()));
        }
    }

    // Find the start of the current "word" (not whitespace, not operator chars)
    let word_start = trimmed
        .rfind(|c: char| {
            c.is_whitespace()
                || c == '('
                || c == ')'
                || c == '{'
                || c == '}'
                || c == ','
                || c == '['
                || c == ']'
        })
        .map_or(0, |i| i + 1);

    let current_word = &trimmed[word_start..];

    // Skip if it's an operator keyword or empty
    if matches!(
        current_word,
        "AND" | "OR" | "!" | "(" | ")" | "{" | "}" | "," | "[" | "]" | "*" | ""
    ) {
        return None;
    }

    // Check what comes after the cursor to determine if we're mid-word
    let next_char = after_cursor.chars().next();
    let at_word_boundary = next_char.is_none_or(|c| {
        c.is_whitespace()
            || c == '('
            || c == ')'
            || c == '{'
            || c == '}'
            || c == ','
            || c == '['
            || c == ']'
    });

    // Check if this is a complete function/keyword - let context_from_tokens handle it
    let is_complete_keyword =
        ALL_FUNCTIONS.contains(&current_word) || matches!(current_word, "by" | "without");
    if is_complete_keyword {
        return None;
    }

    // Check if we're typing an aggregation function (partial match)
    if is_partial_aggregation_func(current_word) && !current_word.contains(':') {
        return Some(Context::InAggregationFunc(current_word.to_string()));
    }

    // Check if we're in a label list context (after by/without and opening paren)
    if is_in_label_list_context(trimmed)
        && is_valid_label_char(current_word)
        && !current_word.contains(':')
    {
        // If we're at the end of input with a label, the user might still be typing
        // So return InLabelName for completions
        // But if there's actual content after the cursor (like a comma or paren), treat as complete
        let has_content_after = !after_cursor.trim_start().is_empty();
        if has_content_after {
            return None;
        }
        return Some(Context::InLabelName(current_word.to_string()));
    }

    // Check if we have a colon (key:value pattern)
    if let Some(colon_pos) = current_word.find(':') {
        let key = &current_word[..colon_pos];
        let partial_value = &current_word[colon_pos + 1..];

        // Empty value (just "key:") is always partial - user just typed the colon
        if partial_value.is_empty() {
            return Some(Context::InTagValue {
                key: key.to_string(),
                partial_value: partial_value.to_string(),
            });
        }

        // If we're in the middle of a word (non-whitespace follows), it's partial
        if !at_word_boundary {
            return Some(Context::InTagValue {
                key: key.to_string(),
                partial_value: partial_value.to_string(),
            });
        }

        // At word boundary with complete identifier - treat as complete token
        if is_complete_identifier(current_word) {
            return None;
        }

        // Otherwise it's partial
        return Some(Context::InTagValue {
            key: key.to_string(),
            partial_value: partial_value.to_string(),
        });
    }

    // Typing a key (no colon yet)
    // Note: Complete keywords and partial aggregation functions are already handled above
    if !current_word.is_empty() && is_valid_key_char(current_word) {
        return Some(Context::InTagKey(current_word.to_string()));
    }

    None
}

/// Check if word is a partial aggregation function name.
fn is_partial_aggregation_func(word: &str) -> bool {
    ALL_FUNCTIONS
        .iter()
        .any(|f| f.starts_with(word) && *f != word)
}

/// Check if we're inside a label list (after `by (` or `without (`).
fn is_in_label_list_context(input: &str) -> bool {
    // Look for `by (` or `without (` followed by optional labels and commas
    let input_lower = input.to_lowercase();

    // Find the last occurrence of `by (` or `without (`
    let by_pos = input_lower
        .rfind("by (")
        .or_else(|| input_lower.rfind("by("));
    let without_pos = input_lower
        .rfind("without (")
        .or_else(|| input_lower.rfind("without("));

    let grouping_start = match (by_pos, without_pos) {
        (Some(b), Some(w)) => Some(b.max(w)),
        (Some(b), None) => Some(b),
        (None, Some(w)) => Some(w),
        (None, None) => None,
    };

    if let Some(start) = grouping_start {
        // Check if we have an unclosed paren after the grouping keyword
        let after_grouping = &input[start..];
        let open_count = after_grouping.chars().filter(|&c| c == '(').count();
        let close_count = after_grouping.chars().filter(|&c| c == ')').count();
        return open_count > close_count;
    }

    false
}

/// Check if string contains only valid label characters.
fn is_valid_label_char(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Checks if a word is a complete identifier (matches the lexer's identifier pattern).
fn is_complete_identifier(word: &str) -> bool {
    // Try to tokenize just this word - if it produces exactly one Identifier or Wildcard token,
    // it's complete
    let tokens: Vec<_> = tokenize_filter_query(word).flatten().collect();
    matches!(
        tokens.as_slice(),
        [Token::Identifier(_) | Token::Wildcard(_)]
    )
}

/// Checks if the string contains only valid tag key characters.
fn is_valid_key_char(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Determines context based on the sequence of complete tokens.
fn context_from_tokens(before_cursor: &str) -> Context {
    let trimmed = before_cursor.trim_end();

    // Empty input -> expect query start (could be aggregation or filter)
    if trimmed.is_empty() {
        return Context::ExpectQueryStart;
    }

    // Tokenize all tokens to understand the full context
    let tokens: Vec<_> = tokenize_filter_query(trimmed).flatten().collect();

    if tokens.is_empty() {
        return Context::ExpectQueryStart;
    }

    // Find the aggregation function if present
    let aggregation_func = tokens.iter().find_map(|t| match t {
        Token::Sum => Some("sum"),
        Token::Avg => Some("avg"),
        Token::Min => Some("min"),
        Token::Max => Some("max"),
        Token::Count => Some("count"),
        Token::Rate => Some("rate"),
        Token::Irate => Some("irate"),
        Token::Increase => Some("increase"),
        Token::AvgOverTime => Some("avg_over_time"),
        Token::SumOverTime => Some("sum_over_time"),
        Token::MinOverTime => Some("min_over_time"),
        Token::MaxOverTime => Some("max_over_time"),
        Token::CountOverTime => Some("count_over_time"),
        _ => None,
    });

    let has_aggregation_func = aggregation_func.is_some();

    let last_token = tokens.last();

    // Check if we're inside a label list
    if is_in_label_list_context(trimmed) {
        return match last_token {
            Some(Token::Label(_)) => Context::ExpectLabelListContinue,
            _ => Context::InLabelList,
        };
    }

    // Check if we have a time range already
    let has_time_range = tokens.iter().any(|t| matches!(t, Token::Duration(_)));

    match last_token {
        // After aggregation function -> expect opening delimiter
        Some(
            Token::Sum
            | Token::Avg
            | Token::Min
            | Token::Max
            | Token::Count
            | Token::Rate
            | Token::Irate
            | Token::Increase
            | Token::AvgOverTime
            | Token::SumOverTime
            | Token::MinOverTime
            | Token::MaxOverTime
            | Token::CountOverTime,
        ) => {
            let func_name = match last_token {
                Some(Token::Sum) => "sum",
                Some(Token::Avg) => "avg",
                Some(Token::Min) => "min",
                Some(Token::Max) => "max",
                Some(Token::Count) => "count",
                Some(Token::Rate) => "rate",
                Some(Token::Irate) => "irate",
                Some(Token::Increase) => "increase",
                Some(Token::AvgOverTime) => "avg_over_time",
                Some(Token::SumOverTime) => "sum_over_time",
                Some(Token::MinOverTime) => "min_over_time",
                Some(Token::MaxOverTime) => "max_over_time",
                Some(Token::CountOverTime) => "count_over_time",
                _ => "",
            };
            Context::ExpectAggregationOpen(func_name.to_string())
        }

        // After by/without -> expect opening paren
        Some(Token::By | Token::Without) => Context::ExpectGroupingOpen,

        // After opening bracket -> expect duration
        Some(Token::BracketOpen) => Context::ExpectDuration,

        // After closing bracket (time range) -> expect grouping or end
        Some(Token::BracketClose) if has_aggregation_func => Context::ExpectGroupingOrEnd,

        // After closing paren/brace in aggregation context -> expect time range or grouping
        Some(Token::ParenClose | Token::BraceClose) if has_aggregation_func => {
            // Check if we already have a grouping clause
            let has_grouping = tokens
                .iter()
                .any(|t| matches!(t, Token::By | Token::Without));
            if has_grouping {
                // After closing paren of label list, we're done
                Context::ExpectOperator
            } else if has_time_range {
                // Already have time range, just grouping left
                Context::ExpectGroupingOrEnd
            } else if let Some(func) = aggregation_func {
                // After closing aggregation, might need time range
                Context::ExpectTimeRangeOrGrouping(func.to_string())
            } else {
                Context::ExpectGroupingOrEnd
            }
        }

        // After identifier/wildcard/duration/closing brackets outside aggregation -> expect operator
        Some(
            Token::Identifier(_)
            | Token::Wildcard(_)
            | Token::ParenClose
            | Token::BraceClose
            | Token::BracketClose
            | Token::Label(_)
            | Token::Duration(_),
        ) => Context::ExpectOperator,

        // After AND/OR/!/( /{ -> expect expression
        Some(Token::And | Token::Or | Token::Not | Token::ParenOpen | Token::BraceOpen) => {
            Context::ExpectExpr
        }

        // After comma in label list -> expect label
        Some(Token::Comma) => Context::InLabelList,

        None => Context::ExpectQueryStart,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(analyze("", 0), Context::ExpectQueryStart);
    }

    #[test]
    fn test_after_identifier() {
        assert_eq!(analyze("env:prod", 8), Context::ExpectOperator);
        assert_eq!(analyze("env:prod ", 9), Context::ExpectOperator);
    }

    #[test]
    fn test_after_and() {
        assert_eq!(analyze("env:prod AND", 12), Context::ExpectExpr);
        assert_eq!(analyze("env:prod AND ", 13), Context::ExpectExpr);
    }

    #[test]
    fn test_after_or() {
        assert_eq!(analyze("env:prod OR", 11), Context::ExpectExpr);
        assert_eq!(analyze("env:prod OR ", 12), Context::ExpectExpr);
    }

    #[test]
    fn test_after_not() {
        assert_eq!(analyze("!", 1), Context::ExpectExpr);
        assert_eq!(analyze("! ", 2), Context::ExpectExpr);
    }

    #[test]
    fn test_after_open_paren() {
        assert_eq!(analyze("(", 1), Context::ExpectExpr);
        assert_eq!(analyze("( ", 2), Context::ExpectExpr);
    }

    #[test]
    fn test_after_close_paren() {
        assert_eq!(analyze("(env:prod)", 10), Context::ExpectOperator);
        assert_eq!(analyze("(env:prod) ", 11), Context::ExpectOperator);
    }

    #[test]
    fn test_typing_key() {
        assert_eq!(
            analyze("env:prod AND ser", 16),
            Context::InTagKey("ser".to_string())
        );
    }

    #[test]
    fn test_typing_value() {
        // Just typed colon - definitely partial
        assert_eq!(
            analyze("env:", 4),
            Context::InTagValue {
                key: "env".to_string(),
                partial_value: String::new()
            }
        );

        // Complete identifier at end of input -> expect operator
        // (both "env:pr" and "env:prod" are valid complete identifiers)
        assert_eq!(analyze("env:pr", 6), Context::ExpectOperator);
        assert_eq!(analyze("env:prod", 8), Context::ExpectOperator);
    }

    #[test]
    fn test_typing_value_after_and() {
        // Just typed colon after AND
        assert_eq!(
            analyze("env:prod AND service:", 21),
            Context::InTagValue {
                key: "service".to_string(),
                partial_value: String::new()
            }
        );

        // Complete identifier at end -> expect operator
        assert_eq!(
            analyze("env:prod AND service:db", 23),
            Context::ExpectOperator
        );
    }

    #[test]
    fn test_complex_expression() {
        // After complete nested expression
        assert_eq!(
            analyze("(env:prod OR env:staging)", 25),
            Context::ExpectOperator
        );
        // Typing after AND in complex expression
        assert_eq!(
            analyze("(env:prod OR env:staging) AND ", 30),
            Context::ExpectExpr
        );
    }

    #[test]
    fn test_syntax_suggestions_expect_expr() {
        let suggestions = syntax_suggestions(&Context::ExpectExpr);
        assert!(suggestions.contains(&"!"));
        assert!(suggestions.contains(&"("));
        assert!(suggestions.contains(&"*"));
        assert!(!suggestions.contains(&"AND"));
    }

    #[test]
    fn test_syntax_suggestions_expect_operator() {
        let suggestions = syntax_suggestions(&Context::ExpectOperator);
        assert!(suggestions.contains(&"AND"));
        assert!(suggestions.contains(&"OR"));
        assert!(suggestions.contains(&")"));
        assert!(!suggestions.contains(&"!"));
    }

    #[test]
    fn test_syntax_suggestions_in_tag() {
        let suggestions = syntax_suggestions(&Context::InTagKey("env".to_string()));
        assert!(suggestions.is_empty());

        let suggestions = syntax_suggestions(&Context::InTagValue {
            key: "env".to_string(),
            partial_value: "pr".to_string(),
        });
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_wildcard_identifier() {
        assert_eq!(analyze("service:db.*", 12), Context::ExpectOperator);
        assert_eq!(analyze("service:db.* ", 13), Context::ExpectOperator);
    }

    #[test]
    fn test_cursor_in_middle() {
        // Cursor in middle of "prod" - should detect partial value
        assert_eq!(
            analyze("env:prod AND service:db", 6),
            Context::InTagValue {
                key: "env".to_string(),
                partial_value: "pr".to_string()
            }
        );
    }

    #[test]
    fn test_star_alone() {
        // After * (match all), expect expression (star is not a recognized token)
        assert_eq!(analyze("*", 1), Context::ExpectQueryStart);
    }

    // === New aggregation-related tests ===

    #[test]
    fn test_typing_aggregation_func() {
        // Typing "su" at start should suggest aggregation functions
        assert_eq!(
            analyze("su", 2),
            Context::InAggregationFunc("su".to_string())
        );
        assert_eq!(
            analyze("av", 2),
            Context::InAggregationFunc("av".to_string())
        );
    }

    #[test]
    fn test_after_aggregation_func() {
        assert_eq!(
            analyze("sum", 3),
            Context::ExpectAggregationOpen("sum".to_string())
        );
        assert_eq!(
            analyze("sum ", 4),
            Context::ExpectAggregationOpen("sum".to_string())
        );
        assert_eq!(
            analyze("avg", 3),
            Context::ExpectAggregationOpen("avg".to_string())
        );
    }

    #[test]
    fn test_after_aggregation_open_paren() {
        assert_eq!(analyze("sum(", 4), Context::ExpectExpr);
        assert_eq!(analyze("sum( ", 5), Context::ExpectExpr);
        assert_eq!(analyze("avg{", 4), Context::ExpectExpr);
    }

    #[test]
    fn test_inside_aggregation() {
        assert_eq!(analyze("sum(env:prod", 12), Context::ExpectOperator);
        assert_eq!(analyze("sum(env:prod AND ", 17), Context::ExpectExpr);
    }

    #[test]
    #[allow(clippy::literal_string_with_formatting_args)]
    fn test_after_aggregation_close() {
        // Regular aggregation functions expect time range or grouping
        assert_eq!(
            analyze("sum(env:prod)", 13),
            Context::ExpectTimeRangeOrGrouping("sum".to_string())
        );
        assert_eq!(
            analyze("sum(env:prod) ", 14),
            Context::ExpectTimeRangeOrGrouping("sum".to_string())
        );
        // Test with brace syntax - "avg{env:prod}"
        assert_eq!(
            analyze("avg{env:prod}", 13),
            Context::ExpectTimeRangeOrGrouping("avg".to_string())
        );
    }

    #[test]
    fn test_after_by_keyword() {
        assert_eq!(analyze("sum(env:prod) by", 16), Context::ExpectGroupingOpen);
        assert_eq!(
            analyze("sum(env:prod) by ", 17),
            Context::ExpectGroupingOpen
        );
    }

    #[test]
    fn test_after_without_keyword() {
        assert_eq!(
            analyze("sum(env:prod) without", 21),
            Context::ExpectGroupingOpen
        );
    }

    #[test]
    fn test_in_label_list() {
        assert_eq!(analyze("sum(env:prod) by (", 18), Context::InLabelList);
        assert_eq!(analyze("sum(env:prod) by ( ", 19), Context::InLabelList);
    }

    #[test]
    fn test_typing_label_name() {
        assert_eq!(
            analyze("sum(env:prod) by (reg", 21),
            Context::InLabelName("reg".to_string())
        );
    }

    #[test]
    fn test_after_label_in_list() {
        // Label at end of input - user might still be typing, so offer completions
        assert_eq!(
            analyze("sum(env:prod) by (region", 24),
            Context::InLabelName("region".to_string())
        );
        // With space after, the label is complete
        assert_eq!(
            analyze("sum(env:prod) by (region ", 25),
            Context::ExpectLabelListContinue
        );
    }

    #[test]
    fn test_after_comma_in_label_list() {
        assert_eq!(
            analyze("sum(env:prod) by (region,", 25),
            Context::InLabelList
        );
        assert_eq!(
            analyze("sum(env:prod) by (region, ", 26),
            Context::InLabelList
        );
    }

    #[test]
    fn test_typing_second_label() {
        assert_eq!(
            analyze("sum(env:prod) by (region, ser", 29),
            Context::InLabelName("ser".to_string())
        );
    }

    #[test]
    fn test_after_label_list_close() {
        // After closing the label list, we're done
        assert_eq!(
            analyze("sum(env:prod) by (region)", 25),
            Context::ExpectOperator
        );
    }

    #[test]
    fn test_syntax_suggestions_query_start() {
        let suggestions = syntax_suggestions(&Context::ExpectQueryStart);
        assert!(suggestions.contains(&"sum"));
        assert!(suggestions.contains(&"avg"));
        assert!(suggestions.contains(&"min"));
        assert!(suggestions.contains(&"max"));
        assert!(suggestions.contains(&"count"));
        assert!(suggestions.contains(&"!"));
        assert!(suggestions.contains(&"("));
    }

    #[test]
    fn test_syntax_suggestions_aggregation_open() {
        let suggestions = syntax_suggestions(&Context::ExpectAggregationOpen("sum".to_string()));
        assert!(suggestions.contains(&"("));
        assert!(suggestions.contains(&"{"));
    }

    #[test]
    fn test_syntax_suggestions_grouping_or_end() {
        let suggestions = syntax_suggestions(&Context::ExpectGroupingOrEnd);
        assert!(suggestions.contains(&"by"));
        assert!(suggestions.contains(&"without"));
    }

    #[test]
    fn test_syntax_suggestions_label_list_continue() {
        let suggestions = syntax_suggestions(&Context::ExpectLabelListContinue);
        assert!(suggestions.contains(&","));
        assert!(suggestions.contains(&")"));
    }

    #[test]
    fn test_partial_aggregation_func_suggestions() {
        let suggestions = syntax_suggestions(&Context::InAggregationFunc("su".to_string()));
        assert!(suggestions.contains(&"sum"));
        assert!(suggestions.contains(&"sum_over_time"));
        assert!(!suggestions.contains(&"avg"));
    }

    // === Time range tests ===

    #[test]
    fn test_time_aware_function_typing() {
        // Typing time-aware function names
        assert_eq!(
            analyze("ra", 2),
            Context::InAggregationFunc("ra".to_string())
        );
        assert_eq!(
            analyze("rate", 4),
            Context::ExpectAggregationOpen("rate".to_string())
        );
        assert_eq!(
            analyze("avg_over", 8),
            Context::InAggregationFunc("avg_over".to_string())
        );
        assert_eq!(
            analyze("avg_over_time", 13),
            Context::ExpectAggregationOpen("avg_over_time".to_string())
        );
    }

    #[test]
    fn test_after_time_aware_aggregation_close() {
        // Time-aware functions expect time range
        assert_eq!(
            analyze("rate(env:prod)", 14),
            Context::ExpectTimeRangeOrGrouping("rate".to_string())
        );
        assert_eq!(
            analyze("avg_over_time(env:prod)", 23),
            Context::ExpectTimeRangeOrGrouping("avg_over_time".to_string())
        );
    }

    #[test]
    fn test_time_range_bracket_open() {
        assert_eq!(analyze("rate(env:prod)[", 15), Context::ExpectDuration);
        assert_eq!(analyze("rate(env:prod)[ ", 16), Context::ExpectDuration);
    }

    #[test]
    fn test_typing_duration() {
        assert_eq!(
            analyze("rate(env:prod)[5", 16),
            Context::InDuration("5".to_string())
        );
        assert_eq!(
            analyze("rate(env:prod)[5m", 17),
            Context::InDuration("5m".to_string())
        );
    }

    #[test]
    fn test_after_time_range_close() {
        assert_eq!(
            analyze("rate(env:prod)[5m]", 18),
            Context::ExpectGroupingOrEnd
        );
        assert_eq!(
            analyze("rate(env:prod)[5m] ", 19),
            Context::ExpectGroupingOrEnd
        );
    }

    #[test]
    fn test_time_range_with_grouping() {
        assert_eq!(
            analyze("rate(env:prod)[5m] by", 21),
            Context::ExpectGroupingOpen
        );
        assert_eq!(
            analyze("rate(env:prod)[5m] by (region)", 30),
            Context::ExpectOperator
        );
    }

    #[test]
    fn test_syntax_suggestions_time_range_required() {
        // Time-aware functions should suggest `[` for time range
        let suggestions =
            syntax_suggestions(&Context::ExpectTimeRangeOrGrouping("rate".to_string()));
        assert!(suggestions.contains(&"["));
        assert!(!suggestions.contains(&"by")); // Must have time range first
    }

    #[test]
    fn test_syntax_suggestions_time_range_optional() {
        // Regular aggregations can have optional time range or go straight to grouping
        let suggestions =
            syntax_suggestions(&Context::ExpectTimeRangeOrGrouping("sum".to_string()));
        assert!(suggestions.contains(&"["));
        assert!(suggestions.contains(&"by"));
        assert!(suggestions.contains(&"without"));
    }

    #[test]
    fn test_syntax_suggestions_duration() {
        let suggestions = syntax_suggestions(&Context::ExpectDuration);
        assert!(suggestions.contains(&"1m"));
        assert!(suggestions.contains(&"5m"));
        assert!(suggestions.contains(&"1h"));
        assert!(suggestions.contains(&"1d"));
    }

    #[test]
    fn test_query_start_includes_time_functions() {
        let suggestions = syntax_suggestions(&Context::ExpectQueryStart);
        assert!(suggestions.contains(&"rate"));
        assert!(suggestions.contains(&"irate"));
        assert!(suggestions.contains(&"increase"));
        assert!(suggestions.contains(&"avg_over_time"));
        assert!(suggestions.contains(&"sum_over_time"));
    }
}
