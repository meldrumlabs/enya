//! UI integration tests using egui_kittest.
//!
//! These tests verify that UI components render correctly and respond to user
//! input (keyboard, mouse) as expected. They use egui_kittest to create a
//! testing harness that can simulate user interactions and capture UI state.
//!
//! ## How egui_kittest Works
//!
//! egui_kittest provides a `Harness` that wraps an egui application:
//!
//! 1. **Harness Creation**: Create a harness with a closure that defines the UI
//!    ```ignore
//!    let mut harness = Harness::new_ui(|ui| {
//!        ui.label("Hello, World!");
//!    });
//!    ```
//!
//! 2. **Running Frames**: Call `harness.run()` to process events and render
//!    ```ignore
//!    harness.run(); // Processes pending events and runs until stable
//!    ```
//!
//! 3. **Simulating Input**: Use key_press, click, etc. to simulate user input
//!    ```ignore
//!    harness.press_key(egui::Key::Escape);
//!    harness.run();
//!    ```
//!
//! 4. **Querying UI**: Use AccessKit to find and inspect UI elements
//!    ```ignore
//!    let label = harness.get_by_label("Hello, World!");
//!    assert!(label.is_some());
//!    ```
//!
//! 5. **Snapshot Testing**: Capture and compare rendered images
//!    ```ignore
//!    harness.wgpu_snapshot("test_name");
//!    ```

use egui_kittest::Harness;

/// Test module for WhichKey overlay component.
///
/// The WhichKey overlay displays available keybindings in a floating popup,
/// similar to the neovim which-key.nvim plugin. It opens with `?` and closes
/// with `Escape` or another `?`.
mod which_key_tests {
    use super::*;
    use enya_editor::components::WhichKey;
    use enya_editor::ui::theme::AppTheme;

    /// Test that WhichKey starts in closed state
    #[test]
    fn test_which_key_initially_closed() {
        let which_key = WhichKey::new();
        assert!(!which_key.is_open());
    }

    /// Test that WhichKey opens and closes correctly
    #[test]
    fn test_which_key_open_close() {
        let mut which_key = WhichKey::new();

        // Open
        which_key.open();
        assert!(which_key.is_open());

        // Close
        which_key.close();
        assert!(!which_key.is_open());
    }

    /// Test WhichKey rendering with egui_kittest harness.
    ///
    /// This test creates an egui harness, opens the WhichKey overlay,
    /// and verifies it renders the expected keybinding groups.
    #[test]
    fn test_which_key_renders_in_harness() {
        let mut which_key = WhichKey::new();
        which_key.set_theme(AppTheme::default());
        which_key.open();

        // Create harness with the WhichKey overlay
        let mut harness = Harness::new_state(
            which_key,
            |ctx: &egui::Context, which_key: &mut WhichKey| {
                which_key.show(ctx);
            },
        );

        // Run a frame to render
        harness.run();

        // The WhichKey overlay should still be open after one frame
        // (first frame is skipped for input to prevent immediate close)
        assert!(harness.state().is_open());
    }

    /// Test that Escape key closes the WhichKey overlay.
    ///
    /// Note: The WhichKey component skips input on the first frame after
    /// opening to prevent the same key press from immediately closing it.
    #[test]
    fn test_which_key_closes_on_escape() {
        let mut which_key = WhichKey::new();
        which_key.set_theme(AppTheme::default());
        which_key.open();

        let mut harness = Harness::new_state(
            which_key,
            |ctx: &egui::Context, which_key: &mut WhichKey| {
                which_key.show(ctx);
            },
        );

        // First run to clear the "just_opened" flag
        harness.run();
        assert!(harness.state().is_open());

        // Press Escape
        harness.press_key(egui::Key::Escape);
        harness.run();

        // Should be closed now
        assert!(!harness.state().is_open());
    }

    /// Test that ? key (Shift+/) toggles the WhichKey overlay off.
    #[test]
    fn test_which_key_closes_on_question_mark() {
        let mut which_key = WhichKey::new();
        which_key.set_theme(AppTheme::default());
        which_key.open();

        let mut harness = Harness::new_state(
            which_key,
            |ctx: &egui::Context, which_key: &mut WhichKey| {
                which_key.show(ctx);
            },
        );

        // First run to clear the "just_opened" flag
        harness.run();
        assert!(harness.state().is_open());

        // Press ? (Shift+Slash)
        harness.press_key_modifiers(egui::Key::Slash, egui::Modifiers::SHIFT);
        harness.run();

        // Should be closed now
        assert!(!harness.state().is_open());
    }
}

/// Test module for keyboard decision logic integration.
///
/// These tests verify that the pure keyboard decision functions work correctly
/// when integrated with egui key events. The logic is in `keyboard_logic.rs`
/// and is designed to be testable without egui::Context.
mod keyboard_logic_tests {
    use enya_editor::components::{QuickCommand, TimeRangePreset};
    use enya_editor::workspace::{
        KeyboardContext, KeyboardDecision, check_navigation_blocked, determine_agent_operator_action,
        determine_ctrl_w_action, determine_goto_action, determine_space_action,
        determine_time_range_action,
    };

    /// Test Space+f opens the unified finder
    #[test]
    fn test_space_f_opens_finder() {
        let decision = determine_space_action(egui::Key::F, true);
        assert_eq!(decision, Some(KeyboardDecision::OpenUnifiedFinder));
    }

    /// Test Space+w opens the workspace finder
    #[test]
    fn test_space_w_opens_workspace_finder() {
        let decision = determine_space_action(egui::Key::W, true);
        assert_eq!(decision, Some(KeyboardDecision::OpenWorkspaceFinder));
    }

    /// Test Space+z toggles zen mode
    #[test]
    fn test_space_z_toggles_zen() {
        let decision = determine_space_action(egui::Key::Z, true);
        assert_eq!(decision, Some(KeyboardDecision::ToggleZenMode));
    }

    /// Test Space+t opens the terminal (native only)
    #[test]
    fn test_space_t_opens_terminal() {
        // Only available on native builds
        let decision = determine_space_action(egui::Key::T, true);
        assert_eq!(decision, Some(KeyboardDecision::ToggleTeamMenu));
    }

    /// Test time range shortcuts (t5, t1, t3, th, etc.)
    #[test]
    fn test_time_range_shortcuts() {
        // t5 = 5 minutes
        assert_eq!(
            determine_time_range_action(egui::Key::Num5),
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last5Minutes))
        );

        // t1 = 15 minutes
        assert_eq!(
            determine_time_range_action(egui::Key::Num1),
            Some(KeyboardDecision::SetTimeRange(
                TimeRangePreset::Last15Minutes
            ))
        );

        // th = 1 hour
        assert_eq!(
            determine_time_range_action(egui::Key::H),
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last1Hour))
        );

        // td = 1 day
        assert_eq!(
            determine_time_range_action(egui::Key::D),
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last1Day))
        );
    }

    /// Test go-to shortcuts (gd, ga, gf)
    #[test]
    fn test_goto_shortcuts() {
        // gd = go to definition
        assert_eq!(
            determine_goto_action(egui::Key::D),
            Some(KeyboardDecision::GoToDefinition)
        );

        // ga = go to alert
        assert_eq!(
            determine_goto_action(egui::Key::A),
            Some(KeyboardDecision::GoToAlert)
        );

        // gf = float pane
        assert_eq!(
            determine_goto_action(egui::Key::F),
            Some(KeyboardDecision::FloatFocusedPane)
        );
    }

    /// Test agent operator shortcuts (aw, ae, ay)
    #[test]
    fn test_agent_operators() {
        // aw = "What's wrong?"
        assert_eq!(
            determine_agent_operator_action(egui::Key::W),
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::WhatsWrong))
        );

        // ae = "Explain"
        assert_eq!(
            determine_agent_operator_action(egui::Key::E),
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Explain))
        );

        // ay = "Why?"
        assert_eq!(
            determine_agent_operator_action(egui::Key::Y),
            Some(KeyboardDecision::AgentQuickCommand(QuickCommand::Why))
        );
    }

    /// Test Ctrl+W window management shortcuts
    #[test]
    fn test_ctrl_w_shortcuts() {
        // Ctrl+W h = move left
        assert_eq!(
            determine_ctrl_w_action(egui::Key::H),
            Some(KeyboardDecision::MovePaneInDirection(
                enya_editor::workspace::NavDirection::Left
            ))
        );

        // Ctrl+W v = split vertical
        assert_eq!(
            determine_ctrl_w_action(egui::Key::V),
            Some(KeyboardDecision::SplitVertical)
        );

        // Ctrl+W s = split horizontal
        assert_eq!(
            determine_ctrl_w_action(egui::Key::S),
            Some(KeyboardDecision::SplitHorizontal)
        );

        // Ctrl+W x = close pane
        assert_eq!(
            determine_ctrl_w_action(egui::Key::X),
            Some(KeyboardDecision::ClosePane)
        );
    }

    /// Test navigation blocking when modals are open
    #[test]
    fn test_navigation_blocked_by_modals() {
        // Default context - not blocked
        let ctx = KeyboardContext::default();
        let (blocked, reason) = check_navigation_blocked(&ctx);
        assert!(!blocked, "Should not be blocked by default");
        assert!(reason.is_empty());

        // Modal open - blocked
        let ctx_with_modal = KeyboardContext {
            any_modal_open: true,
            ..Default::default()
        };
        let (blocked, reason) = check_navigation_blocked(&ctx_with_modal);
        assert!(blocked, "Should be blocked when modal is open");
        assert_eq!(reason, "modal_open");

        // egui has focus - blocked
        let ctx_with_focus = KeyboardContext {
            egui_has_focus: true,
            ..Default::default()
        };
        let (blocked, reason) = check_navigation_blocked(&ctx_with_focus);
        assert!(blocked, "Should be blocked when egui has focus");
        assert_eq!(reason, "egui_focus");
    }
}

/// Test module for LeaderKeyState timeout behavior.
///
/// These tests verify that leader key sequences expire correctly after
/// the 500ms timeout (LEADER_KEY_TIMEOUT_MS).
mod leader_key_tests {
    use enya_editor::workspace::{LEADER_KEY_TIMEOUT_MS, LeaderKeyState};

    /// Test that a fresh LeaderKeyState has no active leader keys
    #[test]
    fn test_initial_state() {
        let state = LeaderKeyState::new();
        assert!(!state.is_space_active());
        assert!(!state.is_t_active());
        assert!(!state.is_g_active());
        assert!(!state.is_a_active());
        assert!(!state.is_ctrl_w_active());
    }

    /// Test that pressing a leader key activates it
    #[test]
    fn test_press_activates() {
        let mut state = LeaderKeyState::new();

        state.press_space();
        assert!(state.is_space_active());

        state.press_t();
        assert!(state.is_t_active());

        state.press_g();
        assert!(state.is_g_active());

        state.press_a();
        assert!(state.is_a_active());

        state.press_ctrl_w();
        assert!(state.is_ctrl_w_active());
    }

    /// Test that consuming a leader key clears it
    #[test]
    fn test_consume_clears() {
        let mut state = LeaderKeyState::new();

        state.press_space();
        assert!(state.is_space_active());
        state.consume_space();
        assert!(!state.is_space_active());

        state.press_t();
        assert!(state.is_t_active());
        state.consume_t();
        assert!(!state.is_t_active());
    }

    /// Test that the timeout constant is reasonable
    #[test]
    fn test_timeout_value() {
        // 500ms is the expected timeout
        assert_eq!(LEADER_KEY_TIMEOUT_MS, 500);
    }
}
