//! Utility modules - non-UI helpers for components.

pub mod ai_provider;
pub mod chat_types;
pub mod diff_rendering;
pub mod diff_widget;
pub mod file_opener;
pub mod finder;
pub mod finder_utils;
pub mod id_generator;
pub mod multi_buffer;
pub mod query_completion;
pub mod query_executor;
pub mod query_state;
pub mod query_validation;
pub mod scroll_shadows;
pub mod syntax_highlight;
pub mod text_formatting;

#[cfg(not(target_arch = "wasm32"))]
pub use ai_provider::ManifestFetcher;
pub use ai_provider::{AiModel, AiProvider, ProviderManifest, migrate_legacy_model_name};
pub use chat_types::{
    ActivityItem, ActivityType, ConversationHandoff, HandoffContextPane, MessageRole,
    ResponseStatus,
};
pub use file_opener::{
    ExternalApp, FileOpenerAction, FileOpenerInline, FileOpenerPopup, FileOpenerResult,
};
pub use finder::{Finder, FinderConfig, FinderItem, FinderResult};
pub use finder_utils::{
    FinderColors, FinderKeyboardInput, OverlayColors, OverlayStyle, OverlayStyleVariant,
    draw_backdrop, draw_separator, draw_separator_colored, render_colored_badge, render_key_badge,
    render_key_badge_large, render_split_header, render_split_panels, render_stat_badge,
    render_stat_badge_with_icon,
};
pub use id_generator::{next_id, next_id_usize};
pub use multi_buffer::{MultiBufferMode, MultiBufferState, Selection};
pub use query_completion::{
    CompletionItem, CompletionKind, CompletionResult, QueryCompletion, QueryLanguage,
};
pub use query_executor::{Backend, ExecuteParams, QueryExecutor, QueryPollResult};
pub use query_state::{Granularity, QueryState};
pub use query_validation::{ValidationResult, is_valid_query, validate_query};
pub use scroll_shadows::{
    ScrollAreaWithShadows, ScrollShadowConfig, ScrollState, render_scroll_shadows,
};
pub use syntax_highlight::SyntaxHighlightData;
#[cfg(not(target_arch = "wasm32"))]
pub use syntax_highlight::{HighlightCache, highlight_line_with_spans};
pub use text_formatting::normalize_unicode;
#[cfg(not(target_arch = "wasm32"))]
pub use text_formatting::{truncate_first_line, truncate_path_suffix};
