//! Lightweight scanner for LogQL autocomplete.
//!
//! This module provides simple character-based scanning for autocomplete context
//! detection. LogQL is the query language for Grafana Loki.

/// Scan state tracking nesting depth and context.
#[derive(Debug, Clone, Default)]
pub struct ScanState {
    /// Depth of parentheses nesting.
    pub paren_depth: i32,
    /// Depth of brace nesting (stream selectors).
    pub brace_depth: i32,
    /// Depth of bracket nesting (durations).
    pub bracket_depth: i32,
    /// Whether we're inside a string.
    pub in_string: bool,
    /// The string delimiter if in_string is true.
    pub string_delim: char,
    /// Whether we're after a pipe operator.
    pub after_pipe: bool,
}

impl ScanState {
    /// Check if we're inside a stream selector `{}`.
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

    let mut chars = input[..cursor].chars().peekable();
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
                state.after_pipe = false;
            }
            '(' => {
                state.paren_depth += 1;
                state.after_pipe = false;
            }
            ')' => {
                state.paren_depth = (state.paren_depth - 1).max(0);
                state.after_pipe = false;
            }
            '{' => {
                state.brace_depth += 1;
                state.after_pipe = false;
            }
            '}' => {
                state.brace_depth = (state.brace_depth - 1).max(0);
                state.after_pipe = false;
            }
            '[' => {
                state.bracket_depth += 1;
                state.after_pipe = false;
            }
            ']' => {
                state.bracket_depth = (state.bracket_depth - 1).max(0);
                state.after_pipe = false;
            }
            '|' => {
                // Check for line filter operators |=, |~, or pipe stage
                match chars.peek() {
                    Some('=') | Some('~') => {
                        chars.next();
                        state.after_pipe = false;
                    }
                    _ => {
                        state.after_pipe = true;
                    }
                }
            }
            _ if c.is_whitespace() => {
                // Whitespace doesn't change after_pipe state
            }
            _ => {
                state.after_pipe = false;
            }
        }
    }

    state
}

/// Get the partial word at cursor position.
///
/// Returns the text from the start of the current "word" to the cursor.
/// Word boundaries are whitespace and LogQL delimiters.
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
                        | '|'
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
        '|' => {
            // Check for line filter operators
            if before.ends_with("|=") {
                return Some(("|=", TokenHint::LineFilter));
            }
            if before.ends_with("|~") {
                return Some(("|~", TokenHint::LineFilter));
            }
            return Some(("|", TokenHint::Pipe));
        }
        '=' => {
            // Could be =, ==, |=, !=
            if before.ends_with("|=") {
                return Some(("|=", TokenHint::LineFilter));
            }
            if before.ends_with("!=") {
                return Some(("!=", TokenHint::LineFilter));
            }
            if before.ends_with("==") {
                return Some(("==", TokenHint::Operator));
            }
            return Some(("=", TokenHint::LabelOp));
        }
        '~' => {
            if before.ends_with("!~") {
                return Some(("!~", TokenHint::LineFilter));
            }
            if before.ends_with("|~") {
                return Some(("|~", TokenHint::LineFilter));
            }
            if before.ends_with("=~") {
                return Some(("=~", TokenHint::LabelOp));
            }
        }
        '!' => {
            if before.ends_with("!=") {
                return Some(("!=", TokenHint::LineFilter));
            }
            if before.ends_with("!~") {
                return Some(("!~", TokenHint::LineFilter));
            }
        }
        '+' | '-' | '*' | '/' | '%' | '^' | '<' | '>' => {
            return Some((&before[before.len() - 1..], TokenHint::Operator));
        }
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
                        | '|'
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
    /// Opening brace (stream selector).
    OpenBrace,
    /// Closing brace.
    CloseBrace,
    /// Opening bracket (duration).
    OpenBracket,
    /// Closing bracket.
    CloseBracket,
    /// Comma separator.
    Comma,
    /// Pipe operator (stage separator).
    Pipe,
    /// Line filter operator (|=, !=, |~, !~).
    LineFilter,
    /// Label matcher operator (=, !=, =~, !~).
    LabelOp,
    /// Arithmetic/comparison operator.
    Operator,
    /// A keyword (by, without, and, or, unless, etc.).
    Keyword,
    /// A function name.
    Function,
    /// A parser name (json, logfmt, etc.).
    Parser,
    /// A duration literal.
    Duration,
    /// A number.
    Number,
    /// An identifier (stream name, label name).
    Identifier,
    /// A string literal.
    String,
}

/// LogQL keywords.
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
];

/// LogQL range aggregation functions.
pub const RANGE_FUNCTIONS: &[&str] = &[
    "rate",
    "count_over_time",
    "bytes_rate",
    "bytes_over_time",
    "absent_over_time",
    "sum_over_time",
    "avg_over_time",
    "min_over_time",
    "max_over_time",
    "stdvar_over_time",
    "stddev_over_time",
    "quantile_over_time",
    "first_over_time",
    "last_over_time",
];

/// LogQL aggregation operators.
pub const AGGREGATIONS: &[&str] = &[
    "sum",
    "avg",
    "min",
    "max",
    "count",
    "stddev",
    "stdvar",
    "bottomk",
    "topk",
    "sort",
    "sort_desc",
];

/// LogQL label functions.
pub const LABEL_FUNCTIONS: &[&str] = &["label_replace"];

/// LogQL parsers (used after pipe).
pub const PARSERS: &[&str] = &["json", "logfmt", "pattern", "regexp", "unpack"];

/// LogQL filter expressions.
pub const FILTER_EXPRESSIONS: &[&str] =
    &["line_format", "label_format", "drop", "keep", "decolorize"];

/// LogQL IP functions (used in filter context).
pub const IP_FUNCTIONS: &[&str] = &["ip"];

/// All callable names (functions + aggregations).
pub fn all_callables() -> impl Iterator<Item = &'static str> {
    AGGREGATIONS
        .iter()
        .copied()
        .chain(RANGE_FUNCTIONS.iter().copied())
        .chain(LABEL_FUNCTIONS.iter().copied())
}

/// All stage operations (parsers + filter expressions).
pub fn all_stages() -> impl Iterator<Item = &'static str> {
    PARSERS
        .iter()
        .copied()
        .chain(FILTER_EXPRESSIONS.iter().copied())
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

/// Check if a name is a range function.
#[must_use]
pub fn is_range_function(name: &str) -> bool {
    let lower = name.to_lowercase();
    RANGE_FUNCTIONS.iter().any(|f| *f == lower)
}

/// Check if a name is a parser.
#[must_use]
pub fn is_parser(name: &str) -> bool {
    let lower = name.to_lowercase();
    PARSERS.iter().any(|p| *p == lower)
}

/// Check if a name is callable (function or aggregation).
#[must_use]
pub fn is_callable(name: &str) -> bool {
    is_range_function(name) || is_aggregation(name) || is_label_function(name)
}

/// Check if a name is a label function.
#[must_use]
pub fn is_label_function(name: &str) -> bool {
    let lower = name.to_lowercase();
    LABEL_FUNCTIONS.iter().any(|f| *f == lower)
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
        // Check for "ms" or "ns" suffix
        if let Some(rest) = word.strip_suffix("ms") {
            if rest.chars().all(|c| c.is_ascii_digit()) {
                return TokenHint::Duration;
            }
        }
        if let Some(rest) = word.strip_suffix("ns") {
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

    // Check for parser
    if PARSERS.iter().any(|p| *p == lower) {
        return TokenHint::Parser;
    }

    // Check for function/aggregation
    if AGGREGATIONS.iter().any(|a| *a == lower)
        || RANGE_FUNCTIONS.iter().any(|f| *f == lower)
        || LABEL_FUNCTIONS.iter().any(|f| *f == lower)
    {
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

/// Line filter operators.
pub const LINE_FILTERS: &[&str] = &["|=", "!=", "|~", "!~"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_until_simple() {
        let state = scan_until("{app=\"nginx\"}", 13);
        assert!(!state.in_selector()); // Closed
        assert!(!state.in_duration());
        assert!(!state.in_parens());
    }

    #[test]
    fn test_scan_until_selector() {
        let state = scan_until("{app=", 5);
        assert!(state.in_selector());
        assert!(!state.in_duration());
    }

    #[test]
    fn test_scan_until_duration() {
        let state = scan_until("rate({app=\"x\"}[5", 16);
        assert!(state.in_duration());
    }

    #[test]
    fn test_scan_until_parens() {
        let state = scan_until("sum(count_over_time({app=\"x\"}", 29);
        assert!(state.in_parens());
        assert_eq!(state.paren_depth, 2);
    }

    #[test]
    fn test_scan_until_pipe() {
        let state = scan_until("{app=\"nginx\"} | ", 16);
        assert!(state.after_pipe);

        let state = scan_until("{app=\"nginx\"} |= \"error\"", 24);
        assert!(!state.after_pipe);
    }

    #[test]
    fn test_partial_at_cursor() {
        assert_eq!(partial_at_cursor("rate({app", 9), "app");
        assert_eq!(partial_at_cursor("rate(", 5), "");
        assert_eq!(partial_at_cursor("{app=\"nginx\"} | js", 18), "js");
        assert_eq!(partial_at_cursor("{app=\"nginx\"} |= ", 17), "");
    }

    #[test]
    fn test_last_token_before() {
        assert_eq!(
            last_token_before("rate(", 5),
            Some(("(", TokenHint::OpenParen))
        );
        assert_eq!(
            last_token_before("{app=\"nginx\"} |", 15),
            Some(("|", TokenHint::Pipe))
        );
        assert_eq!(
            last_token_before("{app=\"nginx\"} |=", 16),
            Some(("|=", TokenHint::LineFilter))
        );
        assert_eq!(
            last_token_before("json", 4),
            Some(("json", TokenHint::Parser))
        );
    }

    #[test]
    fn test_classify_word() {
        assert_eq!(classify_word("sum"), TokenHint::Function);
        assert_eq!(classify_word("rate"), TokenHint::Function);
        assert_eq!(classify_word("json"), TokenHint::Parser);
        assert_eq!(classify_word("by"), TokenHint::Keyword);
        assert_eq!(classify_word("5m"), TokenHint::Duration);
        assert_eq!(classify_word("123"), TokenHint::Number);
        assert_eq!(classify_word("nginx"), TokenHint::Identifier);
        assert_eq!(classify_word(r#""value""#), TokenHint::String);
    }

    #[test]
    fn test_is_callable() {
        assert!(is_callable("sum"));
        assert!(is_callable("rate"));
        assert!(is_callable("count_over_time"));
        assert!(is_callable("SUM")); // case insensitive
        assert!(!is_callable("by"));
        assert!(!is_callable("json")); // parser, not callable
    }

    #[test]
    fn test_is_parser() {
        assert!(is_parser("json"));
        assert!(is_parser("logfmt"));
        assert!(is_parser("JSON")); // case insensitive
        assert!(!is_parser("rate"));
    }
}
