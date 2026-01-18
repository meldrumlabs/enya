//! Query plan extraction and visualization.

use std::sync::Arc;
use std::time::Duration;

use rustc_hash::FxHashMap;

use datafusion::physical_plan::displayable;
use datafusion::physical_plan::{ExecutionPlan, ExecutionPlanProperties};

use crate::types::{OperatorMetrics, PlanNode};

/// Extract a plan tree from a DataFusion physical plan.
pub fn extract_plan_tree(plan: &Arc<dyn ExecutionPlan>) -> PlanNode {
    extract_node(plan.as_ref(), true)
}

fn extract_node(plan: &dyn ExecutionPlan, _is_root: bool) -> PlanNode {
    let display = displayable(plan);

    // Get operator name from the plan type
    let operator = plan.name().to_string();

    // Build description from the one-line display
    let description = format!("{}", display.one_line());

    // Extract properties
    let mut properties = FxHashMap::default();

    // Add schema info
    let schema = plan.schema();
    let field_names: Vec<_> = schema.fields().iter().map(|f| f.name().clone()).collect();
    properties.insert("output_columns".to_string(), field_names.join(", "));

    // Add partitioning info
    let partitioning = plan.output_partitioning();
    properties.insert(
        "partitions".to_string(),
        partitioning.partition_count().to_string(),
    );

    // Try to extract metrics if available
    let metrics = plan.metrics().map(|m| {
        let mut op_metrics = OperatorMetrics::default();

        // Aggregate metrics from all partitions
        let agg = m.aggregate_by_name();
        for metric in agg.iter() {
            let name = metric.value().name();
            let value = metric.value().as_usize();
            match name {
                "output_rows" => {
                    op_metrics.output_rows = value;
                }
                "elapsed_compute" => {
                    op_metrics.elapsed_time = Duration::from_nanos(value as u64);
                }
                "spill_count" => {
                    op_metrics.spill_count = value;
                }
                "spilled_bytes" => {
                    op_metrics.spill_bytes = value;
                }
                "mem_used" => {
                    op_metrics.memory_bytes = value;
                }
                _ => {}
            }
        }

        op_metrics
    });

    // Recursively process children
    let children = plan
        .children()
        .iter()
        .map(|child| extract_node(child.as_ref(), false))
        .collect();

    PlanNode {
        operator,
        description,
        properties,
        children,
        metrics,
    }
}

/// Format a plan tree as indented text.
pub fn format_plan_tree(node: &PlanNode, indent: usize) -> String {
    let mut output = String::new();
    format_node(&mut output, node, indent, true);
    output
}

fn format_node(output: &mut String, node: &PlanNode, indent: usize, is_last: bool) {
    let prefix = if indent == 0 {
        String::new()
    } else {
        let mut p = "  ".repeat(indent - 1);
        p.push_str(if is_last { "└─ " } else { "├─ " });
        p
    };

    // Operator line
    output.push_str(&prefix);
    output.push_str(&node.operator);

    // Add metrics if available
    if let Some(metrics) = &node.metrics {
        output.push_str(&format!(
            " [rows={}, time={:?}]",
            metrics.output_rows, metrics.elapsed_time
        ));
    }

    output.push('\n');

    // Description on next line if non-empty
    if !node.description.is_empty() && node.description != node.operator {
        let desc_prefix = "  ".repeat(indent);
        output.push_str(&desc_prefix);
        output.push_str("  ");
        output.push_str(&node.description);
        output.push('\n');
    }

    // Children
    let child_count = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        format_node(output, child, indent + 1, i == child_count - 1);
    }
}

/// Calculate the total time spent across all operators.
pub fn total_plan_time(node: &PlanNode) -> Duration {
    let self_time = node
        .metrics
        .as_ref()
        .map_or(Duration::ZERO, |m| m.elapsed_time);
    let child_time: Duration = node.children.iter().map(total_plan_time).sum();
    self_time + child_time
}

/// Find the operator with the highest elapsed time.
pub fn find_bottleneck(node: &PlanNode) -> Option<&PlanNode> {
    find_bottleneck_inner(node, None)
}

fn find_bottleneck_inner<'a>(
    node: &'a PlanNode,
    current_max: Option<&'a PlanNode>,
) -> Option<&'a PlanNode> {
    let node_time = node
        .metrics
        .as_ref()
        .map_or(Duration::ZERO, |m| m.elapsed_time);
    let max_time = current_max
        .and_then(|n| n.metrics.as_ref())
        .map_or(Duration::ZERO, |m| m.elapsed_time);

    let mut best = if node_time > max_time {
        Some(node)
    } else {
        current_max
    };

    for child in &node.children {
        best = find_bottleneck_inner(child, best);
    }

    best
}

// =============================================================================
// Plan Text Parsing
// =============================================================================

/// Parse EXPLAIN output text into a PlanNode structure.
///
/// Format is typically:
/// ```text
/// ProjectionExec: ...
///   FilterExec: ...
///     TableScan: ...
/// ```
pub fn parse_plan_text(text: &str) -> PlanNode {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return PlanNode {
            operator: "EmptyPlan".to_string(),
            description: String::new(),
            properties: FxHashMap::default(),
            children: vec![],
            metrics: None,
        };
    }

    // Parse with a stack-based approach
    let mut root: Option<PlanNode> = None;
    let mut stack: Vec<(usize, PlanNode)> = Vec::new();

    for line in lines {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let depth = indent / 2; // Assume 2-space indentation

        // Parse operator and description
        let (operator, description) = if let Some(colon_pos) = trimmed.find(':') {
            let op = trimmed[..colon_pos].trim().to_string();
            let desc = trimmed[colon_pos + 1..].trim().to_string();
            (op, desc)
        } else {
            (trimmed.to_string(), String::new())
        };

        // Parse metrics if present
        let metrics = parse_metrics(&description);

        let node = PlanNode {
            operator,
            description: description.clone(),
            properties: FxHashMap::default(),
            children: vec![],
            metrics,
        };

        // Pop nodes from stack that are at same or deeper level
        while let Some((d, _)) = stack.last() {
            if *d >= depth {
                let (_, child) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.insert(0, child);
                } else {
                    root = Some(child);
                }
            } else {
                break;
            }
        }

        stack.push((depth, node));
    }

    // Pop remaining nodes
    while let Some((_, child)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.insert(0, child);
        } else {
            root = Some(child);
        }
    }

    root.unwrap_or(PlanNode {
        operator: "Unknown".to_string(),
        description: String::new(),
        properties: FxHashMap::default(),
        children: vec![],
        metrics: None,
    })
}

/// Parse metrics from a description string.
///
/// Handles formats like:
/// - `metrics=[output_rows=5, elapsed_compute=52.06µs, output_bytes=1920.0 B]`
/// - `[rows=100, time=5.2ms, mem=1KB]`
pub fn parse_metrics(description: &str) -> Option<OperatorMetrics> {
    if !description.contains('[') {
        return None;
    }

    let mut metrics = OperatorMetrics::default();
    let mut found = false;

    // Parse output_rows (new format) or rows (old format)
    if let Some(rows) = parse_metric_usize(description, "output_rows=")
        .or_else(|| parse_metric_usize(description, "rows="))
    {
        metrics.output_rows = rows;
        found = true;
    }

    // Parse elapsed_compute (new format) or time (old format)
    if let Some(duration) = parse_metric_duration(description, "elapsed_compute=")
        .or_else(|| parse_metric_duration(description, "time="))
    {
        metrics.elapsed_time = duration;
        found = true;
    }

    // Parse output_bytes (new format) or mem (old format)
    if let Some(bytes) = parse_metric_bytes(description, "output_bytes=")
        .or_else(|| parse_metric_bytes(description, "mem="))
    {
        metrics.memory_bytes = bytes;
        found = true;
    }

    // Parse spill metrics
    if let Some(spill_count) = parse_metric_usize(description, "spill_count=") {
        metrics.spill_count = spill_count;
        found = true;
    }
    if let Some(spill_bytes) = parse_metric_bytes(description, "spilled_bytes=") {
        metrics.spill_bytes = spill_bytes;
        found = true;
    }

    if found { Some(metrics) } else { None }
}

/// Parse a usize metric value from a string.
pub fn parse_metric_usize(description: &str, key: &str) -> Option<usize> {
    let start = description.find(key)?;
    let rest = &description[start + key.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Parse a duration metric value (e.g., "52.06µs", "5.2ms", "1.5s").
pub fn parse_metric_duration(description: &str, key: &str) -> Option<Duration> {
    let start = description.find(key)?;
    let rest = &description[start + key.len()..];

    // Find end of numeric part
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());

    let time_val: f64 = rest[..end].parse().ok()?;
    let unit = &rest[end..];

    let duration = if unit.starts_with("ms") {
        Duration::from_secs_f64(time_val / 1000.0)
    } else if unit.starts_with("µs") || unit.starts_with("us") {
        Duration::from_secs_f64(time_val / 1_000_000.0)
    } else if unit.starts_with("ns") {
        Duration::from_nanos(time_val as u64)
    } else if unit.starts_with('s') {
        Duration::from_secs_f64(time_val)
    } else {
        // Default to microseconds
        Duration::from_secs_f64(time_val / 1_000_000.0)
    };

    Some(duration)
}

/// Parse a bytes metric value (e.g., "1920.0 B", "1.5 KB", "256 MB").
pub fn parse_metric_bytes(description: &str, key: &str) -> Option<usize> {
    let start = description.find(key)?;
    let rest = &description[start + key.len()..];

    // Find end of numeric part (allow spaces before unit)
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != ' ')
        .unwrap_or(rest.len());

    let num_str = rest[..end].trim();
    let mem_val: f64 = num_str.parse().ok()?;

    // Get the unit (skip any spaces)
    let unit = rest[end..].trim_start();

    let bytes = if unit.starts_with("KB") || unit.starts_with("KiB") {
        (mem_val * 1024.0) as usize
    } else if unit.starts_with("MB") || unit.starts_with("MiB") {
        (mem_val * 1024.0 * 1024.0) as usize
    } else if unit.starts_with("GB") || unit.starts_with("GiB") {
        (mem_val * 1024.0 * 1024.0 * 1024.0) as usize
    } else if unit.starts_with('B') {
        mem_val as usize
    } else {
        // Default to bytes
        mem_val as usize
    };

    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_node() {
        let node = PlanNode {
            operator: "ProjectionExec".to_string(),
            description: "a, b, c".to_string(),
            properties: FxHashMap::default(),
            children: vec![],
            metrics: None,
        };

        let output = format_plan_tree(&node, 0);
        assert!(output.contains("ProjectionExec"));
    }
}
