//! SQL syntax highlighting for the SQL pane.
//!
//! Provides tokenization and highlighting of SQL text including:
//! - Keywords (SELECT, FROM, WHERE, etc.)
//! - Functions (COUNT, SUM, etc.)
//! - Strings, numbers, comments
//! - Dot commands (/connect, /tables, etc.)

use egui::TextFormat;
use egui::text::LayoutJob;

use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// SQL keywords for syntax highlighting.
pub const SQL_KEYWORDS: &[&str] = &[
    // DML
    "SELECT",
    "FROM",
    "WHERE",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    // Joins
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "FULL",
    "OUTER",
    "CROSS",
    "ON",
    "USING",
    // Clauses
    "ORDER",
    "BY",
    "GROUP",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "DISTINCT",
    "ALL",
    "AS",
    // Logical operators
    "AND",
    "OR",
    "NOT",
    "IN",
    "BETWEEN",
    "LIKE",
    "ILIKE",
    "IS",
    "NULL",
    "EXISTS",
    // DDL
    "CREATE",
    "DROP",
    "ALTER",
    "TABLE",
    "VIEW",
    "INDEX",
    "DATABASE",
    "SCHEMA",
    // Other
    "UNION",
    "INTERSECT",
    "EXCEPT",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "WITH",
    "RECURSIVE",
    "OVER",
    "PARTITION",
    "WINDOW",
    "ROWS",
    "RANGE",
    "EXPLAIN",
    "ANALYZE",
    "VERBOSE",
    "FORMAT",
    "ASC",
    "DESC",
    "NULLS",
    "FIRST",
    "LAST",
    "CAST",
    "FILTER",
    "WITHIN",
    "FETCH",
    "NEXT",
    "ONLY",
    // Boolean values
    "TRUE",
    "FALSE",
    // Types (common)
    "INT",
    "INTEGER",
    "BIGINT",
    "SMALLINT",
    "TINYINT",
    "FLOAT",
    "DOUBLE",
    "REAL",
    "DECIMAL",
    "NUMERIC",
    "VARCHAR",
    "CHAR",
    "TEXT",
    "STRING",
    "BOOLEAN",
    "BOOL",
    "DATE",
    "TIME",
    "TIMESTAMP",
    "INTERVAL",
    "BINARY",
    "VARBINARY",
    "BLOB",
];

/// SQL aggregate and common functions for highlighting.
pub const SQL_FUNCTIONS: &[&str] = &[
    // Aggregate
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "ARRAY_AGG",
    "STRING_AGG",
    "FIRST_VALUE",
    "LAST_VALUE",
    "NTH_VALUE",
    // Window
    "ROW_NUMBER",
    "RANK",
    "DENSE_RANK",
    "NTILE",
    "LAG",
    "LEAD",
    // String
    "CONCAT",
    "SUBSTRING",
    "SUBSTR",
    "TRIM",
    "LTRIM",
    "RTRIM",
    "UPPER",
    "LOWER",
    "LENGTH",
    "CHAR_LENGTH",
    "REPLACE",
    "SPLIT_PART",
    "REGEXP_REPLACE",
    // Numeric
    "ABS",
    "CEIL",
    "CEILING",
    "FLOOR",
    "ROUND",
    "TRUNC",
    "MOD",
    "POWER",
    "SQRT",
    "LOG",
    "LOG10",
    "LN",
    "EXP",
    "SIGN",
    "RANDOM",
    // Date/Time
    "NOW",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "DATE_TRUNC",
    "DATE_PART",
    "EXTRACT",
    "TO_DATE",
    "TO_TIMESTAMP",
    // Null handling
    "COALESCE",
    "NULLIF",
    "IFNULL",
    "NVL",
    // Type conversion
    "CAST",
    "TRY_CAST",
    "TYPEOF",
    // Conditional
    "IF",
    "IIF",
    "GREATEST",
    "LEAST",
    // DataFusion specific
    "ARROW_TYPEOF",
    "TO_CHAR",
    "UNNEST",
    "GENERATE_SERIES",
    "MAKE_ARRAY",
    "STRUCT",
    "NAMED_STRUCT",
    // Array
    "ARRAY_ELEMENT",
    "ARRAY_LENGTH",
    "ARRAY_APPEND",
    "ARRAY_CONCAT",
    "ARRAY_SLICE",
    "ARRAY_TO_STRING",
    "CARDINALITY",
    "FLATTEN",
    "RANGE",
    "LIST_SORT",
    "ARRAY_PREPEND",
    "ARRAY_REPEAT",
    "ARRAY_POSITIONS",
    "ARRAY_DIMS",
    "ARRAY_NDIMS",
    // Hashing
    "MD5",
    "SHA224",
    "SHA256",
    "SHA384",
    "SHA512",
    "DIGEST",
    // Additional string
    "STARTS_WITH",
    "ENDS_WITH",
    "INITCAP",
    "LEFT",
    "RIGHT",
    "LPAD",
    "RPAD",
    "REVERSE",
    "REPEAT",
    "BTRIM",
    "TRANSLATE",
    "ASCII",
    "CHR",
    "OCTET_LENGTH",
    "BIT_LENGTH",
    "REGEXP_MATCH",
    "OVERLAY",
    "POSITION",
    // Encoding
    "ENCODE",
    "DECODE",
];

/// Token types for SQL highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlToken {
    Keyword,
    Function,
    String,
    Number,
    Comment,
    Operator,
    Identifier,
    Whitespace,
    DotCommand,
    SlashCommand,
    TypeCast,
    TableRef,
}

/// A token with its position and type.
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: SqlToken,
}

/// Tokenize SQL text for syntax highlighting.
pub fn tokenize_sql(text: &str, table_names: &[&str]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let start = i;
        let ch = chars[i];

        // Slash commands (/explain, /connect, etc.) at start of input
        if ch == '/' && (start == 0 || (start > 0 && chars[start - 1] == '\n')) {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if j > i + 1 {
                tokens.push(Token {
                    start,
                    end: j,
                    kind: SqlToken::SlashCommand,
                });
                i = j;
                continue;
            }
        }

        // Dot commands (.help, .open, etc.)
        if ch == '.' && start == 0 || (start > 0 && chars[start - 1] == '\n') {
            // Check if this looks like a dot command at start of line
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if j > i + 1 {
                tokens.push(Token {
                    start,
                    end: j,
                    kind: SqlToken::DotCommand,
                });
                i = j;
                continue;
            }
        }

        // Whitespace
        if ch.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Whitespace,
            });
            continue;
        }

        // Single-line comment: --
        if ch == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Comment,
            });
            continue;
        }

        // Block comment: /* ... */
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < chars.len() {
                i += 2; // Skip */
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Comment,
            });
            continue;
        }

        // String: 'single quoted'
        if ch == '\'' {
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    // Check for escaped quote ''
                    if i + 1 < chars.len() && chars[i + 1] == '\'' {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::String,
            });
            continue;
        }

        // Double-quoted identifier: "identifier"
        if ch == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            if i < chars.len() {
                i += 1; // Skip closing "
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Identifier,
            });
            continue;
        }

        // Number
        if ch.is_ascii_digit()
            || (ch == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'e'
                    || chars[i] == 'E')
            {
                if (chars[i] == 'e' || chars[i] == 'E')
                    && i + 1 < chars.len()
                    && (chars[i + 1] == '+' || chars[i + 1] == '-')
                {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            tokens.push(Token {
                start,
                end: i,
                kind: SqlToken::Number,
            });
            continue;
        }

        // Identifier or keyword
        if ch.is_alphabetic() || ch == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let upper = word.to_uppercase();

            // Check if it's a function (followed by open paren)
            let is_function = SQL_FUNCTIONS.contains(&upper.as_str());
            let is_keyword = SQL_KEYWORDS.contains(&upper.as_str());

            let is_table_ref = !table_names.is_empty()
                && table_names.iter().any(|t| t.eq_ignore_ascii_case(&word));

            let kind = if is_function {
                // Look ahead for ( to confirm it's a function call
                let mut peek = i;
                while peek < chars.len() && chars[peek].is_whitespace() {
                    peek += 1;
                }
                if peek < chars.len() && chars[peek] == '(' {
                    SqlToken::Function
                } else if is_keyword {
                    SqlToken::Keyword
                } else {
                    SqlToken::Identifier
                }
            } else if is_keyword {
                SqlToken::Keyword
            } else if is_table_ref {
                SqlToken::TableRef
            } else {
                SqlToken::Identifier
            };

            tokens.push(Token {
                start,
                end: i,
                kind,
            });
            continue;
        }

        // Type cast: ::type (PostgreSQL-style)
        if ch == ':' && i + 1 < chars.len() && chars[i + 1] == ':' {
            let mut j = i + 2;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if j > i + 2 {
                tokens.push(Token {
                    start,
                    end: j,
                    kind: SqlToken::TypeCast,
                });
                i = j;
                continue;
            }
        }

        // Operators and punctuation
        i += 1;
        tokens.push(Token {
            start,
            end: i,
            kind: SqlToken::Operator,
        });
    }

    tokens
}

/// Create a highlighted LayoutJob for SQL text.
///
/// When `table_names` is non-empty, identifiers matching known table names
/// are highlighted with the variable color for visual distinction.
pub fn highlight_sql(text: &str, theme: AppTheme, table_names: &[&str]) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = typography::monospace(typography::SM);

    if text.is_empty() {
        return job;
    }

    let tokens = tokenize_sql(text, table_names);
    let mut last_end = 0;

    for token in tokens {
        // Handle any gap (shouldn't happen, but be safe)
        if token.start > last_end {
            if let Some(gap_text) = text.get(last_end..token.start) {
                job.append(
                    gap_text,
                    0.0,
                    TextFormat::simple(font_id.clone(), theme.text_primary()),
                );
            }
        }

        if let Some(token_text) = text.get(token.start..token.end) {
            let color = match token.kind {
                SqlToken::Keyword => theme.syntax_keyword(),
                SqlToken::Function => theme.syntax_function(),
                SqlToken::String => theme.syntax_value(),
                SqlToken::Number => theme.syntax_number(),
                SqlToken::Comment => theme.syntax_comment(),
                SqlToken::Operator => theme.syntax_punctuation(),
                SqlToken::DotCommand | SqlToken::SlashCommand => theme.accent_primary(),
                SqlToken::TypeCast => theme.syntax_type(),
                SqlToken::TableRef => theme.syntax_variable(),
                SqlToken::Identifier => theme.text_primary(),
                SqlToken::Whitespace => theme.text_primary(),
            };

            job.append(token_text, 0.0, TextFormat::simple(font_id.clone(), color));
        }

        last_end = token.end;
    }

    // Handle any remaining text
    if last_end < text.len() {
        if let Some(remaining) = text.get(last_end..) {
            job.append(
                remaining,
                0.0,
                TextFormat::simple(font_id.clone(), theme.text_primary()),
            );
        }
    }

    job
}
