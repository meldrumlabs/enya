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

/// Completion context indicating what type of input is expected at the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// Expecting an expression: identifier, `!`, `(`, or `*`.
    /// Occurs at start of input, after `(`, after `AND`/`OR`, or after `!`.
    ExpectExpr,

    /// Expecting a binary operator (`AND`, `OR`) or `)`.
    /// Occurs after a complete identifier or after `)`.
    ExpectOperator,

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
}

/// Returns syntax-based completion suggestions for the given context.
///
/// These are the keywords and operators valid at the current position.
/// Tag keys and values should be provided separately from domain knowledge.
#[must_use]
pub fn syntax_suggestions(ctx: &Context) -> SmallVec<[&'static str; 4]> {
    match ctx {
        Context::ExpectExpr => smallvec!["!", "(", "*"],
        Context::ExpectOperator => smallvec!["AND", "OR", ")"],
        Context::InTagKey(_) | Context::InTagValue { .. } => smallvec![],
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

    // Otherwise, determine context from the last complete token
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

    // Find the start of the current "word" (not whitespace, not operator chars)
    let word_start = trimmed
        .rfind(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .map_or(0, |i| i + 1);

    let current_word = &trimmed[word_start..];

    // Skip if it's an operator keyword or empty
    if matches!(current_word, "AND" | "OR" | "!" | "(" | ")" | "*" | "") {
        return None;
    }

    // Check what comes after the cursor to determine if we're mid-word
    let next_char = after_cursor.chars().next();
    let at_word_boundary = next_char.is_none_or(|c| c.is_whitespace() || c == '(' || c == ')');

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
    if !current_word.is_empty() && is_valid_key_char(current_word) {
        return Some(Context::InTagKey(current_word.to_string()));
    }

    None
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

    // Empty input -> expect expression
    if trimmed.is_empty() {
        return Context::ExpectExpr;
    }

    // Tokenize and find the last complete token
    let last_token = tokenize_filter_query(trimmed).flatten().last();

    match last_token {
        // After identifier/wildcard/) -> expect operator
        Some(Token::Identifier(_) | Token::Wildcard(_) | Token::ParenClose) => {
            Context::ExpectOperator
        }
        // After AND/OR/!/( -> expect expression
        Some(Token::And | Token::Or | Token::Not | Token::ParenOpen) | None => Context::ExpectExpr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(analyze("", 0), Context::ExpectExpr);
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
        assert_eq!(analyze("en", 2), Context::InTagKey("en".to_string()));
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
        // After * (match all), expect operator
        // Note: * alone isn't tokenized by our lexer as a special token,
        // so this tests the fallback behavior
        assert_eq!(analyze("*", 1), Context::ExpectExpr);
    }
}
