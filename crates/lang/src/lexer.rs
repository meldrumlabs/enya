//! Lexer for filter query expressions.

use logos::Logos;

/// Token type for filter query lexing.
#[derive(Logos, Debug, PartialEq, Eq, Clone)]
#[logos(skip r"[ \r\t\n\f]+")] // Ignore whitespace between tokens
pub enum Token<'a> {
    /// NOT operator
    #[token("!")]
    Not,

    /// AND operator
    #[token("AND")]
    And,

    /// OR operator
    #[token("OR")]
    Or,

    /// Opening parenthesis
    #[token("(")]
    ParenOpen,

    /// Closing parenthesis
    #[token(")")]
    ParenClose,

    /// Opening brace for filter blocks in aggregations
    #[token("{")]
    BraceOpen,

    /// Closing brace for filter blocks in aggregations
    #[token("}")]
    BraceClose,

    /// Opening bracket for time ranges
    #[token("[")]
    BracketOpen,

    /// Closing bracket for time ranges
    #[token("]")]
    BracketClose,

    /// Duration literal (e.g., 5m, 1h, 30s)
    #[regex("[0-9]+[smhdwy]")]
    Duration(&'a str),

    /// Comma separator for label lists
    #[token(",")]
    Comma,

    // === Aggregation functions ===
    /// Sum aggregation function
    #[token("sum")]
    Sum,

    /// Average aggregation function
    #[token("avg")]
    Avg,

    /// Minimum aggregation function
    #[token("min")]
    Min,

    /// Maximum aggregation function
    #[token("max")]
    Max,

    /// Count aggregation function
    #[token("count")]
    Count,

    // === Time-aware aggregation functions ===
    /// Rate function (per-second rate of increase)
    #[token("rate")]
    Rate,

    /// Instant rate function (per-second instant rate)
    #[token("irate")]
    Irate,

    /// Increase function (total increase over time range)
    #[token("increase")]
    Increase,

    /// Average over time function
    #[token("avg_over_time")]
    AvgOverTime,

    /// Sum over time function
    #[token("sum_over_time")]
    SumOverTime,

    /// Min over time function
    #[token("min_over_time")]
    MinOverTime,

    /// Max over time function
    #[token("max_over_time")]
    MaxOverTime,

    /// Count over time function
    #[token("count_over_time")]
    CountOverTime,

    // === Grouping keywords ===
    /// Group by specified labels
    #[token("by")]
    By,

    /// Group by all labels except specified
    #[token("without")]
    Without,

    /// Wildcard match (e.g., `service:db.*`)
    #[regex("[a-zA-Z_-]+:[a-zA-Z0-9_\\-.]*\\*")]
    Wildcard(&'a str),

    /// Exact match identifier (e.g., `env:prod`)
    #[regex("[a-zA-Z_-]+:[a-zA-Z0-9_\\-.]+")]
    Identifier(&'a str),

    /// Bare word (label name without colon, used in by/without clauses)
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*")]
    Label(&'a str),
}

/// Tokenize a filter query expression.
pub fn tokenize_filter_query(s: &str) -> impl Iterator<Item = Result<Token, ()>> + '_ {
    Token::lexer(s)
}
