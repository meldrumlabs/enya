//! YAML alert rule scanner implementation using tree-sitter.
//!
//! Scans YAML files to find Prometheus alerting rules and extracts
//! alert names, expressions, and metadata.

use std::fs;
use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Node, Parser, Query, QueryCursor};

use super::AlertRule;
use crate::parser::ParseError;

/// Tree-sitter query to find alert rule blocks in Prometheus YAML files.
///
/// Matches `block_mapping_pair` nodes where the key is "alert" to find alert definitions.
const ALERT_QUERY: &str = r#"
(block_mapping_pair
  key: (flow_node) @key
  value: (flow_node) @value
  (#eq? @key "alert"))
"#;

/// Scanner for Prometheus alert rules in YAML files using tree-sitter.
///
/// Detects alerting rules in the standard Prometheus alerting rule format:
/// ```yaml
/// groups:
///   - name: example
///     rules:
///       - alert: HighErrorRate
///         expr: rate(errors_total[5m]) > 0.1
///         labels:
///           severity: critical
///         annotations:
///           message: "Error rate is high"
/// ```
pub struct YamlAlertScanner {
    parser: Parser,
}

impl YamlAlertScanner {
    /// Creates a new YAML alert scanner.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the tree-sitter parser cannot be initialized.
    pub fn new() -> Result<Self, ParseError> {
        let mut parser = Parser::new();
        let language = tree_sitter_yaml::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| ParseError(format!("Failed to set YAML language: {e}")))?;
        Ok(Self { parser })
    }

    /// Scan a file for Prometheus alert rules.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the file cannot be read or parsed.
    pub fn scan_file(&mut self, path: &Path) -> Result<Vec<AlertRule>, ParseError> {
        let content = fs::read_to_string(path)
            .map_err(|e| ParseError(format!("Failed to read file: {e}")))?;

        self.scan_content(&content, path)
    }

    /// Scan YAML content for alert rules.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the content cannot be parsed.
    pub fn scan_content(
        &mut self,
        content: &str,
        path: &Path,
    ) -> Result<Vec<AlertRule>, ParseError> {
        let tree = self
            .parser
            .parse(content, None)
            .ok_or_else(|| ParseError("Failed to parse YAML".to_string()))?;

        let query = Query::new(&tree_sitter_yaml::LANGUAGE.into(), ALERT_QUERY)
            .map_err(|e| ParseError(format!("Failed to create query: {e}")))?;

        let mut cursor = QueryCursor::new();
        let mut alerts = Vec::new();

        let value_idx = query
            .capture_index_for_name("value")
            .ok_or_else(|| ParseError("Query missing value capture".to_string()))?;

        let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                if capture.index == value_idx {
                    // Found an alert name, now find the parent block_mapping to get all fields
                    if let Some(alert) = extract_alert_from_context(capture.node, content, path) {
                        alerts.push(alert);
                    }
                }
            }
        }

        Ok(alerts)
    }
}

/// Extract alert information from the context around an alert name node.
fn extract_alert_from_context(
    alert_name_node: Node<'_>,
    content: &str,
    path: &Path,
) -> Option<AlertRule> {
    let alert_name = get_node_text(alert_name_node, content)?;

    // Navigate up to find the containing block_mapping (the rule block)
    let rule_block = find_parent_block_mapping(alert_name_node)?;

    // Extract fields from the rule block
    let expr = find_field_value(rule_block, "expr", content);
    let expr_str = expr?;

    // Extract metric name from the PromQL expression
    let metric_name = enya_promql::extract_metric_name(&expr_str);

    // Look for labels block
    let labels_block = find_nested_block(rule_block, "labels", content);
    let severity = labels_block.and_then(|b| find_field_value(b, "severity", content));

    // Look for annotations block
    let annotations_block = find_nested_block(rule_block, "annotations", content);
    let message = annotations_block.and_then(|b| {
        find_field_value(b, "message", content)
            .or_else(|| find_field_value(b, "description", content))
            .or_else(|| find_field_value(b, "summary", content))
    });
    let runbook_url = annotations_block.and_then(|b| find_field_value(b, "runbook_url", content));

    // Get line number from the alert name's parent pair
    let line = alert_name_node.start_position().row + 1; // 1-indexed

    Some(AlertRule {
        name: alert_name,
        expr: expr_str,
        metric_name,
        severity,
        message,
        runbook_url,
        file: path.to_path_buf(),
        line,
        column: alert_name_node.start_position().column,
    })
}

/// Get the text content of a node, stripping quotes if present.
fn get_node_text(node: Node<'_>, content: &str) -> Option<String> {
    let text = node.utf8_text(content.as_bytes()).ok()?;
    let text = text.trim();

    // Strip surrounding quotes if present
    let text = if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        &text[1..text.len() - 1]
    } else {
        text
    };

    Some(text.to_string())
}

/// Find the parent `block_mapping` node (represents a YAML mapping/object).
fn find_parent_block_mapping(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(n) = current {
        if n.kind() == "block_mapping" {
            return Some(n);
        }
        current = n.parent();
    }
    None
}

/// Find a field value within a `block_mapping` by key name.
fn find_field_value(block: Node<'_>, key_name: &str, content: &str) -> Option<String> {
    let mut cursor = block.walk();

    for child in block.children(&mut cursor) {
        if child.kind() == "block_mapping_pair" {
            if let (Some(key_node), Some(value_node)) = (
                child.child_by_field_name("key"),
                child.child_by_field_name("value"),
            ) {
                if let Some(key_text) = get_node_text(key_node, content) {
                    if key_text == key_name {
                        // For multi-line values (like expr with |), get the full text
                        return get_full_value_text(value_node, content);
                    }
                }
            }
        }
    }
    None
}

/// Get the full text of a value node, handling block scalars (| and >).
fn get_full_value_text(node: Node<'_>, content: &str) -> Option<String> {
    let text = node.utf8_text(content.as_bytes()).ok()?;
    let text = text.trim();

    // Handle block scalar indicators
    if text.starts_with('|') || text.starts_with('>') {
        // For block scalars, we need to get the indented content that follows
        // The node should include the full block scalar content
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() > 1 {
            // Skip the indicator line and join the rest
            let content_lines: Vec<&str> = lines[1..].iter().map(|l| l.trim()).collect();
            return Some(content_lines.join("\n").trim().to_string());
        }
        return Some(String::new());
    }

    // Strip surrounding quotes if present
    let text = if (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('\'') && text.ends_with('\''))
    {
        &text[1..text.len() - 1]
    } else {
        text
    };

    Some(text.to_string())
}

/// Find a nested block mapping by key name (e.g., "labels" or "annotations").
fn find_nested_block<'a>(block: Node<'a>, key_name: &str, content: &str) -> Option<Node<'a>> {
    let mut cursor = block.walk();

    for child in block.children(&mut cursor) {
        if child.kind() == "block_mapping_pair" {
            if let Some(key_node) = child.child_by_field_name("key") {
                if let Some(key_text) = get_node_text(key_node, content) {
                    if key_text == key_name {
                        // The value should be a block_node containing a block_mapping
                        if let Some(value_node) = child.child_by_field_name("value") {
                            return find_block_mapping_in_subtree(value_node);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Find a `block_mapping` node within a subtree.
fn find_block_mapping_in_subtree(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "block_mapping" {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_block_mapping_in_subtree(child) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prometheus_alert() {
        let content = r"
groups:
  - name: example
    rules:
      - alert: HighErrorRate
        expr: rate(errors_total[5m]) > 0.1
        labels:
          severity: critical
        annotations:
          message: Error rate is high
";

        let mut scanner = YamlAlertScanner::new().expect("Failed to create scanner");
        let alerts = scanner
            .scan_content(content, Path::new("test.yaml"))
            .expect("Should parse");

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].name, "HighErrorRate");
        assert_eq!(alerts[0].expr, "rate(errors_total[5m]) > 0.1");
        assert_eq!(alerts[0].metric_name, Some("errors_total".to_string()));
        assert_eq!(alerts[0].severity, Some("critical".to_string()));
        assert_eq!(alerts[0].message, Some("Error rate is high".to_string()));
    }

    #[test]
    fn test_parse_multiple_alerts() {
        let content = r"
groups:
  - name: alerts
    rules:
      - alert: HighLatency
        expr: rate(http_request_duration_seconds[5m]) > 1
        labels:
          severity: warning
      - alert: HighErrorRate
        expr: sum(rate(http_errors_total[5m])) > 10
        labels:
          severity: critical
";

        let mut scanner = YamlAlertScanner::new().expect("Failed to create scanner");
        let alerts = scanner
            .scan_content(content, Path::new("test.yaml"))
            .expect("Should parse");

        assert_eq!(alerts.len(), 2);
        assert_eq!(alerts[0].name, "HighLatency");
        assert_eq!(
            alerts[0].metric_name,
            Some("http_request_duration_seconds".to_string())
        );
        assert_eq!(alerts[1].name, "HighErrorRate");
        assert_eq!(alerts[1].metric_name, Some("http_errors_total".to_string()));
    }

    #[test]
    fn test_parse_atlas_style_alert() {
        let content = r#"
groups:
  - name: atlas
    rules:
      - alert: Atlas Live Consumer Error
        annotations:
          message: "Atlas Live Consumer is returning more than 3 errors per second on pod {{ $labels.kubernetes_pod_name }}"
          runbook_url: https://atlas-docs.ny2.polygon.io/
        expr: |
          sum(rate(atlas_live_consumer_errors_total{status!~"0|1"}[1m])) > 3
"#;

        let mut scanner = YamlAlertScanner::new().expect("Failed to create scanner");
        let alerts = scanner
            .scan_content(content, Path::new("test.yaml"))
            .expect("Should parse");

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].name, "Atlas Live Consumer Error");
        assert_eq!(
            alerts[0].metric_name,
            Some("atlas_live_consumer_errors_total".to_string())
        );
        assert!(alerts[0].message.is_some());
        assert_eq!(
            alerts[0].runbook_url,
            Some("https://atlas-docs.ny2.polygon.io/".to_string())
        );
    }

    #[test]
    fn test_ignores_recording_rules() {
        let content = r"
groups:
  - name: records
    rules:
      - record: job:http_requests:rate5m
        expr: sum(rate(http_requests_total[5m])) by (job)
";

        let mut scanner = YamlAlertScanner::new().expect("Failed to create scanner");
        let alerts = scanner
            .scan_content(content, Path::new("test.yaml"))
            .expect("Should parse");

        assert!(alerts.is_empty());
    }
}
