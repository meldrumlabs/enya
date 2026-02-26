//! Keyboard input handling for workspace navigation.
//!
//! This module contains vim-style keyboard navigation handlers for the workspace,
//! including normal mode navigation (h/j/k/l), visual-multi mode selection,
//! and leader key sequences (Space+w, Space+f, etc.).

use egui_tiles::{Tile, TileId};

use super::{NavDirection, Workspace, WorkspaceAction};
use crate::components::{
    Buffer, BufferMode, EditExcerpt, QueryPane, QuickCommand, TimeRangePreset,
};

impl Workspace {
    /// Handle vim-style keyboard navigation for the viewport.
    /// Returns an optional WorkspaceAction if a key triggered an action.
    #[profiling::function]
    pub fn handle_viewport_keyboard(&mut self, ctx: &egui::Context) -> Option<WorkspaceAction> {
        // Global ':' handler - command palette should ALWAYS be openable on top of any overlay
        // (except when command palette itself is already open, or a text field has focus)
        // This is checked FIRST, before any overlay blocks, so :style works on top of diff viewer, etc.
        if !self.command_palette.is_open() && !ctx.memory(|mem| mem.focused().is_some()) {
            let mut open_command_palette = false;
            ctx.input_mut(|input| {
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Colon) {
                    open_command_palette = true;
                }
            });
            if open_command_palette {
                self.command_palette.open();
                ctx.request_repaint();
                return None;
            }
        }

        // Don't handle keys if a text field or modal has focus.
        //
        // IMPORTANT: When closing overlays or text inputs that should return to vim navigation,
        // you must clear BOTH widget-level AND global egui focus:
        //   1. response.surrender_focus()  - clears widget-level focus
        //   2. ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL))  - clears global focus
        //
        // If only widget-level focus is cleared, this check still returns early and vim keys
        // won't work until the user clicks elsewhere. See chat_view.rs and style_picker.rs
        // for examples of the correct pattern.
        if ctx.memory(|mem| mem.focused().is_some()) {
            return None;
        }

        // Don't handle if any modal is open
        #[cfg(not(target_arch = "wasm32"))]
        let codebase_finder_open = self.codebase_finder.is_open();
        #[cfg(target_arch = "wasm32")]
        let codebase_finder_open = false;

        if self.unified_finder.is_open()
            || self.command_palette.is_open()
            || self.buffer_editor.is_open()
            || self.multi_edit_overlay.is_open()
            || self.which_key.is_open()
            || self.viewport_filter.is_open()
            || self.tutorial_overlay.is_open()
            || self.plugins_overlay.is_open()
            || self.source_preview.is_open()
            || self.style_picker.is_open()
            || codebase_finder_open
        // Note: agent_panel.is_open() intentionally NOT checked here.
        // The agent panel can be open while viewport has focus (agent_panel_focused is checked separately).
        {
            return None;
        }

        // Handle agent mode - keyboard is handled by AgentInputBar
        if self.agent_mode_active {
            // Agent input bar handles its own keyboard in show()
            return None;
        }

        // Handle visual-multi mode keyboard shortcuts
        if self.visual_multi_state.is_some() {
            return self.handle_visual_multi_keyboard(ctx);
        }

        // When agent panel has focus, let it handle h/j/k/l navigation
        // (viewport keyboard handling is skipped)
        if self.agent_panel_focused {
            return None;
        }

        // Check if any buffer is in insert mode - if so, don't handle navigation keys
        if self.is_any_buffer_in_insert_mode() {
            return None;
        }

        let pane_ids = self.get_pane_tile_ids();
        let current_focus = self.behavior.focused_tile();

        let mut consumed = false;
        let mut should_clear_focus = false;
        let mut should_close_focused = false;
        let mut should_toggle_zen = false;
        let mut should_toggle_fullscreen = false;
        let mut should_share_pane = false;
        let mut should_open_which_key = false;
        let mut should_enter_visual_multi = false;
        let mut should_cycle_visualization = false;
        let mut should_open_unified_finder = false;
        #[cfg(not(target_arch = "wasm32"))]
        let mut should_open_codebase_finder = false;
        let mut should_show_home = false;
        let mut should_toggle_diagnostics = false;
        let mut should_open_plugins_overlay = false;
        let mut should_edit_buffer = false;
        let mut should_go_to_definition = false;
        let mut should_go_to_alert = false;
        let mut should_show_definition_demo = false;
        let mut should_toggle_agent_panel = false;
        let mut should_toggle_project_sidebar = false;
        let mut should_enter_agent_mode = false;
        let mut should_enter_agent_mode_typing = false;
        let mut agent_quick_command: Option<QuickCommand> = None;
        let mut new_tile_id: Option<TileId> = None;
        let mut time_range_preset: Option<TimeRangePreset> = None;
        let mut should_move_pane_left = false;
        let mut should_move_pane_right = false;
        let mut should_move_pane_up = false;
        let mut should_move_pane_down = false;
        let mut should_tab_pane_left = false;
        let mut should_tab_pane_right = false;
        let mut should_tab_pane_up = false;
        let mut should_tab_pane_down = false;
        let mut should_float_focused_pane = false;
        let mut should_focus_agent_panel = false;
        let mut should_focus_sidebar = false;
        let mut should_undo = false;
        let mut should_open_time_range_picker = false;

        ctx.input_mut(|input| {
            // yy - share focused pane (vim-style yank)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Y) && current_focus.is_some() {
                if self.leader_keys.is_yy_active() {
                    // Second y within timeout - trigger share
                    should_share_pane = true;
                    self.leader_keys.clear_y();
                    consumed = true;
                    return;
                }
                // First y - record time
                self.leader_keys.press_y();
                consumed = true;
                return;
            }

            // cv - cycle visualization type on focused pane (time series -> stat -> ...)
            // Only handle if Space leader key is NOT active (Space+c is codebase finder)
            // NOTE: Check space_active BEFORE consume_key, as consume_key has side effects
            if !self.leader_keys.is_space_active()
                && current_focus.is_some()
                && input.consume_key(egui::Modifiers::NONE, egui::Key::C)
            {
                // Record c press time for cv detection
                self.leader_keys.press_c();
                consumed = true;
                return;
            }

            if input.consume_key(egui::Modifiers::NONE, egui::Key::V) && current_focus.is_some() {
                // Check if this is part of a cv sequence
                if self.leader_keys.is_cv_ready() {
                    should_cycle_visualization = true;
                    self.leader_keys.clear_c();
                    consumed = true;
                    return;
                }
            }

            // Space - leader key for sequences (Space+w, Space+f, Space+a, etc.)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Space) {
                self.leader_keys.press_space();
                consumed = true;
                return;
            }

            // Leader key sequences (must follow Space within timeout)
            if self.leader_keys.is_space_active() {
                // Space+f - open unified finder (Telescope-style)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::F) {
                    should_open_unified_finder = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Space+h - show home/landing page
                if input.consume_key(egui::Modifiers::NONE, egui::Key::H) {
                    should_show_home = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Space+d - toggle diagnostics overlay
                if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                    should_toggle_diagnostics = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Space+a - open/focus agent pane (Claude Code)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::A) {
                    log::info!(
                        "Keyboard: Space+a pressed, setting should_toggle_agent_panel = true"
                    );
                    should_toggle_agent_panel = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Space+c - open codebase finder (native only)
                #[cfg(not(target_arch = "wasm32"))]
                if input.consume_key(egui::Modifiers::NONE, egui::Key::C) {
                    should_open_codebase_finder = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Space+t - open time range picker
                if input.consume_key(egui::Modifiers::NONE, egui::Key::T) {
                    should_open_time_range_picker = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Space+b - toggle project sidebar
                if input.consume_key(egui::Modifiers::NONE, egui::Key::B) {
                    should_toggle_project_sidebar = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Space+p - open plugins overlay
                if input.consume_key(egui::Modifiers::NONE, egui::Key::P) {
                    should_open_plugins_overlay = true;
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Escape - cancel space leader key (dismiss the leader popup)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    self.leader_keys.clear_space();
                    consumed = true;
                    return;
                }

                // Any other key press dismisses the leader popup (neovim which-key behavior)
                // Check if any non-modifier key was pressed
                let any_key_pressed = input
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }));
                if any_key_pressed {
                    self.leader_keys.clear_space();
                    // Don't consume - let the key be handled by other handlers
                    return;
                }
            }

            // t - time range leader key (t5, t1, t3, th, t6, td, tw)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::T) {
                self.leader_keys.press_t();
                consumed = true;
                return;
            }

            // g - go-to leader key (gd = go to definition)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::G) {
                log::debug!("'g' key pressed - setting go-to leader key");
                self.leader_keys.press_g();
                consumed = true;
                return;
            }

            // Time range shortcuts (must follow 't' within timeout)
            if self.leader_keys.is_t_active() {
                // t5 - Last 5 minutes
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num5) {
                    time_range_preset = Some(TimeRangePreset::Last5Minutes);
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
                // t1 - Last 15 minutes (default, easy to type)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num1) {
                    time_range_preset = Some(TimeRangePreset::Last15Minutes);
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
                // t3 - Last 30 minutes
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num3) {
                    time_range_preset = Some(TimeRangePreset::Last30Minutes);
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
                // th - Last 1 hour
                if input.consume_key(egui::Modifiers::NONE, egui::Key::H) {
                    time_range_preset = Some(TimeRangePreset::Last1Hour);
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
                // t6 - Last 6 hours
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Num6) {
                    time_range_preset = Some(TimeRangePreset::Last6Hours);
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
                // td - Last 24 hours (day)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                    time_range_preset = Some(TimeRangePreset::Last24Hours);
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
                // tw - Last 7 days (week)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::W) {
                    time_range_preset = Some(TimeRangePreset::Last7Days);
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
                // tc - Custom time range picker
                if input.consume_key(egui::Modifiers::NONE, egui::Key::C) {
                    should_open_time_range_picker = true;
                    self.leader_keys.clear_t();
                    consumed = true;
                    return;
                }
            }

            // Go-to shortcuts (must follow 'g' within timeout)
            if self.leader_keys.is_g_active() {
                log::debug!("g leader key is active, checking for d/a/p");
                // gd - go to definition
                if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                    log::debug!("gd shortcut triggered - go to definition");
                    should_go_to_definition = true;
                    self.leader_keys.clear_g();
                    consumed = true;
                    return;
                }
                // ga - go to alert
                if input.consume_key(egui::Modifiers::NONE, egui::Key::A) {
                    log::debug!("ga shortcut triggered - go to alert");
                    should_go_to_alert = true;
                    self.leader_keys.clear_g();
                    consumed = true;
                    return;
                }
                // gp - show definition demo/preview overlay (for testing)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::P) {
                    log::debug!("gp shortcut triggered - showing definition demo");
                    should_show_definition_demo = true;
                    self.leader_keys.clear_g();
                    consumed = true;
                    return;
                }
                // gf - float focused pane (detach to floating window)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::F) {
                    log::debug!("gf shortcut triggered - float focused pane");
                    should_float_focused_pane = true;
                    self.leader_keys.clear_g();
                    consumed = true;
                    return;
                }
            }

            // Agent operator shortcuts (must follow 'a' within timeout)
            // Check these BEFORE other single-key shortcuts to prevent e/f/h/etc from being consumed
            if self.leader_keys.is_a_active() {
                // aw - What's wrong? (triage)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::W) {
                    agent_quick_command = Some(QuickCommand::WhatsWrong);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // ae - Explain (focused element)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::E) {
                    agent_quick_command = Some(QuickCommand::Explain);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // ay - Why? (root cause)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Y) {
                    agent_quick_command = Some(QuickCommand::Why);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // ac - Compare (to baseline)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::C) {
                    agent_quick_command = Some(QuickCommand::Compare);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // ar - Related (correlated metrics)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::R) {
                    agent_quick_command = Some(QuickCommand::Related);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // af - Fix (remediation suggestions)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::F) {
                    agent_quick_command = Some(QuickCommand::Fix);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // as - Summarize (incident summary)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::S) {
                    agent_quick_command = Some(QuickCommand::Summarize);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // ah - History (past similar incidents)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::H) {
                    agent_quick_command = Some(QuickCommand::History);
                    should_enter_agent_mode = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
                // aa - just enter agent mode in typing mode (double tap)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::A) {
                    should_enter_agent_mode_typing = true;
                    self.leader_keys.clear_a();
                    consumed = true;
                    return;
                }
            }

            // e - enter edit mode on focused pane (vim-style)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::E) && current_focus.is_some() {
                should_edit_buffer = true;
                consumed = true;
                return;
            }

            // Z - toggle zen mode (works even with no panes)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Z) {
                should_toggle_zen = true;
                consumed = true;
                return;
            }
            // F - toggle fullscreen for focused pane
            if input.consume_key(egui::Modifiers::NONE, egui::Key::F) {
                should_toggle_fullscreen = true;
                consumed = true;
                return;
            }

            // a - agent operator (aw, ae, ay, ac, ar, af, as, ah) or just enter agent mode
            if input.consume_key(egui::Modifiers::NONE, egui::Key::A) {
                log::info!("Keyboard: standalone 'a' pressed (agent operator), NOT Space+a");
                // Record 'a' press time for agent operator detection
                self.leader_keys.press_a();
                consumed = true;
                return;
            }

            // Ctrl+W - window management leader key (vim-style Ctrl+W h/j/k/l)
            if input.consume_key(egui::Modifiers::CTRL, egui::Key::W) {
                self.leader_keys.press_ctrl_w();
                consumed = true;
                return;
            }

            // Ctrl+W sequences (must follow Ctrl+W within timeout)
            // Note: We accept keys with Ctrl still held since users often keep Ctrl pressed
            // throughout the sequence (especially on macOS).
            if self.leader_keys.is_ctrl_w_active() {
                let ctrl_only = egui::Modifiers {
                    ctrl: true,
                    ..Default::default()
                };

                // Ctrl+W t - enter tab mode (merge focused pane into tab with neighbor)
                if input.consume_key(egui::Modifiers::NONE, egui::Key::T)
                    || input.consume_key(ctrl_only, egui::Key::T)
                {
                    self.leader_keys.press_ctrl_w_t();
                    self.leader_keys.clear_ctrl_w();
                    consumed = true;
                    return;
                }

                // Ctrl+W h - move pane to far left
                if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                    || input.consume_key(ctrl_only, egui::Key::H)
                {
                    should_move_pane_left = true;
                    self.leader_keys.clear_ctrl_w();
                    consumed = true;
                    return;
                }
                // Ctrl+W l - move pane to far right
                if input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                    || input.consume_key(ctrl_only, egui::Key::L)
                {
                    should_move_pane_right = true;
                    self.leader_keys.clear_ctrl_w();
                    consumed = true;
                    return;
                }
                // Ctrl+W k - move pane to top
                if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                    || input.consume_key(ctrl_only, egui::Key::K)
                {
                    should_move_pane_up = true;
                    self.leader_keys.clear_ctrl_w();
                    consumed = true;
                    return;
                }
                // Ctrl+W j - move pane to bottom
                if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                    || input.consume_key(ctrl_only, egui::Key::J)
                {
                    should_move_pane_down = true;
                    self.leader_keys.clear_ctrl_w();
                    consumed = true;
                    return;
                }
            }

            // Ctrl+W t sequences - merge focused pane into tab with neighbor in direction
            // Accept direction keys with no modifiers OR with Ctrl still held (common on macOS)
            if self.leader_keys.is_ctrl_w_t_active() {
                let ctrl_only = egui::Modifiers {
                    ctrl: true,
                    ..Default::default()
                };

                // Ctrl+W t h - merge with pane to the left
                if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                    || input.consume_key(ctrl_only, egui::Key::H)
                {
                    should_tab_pane_left = true;
                    self.leader_keys.clear_ctrl_w_t();
                    consumed = true;
                    return;
                }
                // Ctrl+W t l - merge with pane to the right
                if input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                    || input.consume_key(ctrl_only, egui::Key::L)
                {
                    should_tab_pane_right = true;
                    self.leader_keys.clear_ctrl_w_t();
                    consumed = true;
                    return;
                }
                // Ctrl+W t k - merge with pane above
                if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                    || input.consume_key(ctrl_only, egui::Key::K)
                {
                    should_tab_pane_up = true;
                    self.leader_keys.clear_ctrl_w_t();
                    consumed = true;
                    return;
                }
                // Ctrl+W t j - merge with pane below
                if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                    || input.consume_key(ctrl_only, egui::Key::J)
                {
                    should_tab_pane_down = true;
                    self.leader_keys.clear_ctrl_w_t();
                    consumed = true;
                    return;
                }
            }

            // Ctrl+V - enter visual-block (multi-select) mode
            // If no pane is focused, auto-focus the first (topmost) pane
            if input.consume_key(egui::Modifiers::CTRL, egui::Key::V) {
                if current_focus.is_none() {
                    new_tile_id = pane_ids.first().copied();
                }
                should_enter_visual_multi = true;
                consumed = true;
                return;
            }

            // ? - open which-key help overlay (Shift+/ on US keyboards)
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::Slash) {
                should_open_which_key = true;
                consumed = true;
                return;
            }

            // h or left arrow - move left (or focus sidebar at left edge)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
            {
                if let Some(current_id) = current_focus {
                    let sibling = self.find_sibling_in_direction(current_id, NavDirection::Left);
                    if sibling.is_some() {
                        new_tile_id = sibling;
                    } else {
                        // At left edge — focus the sidebar
                        should_focus_sidebar = true;
                    }
                } else {
                    // No focus — focus the sidebar
                    should_focus_sidebar = true;
                }
                consumed = true;
                return;
            }

            // l or right arrow - move right
            if input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
            {
                if let Some(current_id) = current_focus {
                    let sibling = self.find_sibling_in_direction(current_id, NavDirection::Right);
                    if sibling.is_some() {
                        new_tile_id = sibling;
                    } else if self.agent_panel.is_open() {
                        // At right edge with agent panel open - focus the panel
                        should_focus_agent_panel = true;
                    }
                } else if self.agent_panel.is_open() {
                    // No focus and agent panel open - focus the panel
                    should_focus_agent_panel = true;
                } else {
                    new_tile_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // j or down arrow - move down
            if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                if let Some(current_id) = current_focus {
                    new_tile_id = self.find_sibling_in_direction(current_id, NavDirection::Down);
                } else {
                    new_tile_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // k or up arrow - move up
            if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                if let Some(current_id) = current_focus {
                    new_tile_id = self.find_sibling_in_direction(current_id, NavDirection::Up);
                } else {
                    new_tile_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // u - undo last action (vim-style)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::U) {
                should_undo = true;
                consumed = true;
                return;
            }

            // x - close focused pane
            if input.consume_key(egui::Modifiers::NONE, egui::Key::X) && current_focus.is_some() {
                should_close_focused = true;
                consumed = true;
                return;
            }

            // Escape - clear focus
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_clear_focus = true;
                consumed = true;
            }
        });

        // Handle share pane action (yy)
        if should_share_pane {
            if let Some(tile_id) = current_focus {
                // Find the pane index for the focused tile
                if let Some(pane_index) = self.get_pane_index(tile_id) {
                    // Trigger yank flash visual effect
                    self.behavior.trigger_yank_flash(tile_id);
                    ctx.request_repaint();
                    return Some(WorkspaceAction::SharePane(pane_index));
                }
            }
        }

        // Handle undo action (u)
        if should_undo {
            self.execute_undo();
            ctx.request_repaint();
        }

        // Handle time range preset changes (t5, t1, th, td, tw, etc.)
        if let Some(preset) = time_range_preset {
            self.time_range_toolbar.set_preset(preset);
            // Trigger global refresh of all panes (Grafana-style)
            self.refresh_all_panes();
            log::debug!("Time range set to {preset:?} via keyboard, refreshing all panes");
            ctx.request_repaint();
        }

        // Handle workspace finder (w key)
        // Handle unified finder (Space+f)
        if should_open_unified_finder {
            self.open_unified_finder();
            ctx.request_repaint();
        }

        // Handle codebase finder (Space+c) - native only
        #[cfg(not(target_arch = "wasm32"))]
        if should_open_codebase_finder {
            self.codebase_finder.open();
            ctx.request_repaint();
        }

        if should_show_home {
            self.show_landing = true;
            self.close_all_charts();
            ctx.request_repaint();
        }

        if should_toggle_diagnostics {
            self.toggle_diagnostics();
            ctx.request_repaint();
        }

        if should_open_plugins_overlay {
            self.plugins_overlay.open();
            ctx.request_repaint();
        }

        if should_open_time_range_picker {
            self.pending_open_time_range_picker = true;
            ctx.request_repaint();
        }

        if should_toggle_agent_panel {
            // Toggle the agent panel (right-side layout panel)
            log::info!("Keyboard: toggling agent panel");
            self.agent_panel.toggle();
            // Set focus state based on whether panel is now open
            if self.agent_panel.is_open() {
                log::info!("Keyboard: agent panel opened, setting agent_panel_focused = true");
                self.agent_panel_focused = true;
                self.agent_panel.set_focus(true);
                // Clear viewport pane focus so it doesn't appear highlighted
                self.behavior.set_focused_tile(None);

                // Clear egui widget focus so vim keys work immediately in the panel
                // (otherwise a TextEdit or ComboBox from viewport might still consume keys)
                ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            } else {
                log::info!("Keyboard: agent panel closed, setting agent_panel_focused = false");
                self.agent_panel_focused = false;
                self.agent_panel.set_focus(false);
            }
            ctx.request_repaint();
        }

        if should_enter_agent_mode {
            if let Some(command) = agent_quick_command {
                self.enter_agent_mode_with_command(command);
            } else {
                self.enter_agent_mode();
            }
            ctx.request_repaint();
        }

        if should_enter_agent_mode_typing {
            self.enter_agent_mode_typing();
            ctx.request_repaint();
        }

        // Handle pane movement (Ctrl+W h/j/k/l)
        if should_move_pane_left {
            self.move_pane_to_far_left();
            ctx.request_repaint();
        } else if should_move_pane_right {
            self.move_pane_to_far_right();
            ctx.request_repaint();
        } else if should_move_pane_up {
            self.move_pane_to_top();
            ctx.request_repaint();
        } else if should_move_pane_down {
            self.move_pane_to_bottom();
            ctx.request_repaint();
        }

        // Handle pane tabbing (Ctrl+W t h/j/k/l)
        if should_tab_pane_left {
            self.move_pane_to_tab_with(NavDirection::Left);
            ctx.request_repaint();
        } else if should_tab_pane_right {
            self.move_pane_to_tab_with(NavDirection::Right);
            ctx.request_repaint();
        } else if should_tab_pane_up {
            self.move_pane_to_tab_with(NavDirection::Up);
            ctx.request_repaint();
        } else if should_tab_pane_down {
            self.move_pane_to_tab_with(NavDirection::Down);
            ctx.request_repaint();
        }

        // Handle focus transfer to agent panel (vim l at right edge)
        if should_focus_agent_panel {
            self.agent_panel_focused = true;
            self.agent_panel.set_focus(true);
            // Clear viewport pane focus so it doesn't appear highlighted
            self.behavior.set_focused_tile(None);
            // Clear egui widget focus so vim keys work immediately in the panel
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            ctx.request_repaint();
        }

        // Handle focus transfer to project sidebar (vim h at left edge)
        if should_focus_sidebar {
            // Clear viewport pane focus
            self.behavior.set_focused_tile(None);
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            ctx.request_repaint();
            return Some(WorkspaceAction::FocusProjectSidebar);
        }

        // Handle Space+b toggle project sidebar
        if should_toggle_project_sidebar {
            self.behavior.set_focused_tile(None);
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            ctx.request_repaint();
            return Some(WorkspaceAction::ToggleProjectSidebar);
        }

        if should_open_which_key {
            self.which_key.open();
        } else if should_enter_visual_multi {
            // Use the newly auto-focused tile if we set one, otherwise use current focus
            let starting_tile = new_tile_id.or(current_focus);
            if let Some(tile_id) = starting_tile {
                self.enter_visual_multi_mode(tile_id);
            }
        } else if should_edit_buffer {
            self.edit_focused_buffer();
        } else if should_go_to_definition {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(metric_name) = self.get_focused_metric_name() {
                self.open_metric_definition(&metric_name);
            }
        } else if should_go_to_alert {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(metric_name) = self.get_focused_metric_name() {
                self.open_alert_for_metric(&metric_name);
            }
        } else if should_show_definition_demo {
            log::debug!("Executing open_source_preview_demo()");
            #[cfg(not(target_arch = "wasm32"))]
            self.open_source_preview_demo();
        } else if should_cycle_visualization {
            self.cycle_focused_visualization();
        } else if should_toggle_zen {
            self.toggle_zen_mode();
        } else if should_toggle_fullscreen {
            self.toggle_fullscreen();
        } else if should_float_focused_pane {
            self.float_focused_pane(None);
        } else if should_close_focused {
            if let Some(tile_id) = current_focus {
                self.close_tile(tile_id);
            }
        } else if should_clear_focus {
            self.behavior.set_focused_tile(None);
        } else if let Some(tile_id) = new_tile_id {
            // Set focus and also switch to that tab if it's in a tabs container
            self.behavior.set_focused_tile(Some(tile_id));
            self.activate_tile(tile_id);
            // Trigger smooth scroll to bring the focused tile into view
            self.scroll_to_focused_tile(ctx);
        }

        if consumed {
            ctx.request_repaint();
            log::debug!(
                "Workspace navigation: focus is now {:?}",
                self.behavior.focused_tile()
            );
        }

        None
    }

    /// Handle keyboard input while in visual-multi mode.
    pub(super) fn handle_visual_multi_keyboard(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<WorkspaceAction> {
        let pane_ids = self.get_pane_tile_ids();

        // Get current cursor position from visual-multi state, validating it still exists
        let cursor_tile_id = self
            .visual_multi_state
            .as_ref()
            .and_then(|s| s.cursor_tile_id)
            .filter(|id| pane_ids.contains(id));

        // If cursor was invalid, reset to first pane
        if cursor_tile_id.is_none() {
            if let Some(state) = self.visual_multi_state.as_mut() {
                if let Some(&first_pane) = pane_ids.first() {
                    state.set_cursor(first_pane);
                }
            }
        }

        // Re-read cursor after potential reset
        let cursor_tile_id = self
            .visual_multi_state
            .as_ref()
            .and_then(|s| s.cursor_tile_id);

        let mut consumed = false;
        let mut should_exit = false;
        let mut should_toggle_selection = false;
        let mut should_select_all = false;
        let mut should_clear_selection = false;
        let mut should_open_multi_edit = false;
        let mut should_close_selected = false;
        let mut should_refresh_selected = false;
        let mut should_enter_agent_mode = false;
        let mut should_share_selected = false;
        let mut new_cursor_id: Option<TileId> = None;

        ctx.input_mut(|input| {
            // yy - share selected panes (vim-style yank)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Y) {
                if self.leader_keys.is_yy_active() {
                    // Second y within timeout - trigger share
                    should_share_selected = true;
                    self.leader_keys.clear_y();
                    consumed = true;
                    return;
                }
                // First y - record time
                self.leader_keys.press_y();
                consumed = true;
                return;
            }

            // Escape - exit visual-multi mode
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_exit = true;
                consumed = true;
                return;
            }

            // e - open multi-edit overlay for selected panes
            if input.consume_key(egui::Modifiers::NONE, egui::Key::E) {
                should_open_multi_edit = true;
                consumed = true;
                return;
            }

            // x - close all selected panes
            if input.consume_key(egui::Modifiers::NONE, egui::Key::X) {
                should_close_selected = true;
                consumed = true;
                return;
            }

            // r - refresh all selected panes
            if input.consume_key(egui::Modifiers::NONE, egui::Key::R) {
                should_refresh_selected = true;
                consumed = true;
                return;
            }

            // Space - toggle selection on current pane
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Space) {
                should_toggle_selection = true;
                consumed = true;
                return;
            }

            // a - enter agent mode with selected panes as context
            if input.consume_key(egui::Modifiers::NONE, egui::Key::A) {
                should_enter_agent_mode = true;
                consumed = true;
                return;
            }

            // A (Shift+A) - select all panes
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::A) {
                should_select_all = true;
                consumed = true;
                return;
            }

            // n - clear all selections (select none)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::N) {
                should_clear_selection = true;
                consumed = true;
                return;
            }

            // j or down arrow - move cursor down
            if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                new_cursor_id =
                    self.visual_multi_navigate(cursor_tile_id, NavDirection::Down, &pane_ids);
                consumed = true;
                return;
            }

            // k or up arrow - move cursor up
            if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                new_cursor_id =
                    self.visual_multi_navigate(cursor_tile_id, NavDirection::Up, &pane_ids);
                consumed = true;
                return;
            }

            // h or left arrow - move cursor left
            if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
            {
                new_cursor_id =
                    self.visual_multi_navigate(cursor_tile_id, NavDirection::Left, &pane_ids);
                consumed = true;
                return;
            }

            // l or right arrow - move cursor right
            if input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
            {
                new_cursor_id =
                    self.visual_multi_navigate(cursor_tile_id, NavDirection::Right, &pane_ids);
                consumed = true;
            }
        });

        // Handle share selected panes (yy in visual-multi)
        if should_share_selected {
            if let Some(state) = &self.visual_multi_state {
                // Collect selected tile IDs and convert to sorted pane indices
                let mut indices: Vec<usize> = state
                    .selected_tile_ids
                    .iter()
                    .filter_map(|&tile_id| self.get_pane_index(tile_id))
                    .collect();
                indices.sort_unstable();

                if !indices.is_empty() {
                    // Trigger yank flash on each selected pane
                    for &tile_id in &state.selected_tile_ids {
                        self.behavior.trigger_yank_flash(tile_id);
                    }
                    // Exit visual-multi mode
                    self.exit_visual_multi_mode();
                    self.multi_buffer_state.reset();
                    ctx.request_repaint();
                    return Some(WorkspaceAction::ShareSelectedPanes(indices));
                }
            }
        }

        // Apply actions
        if should_exit {
            self.exit_visual_multi_mode();
            self.multi_buffer_state.reset();
        } else if should_enter_agent_mode {
            // Enter agent mode with selected panes as context
            // enter_agent_mode() will transfer the visual selection
            self.enter_agent_mode();
        } else if should_close_selected {
            self.close_selected_panes();
        } else if should_refresh_selected {
            self.refresh_selected_panes();
        } else if should_open_multi_edit {
            self.open_multi_edit_for_selected();
        } else if should_toggle_selection {
            if let (Some(state), Some(tile_id)) = (self.visual_multi_state.as_mut(), cursor_tile_id)
            {
                state.toggle_selection(tile_id);
            }
        } else if should_select_all {
            if let Some(state) = self.visual_multi_state.as_mut() {
                state.select_all(&pane_ids);
            }
        } else if should_clear_selection {
            if let Some(state) = self.visual_multi_state.as_mut() {
                state.clear_selection();
            }
        } else if let Some(tile_id) = new_cursor_id {
            // Move cursor to the new pane and select it (visual-line style)
            if let Some(state) = self.visual_multi_state.as_mut() {
                state.set_cursor(tile_id);
                // Auto-select the pane when navigating to it
                state.selected_tile_ids.insert(tile_id);
            }
            // Also update the behavior's focused tile to show the focus border
            self.behavior.set_focused_tile(Some(tile_id));
            self.activate_tile(tile_id);
            self.scroll_to_focused_tile(ctx);
        }

        if consumed {
            ctx.request_repaint();
            log::debug!(
                "Visual-multi mode: cursor is now {:?}, {} selected, IDs: {:?}",
                self.visual_multi_state
                    .as_ref()
                    .and_then(|s| s.cursor_tile_id),
                self.visual_multi_selection_count(),
                self.visual_multi_state.as_ref().map(|s| s
                    .selected_tile_ids
                    .iter()
                    .copied()
                    .collect::<Vec<_>>())
            );
        }

        None
    }

    // =========================================================================
    // Navigation Helpers
    // =========================================================================

    /// Get the pane index for a given tile ID (0-indexed position in the pane list).
    pub(super) fn get_pane_index(&self, tile_id: TileId) -> Option<usize> {
        self.get_pane_tile_ids()
            .iter()
            .position(|&id| id == tile_id)
    }

    /// Check if any buffer in the viewport is currently in insert mode.
    pub(super) fn is_any_buffer_in_insert_mode(&self) -> bool {
        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                // Check QueryPane
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    if query_pane.buffer_mode() == BufferMode::Insert {
                        return true;
                    }
                }
                // Check Buffer
                if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                    if buffer.mode() == BufferMode::Insert {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Find sibling tile in a given direction, respecting container layout.
    pub(super) fn find_sibling_in_direction(
        &self,
        current_id: TileId,
        direction: NavDirection,
    ) -> Option<TileId> {
        // Find the parent container of the current tile
        if let Some(root_id) = self.viewport_tree.root() {
            return self.find_sibling_recursive(root_id, current_id, direction);
        }
        None
    }

    /// Navigate in visual-multi mode using tree-based sibling navigation.
    fn visual_multi_navigate(
        &self,
        cursor_tile_id: Option<TileId>,
        direction: NavDirection,
        _pane_ids: &[TileId],
    ) -> Option<TileId> {
        if let Some(current_id) = cursor_tile_id {
            self.find_sibling_in_direction(current_id, direction)
        } else {
            _pane_ids.first().copied()
        }
    }

    /// Recursively search for a sibling in the given direction.
    fn find_sibling_recursive(
        &self,
        container_id: TileId,
        target_id: TileId,
        direction: NavDirection,
    ) -> Option<TileId> {
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            let children: Vec<TileId> = container.children().copied().collect();

            // Check if target is a direct child
            if let Some(idx) = children.iter().position(|&id| id == target_id) {
                // Determine if direction matches container orientation
                let container_kind = container.kind();
                let container_is_horizontal = matches!(
                    container_kind,
                    egui_tiles::ContainerKind::Tabs
                        | egui_tiles::ContainerKind::Horizontal
                        | egui_tiles::ContainerKind::Grid
                );
                let container_is_vertical =
                    matches!(container_kind, egui_tiles::ContainerKind::Vertical);

                let nav_is_horizontal =
                    matches!(direction, NavDirection::Left | NavDirection::Right);
                let nav_is_vertical = matches!(direction, NavDirection::Up | NavDirection::Down);

                // Navigate within this container if orientation matches
                if (container_is_horizontal && nav_is_horizontal)
                    || (container_is_vertical && nav_is_vertical)
                {
                    let next_idx = match direction {
                        NavDirection::Left | NavDirection::Up => {
                            if idx > 0 {
                                Some(idx - 1)
                            } else {
                                None
                            }
                        }
                        NavDirection::Right | NavDirection::Down => {
                            if idx + 1 < children.len() {
                                Some(idx + 1)
                            } else {
                                None
                            }
                        }
                    };

                    if let Some(next_idx) = next_idx {
                        // Get the target tile (might be a container, so get first/last pane)
                        let next_tile_id = children[next_idx];
                        return Some(self.get_edge_pane(next_tile_id, direction));
                    }
                }
                // Target is direct child but direction doesn't match container orientation
                // No sibling in this direction at this level
                return None;
            }

            // Check if target is in a nested container (target is NOT a direct child)
            for &child_id in &children {
                if child_id != target_id && self.contains_tile(child_id, target_id) {
                    // First try to find sibling within the nested container
                    if let Some(sibling) =
                        self.find_sibling_recursive(child_id, target_id, direction)
                    {
                        return Some(sibling);
                    }
                    // If not found in nested container, try to find sibling at this level
                    // by treating the nested container as the target
                    return self.find_sibling_recursive(container_id, child_id, direction);
                }
            }
        }
        None
    }

    /// Check if a container (recursively) contains a specific tile.
    fn contains_tile(&self, container_id: TileId, target_id: TileId) -> bool {
        if container_id == target_id {
            return true;
        }
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            for child_id in container.children() {
                if self.contains_tile(*child_id, target_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Get the first or last pane within a tile (handles nested containers).
    fn get_edge_pane(&self, tile_id: TileId, direction: NavDirection) -> TileId {
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(tile_id) {
            let children: Vec<TileId> = container.children().copied().collect();
            if !children.is_empty() {
                // When navigating right/down, get the first child; when left/up, get the last
                let edge_child = match direction {
                    NavDirection::Right | NavDirection::Down => children[0],
                    NavDirection::Left | NavDirection::Up => children[children.len() - 1],
                };
                return self.get_edge_pane(edge_child, direction);
            }
        }
        // It's a pane or empty container
        tile_id
    }

    // =========================================================================
    // Visual Multi-Select Mode Helpers
    // =========================================================================

    /// Enter visual-multi mode starting from the given pane.
    pub(super) fn enter_visual_multi_mode(&mut self, starting_tile_id: TileId) {
        use super::VisualMultiState;

        let pane_ids = self.get_pane_tile_ids();

        // Validate that the starting tile exists in the current pane list
        // (it might be stale after a :split operation)
        let valid_starting_tile = if pane_ids.contains(&starting_tile_id) {
            starting_tile_id
        } else {
            // Fall back to first pane if the starting tile is invalid
            log::debug!(
                "Starting tile {starting_tile_id:?} not found in panes, falling back to first pane"
            );
            match pane_ids.first() {
                Some(&first) => first,
                None => {
                    log::debug!("No panes available for visual-multi mode");
                    return;
                }
            }
        };

        log::debug!("Entering visual-multi mode with tile {valid_starting_tile:?}");
        self.visual_multi_state = Some(VisualMultiState::new(valid_starting_tile));
        // Sync the cursor to the behavior so the focus border is drawn
        self.behavior.set_focused_tile(Some(valid_starting_tile));
    }

    /// Exit visual-multi mode.
    pub(super) fn exit_visual_multi_mode(&mut self) {
        log::debug!("Exiting visual-multi mode");
        self.visual_multi_state = None;
    }

    /// Close all selected panes in visual-multi mode.
    pub(super) fn close_selected_panes(&mut self) {
        let selected_ids: Vec<TileId> = self
            .visual_multi_state
            .as_ref()
            .map(|s| s.selected_tile_ids.iter().copied().collect())
            .unwrap_or_default();

        if selected_ids.is_empty() {
            log::debug!("No panes selected to close");
            return;
        }

        log::debug!(
            "Closing {} selected panes: {:?}",
            selected_ids.len(),
            selected_ids
        );

        // Close each selected tile
        for tile_id in selected_ids {
            self.close_tile(tile_id);
        }

        // Exit visual-multi mode after closing
        self.exit_visual_multi_mode();
        self.multi_buffer_state.reset();
    }

    /// Refresh all selected panes in visual-multi mode.
    pub(super) fn refresh_selected_panes(&mut self) {
        let selected_ids: Vec<TileId> = self
            .visual_multi_state
            .as_ref()
            .map(|s| s.selected_tile_ids.iter().copied().collect())
            .unwrap_or_default();

        if selected_ids.is_empty() {
            log::debug!("No panes selected to refresh");
            return;
        }

        log::debug!(
            "Refreshing {} selected panes: {:?}",
            selected_ids.len(),
            selected_ids
        );

        // Refresh each selected pane
        for tile_id in selected_ids {
            if let Some(egui_tiles::Tile::Pane(pane)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(query_pane) = pane.as_any_mut().downcast_mut::<QueryPane>() {
                    query_pane.refresh();
                }
            }
        }
    }

    /// Open the multi-edit overlay for all selected panes in visual-multi mode.
    pub(super) fn open_multi_edit_for_selected(&mut self) {
        let selected_ids: Vec<TileId> = self
            .visual_multi_state
            .as_ref()
            .map(|s| s.selected_tile_ids.iter().copied().collect())
            .unwrap_or_default();

        log::debug!(
            "open_multi_edit_for_selected: {} tile IDs selected: {:?}",
            selected_ids.len(),
            selected_ids
        );

        if selected_ids.is_empty() {
            log::debug!("No panes selected for multi-edit");
            return;
        }

        // Collect excerpts from selected panes
        let mut excerpts = Vec::new();
        for tile_id in &selected_ids {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(*tile_id)
            {
                // Try to get query content from QueryPane
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    log::debug!(
                        "  tile {:?} -> QueryPane '{}' with query '{}'",
                        tile_id,
                        query_pane.name(),
                        query_pane.query()
                    );
                    excerpts.push(EditExcerpt::new(
                        query_pane.id(),
                        query_pane.name().to_string(),
                        query_pane.query().to_string(),
                    ));
                }
                // Try to get content from Buffer
                else if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                    log::debug!(
                        "  tile {:?} -> Buffer '{}' with content '{}'",
                        tile_id,
                        buffer.name(),
                        buffer.content()
                    );
                    excerpts.push(EditExcerpt::new(
                        buffer.id(),
                        buffer.name().to_string(),
                        buffer.content().to_string(),
                    ));
                } else {
                    log::debug!(
                        "  tile {tile_id:?} -> Unknown component type (not QueryPane or Buffer)"
                    );
                }
            } else {
                log::debug!("  tile {tile_id:?} -> Not found or not a Pane");
            }
        }

        if excerpts.is_empty() {
            log::debug!("No query panes found in selection");
            return;
        }

        log::debug!("Opening multi-edit with {} excerpts", excerpts.len());
        self.multi_edit_overlay.open(excerpts);

        // Exit visual-multi mode when opening the overlay
        self.exit_visual_multi_mode();
    }
}
