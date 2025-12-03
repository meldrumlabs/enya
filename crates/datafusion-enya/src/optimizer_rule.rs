//! Physical optimizer rule that instruments execution plans with metrics collection.

use crate::MetricsExecWrapper;
use datafusion::common::Result;
use datafusion::config::ConfigOptions;
use datafusion::physical_optimizer::PhysicalOptimizerRule;
use datafusion::physical_plan::ExecutionPlan;
use std::sync::Arc;

/// A physical optimizer rule that wraps the root execution plan with a
/// [`MetricsExecWrapper`] to automatically record DataFusion metrics.
///
/// # How It Works
///
/// This rule runs as the last physical optimizer rule and wraps the root of
/// the execution plan tree. When the query executes and the stream completes,
/// the wrapper harvests metrics from all operators and records them via
/// the `metrics` crate.
///
/// # Metrics Recorded
///
/// - `datafusion.output_rows` - Rows output by each operator
/// - `datafusion.output_bytes` - Bytes output by each operator
/// - `datafusion.elapsed_compute_ns` - CPU time spent in computation
/// - `datafusion.bytes_scanned` - Bytes read from data sources
/// - `datafusion.row_groups_pruned_*` - Parquet predicate pushdown stats
/// - `datafusion.spilled_bytes` - Memory spill statistics
///
/// # Example
///
/// ```ignore
/// use datafusion::execution::SessionStateBuilder;
/// use datafusion::prelude::SessionContext;
/// use datafusion_enya::EnyaPhysicalOptimizerRule;
/// use std::sync::Arc;
///
/// let rule = Arc::new(EnyaPhysicalOptimizerRule::new());
/// let state = SessionStateBuilder::new()
///     .with_default_features()
///     .with_physical_optimizer_rule(rule)
///     .build();
/// let ctx = SessionContext::new_with_state(state);
/// ```
#[derive(Debug, Clone)]
pub struct EnyaPhysicalOptimizerRule {
    /// Optional query ID for metric correlation.
    query_id: Option<String>,
}

impl EnyaPhysicalOptimizerRule {
    /// Creates a new optimizer rule without a query ID.
    pub fn new() -> Self {
        Self { query_id: None }
    }

    /// Creates a new optimizer rule with a query ID for metric correlation.
    ///
    /// The query ID is added as a tag to all metrics, allowing you to
    /// correlate metrics across queries (e.g., per commit, test run, etc.).
    pub fn with_query_id(query_id: impl Into<String>) -> Self {
        Self {
            query_id: Some(query_id.into()),
        }
    }
}

impl Default for EnyaPhysicalOptimizerRule {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicalOptimizerRule for EnyaPhysicalOptimizerRule {
    fn optimize(
        &self,
        plan: Arc<dyn ExecutionPlan>,
        _config: &ConfigOptions,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        // Wrap the root plan with our metrics wrapper
        let wrapped = if let Some(ref qid) = self.query_id {
            MetricsExecWrapper::with_query_id(plan, qid.clone())
        } else {
            MetricsExecWrapper::new(plan)
        };

        Ok(Arc::new(wrapped))
    }

    fn name(&self) -> &str {
        "EnyaPhysicalOptimizerRule"
    }

    fn schema_check(&self) -> bool {
        true
    }
}
