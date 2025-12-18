//! Lightweight scanner for PromQL autocomplete.
//!
//! This module provides simple character-based scanning for autocomplete context
//! detection. We don't need a full lexer since `promql-parser` handles actual parsing.

/// Scan state tracking nesting depth and context.
#[derive(Debug, Clone, Default)]
pub struct ScanState {
    /// Depth of parentheses nesting.
    pub paren_depth: i32,
    /// Depth of brace nesting (label selectors).
    pub brace_depth: i32,
    /// Depth of bracket nesting (durations).
    pub bracket_depth: i32,
    /// Whether we're inside a string.
    pub in_string: bool,
    /// The string delimiter if in_string is true.
    pub string_delim: char,
}

impl ScanState {
    /// Check if we're inside a label selector `{}`.
    #[must_use]
    pub const fn in_selector(&self) -> bool {
        self.brace_depth > 0
    }

    /// Check if we're inside a duration bracket `[]`.
    #[must_use]
    pub const fn in_duration(&self) -> bool {
        self.bracket_depth > 0
    }

    /// Check if we're inside parentheses.
    #[must_use]
    pub const fn in_parens(&self) -> bool {
        self.paren_depth > 0
    }
}

/// Scan input up to cursor position and return the state.
#[must_use]
pub fn scan_until(input: &str, cursor: usize) -> ScanState {
    let mut state = ScanState::default();
    let cursor = cursor.min(input.len());

    let mut chars = input[..cursor].chars();
    while let Some(c) = chars.next() {
        if state.in_string {
            if c == '\\' {
                // Skip escaped character
                chars.next();
            } else if c == state.string_delim {
                state.in_string = false;
            }
            continue;
        }

        match c {
            '"' | '\'' | '`' => {
                state.in_string = true;
                state.string_delim = c;
            }
            '(' => state.paren_depth += 1,
            ')' => state.paren_depth = (state.paren_depth - 1).max(0),
            '{' => state.brace_depth += 1,
            '}' => state.brace_depth = (state.brace_depth - 1).max(0),
            '[' => state.bracket_depth += 1,
            ']' => state.bracket_depth = (state.bracket_depth - 1).max(0),
            _ => {}
        }
    }

    state
}

/// Get the partial word at cursor position.
///
/// Returns the text from the start of the current "word" to the cursor.
/// Word boundaries are whitespace and PromQL delimiters.
#[must_use]
pub fn partial_at_cursor(input: &str, cursor: usize) -> &str {
    if cursor == 0 || cursor > input.len() {
        return "";
    }

    let before = &input[..cursor];

    // Find the start of the current word
    let start = before
        .rfind(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '(' | ')'
                        | '{'
                        | '}'
                        | '['
                        | ']'
                        | ','
                        | '='
                        | '!'
                        | '~'
                        | '<'
                        | '>'
                        | '+'
                        | '-'
                        | '*'
                        | '/'
                        | '%'
                        | '^'
                        | '@'
                        | ':'
                        | '"'
                        | '\''
                        | '`'
                )
        })
        .map_or(0, |i| i + 1);

    &before[start..]
}

/// Find the last significant token before cursor.
///
/// Returns the token text and its type hint.
#[must_use]
pub fn last_token_before(input: &str, cursor: usize) -> Option<(&str, TokenHint)> {
    let cursor = cursor.min(input.len());
    let before = input[..cursor].trim_end();

    if before.is_empty() {
        return None;
    }

    // Check for delimiter at the end
    let last_char = before.chars().last()?;
    match last_char {
        '(' => return Some(("(", TokenHint::OpenParen)),
        ')' => return Some((")", TokenHint::CloseParen)),
        '{' => return Some(("{", TokenHint::OpenBrace)),
        '}' => return Some(("}", TokenHint::CloseBrace)),
        '[' => return Some(("[", TokenHint::OpenBracket)),
        ']' => return Some(("]", TokenHint::CloseBracket)),
        ',' => return Some((",", TokenHint::Comma)),
        '=' => {
            // Could be =, ==, =~
            if before.ends_with("==") {
                return Some(("==", TokenHint::Operator));
            }
            if before.ends_with("=~") {
                return Some(("=~", TokenHint::LabelOp));
            }
            return Some(("=", TokenHint::LabelOp));
        }
        '~' => {
            if before.ends_with("!~") {
                return Some(("!~", TokenHint::LabelOp));
            }
            if before.ends_with("=~") {
                return Some(("=~", TokenHint::LabelOp));
            }
        }
        '!' => {
            if before.ends_with("!=") {
                return Some(("!=", TokenHint::LabelOp));
            }
        }
        '+' | '-' | '*' | '/' | '%' | '^' | '<' | '>' => {
            return Some((&before[before.len() - 1..], TokenHint::Operator));
        }
        '@' => return Some(("@", TokenHint::At)),
        _ => {}
    }

    // Find the last word
    let word_start = before
        .rfind(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '(' | ')'
                        | '{'
                        | '}'
                        | '['
                        | ']'
                        | ','
                        | '='
                        | '!'
                        | '~'
                        | '<'
                        | '>'
                        | '+'
                        | '-'
                        | '*'
                        | '/'
                        | '%'
                        | '^'
                        | '@'
                )
        })
        .map_or(0, |i| i + 1);

    let word = &before[word_start..];
    if word.is_empty() {
        return None;
    }

    // Classify the word
    let hint = classify_word(word);
    Some((word, hint))
}

/// Hint about what kind of token this might be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenHint {
    /// Opening parenthesis.
    OpenParen,
    /// Closing parenthesis.
    CloseParen,
    /// Opening brace (label selector).
    OpenBrace,
    /// Closing brace.
    CloseBrace,
    /// Opening bracket (duration).
    OpenBracket,
    /// Closing bracket.
    CloseBracket,
    /// Comma separator.
    Comma,
    /// Label matcher operator (=, !=, =~, !~).
    LabelOp,
    /// Arithmetic/comparison operator.
    Operator,
    /// At modifier (@).
    At,
    /// A keyword (by, without, and, or, unless, etc.).
    Keyword,
    /// A function or aggregation name.
    Function,
    /// A duration literal.
    Duration,
    /// A number.
    Number,
    /// An identifier (metric name, label name).
    Identifier,
    /// A string literal.
    String,
}

/// PromQL keywords.
pub const KEYWORDS: &[&str] = &[
    "and",
    "or",
    "unless",
    "on",
    "ignoring",
    "group_left",
    "group_right",
    "by",
    "without",
    "offset",
    "bool",
];

/// PromQL aggregation operators.
pub const AGGREGATIONS: &[&str] = &[
    "sum",
    "avg",
    "min",
    "max",
    "count",
    "stddev",
    "stdvar",
    "topk",
    "bottomk",
    "quantile",
    "count_values",
    "group",
];

/// PromQL functions.
pub const FUNCTIONS: &[&str] = &[
    // Aggregation over time
    "avg_over_time",
    "min_over_time",
    "max_over_time",
    "sum_over_time",
    "count_over_time",
    "quantile_over_time",
    "stddev_over_time",
    "stdvar_over_time",
    "last_over_time",
    "present_over_time",
    // Rate functions
    "rate",
    "irate",
    "increase",
    "delta",
    "idelta",
    "deriv",
    // Counter functions
    "resets",
    "changes",
    // Math functions
    "abs",
    "ceil",
    "floor",
    "round",
    "sqrt",
    "exp",
    "ln",
    "log2",
    "log10",
    "sgn",
    // Trigonometric functions
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "sinh",
    "cosh",
    "tanh",
    "asinh",
    "acosh",
    "atanh",
    "deg",
    "rad",
    // Time functions
    "time",
    "timestamp",
    "year",
    "month",
    "day_of_month",
    "day_of_week",
    "day_of_year",
    "days_in_month",
    "hour",
    "minute",
    // Label functions
    "label_replace",
    "label_join",
    // Other functions
    "absent",
    "absent_over_time",
    "scalar",
    "vector",
    "sort",
    "sort_desc",
    "sort_by_label",
    "sort_by_label_desc",
    "histogram_quantile",
    "clamp",
    "clamp_min",
    "clamp_max",
    "predict_linear",
    "holt_winters",
];

/// All callable names (functions + aggregations).
pub fn all_callables() -> impl Iterator<Item = &'static str> {
    AGGREGATIONS
        .iter()
        .copied()
        .chain(FUNCTIONS.iter().copied())
}

/// Check if a name is a keyword.
#[must_use]
pub fn is_keyword(name: &str) -> bool {
    let lower = name.to_lowercase();
    KEYWORDS.iter().any(|k| *k == lower)
}

/// Check if a name is an aggregation.
#[must_use]
pub fn is_aggregation(name: &str) -> bool {
    let lower = name.to_lowercase();
    AGGREGATIONS.iter().any(|a| *a == lower)
}

/// Check if a name is a function.
#[must_use]
pub fn is_function(name: &str) -> bool {
    let lower = name.to_lowercase();
    FUNCTIONS.iter().any(|f| *f == lower)
}

/// Check if a name is callable (function or aggregation).
#[must_use]
pub fn is_callable(name: &str) -> bool {
    is_function(name) || is_aggregation(name)
}

/// Classify a word as a token hint.
fn classify_word(word: &str) -> TokenHint {
    // Check for string
    if (word.starts_with('"') && word.ends_with('"'))
        || (word.starts_with('\'') && word.ends_with('\''))
        || (word.starts_with('`') && word.ends_with('`'))
    {
        return TokenHint::String;
    }

    // Check for duration (number followed by time unit)
    if word.len() >= 2 {
        let last = word.chars().last().unwrap_or(' ');
        let rest = &word[..word.len() - 1];
        if matches!(last, 's' | 'm' | 'h' | 'd' | 'w' | 'y')
            && rest.chars().all(|c| c.is_ascii_digit())
        {
            return TokenHint::Duration;
        }
        // Check for "ms" suffix
        if let Some(rest) = word.strip_suffix("ms") {
            if rest.chars().all(|c| c.is_ascii_digit()) {
                return TokenHint::Duration;
            }
        }
    }

    // Check for number
    if word
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-')
        && word
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == '.')
    {
        return TokenHint::Number;
    }

    let lower = word.to_lowercase();

    // Check for keyword
    if KEYWORDS.iter().any(|k| *k == lower) {
        return TokenHint::Keyword;
    }

    // Check for function/aggregation
    if AGGREGATIONS.iter().any(|a| *a == lower) || FUNCTIONS.iter().any(|f| *f == lower) {
        return TokenHint::Function;
    }

    TokenHint::Identifier
}

/// Common duration suggestions.
pub const DURATION_SUGGESTIONS: &[&str] =
    &["1m", "5m", "15m", "30m", "1h", "6h", "12h", "1d", "7d"];

/// Binary operators.
pub const BINARY_OPS: &[&str] = &[
    "+", "-", "*", "/", "%", "^", "==", "!=", "<", ">", "<=", ">=", "and", "or", "unless",
];

/// Label matcher operators.
pub const LABEL_OPS: &[&str] = &["=", "!=", "=~", "!~"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_until_simple() {
        let state = scan_until("http_requests_total", 19);
        assert!(!state.in_selector());
        assert!(!state.in_duration());
        assert!(!state.in_parens());
    }

    #[test]
    fn test_scan_until_selector() {
        let state = scan_until("http_requests{method=", 21);
        assert!(state.in_selector());
        assert!(!state.in_duration());
    }

    #[test]
    fn test_scan_until_duration() {
        let state = scan_until("rate(x[5", 8);
        assert!(state.in_duration());
    }

    #[test]
    fn test_scan_until_parens() {
        let state = scan_until("sum(rate(x", 10);
        assert!(state.in_parens());
        assert_eq!(state.paren_depth, 2);
    }

    #[test]
    fn test_scan_until_string() {
        let state = scan_until(r#"x{a="val"#, 9);
        assert!(state.in_string);
        assert_eq!(state.string_delim, '"');
    }

    #[test]
    fn test_partial_at_cursor() {
        assert_eq!(partial_at_cursor("sum(http", 8), "http");
        assert_eq!(partial_at_cursor("sum(", 4), "");
        assert_eq!(partial_at_cursor("http_requests", 5), "http_");
        assert_eq!(partial_at_cursor(r#"{method=""#, 9), "");
        assert_eq!(partial_at_cursor("rate", 4), "rate");
    }

    #[test]
    fn test_last_token_before() {
        assert_eq!(
            last_token_before("sum(", 4),
            Some(("(", TokenHint::OpenParen))
        );
        assert_eq!(
            last_token_before("sum", 3),
            Some(("sum", TokenHint::Function))
        );
        assert_eq!(
            last_token_before("http_requests", 13),
            Some(("http_requests", TokenHint::Identifier))
        );
        assert_eq!(
            last_token_before("x{a=", 4),
            Some(("=", TokenHint::LabelOp))
        );
    }

    #[test]
    fn test_classify_word() {
        assert_eq!(classify_word("sum"), TokenHint::Function);
        assert_eq!(classify_word("rate"), TokenHint::Function);
        assert_eq!(classify_word("by"), TokenHint::Keyword);
        assert_eq!(classify_word("5m"), TokenHint::Duration);
        assert_eq!(classify_word("123"), TokenHint::Number);
        assert_eq!(classify_word("http_requests"), TokenHint::Identifier);
        assert_eq!(classify_word(r#""value""#), TokenHint::String);
    }

    #[test]
    fn test_is_callable() {
        assert!(is_callable("sum"));
        assert!(is_callable("rate"));
        assert!(is_callable("SUM")); // case insensitive
        assert!(!is_callable("by"));
        assert!(!is_callable("http_requests"));
    }
}
