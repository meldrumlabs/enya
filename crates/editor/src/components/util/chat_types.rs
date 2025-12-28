//! Shared types for AI chat components.
//!
//! Provides common types used by both the `AgentPanel` overlay and `AgentPane` pane
//! for chat interactions with AI agents.

/// Role of a message sender in a chat conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    /// Message from the user
    User,
    /// Message from the AI assistant
    Assistant,
    /// System message (errors, notifications)
    System,
}

/// Type of activity shown in the agent activity log.
///
/// Activities track the agent's current state and actions during response generation.
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityType {
    /// Agent is thinking/reasoning (with optional thinking text)
    Thinking(String),
    /// Agent is using a tool (tool name, summary of what it's doing)
    ToolUse {
        /// Name of the tool being used
        tool: String,
        /// Brief summary of the tool action
        summary: String,
    },
    /// An error occurred
    Error(String),
    /// Final text response (summary for activity log)
    Response(String),
}

/// An activity item in the agent activity log.
///
/// Activities are displayed below user messages to show what the agent
/// is doing while generating a response.
#[derive(Debug, Clone)]
pub struct ActivityItem {
    /// The type of activity
    pub activity_type: ActivityType,
    /// Whether this activity is still in progress
    pub in_progress: bool,
}

/// Status of the agent's response generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseStatus {
    /// Waiting for response to start
    #[default]
    Waiting,
    /// Agent is thinking/reasoning
    Thinking,
    /// Agent is generating text
    Responding,
    /// Response is complete
    Complete,
}
