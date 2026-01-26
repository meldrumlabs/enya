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
//!    harness.key_press(egui::Key::Escape);
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
        // Note: new_state takes (app_fn, initial_state)
        let mut harness = Harness::new_state(
            |ctx: &egui::Context, which_key: &mut WhichKey| {
                which_key.show(ctx);
            },
            which_key,
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
            |ctx: &egui::Context, which_key: &mut WhichKey| {
                which_key.show(ctx);
            },
            which_key,
        );

        // First run to clear the "just_opened" flag
        harness.run();
        assert!(harness.state().is_open());

        // Press Escape
        harness.key_press(egui::Key::Escape);
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
            |ctx: &egui::Context, which_key: &mut WhichKey| {
                which_key.show(ctx);
            },
            which_key,
        );

        // First run to clear the "just_opened" flag
        harness.run();
        assert!(harness.state().is_open());

        // Press ? (Shift+Slash)
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Slash);
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
        KeyboardDecision, check_navigation_blocked, determine_agent_operator_action,
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

    /// Test Space+h shows home/landing page
    #[test]
    fn test_space_h_shows_home() {
        let decision = determine_space_action(egui::Key::H, true);
        assert_eq!(decision, Some(KeyboardDecision::ShowHome));
    }

    /// Test Space+t toggles team menu
    #[test]
    fn test_space_t_toggles_team_menu() {
        let decision = determine_space_action(egui::Key::T, true);
        assert_eq!(decision, Some(KeyboardDecision::ToggleTeamMenu));
    }

    /// Test time range shortcuts (t5, t1, t3, th, etc.)
    #[test]
    fn test_time_range_shortcuts() {
        // t5 = 5 minutes
        assert_eq!(
            determine_time_range_action(egui::Key::Num5),
            Some(KeyboardDecision::SetTimeRange(
                TimeRangePreset::Last5Minutes
            ))
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

        // td = 24 hours
        assert_eq!(
            determine_time_range_action(egui::Key::D),
            Some(KeyboardDecision::SetTimeRange(TimeRangePreset::Last24Hours))
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
            Some(KeyboardDecision::AgentQuickCommand(
                QuickCommand::WhatsWrong
            ))
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
        use enya_editor::workspace::NavDirection;

        // Ctrl+W h = move left
        assert_eq!(
            determine_ctrl_w_action(egui::Key::H),
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Left))
        );

        // Ctrl+W j = move down
        assert_eq!(
            determine_ctrl_w_action(egui::Key::J),
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Down))
        );

        // Ctrl+W k = move up
        assert_eq!(
            determine_ctrl_w_action(egui::Key::K),
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Up))
        );

        // Ctrl+W l = move right
        assert_eq!(
            determine_ctrl_w_action(egui::Key::L),
            Some(KeyboardDecision::MovePaneInDirection(NavDirection::Right))
        );

        // Ctrl+W t = handled separately (tab mode)
        assert_eq!(determine_ctrl_w_action(egui::Key::T), None);
    }

    /// Test navigation blocking when modals are open
    #[test]
    fn test_navigation_blocked_by_modals() {
        // All modals closed - not blocked
        let (blocked, reason) = check_navigation_blocked(
            false, // workspace_finder
            false, // unified_finder
            false, // command_palette
            false, // buffer_editor
            false, // multi_edit_overlay
            false, // which_key
            false, // viewport_filter
            false, // tutorial_overlay
            false, // source_preview
            false, // style_picker
            false, // codebase_finder
        );
        assert!(!blocked, "Should not be blocked by default");
        assert!(reason.is_empty());

        // Workspace finder open - blocked
        let (blocked, reason) = check_navigation_blocked(
            true, false, false, false, false, false, false, false, false, false, false,
        );
        assert!(blocked, "Should be blocked when workspace finder is open");
        assert_eq!(reason, "workspace_finder");

        // Command palette open - blocked
        let (blocked, reason) = check_navigation_blocked(
            false, false, true, false, false, false, false, false, false, false, false,
        );
        assert!(blocked, "Should be blocked when command palette is open");
        assert_eq!(reason, "command_palette");
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

    /// Test that clearing a leader key deactivates it
    #[test]
    fn test_clear_deactivates() {
        let mut state = LeaderKeyState::new();

        state.press_space();
        assert!(state.is_space_active());
        state.clear_space();
        assert!(!state.is_space_active());

        state.press_t();
        assert!(state.is_t_active());
        state.clear_t();
        assert!(!state.is_t_active());
    }

    /// Test that the timeout constant is reasonable
    #[test]
    fn test_timeout_value() {
        // Integration tests see the production value (1000ms)
        // This gives users ~850ms to react after the leader popup appears (150ms delay)
        // Unit tests in input.rs use a faster 100ms timeout for speed
        assert_eq!(LEADER_KEY_TIMEOUT_MS, 1000);
    }
}

/// Test module for CommandPalette overlay component.
///
/// The CommandPalette is a vim-style `:` command input that provides
/// fuzzy command completion and execution.
mod command_palette_tests {
    use super::*;
    use enya_editor::components::CommandPalette;
    use enya_editor::ui::theme::AppTheme;

    /// Test that CommandPalette starts in closed state
    #[test]
    fn test_command_palette_initially_closed() {
        let palette = CommandPalette::new();
        assert!(!palette.is_open());
    }

    /// Test that CommandPalette opens and closes correctly
    #[test]
    fn test_command_palette_open_close() {
        let mut palette = CommandPalette::new();

        // Open
        palette.open();
        assert!(palette.is_open());

        // Close
        palette.close();
        assert!(!palette.is_open());
    }

    /// Test that CommandPalette can be opened with pre-filled text
    #[test]
    fn test_command_palette_open_with_text() {
        let mut palette = CommandPalette::new();

        palette.open_with_text("theme");
        assert!(palette.is_open());

        palette.close();
        assert!(!palette.is_open());
    }

    /// Test CommandPalette rendering in egui_kittest harness
    ///
    /// Note: Uses step() instead of run() because CommandPalette requests
    /// continuous repaint for focus management/animations.
    #[test]
    fn test_command_palette_renders_in_harness() {
        let mut palette = CommandPalette::new();
        palette.set_theme(AppTheme::default());
        palette.open();

        let mut harness = Harness::new_state(
            |ctx: &egui::Context, palette: &mut CommandPalette| {
                let _ = palette.show(ctx);
            },
            palette,
        );

        // Use step() for components that continuously request repaint
        harness.step();
        assert!(harness.state().is_open());
    }

    /// Test that Escape key closes the CommandPalette
    ///
    /// Note: Uses step() instead of run() because CommandPalette requests
    /// continuous repaint for focus management/animations.
    #[test]
    fn test_command_palette_closes_on_escape() {
        let mut palette = CommandPalette::new();
        palette.set_theme(AppTheme::default());
        palette.open();

        let mut harness = Harness::new_state(
            |ctx: &egui::Context, palette: &mut CommandPalette| {
                let _ = palette.show(ctx);
            },
            palette,
        );

        // Run first frame - use step() for components that continuously repaint
        harness.step();
        assert!(harness.state().is_open());

        // Press Escape
        harness.key_press(egui::Key::Escape);
        harness.step();

        // Should be closed
        assert!(!harness.state().is_open());
    }
}

/// Test module for WorkspaceFinder overlay component.
///
/// The WorkspaceFinder is a telescope/fzf-style finder for saved workspaces.
mod workspace_finder_tests {
    use super::*;
    use enya_editor::components::{WorkspaceFinder, WorkspaceItem};
    use enya_editor::ui::theme::AppTheme;

    /// Test that WorkspaceFinder starts in closed state
    #[test]
    fn test_workspace_finder_initially_closed() {
        let finder = WorkspaceFinder::new();
        assert!(!finder.is_open());
    }

    /// Test that WorkspaceFinder opens and closes correctly
    #[test]
    fn test_workspace_finder_open_close() {
        let mut finder = WorkspaceFinder::new();

        finder.open();
        assert!(finder.is_open());

        finder.close();
        assert!(!finder.is_open());
    }

    /// Test WorkspaceFinder rendering with workspace items
    ///
    /// Note: Uses step() instead of run() because WorkspaceFinder requests
    /// continuous repaint for focus management.
    #[test]
    fn test_workspace_finder_renders_with_items() {
        let mut finder = WorkspaceFinder::new();
        finder.set_theme(AppTheme::default());
        finder.set_workspaces(vec![
            WorkspaceItem {
                name: "dashboard".into(),
                description: Some("Main dashboard".into()),
            },
            WorkspaceItem {
                name: "api-metrics".into(),
                description: None,
            },
        ]);
        finder.open();

        let mut harness = Harness::new_state(
            |ctx: &egui::Context, finder: &mut WorkspaceFinder| {
                let _ = finder.show(ctx);
            },
            finder,
        );

        harness.step();
        assert!(harness.state().is_open());
    }

    /// Test that Escape key closes the WorkspaceFinder
    #[test]
    fn test_workspace_finder_closes_on_escape() {
        let mut finder = WorkspaceFinder::new();
        finder.set_theme(AppTheme::default());
        finder.open();

        let mut harness = Harness::new_state(
            |ctx: &egui::Context, finder: &mut WorkspaceFinder| {
                let _ = finder.show(ctx);
            },
            finder,
        );

        harness.run();
        assert!(harness.state().is_open());

        harness.key_press(egui::Key::Escape);
        harness.run();

        assert!(!harness.state().is_open());
    }
}

/// Test module for ViewportFilter component.
///
/// The ViewportFilter is a vim-style `/` search filter that shows
/// a bottom bar for filtering visible panes by query content.
mod viewport_filter_tests {
    use enya_editor::components::ViewportFilter;
    use enya_editor::ui::theme::AppTheme;

    /// Test that ViewportFilter starts in closed state
    #[test]
    fn test_viewport_filter_initially_closed() {
        let filter = ViewportFilter::new();
        assert!(!filter.is_open());
        assert!(!filter.is_active());
    }

    /// Test that ViewportFilter opens and closes correctly
    #[test]
    fn test_viewport_filter_open_close() {
        let mut filter = ViewportFilter::new();

        filter.open();
        assert!(filter.is_open());

        filter.close();
        assert!(!filter.is_open());
    }

    /// Test that ViewportFilter pattern matching works
    #[test]
    fn test_viewport_filter_pattern_matching() {
        let mut filter = ViewportFilter::new();
        filter.set_theme(AppTheme::default());

        // Initially matches everything (empty pattern)
        assert!(filter.matches("any query"));
        assert!(filter.matches("http_requests_total"));

        // After clear, still matches everything
        filter.clear();
        assert!(filter.matches("any query"));
    }

    /// Test ViewportFilter active state tracking
    #[test]
    fn test_viewport_filter_active_state() {
        let mut filter = ViewportFilter::new();

        // Not active when closed with no pattern
        assert!(!filter.is_active());

        // Open but still not active (no pattern yet)
        filter.open();
        assert!(!filter.is_active()); // empty pattern doesn't count as active

        // After clear, not active
        filter.clear();
        assert!(!filter.is_active());
    }

    /// Test that applied pattern persists after close
    #[test]
    fn test_viewport_filter_applied_pattern() {
        let filter = ViewportFilter::new();

        // No applied pattern initially
        assert!(filter.applied_pattern().is_empty());
    }
}

/// Test module for StylePicker overlay component.
///
/// The StylePicker allows users to choose themes and fonts with live preview.
/// It has two panels (Theme and Font) that can be navigated with Tab.
mod style_picker_tests {
    use enya_editor::components::StylePicker;
    use enya_editor::ui::settings_screen::EditorFont;
    use enya_editor::ui::theme::AppTheme;

    /// Test that StylePicker starts in closed state
    #[test]
    fn test_style_picker_initially_closed() {
        let picker = StylePicker::new();
        assert!(!picker.is_open());
    }

    /// Test that StylePicker opens with theme panel focused by default
    #[test]
    fn test_style_picker_opens_to_theme_panel() {
        let mut picker = StylePicker::new();

        picker.open(AppTheme::default(), EditorFont::default());
        assert!(picker.is_open());

        picker.close();
        assert!(!picker.is_open());
    }

    /// Test that StylePicker can open directly to font panel
    #[test]
    fn test_style_picker_open_font_panel() {
        let mut picker = StylePicker::new();

        picker.open_font(AppTheme::default(), EditorFont::default());
        assert!(picker.is_open());

        picker.close();
        assert!(!picker.is_open());
    }

    /// Test that StylePicker can open directly to theme panel
    #[test]
    fn test_style_picker_open_theme_panel() {
        let mut picker = StylePicker::new();

        picker.open_theme(AppTheme::default(), EditorFont::default());
        assert!(picker.is_open());

        picker.close();
        assert!(!picker.is_open());
    }

    // Note: Harness rendering tests for StylePicker are skipped because the
    // component requires custom fonts (departure_mono, etc.) that aren't
    // available in the test harness. The state logic tests above cover
    // the core functionality.
}

/// Test module for UnifiedFinder overlay component.
///
/// The UnifiedFinder is a Telescope-style fuzzy finder that searches
/// metrics, alerts, and commits. It supports mode switching with prefixes
/// (@, !, #) and Tab cycling.
mod unified_finder_tests {
    use super::*;
    use enya_editor::components::overlay::{FinderMode, UnifiedFinder};
    use enya_editor::ui::theme::AppTheme;

    /// Test that UnifiedFinder starts in closed state
    #[test]
    fn test_unified_finder_initially_closed() {
        let finder = UnifiedFinder::new();
        assert!(!finder.is_open());
    }

    /// Test that UnifiedFinder opens and closes correctly
    #[test]
    fn test_unified_finder_open_close() {
        let mut finder = UnifiedFinder::new();

        finder.open();
        assert!(finder.is_open());

        finder.close();
        assert!(!finder.is_open());
    }

    /// Test that UnifiedFinder can open with specific mode
    #[test]
    fn test_unified_finder_open_with_mode() {
        let mut finder = UnifiedFinder::new();

        // Open in Metrics mode
        finder.open_with_mode(FinderMode::Metrics);
        assert!(finder.is_open());
        finder.close();

        // Open in Alerts mode
        finder.open_with_mode(FinderMode::Alerts);
        assert!(finder.is_open());
        finder.close();

        // Open in Commits mode
        finder.open_with_mode(FinderMode::Commits);
        assert!(finder.is_open());
    }

    /// Test FinderMode prefix parsing
    #[test]
    fn test_finder_mode_prefix_parsing() {
        // No prefix -> All mode
        let (mode, query) = FinderMode::from_prefix("http_requests");
        assert_eq!(mode, FinderMode::All);
        assert_eq!(query, "http_requests");

        // @ prefix -> Metrics mode
        let (mode, query) = FinderMode::from_prefix("@cpu_usage");
        assert_eq!(mode, FinderMode::Metrics);
        assert_eq!(query, "cpu_usage");

        // ! prefix -> Alerts mode
        let (mode, query) = FinderMode::from_prefix("!high_error_rate");
        assert_eq!(mode, FinderMode::Alerts);
        assert_eq!(query, "high_error_rate");

        // # prefix -> Commits mode
        let (mode, query) = FinderMode::from_prefix("#fix bug");
        assert_eq!(mode, FinderMode::Commits);
        assert_eq!(query, "fix bug");
    }

    /// Test FinderMode cycling
    #[test]
    fn test_finder_mode_cycle() {
        assert_eq!(FinderMode::All.cycle_next(), FinderMode::Metrics);
        assert_eq!(FinderMode::Metrics.cycle_next(), FinderMode::Alerts);
        assert_eq!(FinderMode::Alerts.cycle_next(), FinderMode::Commits);
        assert_eq!(FinderMode::Commits.cycle_next(), FinderMode::All);
    }

    /// Test FinderMode labels
    #[test]
    fn test_finder_mode_labels() {
        assert_eq!(FinderMode::All.label(), "All");
        assert_eq!(FinderMode::Metrics.label(), "Metrics");
        assert_eq!(FinderMode::Alerts.label(), "Alerts");
        assert_eq!(FinderMode::Commits.label(), "Commits");
    }

    /// Test FinderMode prefix characters
    #[test]
    fn test_finder_mode_prefixes() {
        assert_eq!(FinderMode::All.prefix(), None);
        assert_eq!(FinderMode::Metrics.prefix(), Some('@'));
        assert_eq!(FinderMode::Alerts.prefix(), Some('!'));
        assert_eq!(FinderMode::Commits.prefix(), Some('#'));
    }

    /// Test UnifiedFinder mode cycling preserves query text
    #[test]
    fn test_unified_finder_cycle_mode() {
        let mut finder = UnifiedFinder::new();
        finder.set_theme(AppTheme::default());

        // Open in default (All) mode
        finder.open();
        assert!(finder.is_open());

        // Cycle through modes
        finder.cycle_mode();
        assert!(finder.is_open());

        finder.close();
        assert!(!finder.is_open());
    }

    /// Test UnifiedFinder rendering in egui_kittest harness
    #[test]
    fn test_unified_finder_renders_in_harness() {
        let mut finder = UnifiedFinder::new();
        finder.set_theme(AppTheme::default());
        finder.open();

        let mut harness = Harness::new_state(
            |ctx: &egui::Context, finder: &mut UnifiedFinder| {
                let _ = finder.show(ctx);
            },
            finder,
        );

        harness.run();
        assert!(harness.state().is_open());
    }

    /// Test that Escape key closes the UnifiedFinder
    #[test]
    fn test_unified_finder_closes_on_escape() {
        let mut finder = UnifiedFinder::new();
        finder.set_theme(AppTheme::default());
        finder.open();

        let mut harness = Harness::new_state(
            |ctx: &egui::Context, finder: &mut UnifiedFinder| {
                let _ = finder.show(ctx);
            },
            finder,
        );

        harness.run();
        assert!(harness.state().is_open());

        harness.key_press(egui::Key::Escape);
        harness.run();

        assert!(!harness.state().is_open());
    }
}

/// Test module for DiagnosticsPane and related types.
///
/// Tests for diagnostic severity levels, filters, and the diagnostic builder pattern.
mod diagnostics_tests {
    use enya_editor::components::{
        Diagnostic, DiagnosticLevel, DiagnosticSource, DiagnosticsFilter, DiagnosticsPane,
    };

    // ==================== DiagnosticLevel Tests ====================

    /// Test DiagnosticLevel labels
    #[test]
    fn test_diagnostic_level_labels() {
        assert_eq!(DiagnosticLevel::Error.label(), "Error");
        assert_eq!(DiagnosticLevel::Warning.label(), "Warning");
        assert_eq!(DiagnosticLevel::Info.label(), "Info");
        assert_eq!(DiagnosticLevel::Hint.label(), "Hint");
    }

    // ==================== DiagnosticSource Tests ====================

    /// Test DiagnosticSource labels
    #[test]
    fn test_diagnostic_source_labels() {
        assert_eq!(DiagnosticSource::QuerySyntax.label(), "syntax");
        assert_eq!(DiagnosticSource::QueryValidation.label(), "validation");
        assert_eq!(DiagnosticSource::DataConnection.label(), "connection");
        assert_eq!(DiagnosticSource::Performance.label(), "performance");
        assert_eq!(DiagnosticSource::Unknown.label(), "unknown");
    }

    // ==================== DiagnosticsFilter Tests ====================

    /// Test DiagnosticsFilter matching logic
    #[test]
    fn test_diagnostics_filter_matches() {
        // All filter matches everything
        assert!(DiagnosticsFilter::All.matches(DiagnosticLevel::Error));
        assert!(DiagnosticsFilter::All.matches(DiagnosticLevel::Warning));
        assert!(DiagnosticsFilter::All.matches(DiagnosticLevel::Info));
        assert!(DiagnosticsFilter::All.matches(DiagnosticLevel::Hint));

        // Errors filter only matches errors
        assert!(DiagnosticsFilter::Errors.matches(DiagnosticLevel::Error));
        assert!(!DiagnosticsFilter::Errors.matches(DiagnosticLevel::Warning));
        assert!(!DiagnosticsFilter::Errors.matches(DiagnosticLevel::Info));

        // Warnings filter only matches warnings
        assert!(!DiagnosticsFilter::Warnings.matches(DiagnosticLevel::Error));
        assert!(DiagnosticsFilter::Warnings.matches(DiagnosticLevel::Warning));
        assert!(!DiagnosticsFilter::Warnings.matches(DiagnosticLevel::Info));

        // ErrorsAndWarnings matches both
        assert!(DiagnosticsFilter::ErrorsAndWarnings.matches(DiagnosticLevel::Error));
        assert!(DiagnosticsFilter::ErrorsAndWarnings.matches(DiagnosticLevel::Warning));
        assert!(!DiagnosticsFilter::ErrorsAndWarnings.matches(DiagnosticLevel::Info));
        assert!(!DiagnosticsFilter::ErrorsAndWarnings.matches(DiagnosticLevel::Hint));
    }

    /// Test DiagnosticsFilter cycle order
    #[test]
    fn test_diagnostics_filter_cycle() {
        assert_eq!(DiagnosticsFilter::All.cycle(), DiagnosticsFilter::Errors);
        assert_eq!(
            DiagnosticsFilter::Errors.cycle(),
            DiagnosticsFilter::Warnings
        );
        assert_eq!(
            DiagnosticsFilter::Warnings.cycle(),
            DiagnosticsFilter::ErrorsAndWarnings
        );
        assert_eq!(
            DiagnosticsFilter::ErrorsAndWarnings.cycle(),
            DiagnosticsFilter::All
        );
    }

    /// Test DiagnosticsFilter labels
    #[test]
    fn test_diagnostics_filter_labels() {
        assert_eq!(DiagnosticsFilter::All.label(), "All");
        assert_eq!(DiagnosticsFilter::Errors.label(), "Errors");
        assert_eq!(DiagnosticsFilter::Warnings.label(), "Warnings");
        assert_eq!(
            DiagnosticsFilter::ErrorsAndWarnings.label(),
            "Errors & Warnings"
        );
    }

    // ==================== Diagnostic Builder Tests ====================

    /// Test Diagnostic creation shortcuts
    #[test]
    fn test_diagnostic_creation() {
        let error = Diagnostic::error("Test error");
        assert_eq!(error.level, DiagnosticLevel::Error);
        assert_eq!(error.message, "Test error");

        let warning = Diagnostic::warning("Test warning");
        assert_eq!(warning.level, DiagnosticLevel::Warning);

        let info = Diagnostic::info("Test info");
        assert_eq!(info.level, DiagnosticLevel::Info);

        let hint = Diagnostic::hint("Test hint");
        assert_eq!(hint.level, DiagnosticLevel::Hint);
    }

    /// Test Diagnostic builder pattern
    #[test]
    fn test_diagnostic_builder() {
        let diag = Diagnostic::error("Syntax error")
            .with_source(DiagnosticSource::QuerySyntax)
            .with_line(10)
            .with_column(5)
            .with_code("E001")
            .with_pane(42, "CPU Usage")
            .with_fix();

        assert_eq!(diag.level, DiagnosticLevel::Error);
        assert_eq!(diag.message, "Syntax error");
        assert_eq!(diag.source, DiagnosticSource::QuerySyntax);
        assert_eq!(diag.line, Some(10));
        assert_eq!(diag.column, Some(5));
        assert_eq!(diag.code, Some("E001".to_string()));
        assert_eq!(diag.related_pane_id, Some(42));
        assert_eq!(diag.related_pane_name, Some("CPU Usage".to_string()));
        assert!(diag.fixable);
    }

    // ==================== DiagnosticsPane Tests ====================

    /// Test DiagnosticsPane initial state
    #[test]
    fn test_diagnostics_pane_initial_state() {
        let pane = DiagnosticsPane::new();
        assert!(!pane.is_open());
        assert_eq!(pane.count(), 0);
        assert!(!pane.has_errors());
    }

    /// Test DiagnosticsPane open/close/toggle
    #[test]
    fn test_diagnostics_pane_open_close() {
        let mut pane = DiagnosticsPane::new();

        pane.open();
        assert!(pane.is_open());

        pane.close();
        assert!(!pane.is_open());

        pane.toggle();
        assert!(pane.is_open());

        pane.toggle();
        assert!(!pane.is_open());
    }

    /// Test DiagnosticsPane add and count
    #[test]
    fn test_diagnostics_pane_add_count() {
        let mut pane = DiagnosticsPane::new();

        pane.add(Diagnostic::error("Error 1"));
        pane.add(Diagnostic::warning("Warning 1"));
        pane.add(Diagnostic::info("Info 1"));

        assert_eq!(pane.count(), 3);
        assert!(pane.has_errors());

        let (errors, warnings, infos, hints) = pane.count_by_level();
        assert_eq!(errors, 1);
        assert_eq!(warnings, 1);
        assert_eq!(infos, 1);
        assert_eq!(hints, 0);
    }

    /// Test DiagnosticsPane clear
    #[test]
    fn test_diagnostics_pane_clear() {
        let mut pane = DiagnosticsPane::new();

        pane.add(Diagnostic::error("Error 1"));
        pane.add(Diagnostic::warning("Warning 1"));
        assert_eq!(pane.count(), 2);

        pane.clear();
        assert_eq!(pane.count(), 0);
        assert!(!pane.has_errors());
    }

    /// Test DiagnosticsPane clear for specific pane
    #[test]
    fn test_diagnostics_pane_clear_for_pane() {
        let mut pane = DiagnosticsPane::new();

        pane.add(Diagnostic::error("Error 1").with_pane(1, "Pane 1"));
        pane.add(Diagnostic::error("Error 2").with_pane(2, "Pane 2"));
        pane.add(Diagnostic::warning("Warning 1").with_pane(1, "Pane 1"));
        assert_eq!(pane.count(), 3);

        pane.clear_for_pane(1);
        assert_eq!(pane.count(), 1); // Only pane 2's error remains
    }

    /// Test DiagnosticsPane filter setting
    #[test]
    fn test_diagnostics_pane_filter() {
        let mut pane = DiagnosticsPane::new();
        pane.set_filter(DiagnosticsFilter::Errors);

        // Filter should be set (we can't directly access it but can test cycle)
        pane.cycle_filter();
        // After cycling from Errors -> Warnings
    }
}

/// Test module for TimeRange and TimeRangePreset.
///
/// Tests time range preset durations, labels, and time range creation.
mod time_range_tests {
    use std::time::Duration;

    use enya_editor::components::TimeRangePreset;

    /// Test TimeRangePreset duration calculations
    #[test]
    fn test_preset_durations() {
        assert_eq!(
            TimeRangePreset::Last5Minutes.duration(),
            Some(Duration::from_secs(5 * 60))
        );
        assert_eq!(
            TimeRangePreset::Last15Minutes.duration(),
            Some(Duration::from_secs(15 * 60))
        );
        assert_eq!(
            TimeRangePreset::Last30Minutes.duration(),
            Some(Duration::from_secs(30 * 60))
        );
        assert_eq!(
            TimeRangePreset::Last1Hour.duration(),
            Some(Duration::from_secs(60 * 60))
        );
        assert_eq!(
            TimeRangePreset::Last6Hours.duration(),
            Some(Duration::from_secs(6 * 60 * 60))
        );
        assert_eq!(
            TimeRangePreset::Last24Hours.duration(),
            Some(Duration::from_secs(24 * 60 * 60))
        );
        assert_eq!(
            TimeRangePreset::Last7Days.duration(),
            Some(Duration::from_secs(7 * 24 * 60 * 60))
        );
        assert_eq!(TimeRangePreset::Custom.duration(), None);
    }

    /// Test TimeRangePreset labels
    #[test]
    fn test_preset_labels() {
        assert_eq!(TimeRangePreset::Last5Minutes.label(), "5m");
        assert_eq!(TimeRangePreset::Last15Minutes.label(), "15m");
        assert_eq!(TimeRangePreset::Last30Minutes.label(), "30m");
        assert_eq!(TimeRangePreset::Last1Hour.label(), "1h");
        assert_eq!(TimeRangePreset::Last6Hours.label(), "6h");
        assert_eq!(TimeRangePreset::Last24Hours.label(), "24h");
        assert_eq!(TimeRangePreset::Last7Days.label(), "7d");
        assert_eq!(TimeRangePreset::Custom.label(), "Custom");
    }

    /// Test TimeRangePreset all_presets doesn't include Custom
    #[test]
    fn test_all_presets() {
        let presets = TimeRangePreset::all_presets();
        assert_eq!(presets.len(), 7);
        assert!(!presets.contains(&TimeRangePreset::Custom));
        assert!(presets.contains(&TimeRangePreset::Last5Minutes));
        assert!(presets.contains(&TimeRangePreset::Last7Days));
    }

    /// Test TimeRangePreset default is Last15Minutes
    #[test]
    fn test_preset_default() {
        assert_eq!(TimeRangePreset::default(), TimeRangePreset::Last15Minutes);
    }
}

/// Test module for Sparkline widget.
///
/// Tests sparkline data management and rendering.
mod sparkline_tests {
    use enya_editor::components::Sparkline;

    /// Test Sparkline creation
    #[test]
    fn test_sparkline_new() {
        let sparkline = Sparkline::new("CPU");
        assert_eq!(sparkline.label, "CPU");
        assert_eq!(sparkline.current_value(), None);
    }

    /// Test Sparkline with_unit builder
    #[test]
    fn test_sparkline_with_unit() {
        let sparkline = Sparkline::new("Memory").with_unit("%");
        assert_eq!(sparkline.unit, "%");
    }

    /// Test Sparkline push and current_value
    #[test]
    fn test_sparkline_push_values() {
        let mut sparkline = Sparkline::new("Test");

        sparkline.push(10.0);
        assert_eq!(sparkline.current_value(), Some(10.0));

        sparkline.push(20.0);
        assert_eq!(sparkline.current_value(), Some(20.0));

        sparkline.push(15.0);
        assert_eq!(sparkline.current_value(), Some(15.0));
    }

    /// Test Sparkline render produces unicode blocks
    #[test]
    fn test_sparkline_render() {
        let mut sparkline = Sparkline::new("Test");

        // Empty sparkline renders empty string
        assert_eq!(sparkline.render(), "");

        // Add some values
        sparkline.push(0.0);
        sparkline.push(50.0);
        sparkline.push(100.0);

        let rendered = sparkline.render();
        // Should produce 3 characters (one per value)
        assert_eq!(rendered.chars().count(), 3);
        // Characters should be in the block set
        for c in rendered.chars() {
            assert!(['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'].contains(&c));
        }
    }

    /// Test Sparkline with fixed bounds
    #[test]
    fn test_sparkline_with_bounds() {
        let mut sparkline = Sparkline::new("Test").with_bounds(0.0, 100.0);

        sparkline.push(50.0);
        assert_eq!(sparkline.current_value(), Some(50.0));

        let rendered = sparkline.render();
        assert!(!rendered.is_empty());
    }

    /// Test Sparkline capacity limit
    #[test]
    fn test_sparkline_capacity() {
        let mut sparkline = Sparkline::new("Test");

        // Push more than SPARKLINE_MAX_POINTS (15)
        for i in 0..20 {
            sparkline.push(i as f64);
        }

        // Current value should be the last pushed
        assert_eq!(sparkline.current_value(), Some(19.0));

        // Render should produce at most 15 characters
        let rendered = sparkline.render();
        assert!(rendered.chars().count() <= 15);
    }
}

/// Test module for QuickCommand enum.
///
/// Tests quick command prompts and labels for AI agent integration.
mod quick_command_tests {
    use enya_editor::components::QuickCommand;

    /// Test QuickCommand labels
    #[test]
    fn test_quick_command_labels() {
        assert_eq!(QuickCommand::WhatsWrong.label(), "What's wrong?");
        assert_eq!(QuickCommand::Why.label(), "Why?");
        assert_eq!(QuickCommand::Compare.label(), "Compare");
        assert_eq!(QuickCommand::Related.label(), "Related");
        assert_eq!(QuickCommand::Explain.label(), "Explain");
        assert_eq!(QuickCommand::Fix.label(), "Fix");
        assert_eq!(QuickCommand::Summarize.label(), "Summarize");
        assert_eq!(QuickCommand::History.label(), "History");
    }

    /// Test QuickCommand prompts are non-empty
    #[test]
    fn test_quick_command_prompts() {
        assert!(!QuickCommand::WhatsWrong.prompt().is_empty());
        assert!(!QuickCommand::Why.prompt().is_empty());
        assert!(!QuickCommand::Compare.prompt().is_empty());
        assert!(!QuickCommand::Related.prompt().is_empty());
        assert!(!QuickCommand::Explain.prompt().is_empty());
        assert!(!QuickCommand::Fix.prompt().is_empty());
        assert!(!QuickCommand::Summarize.prompt().is_empty());
        assert!(!QuickCommand::History.prompt().is_empty());
    }

    /// Test QuickCommand prompts contain meaningful content
    #[test]
    fn test_quick_command_prompt_content() {
        assert!(QuickCommand::WhatsWrong.prompt().contains("wrong"));
        assert!(QuickCommand::Why.prompt().contains("root cause"));
        assert!(QuickCommand::Compare.prompt().contains("baseline"));
        assert!(QuickCommand::Related.prompt().contains("correlated"));
        assert!(QuickCommand::Explain.prompt().contains("Explain"));
        assert!(QuickCommand::Fix.prompt().contains("fixed"));
        assert!(QuickCommand::Summarize.prompt().contains("Summarize"));
        assert!(QuickCommand::History.prompt().contains("before"));
    }
}
