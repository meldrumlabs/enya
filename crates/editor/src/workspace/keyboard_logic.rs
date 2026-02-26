//! Pure keyboard decision logic, separated from egui::Context coupling.
//!
//! This module contains testable logic for keyboard navigation decisions.
//! All functions here operate on simple data structures without requiring
//! egui::Context, making them easy to unit test.

use egui_tiles::TileId;

use super::NavDirection;
use crate::components::{QuickCommand, TimeRangePreset};

/// Represents a keyboard-triggered action decision.
///
/// This enum captures all possible actions that can result from keyboard input,
/// allowing the logic to be tested independently from egui key consumption.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardDecision {
    /// No action to take
    None,
    /// Navigation to a new pane
    NavigateTo(TileId),
    /// Close the focused pane
    ClosePane,
    /// Toggle zen mode (hide chrome)
    ToggleZenMode,
    /// Toggle fullscreen for focused pane
    ToggleFullscreen,
    /// Share/yank focused pane
    SharePane,
    /// Open command palette (:)
    OpenCommandPalette,
    /// Open unified finder (Space+f)
    OpenUnifiedFinder,
    /// Open codebase finder (Space+c, native only)
    OpenCodebaseFinder,
    /// Show home/landing page (Space+h)
    ShowHome,
    /// Toggle diagnostics overlay (Space+d)
    ToggleDiagnostics,
    /// Toggle agent panel (Space+a)
    ToggleAgentPanel,
    /// Set time range from preset
    SetTimeRange(TimeRangePreset),
    /// Enter visual-multi mode (Ctrl+V)
    EnterVisualMulti,
    /// Exit visual-multi mode
    ExitVisualMulti,
    /// Enter agent mode
    EnterAgentMode,
    /// Enter agent mode with typing (aa)
    EnterAgentModeTyping,
    /// Execute agent quick command
    AgentQuickCommand(QuickCommand),
    /// Move pane in direction (Ctrl+W h/j/k/l)
    MovePaneInDirection(NavDirection),
    /// Tab/merge pane with neighbor (Ctrl+W t h/j/k/l)
    TabPaneInDirection(NavDirection),
    /// Focus the agent panel
    FocusAgentPanel,
    /// Go to definition (gd)
    GoToDefinition,
    /// Go to alert (ga)
    GoToAlert,
    /// Show definition demo (gp)
    ShowDefinitionDemo,
    /// Float focused pane (gf)
    FloatFocusedPane,
    /// Cycle visualization type (cv)
    CycleVisualization,
    /// Edit buffer (e)
    EditBuffer,
    /// Open which-key help (?)
    OpenWhichKey,
    /// Clear focus (Escape)
    ClearFocus,
    /// Open plugins overlay (Space+p)
    OpenPluginsOverlay,
    /// Toggle project sidebar (Space+b)
    ToggleProjectSidebar,
}

/// Minimal context needed for keyboard decision making.
///
/// This struct captures only the state needed to make keyboard decisions,
/// avoiding the need to pass the entire Workspace struct.
#[derive(Debug, Clone, Default)]
pub struct KeyboardContext {
    /// Currently focused tile (if any)
    pub current_focus: Option<TileId>,
    /// Whether any modal overlay is open that should block navigation
    pub any_modal_open: bool,
    /// Whether visual-multi mode is active
    pub visual_multi_active: bool,
    /// Whether the agent panel has focus
    pub agent_panel_focused: bool,
    /// Whether the agent panel is open (for focus transfer)
    pub agent_panel_open: bool,
    /// Whether any buffer is in insert mode
    pub any_buffer_in_insert_mode: bool,
    /// Whether we're in agent mode
    pub agent_mode_active: bool,
    /// Whether egui has widget focus (text fields, etc.)
    pub egui_has_focus: bool,
}

impl KeyboardContext {
    /// Create a new context with the given focus
    pub fn with_focus(tile_id: TileId) -> Self {
        Self {
            current_focus: Some(tile_id),
            ..Default::default()
        }
    }

    /// Check if keyboard navigation should be blocked
    pub fn is_navigation_blocked(&self) -> bool {
        self.any_modal_open
            || self.visual_multi_active
            || self.agent_panel_focused
            || self.any_buffer_in_insert_mode
            || self.agent_mode_active
            || self.egui_has_focus
    }
}

/// Determine the action for a Space+key leader key sequence.
///
/// Returns the keyboard decision if a valid Space+key sequence is detected,
/// or None if the key doesn't match any Space sequence.
pub fn determine_space_action(key: egui::Key, is_native: bool) -> Option<KeyboardDecision> {
    match key {
        egui::Key::F => Some(KeyboardDecision::OpenUnifiedFinder),
        egui::Key::H => Some(KeyboardDecision::ShowHome),
        egui::Key::D => Some(KeyboardDecision::ToggleDiagnostics),
        egui::Key::A => Some(KeyboardDecision::ToggleAgentPanel),
        egui::Key::P => Some(KeyboardDecision::OpenPluginsOverlay),
        egui::Key::B => Some(KeyboardDecision::ToggleProjectSidebar),
        egui::Key::C if is_native => Some(KeyboardDecision::OpenCodebaseFinder),
        _ => None,
    }
}

/// Determine the action for a t+key time range sequence.
///
/// Returns the keyboard decision if a valid t+key sequence is detected,
/// or None if the key doesn't match any time range shortcut.
pub fn determine_time_range_action(key: egui::Key) -> Option<KeyboardDecision> {
    let preset = match key {
        egui::Key::Num5 => TimeRangePreset::Last5Minutes,
        egui::Key::Num1 => TimeRangePreset::Last15Minutes,
        egui::Key::Num3 => TimeRangePreset::Last30Minutes,
        egui::Key::H => TimeRangePreset::Last1Hour,
        egui::Key::Num6 => TimeRangePreset::Last6Hours,
        egui::Key::D => TimeRangePreset::Last24Hours,
        egui::Key::W => TimeRangePreset::Last7Days,
        _ => return None,
    };
    Some(KeyboardDecision::SetTimeRange(preset))
}

/// Determine the action for a g+key go-to sequence.
///
/// Returns the keyboard decision if a valid g+key sequence is detected,
/// or None if the key doesn't match any go-to shortcut.
pub fn determine_goto_action(key: egui::Key) -> Option<KeyboardDecision> {
    match key {
        egui::Key::D => Some(KeyboardDecision::GoToDefinition),
        egui::Key::A => Some(KeyboardDecision::GoToAlert),
        egui::Key::P => Some(KeyboardDecision::ShowDefinitionDemo),
        egui::Key::F => Some(KeyboardDecision::FloatFocusedPane),
        _ => None,
    }
}

/// Determine the action for an a+key agent operator sequence.
///
/// Returns the keyboard decision if a valid a+key sequence is detected,
/// or None if the key doesn't match any agent operator.
pub fn determine_agent_operator_action(key: egui::Key) -> Option<KeyboardDecision> {
    let command = match key {
        egui::Key::W => QuickCommand::WhatsWrong,
        egui::Key::E => QuickCommand::Explain,
        egui::Key::Y => QuickCommand::Why,
        egui::Key::C => QuickCommand::Compare,
        egui::Key::R => QuickCommand::Related,
        egui::Key::F => QuickCommand::Fix,
        egui::Key::S => QuickCommand::Summarize,
        egui::Key::H => QuickCommand::History,
        egui::Key::A => return Some(KeyboardDecision::EnterAgentModeTyping),
        _ => return None,
    };
    Some(KeyboardDecision::AgentQuickCommand(command))
}

/// Determine the action for a Ctrl+W+key window management sequence.
///
/// Returns the keyboard decision if a valid Ctrl+W+key sequence is detected,
/// or None if the key doesn't match any window management shortcut.
pub fn determine_ctrl_w_action(key: egui::Key) -> Option<KeyboardDecision> {
    let direction = match key {
        egui::Key::H => NavDirection::Left,
        egui::Key::J => NavDirection::Down,
        egui::Key::K => NavDirection::Up,
        egui::Key::L => NavDirection::Right,
        egui::Key::T => return None, // Tab mode handled separately
        _ => return None,
    };
    Some(KeyboardDecision::MovePaneInDirection(direction))
}

/// Determine the action for a Ctrl+W t+key tab merging sequence.
///
/// Returns the keyboard decision if a valid Ctrl+W t+key sequence is detected,
/// or None if the key doesn't match any tab direction.
pub fn determine_ctrl_w_t_action(key: egui::Key) -> Option<KeyboardDecision> {
    let direction = match key {
        egui::Key::H => NavDirection::Left,
        egui::Key::J => NavDirection::Down,
        egui::Key::K => NavDirection::Up,
        egui::Key::L => NavDirection::Right,
        _ => return None,
    };
    Some(KeyboardDecision::TabPaneInDirection(direction))
}

/// Check if keyboard navigation should be blocked and return the reason.
///
/// This function encapsulates the modal blocking logic, making it testable.
/// Returns (is_blocked, reason) for debugging and testing.
#[allow(clippy::too_many_arguments)]
pub fn check_navigation_blocked(
    unified_finder_open: bool,
    command_palette_open: bool,
    buffer_editor_open: bool,
    multi_edit_overlay_open: bool,
    which_key_open: bool,
    viewport_filter_open: bool,
    tutorial_overlay_open: bool,
    source_preview_open: bool,
    style_picker_open: bool,
    codebase_finder_open: bool,
) -> (bool, &'static str) {
    if unified_finder_open {
        return (true, "unified_finder");
    }
    if command_palette_open {
        return (true, "command_palette");
    }
    if buffer_editor_open {
        return (true, "buffer_editor");
    }
    if multi_edit_overlay_open {
        return (true, "multi_edit_overlay");
    }
    if which_key_open {
        return (true, "which_key");
    }
    if viewport_filter_open {
        return (true, "viewport_filter");
    }
    if tutorial_overlay_open {
        return (true, "tutorial_overlay");
    }
    if source_preview_open {
        return (true, "source_preview");
    }
    if style_picker_open {
        return (true, "style_picker");
    }
    if codebase_finder_open {
        return (true, "codebase_finder");
    }
    (false, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== KeyboardContext Tests ====================

    #[test]
    fn test_keyboard_context_default() {
        let ctx = KeyboardContext::default();
        assert!(ctx.current_focus.is_none());
        assert!(!ctx.any_modal_open);
        assert!(!ctx.visual_multi_active);
        assert!(!ctx.is_navigation_blocked());
    }

    #[test]
    fn test_keyboard_context_with_focus() {
        let tile_id = TileId::from_u64(1);
        let ctx = KeyboardContext::with_focus(tile_id);
        assert_eq!(ctx.current_focus, Some(tile_id));
    }

    #[test]
    fn test_navigation_blocked_by_modal() {
        let mut ctx = KeyboardContext::default();
        assert!(!ctx.is_navigation_blocked());

        ctx.any_modal_open = true;
        assert!(ctx.is_navigation_blocked());
    }

    #[test]
    fn test_navigation_blocked_by_visual_multi() {
        let ctx = KeyboardContext {
            visual_multi_active: true,
            ..Default::default()
        };
        assert!(ctx.is_navigation_blocked());
    }

    #[test]
    fn test_navigation_blocked_by_panel_focus() {
        let ctx = KeyboardContext {
            agent_panel_focused: true,
            ..Default::default()
        };
        assert!(ctx.is_navigation_blocked());
    }

    #[test]
    fn test_navigation_blocked_by_insert_mode() {
        let ctx = KeyboardContext {
            any_buffer_in_insert_mode: true,
            ..Default::default()
        };
        assert!(ctx.is_navigation_blocked());
    }

    #[test]
    fn test_navigation_blocked_by_agent_mode() {
        let ctx = KeyboardContext {
            agent_mode_active: true,
            ..Default::default()
        };
        assert!(ctx.is_navigation_blocked());
    }

    #[test]
    fn test_navigation_blocked_by_egui_focus() {
        let ctx = KeyboardContext {
            egui_has_focus: true,
            ..Default::default()
        };
        assert!(ctx.is_navigation_blocked());
    }

    // ==================== Space Leader Key Tests ====================

    #[test]
    fn test_space_f_opens_unified_finder() {
        let result = determine_space_action(egui::Key::F, true);
        assert_eq!(result, Some(KeyboardDecision::OpenUnifiedFinder));
    }

    #[test]
    fn test_space_w_unused() {
        let result = determine_space_action(egui::Key::W, true);
        assert_eq!(result, None);
    }

    #[test]
    fn test_space_h_shows_home() {
        let result = determine_space_action(egui::Key::H, true);
        assert_eq!(result, Some(KeyboardDecision::ShowHome));
    }

    #[test]
    fn test_space_d_toggles_diagnostics() {
        let result = determine_space_action(egui::Key::D, true);
        assert_eq!(result, Some(KeyboardDecision::ToggleDiagnostics));
    }

    #[test]
    fn test_space_a_toggles_agent_panel() {
        let result = determine_space_action(egui::Key::A, true);
        assert_eq!(result, Some(KeyboardDecision::ToggleAgentPanel));
    }

    #[test]
    fn test_space_p_opens_plugins_overlay() {
        let result = determine_space_action(egui::Key::P, true);
        assert_eq!(result, Some(KeyboardDecision::OpenPluginsOverlay));
    }

    #[test]
    fn test_space_c_opens_codebase_finder_native() {
        let result = determine_space_action(egui::Key::C, true);
        assert_eq!(result, Some(KeyboardDecision::OpenCodebaseFinder));
    }

    #[test]
    fn test_space_c_does_nothing_on_wasm() {
        let result = determine_space_action(egui::Key::C, false);
        assert_eq!(result, None);
    }

    #[test]
    fn test_space_unknown_key_returns_none() {
        let result = determine_space_action(egui::Key::Z, true);
        assert_eq!(result, None);
    }

    // ==================== Time Range Tests ====================

    #[test]
    fn test_t5_sets_5_minutes() {
        let result = determine_time_range_action(egui::Key::Num5);
        assert_eq!(
            result,
            Some(KeyboardDecision::SetTimeRange(
                TimeRangePreset::Last5Minutes
            ))
        );
    }

    #[test]
    fn test_t1_sets_15_minutes() {
        let result = determine_time_range_action(egui::Key::Num1);
        assert_eq!(
            result,
            Some(KeyboardDecision::SetTimeRange(
                TimeRangePreset::Last15Minutes
            ))
        );
    }

    #[test]
    fn test_t3_sets_30_minutes() {
        let result = determine_time_range_action(egui::Key::Num3);
        assert_eq!(
            result,
            Some(KeyboardDecision::SetTimeRange(
                TimeRangePreset::Last30Minutes
            ))
        );
    }

    #[test]
    fn test_th_sets_1_hour() {
        let result = determine_time_range_action(egui::Key::H);
        assert_eq!(
            result,
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last1Hour))
        );
    }

    #[test]
    fn test_t6_sets_6_hours() {
        let result = determine_time_range_action(egui::Key::Num6);
        assert_eq!(
            result,
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last6Hours))
        );
    }

    #[test]
    fn test_td_sets_24_hours() {
        let result = determine_time_range_action(egui::Key::D);
        assert_eq!(
            result,
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last24Hours))
        );
    }

    #[test]
    fn test_tw_sets_7_days() {
        let result = determine_time_range_action(egui::Key::W);
        assert_eq!(
            result,
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last7Days))
        );
    }

    #[test]
    fn test_t_unknown_key_returns_none() {
        let result = determine_time_range_action(egui::Key::X);
        assert_eq!(result, None);
    }

    // ==================== Go-To Tests ====================

    #[test]
    fn test_gd_goes_to_definition() {
        let result = determine_goto_action(egui::Key::D);
        assert_eq!(result, Some(KeyboardDecision::GoToDefinition));
    }

    #[test]
    fn test_ga_goes_to_alert() {
        let result = determine_goto_action(egui::Key::A);
        assert_eq!(result, Some(KeyboardDecision::GoToAlert));
    }

    #[test]
    fn test_gp_shows_definition_demo() {
        let result = determine_goto_action(egui::Key::P);
        assert_eq!(result, Some(KeyboardDecision::ShowDefinitionDemo));
    }

    #[test]
    fn test_gf_floats_focused_pane() {
        let result = determine_goto_action(egui::Key::F);
        assert_eq!(result, Some(KeyboardDecision::FloatFocusedPane));
    }

    #[test]
    fn test_g_unknown_key_returns_none() {
        let result = determine_goto_action(egui::Key::Z);
        assert_eq!(result, None);
    }

    // ==================== Agent Operator Tests ====================

    #[test]
    fn test_aw_whats_wrong() {
        let result = determine_agent_operator_action(egui::Key::W);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(
                QuickCommand::WhatsWrong
            ))
        );
    }

    #[test]
    fn test_ae_explain() {
        let result = determine_agent_operator_action(egui::Key::E);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Explain))
        );
    }

    #[test]
    fn test_ay_why() {
        let result = determine_agent_operator_action(egui::Key::Y);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Why))
        );
    }

    #[test]
    fn test_ac_compare() {
        let result = determine_agent_operator_action(egui::Key::C);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Compare))
        );
    }

    #[test]
    fn test_ar_related() {
        let result = determine_agent_operator_action(egui::Key::R);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Related))
        );
    }

    #[test]
    fn test_af_fix() {
        let result = determine_agent_operator_action(egui::Key::F);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Fix))
        );
    }

    #[test]
    fn test_as_summarize() {
        let result = determine_agent_operator_action(egui::Key::S);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Summarize))
        );
    }

    #[test]
    fn test_ah_history() {
        let result = determine_agent_operator_action(egui::Key::H);
        assert_eq!(
            result,
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::History))
        );
    }

    #[test]
    fn test_aa_enters_agent_mode_typing() {
        let result = determine_agent_operator_action(egui::Key::A);
        assert_eq!(result, Some(KeyboardDecision::EnterAgentModeTyping));
    }

    #[test]
    fn test_a_unknown_key_returns_none() {
        let result = determine_agent_operator_action(egui::Key::Z);
        assert_eq!(result, None);
    }

    // ==================== Ctrl+W Tests ====================

    #[test]
    fn test_ctrl_w_h_moves_left() {
        let result = determine_ctrl_w_action(egui::Key::H);
        assert_eq!(
            result,
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Left))
        );
    }

    #[test]
    fn test_ctrl_w_j_moves_down() {
        let result = determine_ctrl_w_action(egui::Key::J);
        assert_eq!(
            result,
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Down))
        );
    }

    #[test]
    fn test_ctrl_w_k_moves_up() {
        let result = determine_ctrl_w_action(egui::Key::K);
        assert_eq!(
            result,
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Up))
        );
    }

    #[test]
    fn test_ctrl_w_l_moves_right() {
        let result = determine_ctrl_w_action(egui::Key::L);
        assert_eq!(
            result,
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Right))
        );
    }

    #[test]
    fn test_ctrl_w_t_returns_none() {
        // Tab mode is handled separately
        let result = determine_ctrl_w_action(egui::Key::T);
        assert_eq!(result, None);
    }

    #[test]
    fn test_ctrl_w_unknown_returns_none() {
        let result = determine_ctrl_w_action(egui::Key::X);
        assert_eq!(result, None);
    }

    // ==================== Ctrl+W t Tests ====================

    #[test]
    fn test_ctrl_w_t_h_tabs_left() {
        let result = determine_ctrl_w_t_action(egui::Key::H);
        assert_eq!(
            result,
            Some(KeyboardDecision::TabPaneInDirection(NavDirection::Left))
        );
    }

    #[test]
    fn test_ctrl_w_t_j_tabs_down() {
        let result = determine_ctrl_w_t_action(egui::Key::J);
        assert_eq!(
            result,
            Some(KeyboardDecision::TabPaneInDirection(NavDirection::Down))
        );
    }

    #[test]
    fn test_ctrl_w_t_k_tabs_up() {
        let result = determine_ctrl_w_t_action(egui::Key::K);
        assert_eq!(
            result,
            Some(KeyboardDecision::TabPaneInDirection(NavDirection::Up))
        );
    }

    #[test]
    fn test_ctrl_w_t_l_tabs_right() {
        let result = determine_ctrl_w_t_action(egui::Key::L);
        assert_eq!(
            result,
            Some(KeyboardDecision::TabPaneInDirection(NavDirection::Right))
        );
    }

    #[test]
    fn test_ctrl_w_t_unknown_returns_none() {
        let result = determine_ctrl_w_t_action(egui::Key::X);
        assert_eq!(result, None);
    }

    // ==================== Modal Blocking Tests ====================

    #[test]
    fn test_no_modals_open_not_blocked() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, false, false, false, false, false, false, false,
        );
        assert!(!blocked);
        assert_eq!(reason, "");
    }

    #[test]
    fn test_unified_finder_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            true, false, false, false, false, false, false, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "unified_finder");
    }

    #[test]
    fn test_command_palette_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, true, false, false, false, false, false, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "command_palette");
    }

    #[test]
    fn test_buffer_editor_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, true, false, false, false, false, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "buffer_editor");
    }

    #[test]
    fn test_multi_edit_overlay_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, true, false, false, false, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "multi_edit_overlay");
    }

    #[test]
    fn test_which_key_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, false, true, false, false, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "which_key");
    }

    #[test]
    fn test_viewport_filter_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, false, false, true, false, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "viewport_filter");
    }

    #[test]
    fn test_tutorial_overlay_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, false, false, false, true, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "tutorial_overlay");
    }

    #[test]
    fn test_source_preview_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, false, false, false, false, true, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "source_preview");
    }

    #[test]
    fn test_style_picker_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, false, false, false, false, false, true, false,
        );
        assert!(blocked);
        assert_eq!(reason, "style_picker");
    }

    #[test]
    fn test_codebase_finder_blocks() {
        let (blocked, reason) = check_navigation_blocked(
            false, false, false, false, false, false, false, false, false, true,
        );
        assert!(blocked);
        assert_eq!(reason, "codebase_finder");
    }

    #[test]
    fn test_first_blocker_takes_precedence() {
        // When multiple modals are open, first one wins
        let (blocked, reason) = check_navigation_blocked(
            true, true, false, false, false, false, false, false, false, false,
        );
        assert!(blocked);
        assert_eq!(reason, "unified_finder");
    }

    // ==================== Focus Management Invariants ====================
    //
    // These tests document the expected focus behavior for keyboard navigation.
    //
    // Key invariant: When an overlay closes, egui focus must be cleared so that
    // vim-style navigation (h/j/k/l) works immediately. This is achieved by calling
    // `ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL))` BEFORE closing
    // the overlay.
    //
    // Overlays that follow this pattern:
    // - command_palette.rs
    // - buffer_editor.rs
    // - multi_edit.rs
    // - which_key.rs
    // - workspace_creator.rs
    // - tutorial.rs
    // - info.rs
    // - about.rs
    // - source_preview.rs
    // - diagnostics.rs
    // - diff_viewer.rs
    // - codebase_finder.rs
    // - unified_finder.rs
    //
    // Testing this directly requires egui::Context, which isn't easily mockable.
    // The invariant is enforced through code review and manual testing.

    #[test]
    fn test_navigation_not_blocked_when_all_overlays_closed() {
        // This test documents the expected state after all overlays close:
        // navigation should not be blocked.
        let (blocked, _) = check_navigation_blocked(
            false, false, false, false, false, false, false, false, false, false,
        );
        assert!(
            !blocked,
            "Navigation should not be blocked when all overlays are closed"
        );
    }

    #[test]
    fn test_modal_open_flag_determines_blocking() {
        // KeyboardContext.any_modal_open should track whether focus-capturing
        // overlays are open. When true, navigation is blocked.
        let mut ctx = KeyboardContext::default();
        assert!(!ctx.is_navigation_blocked());

        ctx.any_modal_open = true;
        assert!(ctx.is_navigation_blocked());

        // After modal closes and focus is surrendered, the flag should be false
        ctx.any_modal_open = false;
        assert!(!ctx.is_navigation_blocked());
    }
}
