//! Translation from enya-lang queries to PromQL.
//!
//! This module converts enya-lang AST nodes to PromQL query strings.
//!
//! # Supported Translations
//!
//! | enya-lang | PromQL |
//! |-----------|--------|
//! | `env:prod` | `{env="prod"}` |
//! | `service:db*` | `{service=~"db.*"}` |
//! | `env:prod AND host:s1` | `{env="prod",host="s1"}` |
//! | `sum(env:prod) by (region)` | `sum(metric{env="prod"}) by (region)` |
//! | `rate(*)[5m]` | `rate(metric[5m])` |
//!
//! # Unsupported (returns error)
//!
//! - OR expressions (would require PromQL union which is complex)
//! - NOT expressions (PromQL negation has different semantics)
//! - `without()` grouping (could be added later)

use enya_lang::{AggregationFunc, Grouping, Node, Query};

use crate::error::ClientError;

/// Result of translating an enya-lang query to PromQL.
#[derive(Debug, Clone)]
pub struct PromQLQuery {
    /// The full PromQL query string.
    pub query: String,
}

/// Translate an enya-lang query to PromQL.
///
/// # Arguments
///
/// * `metric` - The metric name (e.g., "cpu_usage")
/// * `query_str` - The enya-lang query string (e.g., "sum(env:prod) by (region)")
///
/// # Errors
///
/// Returns `ClientError::TranslationError` if the query contains unsupported constructs
/// like OR, NOT, or `without()` grouping.
pub fn translate(metric: &str, query_str: &str) -> Result<PromQLQuery, ClientError> {
    let parsed = enya_lang::parse_query(query_str)
        .map_err(|e| ClientError::TranslationError(format!("failed to parse query: {e}")))?;

    let query = match parsed {
        Query::Filter(filter) => {
            // Simple filter query - just metric + labels
            translate_filter_to_selector(metric, &filter)?
        }
        Query::Aggregation(agg) => {
            // Aggregation query
            let selector = translate_filter_to_selector(metric, &agg.filter)?;

            // Build the aggregation expression
            let func_name = translate_agg_func(agg.func);

            // Handle time range for rate-style functions
            let inner = if let Some(duration) = agg.time_range {
                format!("{selector}[{}s]", duration.seconds)
            } else {
                selector
            };

            // Wrap in aggregation function
            let mut result = format!("{func_name}({inner})");

            // Add grouping clause if present
            if let Some(grouping) = agg.grouping {
                match grouping {
                    Grouping::By(labels) => {
                        result.push_str(" by (");
                        result.push_str(&labels.join(","));
                        result.push(')');
                    }
                    Grouping::Without(_) => {
                        return Err(ClientError::TranslationError(
                            "without() grouping is not yet supported for Prometheus".to_string(),
                        ));
                    }
                }
            }

            result
        }
    };

    Ok(PromQLQuery { query })
}

/// Translate a filter AST node to a PromQL selector string.
///
/// Returns something like `metric_name{label1="value1",label2="value2"}`.
fn translate_filter_to_selector(metric: &str, filter: &Node<'_>) -> Result<String, ClientError> {
    let labels = collect_labels(filter)?;

    if labels.is_empty() {
        // Just the metric name, no labels
        Ok(metric.to_string())
    } else {
        Ok(format!("{metric}{{{}}}", labels.join(",")))
    }
}

/// Collect label matchers from a filter node.
///
/// This flattens AND nodes and collects all label matchers.
/// Returns an error for OR, NOT, or nested structures that can't be represented
/// as a single PromQL selector.
fn collect_labels(node: &Node<'_>) -> Result<Vec<String>, ClientError> {
    match node {
        Node::AllStar => {
            // Match all - empty label set
            Ok(vec![])
        }
        Node::Eq(tag) => {
            // Exact match: key="value"
            Ok(vec![format!(
                "{}=\"{}\"",
                tag.key,
                escape_label_value(tag.value)
            )])
        }
        Node::Wildcard(tag) => {
            // Prefix match: key=~"value.*"
            Ok(vec![format!(
                "{}=~\"{}.*\"",
                tag.key,
                escape_regex(tag.value)
            )])
        }
        Node::And(nodes) => {
            // AND: collect all labels from children
            let mut labels = Vec::new();
            for child in nodes {
                labels.extend(collect_labels(child)?);
            }
            Ok(labels)
        }
        Node::Or(_) => Err(ClientError::TranslationError(
            "OR expressions are not supported for Prometheus (would require complex union queries)"
                .to_string(),
        )),
        Node::Not(_) => Err(ClientError::TranslationError(
            "NOT expressions are not supported for Prometheus".to_string(),
        )),
    }
}

/// Translate an aggregation function to its PromQL name.
fn translate_agg_func(func: AggregationFunc) -> &'static str {
    match func {
        AggregationFunc::Sum => "sum",
        AggregationFunc::Avg => "avg",
        AggregationFunc::Min => "min",
        AggregationFunc::Max => "max",
        AggregationFunc::Count => "count",
        AggregationFunc::Rate => "rate",
        AggregationFunc::Irate => "irate",
        AggregationFunc::Increase => "increase",
        AggregationFunc::AvgOverTime => "avg_over_time",
        AggregationFunc::SumOverTime => "sum_over_time",
        AggregationFunc::MinOverTime => "min_over_time",
        AggregationFunc::MaxOverTime => "max_over_time",
        AggregationFunc::CountOverTime => "count_over_time",
    }
}

/// Escape a label value for use in a PromQL selector.
///
/// PromQL label values are double-quoted strings where `\`, `"`, and newlines
/// need to be escaped.
fn escape_label_value(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            _ => result.push(c),
        }
    }
    result
}

/// Escape a value for use in a PromQL regex matcher.
///
/// Regex special characters need to be escaped so they match literally.
fn escape_regex(value: &str) -> String {
    let mut result = String::with_capacity(value.len() * 2);
    for c in value.chars() {
        match c {
            '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
            | '"' => {
                result.push('\\');
                result.push(c);
            }
            '\n' => result.push_str("\\n"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_filter() {
        let result = translate("cpu_usage", "env:prod").unwrap();
        assert_eq!(result.query, r#"cpu_usage{env="prod"}"#);
    }

    #[test]
    fn test_wildcard_filter() {
        let result = translate("cpu_usage", "service:db*").unwrap();
        assert_eq!(result.query, r#"cpu_usage{service=~"db.*"}"#);
    }

    #[test]
    fn test_and_filter() {
        let result = translate("cpu_usage", "env:prod AND host:server1").unwrap();
        assert_eq!(result.query, r#"cpu_usage{env="prod",host="server1"}"#);
    }

    #[test]
    fn test_match_all() {
        let result = translate("cpu_usage", "*").unwrap();
        assert_eq!(result.query, "cpu_usage");
    }

    #[test]
    fn test_sum_aggregation() {
        let result = translate("cpu_usage", "sum(env:prod)").unwrap();
        assert_eq!(result.query, r#"sum(cpu_usage{env="prod"})"#);
    }

    #[test]
    fn test_aggregation_with_grouping() {
        let result = translate("cpu_usage", "sum(env:prod) by (region)").unwrap();
        assert_eq!(result.query, r#"sum(cpu_usage{env="prod"}) by (region)"#);
    }

    #[test]
    fn test_aggregation_with_multiple_labels() {
        let result = translate("http_requests", "avg(*) by (method, status)").unwrap();
        assert_eq!(result.query, "avg(http_requests) by (method,status)");
    }

    #[test]
    fn test_rate_with_duration() {
        let result = translate("http_requests_total", "rate(*)[5m]").unwrap();
        assert_eq!(result.query, "rate(http_requests_total[300s])");
    }

    #[test]
    fn test_rate_with_filter_and_grouping() {
        let result = translate("http_requests_total", "rate(env:prod)[5m] by (method)").unwrap();
        assert_eq!(
            result.query,
            r#"rate(http_requests_total{env="prod"}[300s]) by (method)"#
        );
    }

    #[test]
    fn test_increase() {
        let result = translate("counter", "increase(env:prod)[1h]").unwrap();
        assert_eq!(result.query, r#"increase(counter{env="prod"}[3600s])"#);
    }

    #[test]
    fn test_avg_over_time() {
        let result = translate("latency", "avg_over_time(*)[30m]").unwrap();
        assert_eq!(result.query, "avg_over_time(latency[1800s])");
    }

    #[test]
    fn test_or_not_supported() {
        let result = translate("cpu_usage", "env:prod OR env:staging");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("OR expressions are not supported")
        );
    }

    #[test]
    fn test_not_not_supported() {
        let result = translate("cpu_usage", "!env:prod");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("NOT expressions are not supported")
        );
    }

    #[test]
    fn test_without_not_supported() {
        let result = translate("cpu_usage", "sum(*) without (instance)");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("without"));
    }

    #[test]
    fn test_escape_label_value() {
        assert_eq!(escape_label_value("simple"), "simple");
        assert_eq!(escape_label_value(r#"with"quote"#), r#"with\"quote"#);
        assert_eq!(escape_label_value(r"with\backslash"), r"with\\backslash");
        assert_eq!(escape_label_value("with\nnewline"), r"with\nnewline");
    }

    #[test]
    fn test_escape_regex() {
        assert_eq!(escape_regex("simple"), "simple");
        assert_eq!(escape_regex("db."), r"db\.");
        assert_eq!(escape_regex("foo*bar"), r"foo\*bar");
        assert_eq!(escape_regex("a+b"), r"a\+b");
    }

    #[test]
    fn test_complex_and_chain() {
        let result = translate("metric", "env:prod AND region:us-east AND service:api").unwrap();
        assert_eq!(
            result.query,
            r#"metric{env="prod",region="us-east",service="api"}"#
        );
    }

    // === Additional aggregation function tests ===

    #[test]
    fn test_min_aggregation() {
        let result = translate("response_time", "min(env:prod)").unwrap();
        assert_eq!(result.query, r#"min(response_time{env="prod"})"#);
    }

    #[test]
    fn test_max_aggregation() {
        let result = translate("response_time", "max(env:prod)").unwrap();
        assert_eq!(result.query, r#"max(response_time{env="prod"})"#);
    }

    #[test]
    fn test_count_aggregation() {
        let result = translate("requests", "count(*)").unwrap();
        assert_eq!(result.query, "count(requests)");
    }

    #[test]
    fn test_irate() {
        let result = translate("cpu_seconds", "irate(*)[1m]").unwrap();
        assert_eq!(result.query, "irate(cpu_seconds[60s])");
    }

    #[test]
    fn test_sum_over_time() {
        let result = translate("errors", "sum_over_time(env:prod)[1h]").unwrap();
        assert_eq!(result.query, r#"sum_over_time(errors{env="prod"}[3600s])"#);
    }

    #[test]
    fn test_min_over_time() {
        let result = translate("temperature", "min_over_time(*)[24h]").unwrap();
        assert_eq!(result.query, "min_over_time(temperature[86400s])");
    }

    #[test]
    fn test_max_over_time() {
        let result = translate("temperature", "max_over_time(*)[24h]").unwrap();
        assert_eq!(result.query, "max_over_time(temperature[86400s])");
    }

    #[test]
    fn test_count_over_time() {
        let result = translate("events", "count_over_time(type:error)[10m]").unwrap();
        assert_eq!(
            result.query,
            r#"count_over_time(events{type="error"}[600s])"#
        );
    }

    // === Nested filter tests ===

    #[test]
    fn test_nested_and_in_parens() {
        let result = translate("metric", "(env:prod AND region:us)").unwrap();
        assert_eq!(result.query, r#"metric{env="prod",region="us"}"#);
    }

    #[test]
    fn test_aggregation_with_nested_filter() {
        let result = translate(
            "cpu",
            "sum((env:prod AND region:us) AND service:api) by (host)",
        )
        .unwrap();
        assert_eq!(
            result.query,
            r#"sum(cpu{env="prod",region="us",service="api"}) by (host)"#
        );
    }

    // === Special character handling ===

    #[test]
    fn test_label_value_with_numbers() {
        let result = translate("metric", "version:v1.2.3").unwrap();
        assert_eq!(result.query, r#"metric{version="v1.2.3"}"#);
    }

    #[test]
    fn test_wildcard_with_dots() {
        // When the prefix has dots, they should be escaped in the regex
        let result = translate("metric", "host:db.server*").unwrap();
        assert_eq!(result.query, r#"metric{host=~"db\.server.*"}"#);
    }

    #[test]
    fn test_label_value_with_dash() {
        let result = translate("metric", "env:us-west-2").unwrap();
        assert_eq!(result.query, r#"metric{env="us-west-2"}"#);
    }

    #[test]
    fn test_label_value_with_underscore() {
        let result = translate("metric", "service:user_service").unwrap();
        assert_eq!(result.query, r#"metric{service="user_service"}"#);
    }

    // === Error cases ===

    #[test]
    fn test_invalid_query_syntax() {
        let result = translate("metric", "invalid query syntax !!!");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failed to parse"));
    }

    #[test]
    fn test_nested_or_not_supported() {
        let result = translate("metric", "(env:prod OR env:staging) AND region:us");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("OR expressions are not supported")
        );
    }

    #[test]
    fn test_not_in_aggregation() {
        let result = translate("metric", "sum(!env:prod)");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("NOT expressions are not supported")
        );
    }

    // === Duration format tests ===

    #[test]
    fn test_rate_with_seconds() {
        let result = translate("metric", "rate(*)[30s]").unwrap();
        assert_eq!(result.query, "rate(metric[30s])");
    }

    #[test]
    fn test_rate_with_days() {
        let result = translate("metric", "avg_over_time(*)[7d]").unwrap();
        assert_eq!(result.query, "avg_over_time(metric[604800s])");
    }

    // === Edge cases ===

    #[test]
    fn test_aggregation_match_all_no_grouping() {
        let result = translate("metric", "sum(*)").unwrap();
        assert_eq!(result.query, "sum(metric)");
    }

    #[test]
    fn test_single_label_grouping() {
        let result = translate("metric", "avg(*) by (host)").unwrap();
        assert_eq!(result.query, "avg(metric) by (host)");
    }

    #[test]
    fn test_many_labels_grouping() {
        let result = translate("metric", "sum(*) by (region, env, service, host)").unwrap();
        assert_eq!(result.query, "sum(metric) by (region,env,service,host)");
    }

    #[test]
    fn test_metric_name_with_underscores() {
        let result = translate("http_requests_total", "env:prod").unwrap();
        assert_eq!(result.query, r#"http_requests_total{env="prod"}"#);
    }

    #[test]
    fn test_metric_name_with_colons() {
        // Prometheus metrics can have colons (recording rules)
        let result = translate("job:request_latency:mean", "*").unwrap();
        assert_eq!(result.query, "job:request_latency:mean");
    }
}
