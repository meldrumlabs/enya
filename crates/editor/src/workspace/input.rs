//! Keyboard input handling for workspace navigation.
//!
//! This module provides vim-style navigation state and direction handling
//! for navigating between panes in the workspace.

use rustc_hash::FxHashSet;

use egui_tiles::TileId;

use crate::util::Instant;

/// Default timeout for leader key sequences (500ms).
pub const LEADER_KEY_TIMEOUT_MS: u128 = 500;

/// Direction for vim-style navigation between panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

impl NavDirection {
    /// Returns true if this is a horizontal direction (Left or Right).
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// Returns true if this is a vertical direction (Up or Down).
    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    /// Returns the opposite direction.
    pub fn opposite(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }
}

/// Tracks the state of leader key sequences (like `t5`, `Space+m`, `yy`, `cv`, `gd`, `aw`).
///
/// Leader keys are keys that must be followed by another key within a timeout
/// to trigger an action. This struct manages the timing and state for all
/// leader key sequences in the workspace.
#[derive(Debug, Clone, Default)]
pub struct LeaderKeyState {
    /// Last time 'y' was pressed (for yy detection)
    pub last_y_press: Option<Instant>,
    /// Last time 'c' was pressed (for cv detection - cycle visualization)
    pub last_c_press: Option<Instant>,
    /// Last time Space was pressed (for leader key sequences like Space+m, Space+q)
    pub last_space_press: Option<Instant>,
    /// Last time 't' was pressed (for time range shortcuts like t5, th, td)
    pub last_t_press: Option<Instant>,
    /// Last time 'g' was pressed (for go-to shortcuts like gd)
    pub last_g_press: Option<Instant>,
    /// Last time 'a' was pressed (for agent operator shortcuts like aw, ae, ay)
    pub last_a_press: Option<Instant>,
    /// Last time Ctrl+W was pressed (for vim-style window management like Ctrl+W H/J/K/L)
    pub last_ctrl_w_press: Option<Instant>,
    /// Last time Ctrl+W t was pressed (for moving pane into tab with neighbor)
    pub last_ctrl_w_t_press: Option<Instant>,
}

impl LeaderKeyState {
    /// Create a new empty leader key state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that 'y' was pressed.
    pub fn press_y(&mut self) {
        self.last_y_press = Some(Instant::now());
    }

    /// Record that 'c' was pressed.
    pub fn press_c(&mut self) {
        self.last_c_press = Some(Instant::now());
    }

    /// Record that Space was pressed.
    pub fn press_space(&mut self) {
        self.last_space_press = Some(Instant::now());
    }

    /// Record that 't' was pressed.
    pub fn press_t(&mut self) {
        self.last_t_press = Some(Instant::now());
    }

    /// Record that 'g' was pressed.
    pub fn press_g(&mut self) {
        self.last_g_press = Some(Instant::now());
    }

    /// Record that 'a' was pressed.
    pub fn press_a(&mut self) {
        self.last_a_press = Some(Instant::now());
    }

    /// Record that Ctrl+W was pressed.
    pub fn press_ctrl_w(&mut self) {
        self.last_ctrl_w_press = Some(Instant::now());
    }

    /// Record that Ctrl+W t was pressed (tab mode).
    pub fn press_ctrl_w_t(&mut self) {
        self.last_ctrl_w_t_press = Some(Instant::now());
    }

    /// Clear the 'y' leader key state.
    pub fn clear_y(&mut self) {
        self.last_y_press = None;
    }

    /// Clear the 'c' leader key state.
    pub fn clear_c(&mut self) {
        self.last_c_press = None;
    }

    /// Clear the Space leader key state.
    pub fn clear_space(&mut self) {
        self.last_space_press = None;
    }

    /// Clear the 't' leader key state.
    pub fn clear_t(&mut self) {
        self.last_t_press = None;
    }

    /// Clear the 'g' leader key state.
    pub fn clear_g(&mut self) {
        self.last_g_press = None;
    }

    /// Clear the 'a' leader key state.
    pub fn clear_a(&mut self) {
        self.last_a_press = None;
    }

    /// Clear the Ctrl+W leader key state.
    pub fn clear_ctrl_w(&mut self) {
        self.last_ctrl_w_press = None;
    }

    /// Clear the Ctrl+W t leader key state.
    pub fn clear_ctrl_w_t(&mut self) {
        self.last_ctrl_w_t_press = None;
    }

    /// Check if 'yy' sequence is active (second 'y' within timeout).
    pub fn is_yy_active(&self) -> bool {
        self.is_active(self.last_y_press)
    }

    /// Check if 'cv' sequence is ready (after 'c' was pressed within timeout).
    pub fn is_cv_ready(&self) -> bool {
        self.is_active(self.last_c_press)
    }

    /// Check if Space leader key is active.
    pub fn is_space_active(&self) -> bool {
        self.is_active(self.last_space_press)
    }

    /// Check if 't' leader key is active.
    pub fn is_t_active(&self) -> bool {
        self.is_active(self.last_t_press)
    }

    /// Check if 'g' leader key is active.
    pub fn is_g_active(&self) -> bool {
        self.is_active(self.last_g_press)
    }

    /// Check if 'a' leader key is active (for agent operators like aw, ae).
    pub fn is_a_active(&self) -> bool {
        self.is_active(self.last_a_press)
    }

    /// Check if Ctrl+W leader key is active (for window management like Ctrl+W H/J/K/L).
    pub fn is_ctrl_w_active(&self) -> bool {
        self.is_active(self.last_ctrl_w_press)
    }

    /// Check if Ctrl+W t leader key is active (for tab merging like Ctrl+W t h/j/k/l).
    pub fn is_ctrl_w_t_active(&self) -> bool {
        self.is_active(self.last_ctrl_w_t_press)
    }

    /// Check if a leader key press is still within the timeout window.
    fn is_active(&self, last_press: Option<Instant>) -> bool {
        last_press.is_some_and(|last| {
            Instant::now().duration_since(last).as_millis() < LEADER_KEY_TIMEOUT_MS
        })
    }
}

/// State for visual multi-select mode.
///
/// Allows selecting multiple panes for batch operations
/// (e.g., find & replace across queries, close multiple panes).
#[derive(Debug, Clone, Default)]
pub struct VisualMultiState {
    /// The panes that are currently selected
    pub selected_tile_ids: FxHashSet<TileId>,
    /// The pane that currently has the cursor (for j/k navigation)
    pub cursor_tile_id: Option<TileId>,
}

impl VisualMultiState {
    /// Create a new visual multi state with the given starting pane
    pub fn new(starting_tile_id: TileId) -> Self {
        let mut selected = FxHashSet::default();
        selected.insert(starting_tile_id);
        Self {
            selected_tile_ids: selected,
            cursor_tile_id: Some(starting_tile_id),
        }
    }

    /// Toggle selection of a pane
    pub fn toggle_selection(&mut self, tile_id: TileId) {
        if self.selected_tile_ids.contains(&tile_id) {
            self.selected_tile_ids.remove(&tile_id);
        } else {
            self.selected_tile_ids.insert(tile_id);
        }
    }

    /// Check if a pane is selected
    pub fn is_selected(&self, tile_id: TileId) -> bool {
        self.selected_tile_ids.contains(&tile_id)
    }

    /// Get the number of selected panes
    pub fn selection_count(&self) -> usize {
        self.selected_tile_ids.len()
    }

    /// Move cursor to a new pane
    pub fn set_cursor(&mut self, tile_id: TileId) {
        self.cursor_tile_id = Some(tile_id);
    }

    /// Select all given panes
    pub fn select_all(&mut self, tile_ids: &[TileId]) {
        for &tile_id in tile_ids {
            self.selected_tile_ids.insert(tile_id);
        }
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected_tile_ids.clear();
    }
}

/// Target of keyboard focus in section-based workspace navigation.
///
/// When using collapsible sections, focus can be either on a section header
/// (for expand/collapse operations) or on a specific pane within a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusTarget {
    /// No focus (initial state or after clearing)
    #[default]
    None,
    /// Focus is on a section header (by section index)
    SectionHeader(usize),
    /// Focus is on a specific pane within a section
    Pane {
        /// Index of the section containing the pane
        section: usize,
        /// Index of the pane within the section
        pane: usize,
    },
}

impl FocusTarget {
    /// Create a focus target for the first pane of the first section
    pub fn first() -> Self {
        Self::Pane {
            section: 0,
            pane: 0,
        }
    }

    /// Get the section index this focus target belongs to (if any)
    pub fn section_index(&self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::SectionHeader(idx) => Some(*idx),
            Self::Pane { section, .. } => Some(*section),
        }
    }

    /// Check if focus is on a section header
    pub fn is_section_header(&self) -> bool {
        matches!(self, Self::SectionHeader(_))
    }

    /// Check if focus is on a pane
    pub fn is_pane(&self) -> bool {
        matches!(self, Self::Pane { .. })
    }

    /// Get the pane index if focus is on a pane
    pub fn pane_index(&self) -> Option<usize> {
        match self {
            Self::Pane { pane, .. } => Some(*pane),
            _ => None,
        }
    }
}

/// Runtime state for a section (separate from persisted config).
///
/// This tracks transient state like collapsed state that may differ
/// from the initial config during a session.
#[derive(Debug, Clone, Default)]
pub struct SectionState {
    /// Whether the section is currently collapsed
    pub collapsed: bool,
}

impl SectionState {
    /// Create a new section state with the given collapsed value
    pub fn new(collapsed: bool) -> Self {
        Self { collapsed }
    }

    /// Toggle the collapsed state
    pub fn toggle(&mut self) {
        self.collapsed = !self.collapsed;
    }

    /// Expand the section (set collapsed to false)
    pub fn expand(&mut self) {
        self.collapsed = false;
    }

    /// Collapse the section (set collapsed to true)
    pub fn collapse(&mut self) {
        self.collapsed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== NavDirection Tests ====================

    #[test]
    fn test_nav_direction_is_horizontal() {
        assert!(NavDirection::Left.is_horizontal());
        assert!(NavDirection::Right.is_horizontal());
        assert!(!NavDirection::Up.is_horizontal());
        assert!(!NavDirection::Down.is_horizontal());
    }

    #[test]
    fn test_nav_direction_is_vertical() {
        assert!(NavDirection::Up.is_vertical());
        assert!(NavDirection::Down.is_vertical());
        assert!(!NavDirection::Left.is_vertical());
        assert!(!NavDirection::Right.is_vertical());
    }

    #[test]
    fn test_nav_direction_opposite() {
        assert_eq!(NavDirection::Left.opposite(), NavDirection::Right);
        assert_eq!(NavDirection::Right.opposite(), NavDirection::Left);
        assert_eq!(NavDirection::Up.opposite(), NavDirection::Down);
        assert_eq!(NavDirection::Down.opposite(), NavDirection::Up);
    }

    #[test]
    fn test_nav_direction_opposite_is_symmetric() {
        for dir in [
            NavDirection::Left,
            NavDirection::Right,
            NavDirection::Up,
            NavDirection::Down,
        ] {
            assert_eq!(dir.opposite().opposite(), dir);
        }
    }

    #[test]
    fn test_nav_direction_equality() {
        assert_eq!(NavDirection::Left, NavDirection::Left);
        assert_ne!(NavDirection::Left, NavDirection::Right);
    }

    // ==================== LeaderKeyState Tests ====================

    #[test]
    fn test_leader_key_state_new() {
        let state = LeaderKeyState::new();
        assert!(state.last_y_press.is_none());
        assert!(state.last_c_press.is_none());
        assert!(state.last_space_press.is_none());
        assert!(state.last_t_press.is_none());
    }

    #[test]
    fn test_leader_key_state_default() {
        let state = LeaderKeyState::default();
        assert!(state.last_y_press.is_none());
        assert!(state.last_c_press.is_none());
        assert!(state.last_space_press.is_none());
        assert!(state.last_t_press.is_none());
    }

    #[test]
    fn test_press_y_sets_timestamp() {
        let mut state = LeaderKeyState::new();
        assert!(state.last_y_press.is_none());
        state.press_y();
        assert!(state.last_y_press.is_some());
    }

    #[test]
    fn test_press_c_sets_timestamp() {
        let mut state = LeaderKeyState::new();
        assert!(state.last_c_press.is_none());
        state.press_c();
        assert!(state.last_c_press.is_some());
    }

    #[test]
    fn test_press_space_sets_timestamp() {
        let mut state = LeaderKeyState::new();
        assert!(state.last_space_press.is_none());
        state.press_space();
        assert!(state.last_space_press.is_some());
    }

    #[test]
    fn test_press_t_sets_timestamp() {
        let mut state = LeaderKeyState::new();
        assert!(state.last_t_press.is_none());
        state.press_t();
        assert!(state.last_t_press.is_some());
    }

    #[test]
    fn test_clear_y() {
        let mut state = LeaderKeyState::new();
        state.press_y();
        assert!(state.last_y_press.is_some());
        state.clear_y();
        assert!(state.last_y_press.is_none());
    }

    #[test]
    fn test_clear_c() {
        let mut state = LeaderKeyState::new();
        state.press_c();
        assert!(state.last_c_press.is_some());
        state.clear_c();
        assert!(state.last_c_press.is_none());
    }

    #[test]
    fn test_clear_space() {
        let mut state = LeaderKeyState::new();
        state.press_space();
        assert!(state.last_space_press.is_some());
        state.clear_space();
        assert!(state.last_space_press.is_none());
    }

    #[test]
    fn test_clear_t() {
        let mut state = LeaderKeyState::new();
        state.press_t();
        assert!(state.last_t_press.is_some());
        state.clear_t();
        assert!(state.last_t_press.is_none());
    }

    #[test]
    fn test_is_yy_active_when_just_pressed() {
        let mut state = LeaderKeyState::new();
        state.press_y();
        assert!(state.is_yy_active());
    }

    #[test]
    fn test_is_yy_active_when_not_pressed() {
        let state = LeaderKeyState::new();
        assert!(!state.is_yy_active());
    }

    #[test]
    fn test_is_cv_ready_when_just_pressed() {
        let mut state = LeaderKeyState::new();
        state.press_c();
        assert!(state.is_cv_ready());
    }

    #[test]
    fn test_is_cv_ready_when_not_pressed() {
        let state = LeaderKeyState::new();
        assert!(!state.is_cv_ready());
    }

    #[test]
    fn test_is_space_active_when_just_pressed() {
        let mut state = LeaderKeyState::new();
        state.press_space();
        assert!(state.is_space_active());
    }

    #[test]
    fn test_is_space_active_when_not_pressed() {
        let state = LeaderKeyState::new();
        assert!(!state.is_space_active());
    }

    #[test]
    fn test_is_t_active_when_just_pressed() {
        let mut state = LeaderKeyState::new();
        state.press_t();
        assert!(state.is_t_active());
    }

    #[test]
    fn test_is_t_active_when_not_pressed() {
        let state = LeaderKeyState::new();
        assert!(!state.is_t_active());
    }

    #[test]
    fn test_leader_key_independence() {
        // Pressing one leader key shouldn't affect others
        let mut state = LeaderKeyState::new();
        state.press_y();

        assert!(state.is_yy_active());
        assert!(!state.is_cv_ready());
        assert!(!state.is_space_active());
        assert!(!state.is_t_active());

        state.press_t();
        assert!(state.is_yy_active());
        assert!(state.is_t_active());

        state.clear_y();
        assert!(!state.is_yy_active());
        assert!(state.is_t_active());
    }

    #[test]
    fn test_leader_key_timeout_constant() {
        assert_eq!(LEADER_KEY_TIMEOUT_MS, 500);
    }

    // ==================== VisualMultiState Tests ====================

    fn make_tile_id(id: u64) -> TileId {
        TileId::from_u64(id)
    }

    #[test]
    fn test_visual_multi_state_new() {
        let tile_id = make_tile_id(1);
        let state = VisualMultiState::new(tile_id);

        assert_eq!(state.cursor_tile_id, Some(tile_id));
        assert!(state.is_selected(tile_id));
        assert_eq!(state.selection_count(), 1);
    }

    #[test]
    fn test_visual_multi_state_default() {
        let state = VisualMultiState::default();
        assert!(state.cursor_tile_id.is_none());
        assert_eq!(state.selection_count(), 0);
    }

    #[test]
    fn test_toggle_selection_add() {
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let mut state = VisualMultiState::new(tile1);

        assert!(!state.is_selected(tile2));
        state.toggle_selection(tile2);
        assert!(state.is_selected(tile2));
        assert_eq!(state.selection_count(), 2);
    }

    #[test]
    fn test_toggle_selection_remove() {
        let tile1 = make_tile_id(1);
        let mut state = VisualMultiState::new(tile1);

        assert!(state.is_selected(tile1));
        state.toggle_selection(tile1);
        assert!(!state.is_selected(tile1));
        assert_eq!(state.selection_count(), 0);
    }

    #[test]
    fn test_toggle_selection_toggle_back() {
        let tile1 = make_tile_id(1);
        let mut state = VisualMultiState::new(tile1);

        state.toggle_selection(tile1); // Remove
        state.toggle_selection(tile1); // Add back
        assert!(state.is_selected(tile1));
        assert_eq!(state.selection_count(), 1);
    }

    #[test]
    fn test_set_cursor() {
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let mut state = VisualMultiState::new(tile1);

        assert_eq!(state.cursor_tile_id, Some(tile1));
        state.set_cursor(tile2);
        assert_eq!(state.cursor_tile_id, Some(tile2));

        // Setting cursor doesn't change selection
        assert!(state.is_selected(tile1));
        assert!(!state.is_selected(tile2));
    }

    #[test]
    fn test_select_all() {
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let tile3 = make_tile_id(3);
        let mut state = VisualMultiState::new(tile1);

        let all_tiles = vec![tile1, tile2, tile3];
        state.select_all(&all_tiles);

        assert!(state.is_selected(tile1));
        assert!(state.is_selected(tile2));
        assert!(state.is_selected(tile3));
        assert_eq!(state.selection_count(), 3);
    }

    #[test]
    fn test_select_all_empty() {
        let tile1 = make_tile_id(1);
        let mut state = VisualMultiState::new(tile1);

        state.select_all(&[]);
        assert_eq!(state.selection_count(), 1); // Original selection preserved
    }

    #[test]
    fn test_select_all_idempotent() {
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let mut state = VisualMultiState::new(tile1);

        let tiles = vec![tile1, tile2];
        state.select_all(&tiles);
        state.select_all(&tiles); // Select again

        assert_eq!(state.selection_count(), 2); // No duplicates
    }

    #[test]
    fn test_clear_selection() {
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let mut state = VisualMultiState::new(tile1);

        state.toggle_selection(tile2);
        assert_eq!(state.selection_count(), 2);

        state.clear_selection();
        assert_eq!(state.selection_count(), 0);
        assert!(!state.is_selected(tile1));
        assert!(!state.is_selected(tile2));
    }

    #[test]
    fn test_clear_selection_preserves_cursor() {
        let tile1 = make_tile_id(1);
        let mut state = VisualMultiState::new(tile1);

        state.clear_selection();
        assert_eq!(state.cursor_tile_id, Some(tile1)); // Cursor unchanged
    }

    #[test]
    fn test_selection_count() {
        let tile1 = make_tile_id(1);
        let tile2 = make_tile_id(2);
        let tile3 = make_tile_id(3);
        let mut state = VisualMultiState::new(tile1);

        assert_eq!(state.selection_count(), 1);
        state.toggle_selection(tile2);
        assert_eq!(state.selection_count(), 2);
        state.toggle_selection(tile3);
        assert_eq!(state.selection_count(), 3);
        state.toggle_selection(tile1);
        assert_eq!(state.selection_count(), 2);
    }

    // ==================== FocusTarget Tests ====================

    #[test]
    fn test_focus_target_default() {
        let target = FocusTarget::default();
        assert_eq!(target, FocusTarget::None);
    }

    #[test]
    fn test_focus_target_first() {
        let target = FocusTarget::first();
        assert_eq!(
            target,
            FocusTarget::Pane {
                section: 0,
                pane: 0
            }
        );
    }

    #[test]
    fn test_focus_target_section_index() {
        assert_eq!(FocusTarget::None.section_index(), None);
        assert_eq!(FocusTarget::SectionHeader(2).section_index(), Some(2));
        assert_eq!(
            FocusTarget::Pane {
                section: 3,
                pane: 1
            }
            .section_index(),
            Some(3)
        );
    }

    #[test]
    fn test_focus_target_is_section_header() {
        assert!(!FocusTarget::None.is_section_header());
        assert!(FocusTarget::SectionHeader(0).is_section_header());
        assert!(
            !FocusTarget::Pane {
                section: 0,
                pane: 0
            }
            .is_section_header()
        );
    }

    #[test]
    fn test_focus_target_is_pane() {
        assert!(!FocusTarget::None.is_pane());
        assert!(!FocusTarget::SectionHeader(0).is_pane());
        assert!(
            FocusTarget::Pane {
                section: 0,
                pane: 0
            }
            .is_pane()
        );
    }

    #[test]
    fn test_focus_target_pane_index() {
        assert_eq!(FocusTarget::None.pane_index(), None);
        assert_eq!(FocusTarget::SectionHeader(0).pane_index(), None);
        assert_eq!(
            FocusTarget::Pane {
                section: 2,
                pane: 5
            }
            .pane_index(),
            Some(5)
        );
    }

    #[test]
    fn test_focus_target_equality() {
        assert_eq!(FocusTarget::None, FocusTarget::None);
        assert_eq!(FocusTarget::SectionHeader(1), FocusTarget::SectionHeader(1));
        assert_ne!(FocusTarget::SectionHeader(1), FocusTarget::SectionHeader(2));
        assert_eq!(
            FocusTarget::Pane {
                section: 1,
                pane: 2
            },
            FocusTarget::Pane {
                section: 1,
                pane: 2
            }
        );
        assert_ne!(
            FocusTarget::Pane {
                section: 1,
                pane: 2
            },
            FocusTarget::Pane {
                section: 1,
                pane: 3
            }
        );
    }

    // ==================== SectionState Tests ====================

    #[test]
    fn test_section_state_default() {
        let state = SectionState::default();
        assert!(!state.collapsed);
    }

    #[test]
    fn test_section_state_new() {
        let state = SectionState::new(true);
        assert!(state.collapsed);

        let state = SectionState::new(false);
        assert!(!state.collapsed);
    }

    #[test]
    fn test_section_state_toggle() {
        let mut state = SectionState::new(false);
        assert!(!state.collapsed);

        state.toggle();
        assert!(state.collapsed);

        state.toggle();
        assert!(!state.collapsed);
    }

    #[test]
    fn test_section_state_expand() {
        let mut state = SectionState::new(true);
        assert!(state.collapsed);

        state.expand();
        assert!(!state.collapsed);

        // Expand again is a no-op
        state.expand();
        assert!(!state.collapsed);
    }

    #[test]
    fn test_section_state_collapse() {
        let mut state = SectionState::new(false);
        assert!(!state.collapsed);

        state.collapse();
        assert!(state.collapsed);

        // Collapse again is a no-op
        state.collapse();
        assert!(state.collapsed);
    }
}
