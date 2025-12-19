//! Overlay components - modal UI that appears on top of the workspace.

pub mod buffer_editor;
pub mod command_palette;
pub mod diagnostics;
pub mod info;
pub mod metrics_finder;
pub mod multi_edit;
pub mod tutorial;
pub mod viewport_filter;
pub mod which_key;
pub mod workspace_finder;

pub use buffer_editor::{BufferEditor, BufferEditorResult};
pub use command_palette::{CommandPalette, CommandResult};
pub use diagnostics::{
    Diagnostic, DiagnosticLevel, DiagnosticSource, DiagnosticsFilter, DiagnosticsPane,
    DiagnosticsPaneAction,
};
pub use info::InfoOverlay;
pub use metrics_finder::{MetricItem, MetricsFinder};
pub use multi_edit::{EditExcerpt, MultiEditOverlay, MultiEditResult};
pub use tutorial::TutorialOverlay;
pub use viewport_filter::{ViewportFilter, ViewportFilterResult};
pub use which_key::WhichKey;
pub use workspace_finder::{WorkspaceFinder, WorkspaceItem};
