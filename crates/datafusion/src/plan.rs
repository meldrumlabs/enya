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
