//! Full-text search for codebase indexing.
//!
//! This module re-exports types from the `enya-search` crate and provides
//! AI agent tool integration. It is only available on native builds (not WASM).

// Re-export core search types from enya-search
pub use enya_search::{
    IndexError, SearchFilter, SearchResult, SearchResultKind, TantivyCodebaseIndex, TantivyPhase,
    TantivyProgress,
};

// AI agent tool for codebase search
mod tools;
pub use tools::{SearchCodebaseTool, SearchToolContext};
