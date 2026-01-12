//! Lightweight LogQL parser and autocomplete for Enya.
//!
//! This crate provides context-aware autocomplete for LogQL queries without
//! requiring a full parser. It's designed for use in editor UIs.
//!
//! # Example
//!
//! ```
//! use enya_logql::{analyze, syntax_suggestions, Context};
//!
//! // Get completion context at cursor position
//! let ctx = analyze("{app=\"nginx\"} | ", 16);
//! assert_eq!(ctx, Context::ExpectStage);
//!
//! // Get static syntax suggestions for this context
//! let suggestions: Vec<_> = syntax_suggestions(&ctx).collect();
//! assert!(suggestions.contains(&"json"));
//! assert!(suggestions.contains(&"logfmt"));
//! ```
//!
//! # LogQL Overview
//!
//! LogQL is the query language for Grafana Loki. It has two main query types:
//!
//! ## Log Queries
//! Return log lines matching the query:
//! ```text
//! {app="nginx"} |= "error" | json | level="error"
//! ```
//!
//! ## Metric Queries
//! Return calculated values over log data:
//! ```text
//! rate({app="nginx"} |= "error" [5m])
//! sum(count_over_time({app="nginx"}[1h])) by (level)
//! ```

pub mod completion;
pub mod lexer;
pub mod validation;

// Re-export main types for convenience
pub use completion::{Context, analyze, syntax_suggestions};
pub use lexer::{
    AGGREGATIONS, BINARY_OPS, DURATION_SUGGESTIONS, FILTER_EXPRESSIONS, KEYWORDS, LABEL_FUNCTIONS,
    LABEL_OPS, LINE_FILTERS, PARSERS, RANGE_FUNCTIONS, ScanState, TokenHint, all_callables,
    all_stages, is_aggregation, is_callable, is_keyword, is_label_function, is_parser,
    is_range_function, last_token_before, partial_at_cursor, scan_until,
};
pub use validation::{ValidationError, ValidationResult, validate};
