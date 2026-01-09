//! Overlay components - modal UI that appears on top of the workspace.

pub mod agent_context;
pub mod agent_panel;
pub mod annotation_editor;
pub mod buffer_editor;
#[cfg(not(target_arch = "wasm32"))]
pub mod codebase_finder;
pub mod command_palette;
pub mod diagnostics;
#[cfg(not(target_arch = "wasm32"))]
pub mod diff_viewer;
pub mod info;
pub mod metrics_finder;
pub mod multi_edit;
#[cfg(target_arch = "wasm32")]
pub mod native_promo;
#[cfg(not(target_arch = "wasm32"))]
mod preview;
pub mod slash_commands;
pub mod source_preview;
#[cfg(not(target_arch = "wasm32"))]
mod syntax_highlight;
pub mod tutorial;
pub mod unified_finder;
pub mod viewport_filter;
pub mod which_key;
pub mod workspace_creator;
pub mod workspace_finder;

#[cfg(not(target_arch = "wasm32"))]
pub use agent_context::build_codebase_context;
pub use agent_context::{
    AgentCommand, CodebaseContext, ConnectionContext, DashboardContext, EditorContext,
    build_connection_context, build_dashboard_context, parse_commands, strip_command_blocks,
};
pub use agent_panel::{AgentPanel, AgentPanelResult, ChatMessage};
#[cfg(not(target_arch = "wasm32"))]
pub use codebase_finder::{CodebaseFinder, CodebaseFinderResult, CodebaseFinderStatus};
// Re-export shared types from util for backwards compatibility
pub use super::util::{AiProvider, MessageRole};
pub use annotation_editor::{AnnotationEditor, AnnotationEditorResult};
pub use buffer_editor::{BufferEditor, BufferEditorResult};
pub use command_palette::{CommandKind, CommandPalette, CommandResult, PaletteCommand};
pub use diagnostics::{
    Diagnostic, DiagnosticLevel, DiagnosticSource, DiagnosticsFilter, DiagnosticsPane,
    DiagnosticsPaneAction,
};
#[cfg(not(target_arch = "wasm32"))]
pub use diff_viewer::{DiffViewerOverlay, DiffViewerResult};
pub use info::InfoOverlay;
pub use metrics_finder::{MetricItem, MetricsFinder};
pub use multi_edit::{EditExcerpt, MultiEditOverlay, MultiEditResult};
#[cfg(target_arch = "wasm32")]
pub use native_promo::NativePromoOverlay;
pub use slash_commands::{
    SLASH_COMMANDS, SlashCommand, SlashCommandCategory, SlashCommandPopup, SlashCommandResult,
};
pub use source_preview::{SourcePreviewOverlay, SourcePreviewResult};
pub use tutorial::TutorialOverlay;
pub use unified_finder::{FinderMode, UnifiedFinder, UnifiedFinderAction, UnifiedResult};
pub use viewport_filter::{ViewportFilter, ViewportFilterResult};
pub use which_key::WhichKey;
pub use workspace_creator::{WorkspaceCreator, WorkspaceCreatorResult};
pub use workspace_finder::{WorkspaceFinder, WorkspaceItem};
