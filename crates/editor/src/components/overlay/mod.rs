//! Overlay components - modal UI that appears on top of the workspace.

pub mod agent_context;
pub mod agent_panel;
pub mod buffer_editor;
pub mod command_palette;
pub mod diagnostics;
pub mod info;
pub mod metrics_finder;
pub mod multi_edit;
pub mod source_preview;
pub mod tutorial;
pub mod viewport_filter;
pub mod which_key;
pub mod workspace_finder;

#[cfg(not(target_arch = "wasm32"))]
pub use agent_context::build_codebase_context;
pub use agent_context::{
    AgentCommand, CodebaseContext, ConnectionContext, DashboardContext, EditorContext,
    build_connection_context, build_dashboard_context, parse_commands, strip_command_blocks,
};
pub use agent_panel::{AgentPanel, AgentPanelResult, ChatMessage};
// Re-export shared types from util for backwards compatibility
pub use super::util::{AiProvider, MessageRole};
pub use buffer_editor::{BufferEditor, BufferEditorResult};
pub use command_palette::{CommandPalette, CommandResult};
pub use diagnostics::{
    Diagnostic, DiagnosticLevel, DiagnosticSource, DiagnosticsFilter, DiagnosticsPane,
    DiagnosticsPaneAction,
};
pub use info::InfoOverlay;
pub use metrics_finder::{MetricItem, MetricsFinder};
pub use multi_edit::{EditExcerpt, MultiEditOverlay, MultiEditResult};
pub use source_preview::{SourcePreviewOverlay, SourcePreviewResult};
pub use tutorial::TutorialOverlay;
pub use viewport_filter::{ViewportFilter, ViewportFilterResult};
pub use which_key::WhichKey;
pub use workspace_finder::{WorkspaceFinder, WorkspaceItem};
