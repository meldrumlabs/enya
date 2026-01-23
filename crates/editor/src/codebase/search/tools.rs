//! AI agent tools for codebase search.

use std::sync::Arc;

use enya_ai::tool::{AgentTool, ToolCategory, ToolContext, ToolError, ToolOutput, ToolResult};
use parking_lot::RwLock;
use serde_json::json;

use super::{SearchFilter, SearchResult, SearchResultKind, TantivyCodebaseIndex};

/// Search the indexed codebase for metrics, alerts, and commits.
///
/// This tool is exposed to AI agents for querying the codebase index.
pub struct SearchCodebaseTool;

impl AgentTool for SearchCodebaseTool {
    fn name(&self) -> &'static str {
        "search_codebase"
    }

    fn description(&self) -> &'static str {
        "Search the indexed codebase for metrics, alert rules, and git commits. \
         Returns ranked results with file locations, line numbers, and relevance scores. \
         Use this to find where metrics are defined, which alerts reference a metric, \
         or to explore the codebase structure."
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Codebase
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (supports fuzzy matching and full-text search)"
                },
                "filter": {
                    "type": "string",
                    "enum": ["all", "metrics", "alerts", "commits"],
                    "description": "Filter results by type. Default: all"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Default: 20, max: 100"
                }
            },
            "required": ["query"]
        })
    }

    fn run(&self, input: serde_json::Value, ctx: &dyn ToolContext) -> ToolResult {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("missing 'query' parameter".into()))?;

        if query.is_empty() {
            return Err(ToolError::InvalidInput("query cannot be empty".into()));
        }

        let filter = input
            .get("filter")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "metrics" => SearchFilter::Metrics,
                "alerts" => SearchFilter::Alerts,
                "commits" => SearchFilter::Commits,
                _ => SearchFilter::All,
            })
            .unwrap_or(SearchFilter::All);

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n.min(100) as usize)
            .unwrap_or(20);

        // Get the Tantivy index from context
        let search_ctx = ctx
            .as_any()
            .downcast_ref::<SearchToolContext>()
            .ok_or_else(|| ToolError::ExecutionFailed("search context not available".into()))?;

        let index_guard = search_ctx.index.read();
        let tantivy = index_guard
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("codebase index not available".into()))?;

        let results = tantivy.search(query, filter, limit);

        // Format results as JSON
        let output = json!({
            "query": query,
            "filter": format!("{filter:?}").to_lowercase(),
            "result_count": results.len(),
            "results": results.iter().map(format_result).collect::<Vec<_>>()
        });

        Ok(ToolOutput::Json(output))
    }
}

/// Format a search result as JSON for the tool output.
fn format_result(result: &SearchResult) -> serde_json::Value {
    let kind = match &result.kind {
        SearchResultKind::Metric(k) => {
            json!({
                "type": "metric",
                "metric_kind": format!("{k:?}").to_lowercase()
            })
        }
        SearchResultKind::Alert { severity } => {
            json!({
                "type": "alert",
                "severity": severity
            })
        }
        SearchResultKind::Commit {
            hash, timestamp, ..
        } => {
            json!({
                "type": "commit",
                "hash": hash,
                "timestamp": timestamp
            })
        }
    };

    json!({
        "name": result.name,
        "file": result.file.display().to_string(),
        "line": result.line,
        "score": result.score,
        "snippet": result.snippet,
        "kind": kind
    })
}

/// Context for the search tool.
///
/// This provides access to the Tantivy index during tool execution.
/// Uses Arc<RwLock> to allow shared ownership with 'static lifetime.
pub struct SearchToolContext {
    /// Shared reference to the Tantivy index.
    pub index: Arc<RwLock<Option<TantivyCodebaseIndex>>>,
}

impl SearchToolContext {
    /// Creates a new search tool context.
    #[must_use]
    pub fn new(index: Arc<RwLock<Option<TantivyCodebaseIndex>>>) -> Self {
        Self { index }
    }
}

impl ToolContext for SearchToolContext {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_schema() {
        let tool = SearchCodebaseTool;
        let schema = tool.input_schema();

        assert!(schema.get("properties").is_some());
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"], json!(["query"]));
    }

    #[test]
    fn test_tool_metadata() {
        let tool = SearchCodebaseTool;

        assert_eq!(tool.name(), "search_codebase");
        assert_eq!(tool.category(), ToolCategory::Codebase);
        assert!(!tool.description().is_empty());
    }
}
