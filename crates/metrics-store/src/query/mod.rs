//! Query parsing and evaluation
//!
//! This module re-exports the query language from `enya-lang` and provides
//! evaluation against the metrics store's tag index.

pub mod evaluate;

pub use enya_lang::{Node, Tag, parse_filter_query};
pub use evaluate::{evaluate_filter, intersection, union};
