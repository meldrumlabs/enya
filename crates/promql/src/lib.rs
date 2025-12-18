//! `PromQL` parser and autocomplete for Enya.
//!
//! This crate provides `PromQL` parsing, validation, and context-aware autocomplete
//! for the Enya metrics editor.
//!
//! # Example
//!
//! ```
//! use enya_promql::{analyze, syntax_suggestions, validate};
//!
//! // Validate a query
//! let result = validate("rate(http_requests_total[5m])");
//! assert!(result.is_valid);
//!
//! // Get completion context at cursor position
//! let query = "sum(http_requests_total{";
//! let cursor = query.len();
//! let ctx = analyze(query, cursor);
//!
//! // Get syntax suggestions for the context
//! let suggestions = syntax_suggestions(&ctx);
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::all, missing_docs)]
#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::multiple_crate_versions)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::option_if_let_else)]

pub mod completion;
pub mod lexer;
pub mod validation;

// Re-export key types
pub use completion::{Context, analyze, syntax_suggestions};
pub use lexer::{
    AGGREGATIONS, BINARY_OPS, DURATION_SUGGESTIONS, FUNCTIONS, KEYWORDS, LABEL_OPS, ScanState,
    TokenHint, all_callables, is_aggregation, is_callable, is_function, is_keyword,
    partial_at_cursor, scan_until,
};
pub use validation::{ValidationResult, validate};

// Re-export promql-parser for AST access if needed
pub use promql_parser::parser::parse;
