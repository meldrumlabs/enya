//! Lexer for filter query expressions

use logos::Logos;

/// Token type for filter query lexing
#[derive(Logos, Debug, PartialEq, Eq)]
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
    ParanOpen,

    /// Closing parenthesis
    #[token(")")]
    ParanClose,

    /// Wildcard match (e.g., `service:db.*`)
    #[regex("[a-zA-Z_-]+:[a-zA-Z0-9_\\-.]*\\*")]
    Wildcard(&'a str),

    /// Exact match identifier (e.g., `env:prod`)
    #[regex("[a-zA-Z_-]+:[a-zA-Z0-9_\\-.]+")]
    Identifier(&'a str),
}

/// Tokenize a filter query expression
pub fn tokenize_filter_query(s: &str) -> impl Iterator<Item = Result<Token, ()>> + '_ {
    Token::lexer(s)
}
