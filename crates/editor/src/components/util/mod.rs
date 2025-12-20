//! Utility modules - non-UI helpers for components.

pub mod finder;
pub mod finder_utils;
pub mod id_generator;
pub mod multi_buffer;
pub mod query_completion;
pub mod query_executor;
pub mod query_state;
pub mod query_validation;

pub use finder::{Finder, FinderConfig, FinderItem, FinderResult};
pub use finder_utils::{
    FinderColors, FinderKeyboardInput, OverlayColors, OverlayStyle, OverlayStyleVariant,
    draw_backdrop, draw_separator, draw_separator_colored, render_key_badge,
    render_key_badge_large,
};
pub use id_generator::{next_id, next_id_usize};
pub use multi_buffer::{MultiBufferMode, MultiBufferState, Selection};
pub use query_completion::{CompletionItem, CompletionKind, CompletionResult, QueryCompletion};
pub use query_executor::{Backend, ExecuteParams, QueryExecutor, QueryPollResult};
pub use query_state::{Granularity, QueryState};
pub use query_validation::{QueryValidator, ValidationResult, is_valid_query, validate_query};
