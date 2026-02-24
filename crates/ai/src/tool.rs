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

    // -- ToolOutput --

    #[test]
    fn tool_output_text_to_content() {
        let output = ToolOutput::Text("hello".to_string());
        assert_eq!(output.to_content(), "hello");
    }

    #[test]
    fn tool_output_json_to_content() {
        let output = ToolOutput::Json(serde_json::json!({"key": "value"}));
        let content = output.to_content();
        assert!(content.contains("\"key\""));
        assert!(content.contains("\"value\""));
    }

    #[test]
    fn tool_output_from_string() {
        let output: ToolOutput = "hello".to_string().into();
        assert_eq!(output.to_content(), "hello");
    }

    #[test]
    fn tool_output_from_str() {
        let output: ToolOutput = "hello".into();
        assert_eq!(output.to_content(), "hello");
    }

    #[test]
    fn tool_output_from_json_value() {
        let output: ToolOutput = serde_json::json!(42).into();
        assert_eq!(output.to_content(), "42");
    }

    // -- ToolCategory serialization --

    #[test]
    fn tool_category_serialization() {
        assert_eq!(
            serde_json::to_string(&ToolCategory::Metrics).unwrap(),
            "\"metrics\""
        );
        assert_eq!(
            serde_json::to_string(&ToolCategory::Codebase).unwrap(),
            "\"codebase\""
        );
        assert_eq!(
            serde_json::to_string(&ToolCategory::Alerts).unwrap(),
            "\"alerts\""
        );
        assert_eq!(
            serde_json::to_string(&ToolCategory::Dangerous).unwrap(),
            "\"dangerous\""
        );
    }

    #[test]
    fn tool_category_deserialization() {
        let cat: ToolCategory = serde_json::from_str("\"metrics\"").unwrap();
        assert_eq!(cat, ToolCategory::Metrics);
    }

    // -- ToolError --

    #[test]
    fn tool_error_display() {
        assert_eq!(
            ToolError::InvalidInput("bad".into()).to_string(),
            "Invalid input: bad"
        );
        assert_eq!(
            ToolError::NotFound("foo".into()).to_string(),
            "Tool not found: foo"
        );
        assert_eq!(
            ToolError::ExecutionFailed("crash".into()).to_string(),
            "Execution failed: crash"
        );
        assert_eq!(
            ToolError::PermissionDenied("nope".into()).to_string(),
            "Permission denied: nope"
        );
        assert_eq!(ToolError::Timeout(5000).to_string(), "Timeout after 5000ms");
    }

    // -- ToolRegistry --

    #[test]
    fn registry_starts_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.get("echo").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn registry_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let defs = registry.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "echo");
        assert_eq!(defs[0].description, "Echoes back the input");
    }

    #[test]
    fn registry_execute_success() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let ctx = EmptyContext;
        let result = registry.execute("echo", serde_json::json!({"message": "hello"}), &ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_content(), "hello");
    }

    #[test]
    fn registry_execute_not_found() {
        let registry = ToolRegistry::new();
        let ctx = EmptyContext;
        let result = registry.execute("missing", serde_json::json!({}), &ctx);
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::NotFound(name) => assert_eq!(name, "missing"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn registry_execute_invalid_input() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let ctx = EmptyContext;
        // EchoTool requires "message" field
        let result = registry.execute("echo", serde_json::json!({}), &ctx);
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidInput(msg) => assert!(msg.contains("message")),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    struct MetricsTool;
    impl AgentTool for MetricsTool {
        fn name(&self) -> &'static str {
            "query"
        }
        fn description(&self) -> &'static str {
            "Query metrics"
        }
        fn category(&self) -> ToolCategory {
            ToolCategory::Metrics
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        fn run(&self, _input: serde_json::Value, _ctx: &dyn ToolContext) -> ToolResult {
            Ok(ToolOutput::Text("result".into()))
        }
    }

    struct DangerousTool;
    impl AgentTool for DangerousTool {
        fn name(&self) -> &'static str {
            "shell"
        }
        fn description(&self) -> &'static str {
            "Run shell command"
        }
        fn category(&self) -> ToolCategory {
            ToolCategory::Dangerous
        }
        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({"type": "object"})
        }
        fn run(&self, _input: serde_json::Value, _ctx: &dyn ToolContext) -> ToolResult {
            Ok(ToolOutput::Text("done".into()))
        }
    }

    #[test]
    fn registry_definitions_for_categories() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool); // Codebase
        registry.register(MetricsTool); // Metrics
        registry.register(DangerousTool); // Dangerous

        let safe = registry.definitions_for_categories(&[ToolCategory::Metrics]);
        assert_eq!(safe.len(), 1);
        assert_eq!(safe[0].name, "query");

        let multiple =
            registry.definitions_for_categories(&[ToolCategory::Metrics, ToolCategory::Codebase]);
        assert_eq!(multiple.len(), 2);
    }

    #[test]
    fn registry_definitions_excluding() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool); // Codebase
        registry.register(MetricsTool); // Metrics
        registry.register(DangerousTool); // Dangerous

        let safe = registry.definitions_excluding(&[ToolCategory::Dangerous]);
        assert_eq!(safe.len(), 2);
        assert!(safe.iter().all(|d| d.name != "shell"));
    }

    // -- AgentTool::to_definition --

    #[test]
    fn tool_to_definition() {
        let tool = EchoTool;
        let def = tool.to_definition();
        assert_eq!(def.name, "echo");
        assert_eq!(def.description, "Echoes back the input");
        assert!(def.input_schema["properties"]["message"].is_object());
    }

    // -- ToolContext downcast --

    #[test]
    fn tool_context_downcast() {
        struct MyContext {
            value: i32,
        }
        impl ToolContext for MyContext {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let ctx = MyContext { value: 42 };
        let any = ctx.as_any();
        let downcasted = any.downcast_ref::<MyContext>().unwrap();
        assert_eq!(downcasted.value, 42);
    }
}
