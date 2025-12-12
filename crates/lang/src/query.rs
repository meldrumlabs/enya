//! Query AST with aggregation support.
//!
//! This module defines the top-level query types that support both simple
//! filter queries and aggregation queries with grouping.
//!
//! # Syntax
//!
//! ## Simple filter query (unchanged)
//! ```text
//! env:prod AND service:db
//! ```
//!
//! ## Aggregation with parentheses
//! ```text
//! sum(env:prod AND service:db)
//! avg(env:prod)
//! ```
//!
//! ## Aggregation with braces
//! ```text
//! sum { env:prod AND service:db }
//! ```
//!
//! ## Aggregation with grouping
//! ```text
//! sum(env:prod) by (region, service)
//! avg(cpu_usage) without (instance)
//! ```

use crate::error::Error;
use crate::filter::{Node, parse_filter_query};
use crate::lexer::{Token, tokenize_filter_query};
use std::fmt;

/// Aggregation function type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationFunc {
    /// Sum of values across series.
    Sum,
    /// Average of values across series.
    Avg,
    /// Minimum value across series.
    Min,
    /// Maximum value across series.
    Max,
    /// Count of series.
    Count,
    /// Per-second rate of increase (requires time range).
    Rate,
    /// Instant per-second rate (requires time range).
    Irate,
    /// Total increase over time range (requires time range).
    Increase,
    /// Average over time (requires time range).
    AvgOverTime,
    /// Sum over time (requires time range).
    SumOverTime,
    /// Minimum over time (requires time range).
    MinOverTime,
    /// Maximum over time (requires time range).
    MaxOverTime,
    /// Count over time (requires time range).
    CountOverTime,
}

impl fmt::Display for AggregationFunc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sum => write!(f, "sum"),
            Self::Avg => write!(f, "avg"),
            Self::Min => write!(f, "min"),
            Self::Max => write!(f, "max"),
            Self::Count => write!(f, "count"),
            Self::Rate => write!(f, "rate"),
            Self::Irate => write!(f, "irate"),
            Self::Increase => write!(f, "increase"),
            Self::AvgOverTime => write!(f, "avg_over_time"),
            Self::SumOverTime => write!(f, "sum_over_time"),
            Self::MinOverTime => write!(f, "min_over_time"),
            Self::MaxOverTime => write!(f, "max_over_time"),
            Self::CountOverTime => write!(f, "count_over_time"),
        }
    }
}

impl AggregationFunc {
    /// Returns true if this function requires a time range.
    #[must_use]
    pub const fn requires_time_range(self) -> bool {
        matches!(
            self,
            Self::Rate
                | Self::Irate
                | Self::Increase
                | Self::AvgOverTime
                | Self::SumOverTime
                | Self::MinOverTime
                | Self::MaxOverTime
                | Self::CountOverTime
        )
    }
}

/// Time duration for range queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    /// Duration in seconds.
    pub seconds: u64,
}

impl Duration {
    /// Parse a duration string (e.g., "5m", "1h", "30s").
    ///
    /// # Errors
    ///
    /// Returns an error if the duration string is invalid.
    pub fn parse(s: &str) -> Result<Self, Error> {
        let s = s.trim();
        if s.is_empty() {
            return Err(Error::InvalidQuery);
        }

        let (num_str, unit) = s.split_at(s.len() - 1);
        let num: u64 = num_str.parse().map_err(|_| Error::InvalidQuery)?;

        let multiplier = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86400,
            "w" => 604_800,
            "y" => 31_536_000,
            _ => return Err(Error::InvalidQuery),
        };

        Ok(Self {
            seconds: num * multiplier,
        })
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display in the most natural unit
        if self.seconds >= 31_536_000 && self.seconds % 31_536_000 == 0 {
            write!(f, "{}y", self.seconds / 31_536_000)
        } else if self.seconds >= 604_800 && self.seconds % 604_800 == 0 {
            write!(f, "{}w", self.seconds / 604_800)
        } else if self.seconds >= 86400 && self.seconds % 86400 == 0 {
            write!(f, "{}d", self.seconds / 86400)
        } else if self.seconds >= 3600 && self.seconds % 3600 == 0 {
            write!(f, "{}h", self.seconds / 3600)
        } else if self.seconds >= 60 && self.seconds % 60 == 0 {
            write!(f, "{}m", self.seconds / 60)
        } else {
            write!(f, "{}s", self.seconds)
        }
    }
}

/// Grouping modifier for aggregations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grouping {
    /// Group by the specified labels only.
    By(Vec<String>),
    /// Group by all labels except the specified ones.
    Without(Vec<String>),
}

impl fmt::Display for Grouping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::By(labels) => write!(f, "by ({})", labels.join(", ")),
            Self::Without(labels) => write!(f, "without ({})", labels.join(", ")),
        }
    }
}

/// An aggregation expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aggregation<'a> {
    /// The aggregation function.
    pub func: AggregationFunc,
    /// The filter expression to aggregate over.
    pub filter: Node<'a>,
    /// Optional time range for time-aware functions.
    pub time_range: Option<Duration>,
    /// Optional grouping modifier.
    pub grouping: Option<Grouping>,
}

impl fmt::Display for Aggregation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.func, self.filter)?;
        if let Some(duration) = &self.time_range {
            write!(f, "[{duration}]")?;
        }
        if let Some(grouping) = &self.grouping {
            write!(f, " {grouping}")?;
        }
        Ok(())
    }
}

/// Top-level query that can be either a filter or an aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query<'a> {
    /// A simple filter query (e.g., `env:prod AND service:db`).
    Filter(Node<'a>),
    /// An aggregation query (e.g., `sum(env:prod) by (region)`).
    Aggregation(Aggregation<'a>),
}

impl fmt::Display for Query<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filter(node) => write!(f, "{node}"),
            Self::Aggregation(agg) => write!(f, "{agg}"),
        }
    }
}

/// Parse a query string into a Query AST.
///
/// Supports both simple filter queries and aggregation queries.
///
/// # Examples
///
/// ```
/// use enya_lang::query::parse_query;
///
/// // Simple filter
/// let q = parse_query("env:prod AND service:db").unwrap();
///
/// // Aggregation
/// let q = parse_query("sum(env:prod)").unwrap();
///
/// // Aggregation with grouping
/// let q = parse_query("avg(cpu_usage) by (host)").unwrap();
/// ```
///
/// # Errors
///
/// Returns an error if the query syntax is invalid.
pub fn parse_query(s: &str) -> Result<Query<'_>, Error> {
    let trimmed = s.trim();

    // Handle special case of "*" which matches all series
    if trimmed == "*" {
        return Ok(Query::Filter(crate::filter::Node::AllStar));
    }

    // Collect first few tokens to determine query type
    let tokens: Vec<_> = tokenize_filter_query(trimmed)
        .take(2)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|()| Error::InvalidQuery)?;

    // Check if this looks like an aggregation query
    let is_aggregation = matches!(
        tokens.first(),
        Some(
            Token::Sum
                | Token::Avg
                | Token::Min
                | Token::Max
                | Token::Count
                | Token::Rate
                | Token::Irate
                | Token::Increase
                | Token::AvgOverTime
                | Token::SumOverTime
                | Token::MinOverTime
                | Token::MaxOverTime
                | Token::CountOverTime
        )
    );

    if is_aggregation {
        parse_aggregation_query(trimmed)
    } else {
        // Fall back to simple filter parsing
        let node = parse_filter_query(trimmed)?;
        Ok(Query::Filter(node))
    }
}

/// Parse an aggregation query.
fn parse_aggregation_query(s: &str) -> Result<Query<'_>, Error> {
    let mut tokens = tokenize_filter_query(s);

    // Parse aggregation function
    let func = match tokens.next() {
        Some(Ok(Token::Sum)) => AggregationFunc::Sum,
        Some(Ok(Token::Avg)) => AggregationFunc::Avg,
        Some(Ok(Token::Min)) => AggregationFunc::Min,
        Some(Ok(Token::Max)) => AggregationFunc::Max,
        Some(Ok(Token::Count)) => AggregationFunc::Count,
        Some(Ok(Token::Rate)) => AggregationFunc::Rate,
        Some(Ok(Token::Irate)) => AggregationFunc::Irate,
        Some(Ok(Token::Increase)) => AggregationFunc::Increase,
        Some(Ok(Token::AvgOverTime)) => AggregationFunc::AvgOverTime,
        Some(Ok(Token::SumOverTime)) => AggregationFunc::SumOverTime,
        Some(Ok(Token::MinOverTime)) => AggregationFunc::MinOverTime,
        Some(Ok(Token::MaxOverTime)) => AggregationFunc::MaxOverTime,
        Some(Ok(Token::CountOverTime)) => AggregationFunc::CountOverTime,
        _ => return Err(Error::InvalidQuery),
    };

    // Expect opening delimiter (paren or brace)
    let use_braces = match tokens.next() {
        Some(Ok(Token::ParenOpen)) => false,
        Some(Ok(Token::BraceOpen)) => true,
        _ => return Err(Error::InvalidQuery),
    };

    // Find the filter expression substring
    let open_pos = find_first_open_delimiter(s)?;
    let close_pos = if use_braces {
        find_matching_brace(s, open_pos)?
    } else {
        find_matching_paren(s, open_pos)?
    };

    // Extract and parse the filter expression
    let filter_str = &s[open_pos + 1..close_pos];
    let filter = if filter_str.trim().is_empty() {
        // Empty filter is invalid (e.g., "sum()" is not allowed)
        return Err(Error::InvalidQuery);
    } else if filter_str.trim() == "*" {
        crate::filter::Node::AllStar
    } else {
        parse_filter_query(filter_str.trim())?
    };

    // Parse the rest after the closing delimiter
    let rest = s[close_pos + 1..].trim();

    // Parse optional time range [duration]
    let (time_range, remaining) = parse_optional_time_range(rest)?;

    // Validate time range requirement
    if func.requires_time_range() && time_range.is_none() {
        return Err(Error::InvalidQuery);
    }

    // Parse optional grouping clause
    let grouping = if remaining.is_empty() {
        None
    } else {
        Some(parse_grouping(remaining)?)
    };

    Ok(Query::Aggregation(Aggregation {
        func,
        filter,
        time_range,
        grouping,
    }))
}

/// Parse an optional time range from the beginning of a string.
/// Returns the duration (if found) and the remaining string.
fn parse_optional_time_range(s: &str) -> Result<(Option<Duration>, &str), Error> {
    let trimmed = s.trim();

    if !trimmed.starts_with('[') {
        return Ok((None, trimmed));
    }

    // Find the closing bracket
    let close_pos = trimmed.find(']').ok_or(Error::InvalidQuery)?;
    let duration_str = &trimmed[1..close_pos];
    let duration = Duration::parse(duration_str)?;
    let remaining = trimmed[close_pos + 1..].trim();

    Ok((Some(duration), remaining))
}

/// Find the position of the first opening delimiter (paren or brace).
fn find_first_open_delimiter(s: &str) -> Result<usize, Error> {
    s.find(['(', '{']).ok_or(Error::InvalidQuery)
}

/// Find the position of the matching closing parenthesis.
fn find_matching_paren(s: &str, open_pos: usize) -> Result<usize, Error> {
    find_matching_delimiter(s, open_pos, '(', ')')
}

/// Find the position of the matching closing brace.
fn find_matching_brace(s: &str, open_pos: usize) -> Result<usize, Error> {
    find_matching_delimiter(s, open_pos, '{', '}')
}

/// Find the position of a matching closing delimiter.
fn find_matching_delimiter(
    s: &str,
    open_pos: usize,
    open: char,
    close: char,
) -> Result<usize, Error> {
    let mut depth = 0;
    for (i, c) in s[open_pos..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Ok(open_pos + i);
            }
        }
    }
    Err(Error::InvalidQuery)
}

/// Parse a grouping clause (by/without).
fn parse_grouping(s: &str) -> Result<Grouping, Error> {
    let tokens: Vec<_> = tokenize_filter_query(s)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|()| Error::InvalidQuery)?;

    if tokens.is_empty() {
        return Err(Error::InvalidQuery);
    }

    // Determine grouping type
    let (is_by, rest) = match tokens.first() {
        Some(Token::By) => (true, &tokens[1..]),
        Some(Token::Without) => (false, &tokens[1..]),
        _ => return Err(Error::InvalidQuery),
    };

    // Expect opening paren
    if !matches!(rest.first(), Some(Token::ParenOpen)) {
        return Err(Error::InvalidQuery);
    }

    // Parse label list
    let mut labels = Vec::new();
    let mut expect_label = true;

    for token in &rest[1..] {
        match token {
            Token::ParenClose => break,
            Token::Label(name) if expect_label => {
                labels.push((*name).to_string());
                expect_label = false;
            }
            Token::Comma if !expect_label => {
                expect_label = true;
            }
            _ => return Err(Error::InvalidQuery),
        }
    }

    if labels.is_empty() {
        return Err(Error::InvalidQuery);
    }

    if is_by {
        Ok(Grouping::By(labels))
    } else {
        Ok(Grouping::Without(labels))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_filter_query() {
        let q = parse_query("env:prod").unwrap();
        assert!(matches!(q, Query::Filter(Node::Eq(_))));
    }

    #[test]
    fn test_complex_filter_query() {
        let q = parse_query("env:prod AND service:db").unwrap();
        assert!(matches!(q, Query::Filter(Node::And(_))));
    }

    #[test]
    fn test_sum_aggregation() {
        let q = parse_query("sum(env:prod)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Sum);
        assert!(matches!(agg.filter, Node::Eq(_)));
        assert!(agg.grouping.is_none());
    }

    #[test]
    fn test_avg_aggregation() {
        let q = parse_query("avg(env:prod AND service:db)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Avg);
        assert!(matches!(agg.filter, Node::And(_)));
    }

    #[test]
    fn test_aggregation_with_braces() {
        let q = parse_query("sum { env:prod }").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Sum);
        assert!(matches!(agg.filter, Node::Eq(_)));
    }

    #[test]
    fn test_aggregation_with_by() {
        let q = parse_query("sum(env:prod) by (region)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Sum);
        assert_eq!(agg.grouping, Some(Grouping::By(vec!["region".to_string()])));
    }

    #[test]
    fn test_aggregation_with_multiple_by_labels() {
        let q = parse_query("avg(env:prod) by (region, service, host)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Avg);
        assert_eq!(
            agg.grouping,
            Some(Grouping::By(vec![
                "region".to_string(),
                "service".to_string(),
                "host".to_string(),
            ]))
        );
    }

    #[test]
    fn test_aggregation_with_without() {
        let q = parse_query("max(env:prod) without (instance)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Max);
        assert_eq!(
            agg.grouping,
            Some(Grouping::Without(vec!["instance".to_string()]))
        );
    }

    #[test]
    fn test_count_aggregation() {
        let q = parse_query("count(*)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Count);
        assert!(matches!(agg.filter, Node::AllStar));
    }

    #[test]
    fn test_min_max_aggregation() {
        let q = parse_query("min(env:prod)").unwrap();
        assert!(matches!(
            q,
            Query::Aggregation(Aggregation {
                func: AggregationFunc::Min,
                ..
            })
        ));

        let q = parse_query("max(env:prod)").unwrap();
        assert!(matches!(
            q,
            Query::Aggregation(Aggregation {
                func: AggregationFunc::Max,
                ..
            })
        ));
    }

    #[test]
    fn test_aggregation_display() {
        let q = parse_query("sum(env:prod) by (region)").unwrap();
        assert_eq!(q.to_string(), "sum(env:prod) by (region)");
    }

    #[test]
    fn test_aggregation_with_nested_parens() {
        let q = parse_query("sum((env:prod OR env:staging) AND service:db)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Sum);
        assert!(matches!(agg.filter, Node::And(_)));
    }

    #[test]
    fn test_invalid_queries() {
        assert!(parse_query("sum").is_err());
        assert!(parse_query("sum()").is_err()); // Empty filter
        assert!(parse_query("sum(env:prod) by ()").is_err()); // Empty labels
        assert!(parse_query("sum(env:prod) by").is_err()); // Missing parens
        assert!(parse_query("unknown(env:prod)").is_err()); // Unknown function
    }

    #[test]
    fn test_wildcard_star() {
        let q = parse_query("*").unwrap();
        assert!(matches!(q, Query::Filter(Node::AllStar)));
    }

    // === Time range tests ===

    #[test]
    fn test_rate_with_time_range() {
        let q = parse_query("rate(env:prod)[5m]").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Rate);
        assert_eq!(agg.time_range, Some(Duration { seconds: 300 }));
        assert!(agg.grouping.is_none());
    }

    #[test]
    fn test_irate_with_time_range() {
        let q = parse_query("irate(env:prod)[1h]").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Irate);
        assert_eq!(agg.time_range, Some(Duration { seconds: 3600 }));
    }

    #[test]
    fn test_increase_with_time_range() {
        let q = parse_query("increase(counter:value)[30s]").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Increase);
        assert_eq!(agg.time_range, Some(Duration { seconds: 30 }));
    }

    #[test]
    fn test_avg_over_time() {
        let q = parse_query("avg_over_time(cpu:usage)[1d]").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::AvgOverTime);
        assert_eq!(agg.time_range, Some(Duration { seconds: 86400 }));
    }

    #[test]
    fn test_sum_over_time() {
        let q = parse_query("sum_over_time(requests:count)[1w]").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::SumOverTime);
        assert_eq!(agg.time_range, Some(Duration { seconds: 604_800 }));
    }

    #[test]
    fn test_time_range_with_grouping() {
        let q = parse_query("rate(env:prod)[5m] by (region)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Rate);
        assert_eq!(agg.time_range, Some(Duration { seconds: 300 }));
        assert_eq!(agg.grouping, Some(Grouping::By(vec!["region".to_string()])));
    }

    #[test]
    fn test_time_range_with_without() {
        let q = parse_query("increase(counter:value)[1h] without (instance)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Increase);
        assert_eq!(agg.time_range, Some(Duration { seconds: 3600 }));
        assert_eq!(
            agg.grouping,
            Some(Grouping::Without(vec!["instance".to_string()]))
        );
    }

    #[test]
    fn test_time_aware_function_without_time_range_fails() {
        // Time-aware functions require a time range
        assert!(parse_query("rate(env:prod)").is_err());
        assert!(parse_query("irate(env:prod)").is_err());
        assert!(parse_query("increase(env:prod)").is_err());
        assert!(parse_query("avg_over_time(env:prod)").is_err());
    }

    #[test]
    fn test_optional_time_range_for_regular_aggregations() {
        // Regular aggregations can have optional time range
        let q = parse_query("sum(env:prod)[5m]").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Sum);
        assert_eq!(agg.time_range, Some(Duration { seconds: 300 }));

        // Without time range is also valid
        let q = parse_query("sum(env:prod)").unwrap();
        let Query::Aggregation(agg) = q else {
            panic!("Expected aggregation");
        };
        assert_eq!(agg.func, AggregationFunc::Sum);
        assert!(agg.time_range.is_none());
    }

    #[test]
    fn test_duration_display() {
        assert_eq!(Duration { seconds: 30 }.to_string(), "30s");
        assert_eq!(Duration { seconds: 60 }.to_string(), "1m");
        assert_eq!(Duration { seconds: 300 }.to_string(), "5m");
        assert_eq!(Duration { seconds: 3600 }.to_string(), "1h");
        assert_eq!(Duration { seconds: 86400 }.to_string(), "1d");
        assert_eq!(Duration { seconds: 604_800 }.to_string(), "1w");
        assert_eq!(
            Duration {
                seconds: 31_536_000
            }
            .to_string(),
            "1y"
        );
    }

    #[test]
    fn test_time_range_display() {
        let q = parse_query("rate(env:prod)[5m] by (region)").unwrap();
        assert_eq!(q.to_string(), "rate(env:prod)[5m] by (region)");
    }

    #[test]
    fn test_all_time_functions() {
        let functions = [
            ("rate(a:b)[1m]", AggregationFunc::Rate),
            ("irate(a:b)[1m]", AggregationFunc::Irate),
            ("increase(a:b)[1m]", AggregationFunc::Increase),
            ("avg_over_time(a:b)[1m]", AggregationFunc::AvgOverTime),
            ("sum_over_time(a:b)[1m]", AggregationFunc::SumOverTime),
            ("min_over_time(a:b)[1m]", AggregationFunc::MinOverTime),
            ("max_over_time(a:b)[1m]", AggregationFunc::MaxOverTime),
            ("count_over_time(a:b)[1m]", AggregationFunc::CountOverTime),
        ];

        for (input, expected_func) in functions {
            let q = parse_query(input).unwrap();
            let Query::Aggregation(agg) = q else {
                panic!("Expected aggregation for {input}");
            };
            assert_eq!(agg.func, expected_func, "Failed for {input}");
        }
    }
}
