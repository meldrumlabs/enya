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

/// Extract the primary metric name from a PromQL query.
///
/// Walks the AST to find the first vector or matrix selector and returns
/// its metric name. For complex queries (aggregations, functions, binary ops),
/// this returns the leftmost/innermost metric name.
///
/// # Example
///
/// ```
/// use enya_promql::extract_metric_name;
///
/// assert_eq!(extract_metric_name("http_requests_total"), Some("http_requests_total".to_string()));
/// assert_eq!(extract_metric_name("rate(http_requests_total[5m])"), Some("http_requests_total".to_string()));
/// assert_eq!(extract_metric_name("sum(rate(my_metric[5m])) by (job)"), Some("my_metric".to_string()));
/// ```
#[must_use]
pub fn extract_metric_name(query: &str) -> Option<String> {
    let expr = promql_parser::parser::parse(query).ok()?;
    extract_metric_from_expr(&expr)
}

fn extract_metric_from_expr(expr: &promql_parser::parser::Expr) -> Option<String> {
    use promql_parser::parser::Expr;

    match expr {
        Expr::VectorSelector(vs) => vs.name.clone(),
        Expr::MatrixSelector(ms) => ms.vs.name.clone(),
        Expr::Call(call) => call.args.args.first().and_then(|arg| extract_metric_from_expr(arg.as_ref())),
        Expr::Aggregate(agg) => extract_metric_from_expr(&agg.expr),
        Expr::Binary(bin) => extract_metric_from_expr(&bin.lhs)
            .or_else(|| extract_metric_from_expr(&bin.rhs)),
        Expr::Unary(un) => extract_metric_from_expr(&un.expr),
        Expr::Paren(p) => extract_metric_from_expr(&p.expr),
        Expr::Subquery(sq) => extract_metric_from_expr(&sq.expr),
        Expr::NumberLiteral(_) | Expr::StringLiteral(_) | Expr::Extension(_) => None,
    }
}
