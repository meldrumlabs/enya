//! Tool trait and execution context.
//!
//! Tools are functions that the AI model can call to interact with
//! external systems (Prometheus, codebase, filesystem, etc.).

use serde::{Deserialize, Serialize};

use crate::types::ToolDefinition;

/// Categories for organizing tools.
///
/// Categories help control which tools are available in different contexts.
/// For example, dangerous tools might require explicit user confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// Query and explore metrics (Prometheus queries, label discovery)
    Metrics,
    /// Codebase operations (file reading, search, git history)
    Codebase,
    /// Alert rules and alert state inspection
    Alerts,
    /// Potentially dangerous operations requiring confirmation
    /// (shell execution, creating/modifying alerts, file writes)
    Dangerous,
}

/// Output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolOutput {
    /// Text output (most common)
    Text(String),
    /// Structured JSON output
    Json(serde_json::Value),
}

impl ToolOutput {
    /// Convert to a string representation for sending back to the model.
    #[must_use]
    pub fn to_content(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Json(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
        }
    }
}

impl From<String> for ToolOutput {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for ToolOutput {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<serde_json::Value> for ToolOutput {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}

/// Result type for tool execution.
pub type ToolResult = Result<ToolOutput, ToolError>;

/// Errors that can occur during tool execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ToolError {
    /// Invalid input parameters
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Tool not found
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// Execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Timeout
    #[error("Timeout after {0}ms")]
    Timeout(u64),
}

/// Context provided to tools during execution.
///
/// This is intentionally opaque - the editor will provide a concrete
/// implementation that has access to `QueryExecutor`, `CodebaseManager`, etc.
///
/// Tools downcast this to access specific context types.
pub trait ToolContext: Send + Sync {
    /// Downcast to a concrete type.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// A tool that can be called by the AI model.
///
/// Tools are synchronous - if they need to do async work (like HTTP requests),
/// they should block internally. This matches the polling pattern used by
/// the editor's `QueryExecutor` and `CodebaseManager`.
pub trait AgentTool: Send + Sync {
    /// Unique name for this tool.
    fn name(&self) -> &'static str;

    /// Human-readable description of what this tool does.
    fn description(&self) -> &'static str;

    /// Category this tool belongs to.
    fn category(&self) -> ToolCategory;

    /// JSON Schema for the tool's input parameters.
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given input.
    ///
    /// # Arguments
    /// * `input` - Parsed JSON input matching `input_schema()`
    /// * `ctx` - Context with access to editor state
    ///
    /// # Returns
    /// The tool output or an error.
    fn run(&self, input: serde_json::Value, ctx: &dyn ToolContext) -> ToolResult;

    /// Convert to a tool definition for sending to the LLM.
    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

/// Registry of available tools.
#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<Box<dyn AgentTool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool.
    pub fn register(&mut self, tool: impl AgentTool + 'static) {
        self.tools.push(Box::new(tool));
    }

    /// Get all tool definitions for sending to the LLM.
    #[must_use]
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.to_definition()).collect()
    }

    /// Get tool definitions filtered by allowed categories.
    #[must_use]
    pub fn definitions_for_categories(&self, categories: &[ToolCategory]) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|t| categories.contains(&t.category()))
            .map(|t| t.to_definition())
            .collect()
    }

    /// Get tool definitions excluding certain categories.
    #[must_use]
    pub fn definitions_excluding(&self, excluded: &[ToolCategory]) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|t| !excluded.contains(&t.category()))
            .map(|t| t.to_definition())
            .collect()
    }

    /// Find a tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn AgentTool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(AsRef::as_ref)
    }

    /// Execute a tool by name.
    pub fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        ctx: &dyn ToolContext,
    ) -> ToolResult {
        match self.get(name) {
            Some(tool) => tool.run(input, ctx),
            None => Err(ToolError::NotFound(name.to_string())),
        }
    }

    /// Number of registered tools.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns true if no tools are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl AgentTool for EchoTool {
        fn name(&self) -> &'static str {
            "echo"
        }

        fn description(&self) -> &'static str {
            "Echoes back the input"
        }

        fn category(&self) -> ToolCategory {
            ToolCategory::Codebase
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            })
        }

        fn run(&self, input: serde_json::Value, _ctx: &dyn ToolContext) -> ToolResult {
            let message = input
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidInput("missing 'message'".into()))?;
            Ok(ToolOutput::Text(message.to_string()))
        }
    }

    struct EmptyContext;
    impl ToolContext for EmptyContext {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_tool_registry() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        assert_eq!(registry.len(), 1);
        assert!(registry.get("echo").is_some());
        assert!(registry.get("unknown").is_none());

        let ctx = EmptyContext;
        let result = registry.execute("echo", serde_json::json!({"message": "hello"}), &ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_content(), "hello");
    }
}
