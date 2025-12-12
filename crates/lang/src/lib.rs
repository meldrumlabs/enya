//! Query language for Enya time series database.
//!
//! Supports filter expressions like:
//! - `env:prod` - exact match
//! - `service:db.*` - wildcard match
//! - `env:prod AND service:db` - AND
//! - `env:prod OR env:staging` - OR
//! - `!env:prod` - NOT
//! - `(env:prod OR env:staging) AND service:db` - grouping
//! - `*` - match all
//!
//! And aggregation queries like:
//! - `sum(env:prod)` - sum aggregation
//! - `avg(env:prod) by (region)` - average with grouping
//! - `max(service:db.*) without (instance)` - max excluding labels

#![forbid(unsafe_code)]
#![deny(clippy::all, missing_docs, clippy::cargo)]
#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::multiple_crate_versions)]

pub mod completion;
mod error;
mod filter;
mod lexer;
pub mod query;

pub use error::{Error, Result};
pub use filter::{Node, Tag, parse_filter_query};
pub use lexer::{Token, tokenize_filter_query};
pub use query::{Aggregation, AggregationFunc, Duration, Grouping, Query, parse_query};
