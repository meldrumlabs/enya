//! Utility modules - non-UI helpers for components.

pub mod ai_provider;
pub mod chat_types;
pub mod finder;
pub mod finder_utils;
pub mod id_generator;
pub mod multi_buffer;
pub mod query_completion;
pub mod query_executor;
pub mod query_state;
pub mod query_validation;
pub mod syntax_highlight;
pub mod text_formatting;

pub use ai_provider::{AiModel, AiProvider};
pub use chat_types::{
    ActivityItem, ActivityType, ConversationHandoff, HandoffContextPane, MessageRole,
    ResponseStatus,
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
pub use syntax_highlight::SyntaxHighlightData;
pub use text_formatting::normalize_unicode;
#[cfg(not(target_arch = "wasm32"))]
pub use text_formatting::{truncate_first_line, truncate_path_suffix};
