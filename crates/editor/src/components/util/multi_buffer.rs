//! Multi-buffer editing utilities for synchronized editing across multiple panes.
//!
//! This module provides utilities for multi-buffer find/replace operations,
//! including pattern matching, selection tracking, and text replacement
//! across multiple query panes simultaneously.
//!
//! The multi-buffer approach allows users to:
//! - Select multiple panes using visual-multi mode (`Ctrl+V` + `j/k`)
//! - Open a find/replace modal to edit all selected buffers at once
//! - Preview matches with live highlighting before applying changes

use crate::util::Instant;
use rustc_hash::FxHashMap;

/// A selection range within a buffer (byte offsets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Start byte offset (inclusive)
    pub start: usize,
    /// End byte offset (exclusive)
    pub end: usize,
}

impl Selection {
    /// Create a new selection range.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Get the length of this selection in bytes.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Check if this selection is empty.
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// The current mode of multi-buffer editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MultiBufferMode {
    /// Multi-buffer editing is not active
    #[default]
    Inactive,
    /// User is typing a search pattern (after pressing 's')
    PatternInput,
    /// Matches are highlighted, waiting for action (c to change, d to delete, etc.)
    MatchesHighlighted,
    /// Active editing - keystrokes are applied to all selections
    Editing,
}

/// State for synchronized multi-buffer editing across multiple panes.
#[derive(Debug, Clone, Default)]
pub struct MultiBufferState {
    /// Current editing mode
    pub mode: MultiBufferMode,
    /// The search pattern being typed or that was searched
    pub search_pattern: String,
    /// Selections per pane: pane_id -> Vec<Selection>
    pub selections: FxHashMap<usize, Vec<Selection>>,
    /// Original content per pane (stored when entering editing mode)
    /// Used to apply replacements from a stable base
    pub original_content: FxHashMap<usize, String>,
    /// Accumulated input during editing mode (replacement text)
    pub input_buffer: String,
    /// Time of last input (for debounced chart refresh)
    pub last_input_time: Option<Instant>,
    /// Total match count across all panes
    pub total_matches: usize,
}

impl MultiBufferState {
    /// Create a new inactive multi-buffer state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start pattern input mode.
    pub fn start_pattern_input(&mut self) {
        self.mode = MultiBufferMode::PatternInput;
        self.search_pattern.clear();
        self.selections.clear();
        self.original_content.clear();
        self.input_buffer.clear();
        self.total_matches = 0;
    }

    /// Find all occurrences of the current search pattern in the given content.
    /// Returns a vector of selections for the matches.
    pub fn find_matches(&self, content: &str) -> Vec<Selection> {
        if self.search_pattern.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();
        let pattern = &self.search_pattern;
        let mut start = 0;

        while let Some(pos) = content[start..].find(pattern) {
            let match_start = start + pos;
            let match_end = match_start + pattern.len();
            matches.push(Selection::new(match_start, match_end));
            start = match_end;
        }

        matches
    }

    /// Update selections for a specific pane based on its content.
    /// Also stores the original content for use during editing.
    pub fn update_selections_for_pane(&mut self, pane_id: usize, content: &str) {
        let matches = self.find_matches(content);
        if matches.is_empty() {
            self.selections.remove(&pane_id);
            self.original_content.remove(&pane_id);
        } else {
            self.selections.insert(pane_id, matches);
            // Store original content for this pane
            self.original_content.insert(pane_id, content.to_string());
        }
        self.recalculate_total_matches();
    }

    /// Recalculate the total match count across all panes.
    fn recalculate_total_matches(&mut self) {
        self.total_matches = self.selections.values().map(|v| v.len()).sum();
    }

    /// Finish pattern input and transition to matches highlighted mode.
    pub fn finish_pattern_input(&mut self) {
        if self.total_matches > 0 {
            self.mode = MultiBufferMode::MatchesHighlighted;
        } else {
            // No matches found, return to inactive
            self.reset();
        }
    }

    /// Enter editing mode (after pressing 'c' to change).
    pub fn enter_editing_mode(&mut self) {
        self.mode = MultiBufferMode::Editing;
        self.input_buffer.clear();
        self.last_input_time = None;
    }

    /// Apply replacement text to the original content, replacing all selections.
    /// Returns the new content with all selections replaced.
    /// Uses the stored original content to ensure byte offsets remain valid.
    pub fn apply_replacement(&self, pane_id: usize) -> Option<String> {
        let selections = self.selections.get(&pane_id)?;
        let original = self.original_content.get(&pane_id)?;

        if selections.is_empty() {
            return None;
        }

        // Sort selections by start position (descending) to process from end to start
        // This preserves byte offsets as we replace
        let mut sorted_selections = selections.clone();
        sorted_selections.sort_by(|a, b| b.start.cmp(&a.start));

        let mut result = original.clone();
        for sel in sorted_selections {
            if sel.start <= result.len() && sel.end <= result.len() {
                result.replace_range(sel.start..sel.end, &self.input_buffer);
            }
        }

        Some(result)
    }

    /// Record input and update the last input time.
    pub fn record_input(&mut self, input: &str) {
        self.input_buffer.push_str(input);
        self.last_input_time = Some(Instant::now());
    }

    /// Check if debounce period has elapsed (300ms).
    pub fn should_refresh_charts(&self) -> bool {
        if let Some(last_time) = self.last_input_time {
            last_time.elapsed().as_millis() > 300
        } else {
            false
        }
    }

    /// Clear the last input time after charts have been refreshed.
    pub fn mark_charts_refreshed(&mut self) {
        self.last_input_time = None;
    }

    /// Get selections for a specific pane.
    pub fn get_selections(&self, pane_id: usize) -> Option<&Vec<Selection>> {
        self.selections.get(&pane_id)
    }

    /// Check if a pane has any selections.
    pub fn has_selections(&self, pane_id: usize) -> bool {
        self.selections
            .get(&pane_id)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    /// Get the number of panes with matches.
    pub fn panes_with_matches(&self) -> usize {
        self.selections.len()
    }

    /// Reset to inactive state.
    pub fn reset(&mut self) {
        self.mode = MultiBufferMode::Inactive;
        self.search_pattern.clear();
        self.selections.clear();
        self.original_content.clear();
        self.input_buffer.clear();
        self.last_input_time = None;
        self.total_matches = 0;
    }

    /// Check if multi-buffer editing is currently active (not inactive).
    pub fn is_active(&self) -> bool {
        self.mode != MultiBufferMode::Inactive
    }

    /// Get a status string for display.
    pub fn status_text(&self) -> String {
        match self.mode {
            MultiBufferMode::Inactive => String::new(),
            MultiBufferMode::PatternInput => {
                format!("SEARCH: {}_", self.search_pattern)
            }
            MultiBufferMode::MatchesHighlighted => {
                format!(
                    "MATCHED: \"{}\" ({} in {} panes) [c]hange [Esc]ape",
                    self.search_pattern,
                    self.total_matches,
                    self.panes_with_matches()
                )
            }
            MultiBufferMode::Editing => {
                format!(
                    "EDITING: {} -> \"{}\" ({} selections)",
                    self.search_pattern, self.input_buffer, self.total_matches
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matches_basic() {
        let mut state = MultiBufferState::new();
        state.search_pattern = "env:prod".to_string();

        let content = "env:prod AND service:api env:prod";
        let matches = state.find_matches(content);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], Selection::new(0, 8));
        assert_eq!(matches[1], Selection::new(25, 33));
    }

    #[test]
    fn test_find_matches_no_match() {
        let mut state = MultiBufferState::new();
        state.search_pattern = "env:staging".to_string();

        let content = "env:prod AND service:api";
        let matches = state.find_matches(content);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_matches_empty_pattern() {
        let state = MultiBufferState::new();
        let matches = state.find_matches("some content");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_apply_replacement_single() {
        let mut state = MultiBufferState::new();
        state.search_pattern = "prod".to_string();
        state.input_buffer = "staging".to_string();
        state.selections.insert(1, vec![Selection::new(4, 8)]);
        state.original_content.insert(1, "env:prod".to_string());

        let result = state.apply_replacement(1);
        assert_eq!(result, Some("env:staging".to_string()));
    }

    #[test]
    fn test_apply_replacement_multiple() {
        let mut state = MultiBufferState::new();
        state.search_pattern = "prod".to_string();
        state.input_buffer = "dev".to_string();
        state
            .selections
            .insert(1, vec![Selection::new(4, 8), Selection::new(17, 21)]);
        state
            .original_content
            .insert(1, "env:prod AND env:prod".to_string());

        let result = state.apply_replacement(1);
        assert_eq!(result, Some("env:dev AND env:dev".to_string()));
    }

    #[test]
    fn test_selection_length() {
        let sel = Selection::new(5, 10);
        assert_eq!(sel.len(), 5);
        assert!(!sel.is_empty());

        let empty = Selection::new(5, 5);
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_state_transitions() {
        let mut state = MultiBufferState::new();
        assert_eq!(state.mode, MultiBufferMode::Inactive);
        assert!(!state.is_active());

        state.start_pattern_input();
        assert_eq!(state.mode, MultiBufferMode::PatternInput);
        assert!(state.is_active());

        state.search_pattern = "test".to_string();
        state.selections.insert(1, vec![Selection::new(0, 4)]);
        state.total_matches = 1;
        state.finish_pattern_input();
        assert_eq!(state.mode, MultiBufferMode::MatchesHighlighted);

        state.enter_editing_mode();
        assert_eq!(state.mode, MultiBufferMode::Editing);
        assert!(state.input_buffer.is_empty());

        state.reset();
        assert_eq!(state.mode, MultiBufferMode::Inactive);
        assert!(state.selections.is_empty());
    }
}
