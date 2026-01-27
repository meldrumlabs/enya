//! Interactive tutorial overlay component for guiding new users.
//!
//! Inspired by vim's `:tutor` command, this component provides a step-by-step
//! walkthrough of the editor's features in a modal overlay.

use egui::{Key, RichText};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, render_key_badge_large};

/// Actions that can be returned from the tutorial overlay
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum TutorialAction {
    /// No action needed
    #[default]
    None,
    /// User requested to close the tutorial
    Close,
    /// User requested to open the style picker
    OpenStylePicker,
}

/// A single step in the tutorial
#[derive(Clone)]
pub struct TutorialStep {
    /// Title for this step
    pub title: &'static str,
    /// Main instruction text
    pub instruction: &'static str,
    /// The key/command to demonstrate (shown as badge)
    pub key_hint: &'static str,
    /// Optional additional tip
    pub tip: Option<&'static str>,
    /// Icon for visual interest
    pub icon: &'static str,
    /// Category for grouping in the step picker
    pub category: StepCategory,
}

/// Categories for grouping tutorial steps
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepCategory {
    Welcome,
    Navigation,
    Editing,
    Time,
    Git,
    Workspace,
    Advanced,
    Help,
}

impl StepCategory {
    fn label(&self) -> &'static str {
        match self {
            Self::Welcome => "WELCOME",
            Self::Navigation => "NAVIGATION",
            Self::Editing => "EDITING",
            Self::Time => "TIME",
            Self::Git => "GIT",
            Self::Workspace => "WORKSPACE",
            Self::Advanced => "ADVANCED",
            Self::Help => "HELP",
        }
    }
}

/// An interactive tutorial overlay that guides users through editor features
pub struct TutorialOverlay {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip input on the first frame after opening
    just_opened: bool,
    /// Current theme (can be Custom with plugin colors)
    theme: AppTheme,
    /// Current step index
    current_step: usize,
    /// Tutorial steps
    steps: Vec<TutorialStep>,
    /// Whether the step picker is showing
    show_step_picker: bool,
    /// Selected index in the step picker
    picker_selected: usize,
}

impl Default for TutorialOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl TutorialOverlay {
    pub fn new() -> Self {
        let is_wasm = cfg!(target_arch = "wasm32");
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            current_step: 0,
            steps: Self::build_tutorial_steps(is_wasm),
            show_step_picker: false,
            picker_selected: 0,
        }
    }

    /// Build the default tutorial steps
    fn build_tutorial_steps(is_wasm: bool) -> Vec<TutorialStep> {
        let mut steps = vec![
            // === Welcome ===
            TutorialStep {
                title: "Welcome to Enya",
                instruction: "This tutorial will guide you through the core features of the editor. Navigate through the steps to learn vim-style keybindings and commands.",
                key_hint: "→ / Enter",
                tip: Some("Press → or Enter to continue, ← to go back, g for overview"),
                icon: semantic_icons::status::INFO,
                category: StepCategory::Welcome,
            },
            TutorialStep {
                title: "Colorscheme",
                instruction: "Select your preferred theme and font.",
                key_hint: "s",
                tip: Some("Press s to open the style picker, or → to skip"),
                icon: semantic_icons::nav::SETTINGS,
                category: StepCategory::Welcome,
            },
            // === Pane Management (grouped) ===
            TutorialStep {
                title: "Navigate Between Panes",
                instruction: "Use vim-style navigation keys to move focus between panes in your workspace.",
                key_hint: "h j k l",
                tip: Some("h=left, j=down, k=up, l=right (or use arrow keys)"),
                icon: semantic_icons::nav::COMPASS,
                category: StepCategory::Navigation,
            },
            TutorialStep {
                title: "Move Panes",
                instruction: "Rearrange your layout by moving panes in any direction.",
                key_hint: "Ctrl+W h/j/k/l",
                tip: Some("Ctrl+W then h/j/k/l to swap pane position"),
                icon: semantic_icons::nav::EXPAND_ALL,
                category: StepCategory::Navigation,
            },
            TutorialStep {
                title: "Merge Into Tabs",
                instruction: "Combine panes into a tabbed group for a cleaner workspace. Merge any pane into its neighbor as a tab — great for comparing related metrics without splitting screen space.",
                key_hint: "Ctrl+W t h/j/k/l",
                tip: Some("Ctrl+W t then h/j/k/l=merge focused pane as tab in that direction"),
                icon: semantic_icons::nav::EXPAND_ALL,
                category: StepCategory::Navigation,
            },
            TutorialStep {
                title: "Split Panes",
                instruction: "Create new panes by splitting horizontally or vertically. Great for comparing metrics side by side.",
                key_hint: ":split / :vsplit",
                tip: Some("Shortcuts: :sp, :vs, :hsplit"),
                icon: semantic_icons::nav::PANES,
                category: StepCategory::Navigation,
            },
            TutorialStep {
                title: "Floating Panes",
                instruction: "Detach any pane into a floating window for side-by-side investigation. Float panes hover above the layout.",
                key_hint: "gf",
                tip: Some("gf=float pane, :dock=return all to layout, :float arrange=grid"),
                icon: semantic_icons::nav::PANES,
                category: StepCategory::Navigation,
            },
            TutorialStep {
                title: "Visual Multi-Select",
                instruction: "Select multiple panes at once to perform batch operations. Press 'e' to multi-edit queries across all selected panes (e.g., change env=\"prod\" to env=\"staging\").",
                key_hint: "Ctrl+V, then e",
                tip: Some("h/j/k/l to extend selection, e to multi-edit, x to close"),
                icon: semantic_icons::mode::VISUAL,
                category: StepCategory::Navigation,
            },
            TutorialStep {
                title: "Filter Panes",
                instruction: "Quickly filter visible panes by query content. Non-matching panes are dimmed so you can focus on what matters.",
                key_hint: "/",
                tip: Some("Type to filter, Enter to apply, Esc twice to clear"),
                icon: semantic_icons::action::SEARCH,
                category: StepCategory::Navigation,
            },
            // === Editing & Commands ===
            TutorialStep {
                title: "Edit a Query",
                instruction: "Press 'e' to open the query editor for the focused pane. Write PromQL queries with syntax highlighting and autocompletion.",
                key_hint: "e",
                tip: Some("Press Enter to save, Esc to cancel"),
                icon: semantic_icons::action::EDIT,
                category: StepCategory::Editing,
            },
            TutorialStep {
                title: "Command Palette",
                instruction: "Access all commands through the command palette. Type commands like :split, :style, :write, and more.",
                key_hint: ":",
                tip: Some("Start typing to filter commands"),
                icon: semantic_icons::action::SEARCH,
                category: StepCategory::Editing,
            },
            TutorialStep {
                title: "Cycle Visualization",
                instruction: "Instantly switch how the focused pane displays data — line chart, bar chart, table, and more. One key to flip through every visualization type.",
                key_hint: "cv",
                tip: Some("cv=cycle forward through visualization types"),
                icon: semantic_icons::action::CHART,
                category: StepCategory::Editing,
            },
            // === Time Navigation ===
            TutorialStep {
                title: "Time Range Controls",
                instruction: "Navigate through time with vim-style motions. Jump to start/end of data, zoom in/out, or reset to default range.",
                key_hint: "gg / gG / , / . / 0",
                tip: Some("gg=jump to start, gG=jump to end, ,=zoom out, .=zoom in, 0=reset"),
                icon: semantic_icons::time::CLOCK,
                category: StepCategory::Time,
            },
            TutorialStep {
                title: "Quick Time Presets",
                instruction: "Instantly set common time ranges with two-key shortcuts. Perfect for quick investigations.",
                key_hint: "t1 / th / td",
                tip: Some("t5=5m, t1=15m, t3=30m, th=1h, t6=6h, td=24h, tw=7d"),
                icon: semantic_icons::time::CLOCK,
                category: StepCategory::Time,
            },
            // === Git Integration ===
            TutorialStep {
                title: "Commit Annotations",
                instruction: "Overlay git commits on your charts to correlate code changes with metric behavior. See exactly when deployments happened.",
                key_hint: "gc",
                tip: Some("gc=toggle commits, ]c=next commit, [c=prev commit"),
                icon: semantic_icons::action::CHART,
                category: StepCategory::Git,
            },
            // === Search ===
            TutorialStep {
                title: "Find Anything",
                instruction: "Use the unified fuzzy finder to search metrics, workspaces, and more. Browse with live preview.",
                key_hint: "Space+f",
                tip: Some("Space+f=find anything, Space+w=workspaces, Space+h=home"),
                icon: semantic_icons::action::CHART,
                category: StepCategory::Workspace,
            },
            // === Workspace & View ===
            TutorialStep {
                title: "Workspace Undo",
                instruction: "Made a mistake? Undo workspace operations like closing, floating, or docking panes. Up to 50 actions remembered.",
                key_hint: "u",
                tip: Some("Works for close, float, and dock operations"),
                icon: semantic_icons::action::EDIT,
                category: StepCategory::Workspace,
            },
            TutorialStep {
                title: "Fullscreen & Zen Mode",
                instruction: "Focus on a single pane in fullscreen, or hide all UI elements with zen mode for distraction-free viewing.",
                key_hint: "f / z",
                tip: Some("f=fullscreen pane, z=zen mode"),
                icon: semantic_icons::mode::VIEW,
                category: StepCategory::Workspace,
            },
            TutorialStep {
                title: "Share",
                instruction: "Share your work in seconds. Yank a shareable URL, capture a screenshot for Slack, or share the full workspace.",
                key_hint: "yy / :ss / :share",
                tip: Some("yy=copy URL, :screenshot=capture PNG, :share=share workspace"),
                icon: semantic_icons::action::COPY,
                category: StepCategory::Workspace,
            },
        ];

        // Native-only features
        if !is_wasm {
            steps.push(TutorialStep {
                title: "Ask the AI Agent",
                instruction: "Get help from the AI assistant. Ask questions about your metrics, request dashboard changes, or investigate anomalies.",
                key_hint: "aa",
                tip: Some("aa=quick ask, Space+a=panel, aw/ae/ay=what/explain/why"),
                icon: semantic_icons::action::BRAIN,
                category: StepCategory::Advanced,
            });
            steps.push(TutorialStep {
                title: "Terminal & SQL",
                instruction: "Open an embedded terminal for shell commands or a SQL pane for querying data directly. Great for incident investigation.",
                key_hint: ":terminal / :sql",
                tip: Some(":term for short, :sync to refresh git index"),
                icon: semantic_icons::action::EDIT,
                category: StepCategory::Advanced,
            });
        }

        // Final step (always last)
        steps.push(TutorialStep {
            title: "Get Help Anytime",
            instruction: "Press ? to see all available keybindings at a glance. You can also run :tutorial to restart this guide.",
            key_hint: "?",
            tip: Some("You're ready to explore! Happy monitoring."),
            icon: semantic_icons::keyboard::KEYBOARD,
            category: StepCategory::Help,
        });

        steps
    }

    /// Set the theme (supports Custom variant with plugin colors)
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay (resumes from last step)
    pub fn open(&mut self) {
        self.is_open = true;
        self.just_opened = true;
        // Don't reset current_step - let users resume where they left off
    }

    /// Open the overlay and start from the beginning
    pub fn open_from_start(&mut self) {
        self.is_open = true;
        self.just_opened = true;
        self.current_step = 0;
    }

    /// Close the overlay (keeps current step for resuming)
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Reset to the first step
    pub fn reset(&mut self) {
        self.current_step = 0;
    }

    /// Check if the overlay is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Go to the next step, or close if at the end
    fn next_step(&mut self) {
        if self.current_step + 1 < self.steps.len() {
            self.current_step += 1;
        } else {
            self.close();
        }
    }

    /// Go to the previous step
    fn prev_step(&mut self) {
        self.current_step = self.current_step.saturating_sub(1);
    }

    /// Jump to a specific step (0-indexed)
    fn go_to_step(&mut self, step: usize) {
        if step < self.steps.len() {
            self.current_step = step;
        }
    }

    /// The index of the "Customize Your Editor" step
    const CUSTOMIZE_STEP_INDEX: usize = 1;

    /// Show the overlay. Returns an action if one was requested.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> TutorialAction {
        if !self.is_open {
            return TutorialAction::None;
        }

        let mut action = TutorialAction::None;

        // Skip input handling on the first frame after opening
        if self.just_opened {
            self.just_opened = false;
        } else if self.show_step_picker {
            // Handle step picker input
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape)
                    || i.consume_key(egui::Modifiers::NONE, Key::G)
                {
                    self.show_step_picker = false;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(egui::Modifiers::NONE, Key::J)
                {
                    self.picker_selected = (self.picker_selected + 1).min(self.steps.len() - 1);
                }
                if i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                    || i.consume_key(egui::Modifiers::NONE, Key::K)
                {
                    self.picker_selected = self.picker_selected.saturating_sub(1);
                }
                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    self.current_step = self.picker_selected;
                    self.show_step_picker = false;
                }
                // Number keys for direct jump in picker
                for (key, step) in [
                    (Key::Num1, 0),
                    (Key::Num2, 1),
                    (Key::Num3, 2),
                    (Key::Num4, 3),
                    (Key::Num5, 4),
                    (Key::Num6, 5),
                    (Key::Num7, 6),
                    (Key::Num8, 7),
                    (Key::Num9, 8),
                ] {
                    if i.consume_key(egui::Modifiers::NONE, key) && step < self.steps.len() {
                        self.current_step = step;
                        self.show_step_picker = false;
                    }
                }
            });
        } else {
            // Handle main tutorial input
            let current_step = self.current_step;
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    action = TutorialAction::Close;
                }
                // 's' to open style picker on the customize step
                if current_step == Self::CUSTOMIZE_STEP_INDEX
                    && i.consume_key(egui::Modifiers::NONE, Key::S)
                {
                    action = TutorialAction::OpenStylePicker;
                }
                // 'g' to open step picker
                if i.consume_key(egui::Modifiers::NONE, Key::G) {
                    self.picker_selected = self.current_step;
                    self.show_step_picker = true;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(egui::Modifiers::NONE, Key::Enter)
                    || i.consume_key(egui::Modifiers::NONE, Key::L)
                {
                    self.next_step();
                }
                if i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft)
                    || i.consume_key(egui::Modifiers::NONE, Key::Backspace)
                    || i.consume_key(egui::Modifiers::NONE, Key::H)
                {
                    self.prev_step();
                }
                // Number keys 1-9 to jump to steps
                for (key, step) in [
                    (Key::Num1, 0),
                    (Key::Num2, 1),
                    (Key::Num3, 2),
                    (Key::Num4, 3),
                    (Key::Num5, 4),
                    (Key::Num6, 5),
                    (Key::Num7, 6),
                    (Key::Num8, 7),
                    (Key::Num9, 8),
                ] {
                    if i.consume_key(egui::Modifiers::NONE, key) {
                        self.go_to_step(step);
                    }
                }
                // 't' to try/practice (close overlay to practice, resume with :tutorial)
                if i.consume_key(egui::Modifiers::NONE, Key::T) {
                    action = TutorialAction::Close;
                }
            });
        }

        // Calculate popup dimensions - narrower but taller for readability
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.5).clamp(500.0, 650.0);

        // Extract colors from theme (Custom variant handles plugin colors internally)
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let separator_color = self.theme.border_subtle();
        let muted_text = self.theme.text_primary().gamma_multiply(0.6);
        let accent_color = self.theme.accent_primary();
        let key_bg = self.theme.bg_elevated();
        let tip_color = self.theme.accent_hover();
        let text_col = self.theme.text_primary();

        egui::Area::new(egui::Id::new("tutorial_overlay_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Get current step
                    let step = &self.steps[self.current_step];

                    // Header section with step indicator
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(RichText::new(step.icon).color(accent_color).size(28.0));
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "Step {} of {}",
                                    self.current_step + 1,
                                    self.steps.len()
                                ))
                                .color(muted_text)
                                .size(typography::SM),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(step.title)
                                    .color(text_col)
                                    .size(typography::HEADING)
                                    .strong(),
                            );
                        });
                    });
                    ui.add_space(16.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(24.0);

                    // Instruction text
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.vertical(|ui| {
                            ui.set_width(popup_width - 48.0);
                            ui.label(
                                RichText::new(step.instruction)
                                    .color(text_col)
                                    .size(typography::LG),
                            );
                        });
                    });
                    ui.add_space(28.0);

                    // Key hint badge (centered)
                    ui.horizontal(|ui| {
                        let available_width = ui.available_width();
                        // Measure actual text width using font metrics
                        let font_id = typography::monospace(typography::MD);
                        let text_width = ui.fonts_mut(|f| {
                            f.layout_no_wrap(
                                step.key_hint.to_string(),
                                font_id,
                                egui::Color32::WHITE,
                            )
                            .size()
                            .x
                        });
                        // Badge has 10px horizontal padding on each side + 1px stroke on each side
                        let badge_width = text_width + 22.0;
                        ui.add_space((available_width - badge_width) / 2.0);
                        render_key_badge_large(ui, step.key_hint, key_bg, accent_color);
                    });
                    ui.add_space(24.0);

                    // Tip section
                    if let Some(tip) = step.tip {
                        ui.horizontal(|ui| {
                            ui.add_space(24.0);
                            ui.label(
                                RichText::new(semantic_icons::diagnostic::HINT)
                                    .color(tip_color)
                                    .size(typography::MD),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(tip)
                                    .color(tip_color)
                                    .size(typography::MD)
                                    .italics(),
                            );
                        });
                        ui.add_space(20.0);
                    }

                    // Progress bar (centered)
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let available_width = ui.available_width();
                        let bar_width = 200.0_f32.min(available_width - 48.0);
                        let bar_height = 4.0;
                        let progress = (self.current_step + 1) as f32 / self.steps.len() as f32;

                        // Center the bar
                        ui.add_space((available_width - bar_width) / 2.0);

                        // Draw background track
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_width, bar_height),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            rect,
                            bar_height / 2.0,
                            muted_text.gamma_multiply(0.3),
                        );

                        // Draw filled portion
                        let filled_rect = egui::Rect::from_min_size(
                            rect.min,
                            egui::vec2(bar_width * progress, bar_height),
                        );
                        ui.painter()
                            .rect_filled(filled_rect, bar_height / 2.0, accent_color);
                    });

                    // Step counter text
                    ui.add_space(6.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} of {}",
                                self.current_step + 1,
                                self.steps.len()
                            ))
                            .color(muted_text)
                            .size(typography::SM),
                        );
                    });
                    ui.add_space(12.0);

                    // Separator above footer
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(12.0);

                    // Footer with navigation hints
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);

                        // Previous
                        if self.current_step > 0 {
                            render_key_badge_large(ui, "← h", key_bg, text_col);
                            ui.label(
                                RichText::new(" prev")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            ui.add_space(16.0);
                        }

                        // Next/Finish
                        let next_label = if self.current_step + 1 >= self.steps.len() {
                            "finish"
                        } else {
                            "next"
                        };
                        render_key_badge_large(ui, "→ l", key_bg, text_col);
                        ui.label(
                            RichText::new(format!(" {next_label}"))
                                .color(muted_text)
                                .size(typography::SM),
                        );

                        // Push hints to the right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(20.0);

                            // Close hint
                            ui.label(
                                RichText::new("close ")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            render_key_badge_large(ui, "Esc", key_bg, text_col);

                            ui.add_space(24.0);

                            // Try it hint
                            ui.label(
                                RichText::new("practice ")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            render_key_badge_large(ui, "t", key_bg, text_col);
                        });
                    });

                    // Resume hint at the bottom
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Press g for step overview, t to practice")
                                .color(muted_text.gamma_multiply(0.7))
                                .size(typography::XS)
                                .italics(),
                        );
                    });
                    ui.add_space(12.0);
                });
            });

        // Render step picker overlay if open
        if self.show_step_picker {
            self.render_step_picker(ctx);
        }

        // Handle close action
        if action == TutorialAction::Close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
        }

        action
    }

    /// Render the step picker overlay
    fn render_step_picker(&self, ctx: &egui::Context) {
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.6).clamp(550.0, 750.0);
        let popup_height = (screen_rect.height() * 0.7).clamp(400.0, 600.0);

        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let muted_text = self.theme.text_primary().gamma_multiply(0.6);
        let accent_color = self.theme.accent_primary();
        let text_col = self.theme.text_primary();
        let separator_color = self.theme.border_subtle();
        let key_bg = self.theme.bg_elevated();
        let hover_bg = self.theme.bg_hover();

        egui::Area::new(egui::Id::new("tutorial_step_picker"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Tooltip) // Above the tutorial overlay
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);
                    ui.set_max_height(popup_height);

                    // Header
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("Jump to Step")
                                .color(text_col)
                                .size(typography::HEADING)
                                .strong(),
                        );
                    });
                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(16.0);

                    // Scrollable step list organized by category
                    egui::ScrollArea::vertical()
                        .max_height(popup_height - 140.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(20.0);
                                ui.vertical(|ui| {
                                    // Render in two columns
                                    // Distribute categories to balance content:
                                    // Left: Welcome(1) + Navigation(6) + Editing(2) = 9 steps
                                    // Right: Time(2) + Git(1) + Workspace(4) + Advanced(0-2) + Help(1) = 8-10 steps
                                    ui.columns(2, |columns| {
                                        let left_categories = [
                                            StepCategory::Welcome,
                                            StepCategory::Navigation,
                                            StepCategory::Editing,
                                        ];
                                        let right_categories = [
                                            StepCategory::Time,
                                            StepCategory::Git,
                                            StepCategory::Workspace,
                                            StepCategory::Advanced,
                                            StepCategory::Help,
                                        ];

                                        // Left column
                                        for category in left_categories {
                                            Self::render_category_section(
                                                &mut columns[0],
                                                &self.steps,
                                                category,
                                                self.picker_selected,
                                                self.current_step,
                                                accent_color,
                                                muted_text,
                                                text_col,
                                                hover_bg,
                                            );
                                        }

                                        // Right column
                                        for category in right_categories {
                                            Self::render_category_section(
                                                &mut columns[1],
                                                &self.steps,
                                                category,
                                                self.picker_selected,
                                                self.current_step,
                                                accent_color,
                                                muted_text,
                                                text_col,
                                                hover_bg,
                                            );
                                        }
                                    });
                                });
                            });
                        });

                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(12.0);

                    // Footer with hints
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        render_key_badge_large(ui, "j/k", key_bg, text_col);
                        ui.label(
                            RichText::new(" navigate")
                                .color(muted_text)
                                .size(typography::SM),
                        );
                        ui.add_space(16.0);
                        render_key_badge_large(ui, "Enter", key_bg, text_col);
                        ui.label(
                            RichText::new(" select")
                                .color(muted_text)
                                .size(typography::SM),
                        );
                        ui.add_space(16.0);
                        render_key_badge_large(ui, "1-9", key_bg, text_col);
                        ui.label(
                            RichText::new(" jump")
                                .color(muted_text)
                                .size(typography::SM),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new("close ")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            render_key_badge_large(ui, "Esc", key_bg, text_col);
                        });
                    });
                    ui.add_space(16.0);
                });
            });
    }

    /// Render a category section in the step picker
    #[allow(clippy::too_many_arguments)]
    fn render_category_section(
        ui: &mut egui::Ui,
        steps: &[TutorialStep],
        category: StepCategory,
        picker_selected: usize,
        current_step: usize,
        accent_color: egui::Color32,
        muted_text: egui::Color32,
        text_col: egui::Color32,
        hover_bg: egui::Color32,
    ) {
        // Find steps in this category
        let category_steps: Vec<(usize, &TutorialStep)> = steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.category == category)
            .collect();

        if category_steps.is_empty() {
            return;
        }

        // Clip content to available width to prevent overflow
        let max_width = ui.available_width() - 10.0;

        // Category header
        ui.add_space(8.0);
        ui.label(
            RichText::new(category.label())
                .color(muted_text)
                .size(typography::XS)
                .strong(),
        );
        ui.add_space(4.0);

        // Steps in this category
        for (idx, step) in category_steps {
            let is_selected = idx == picker_selected;
            let is_current = idx == current_step;

            // Truncate long titles to fit in column
            let title = if step.title.len() > 22 {
                format!("{}…", &step.title[..21])
            } else {
                step.title.to_string()
            };
            let step_text = format!("{:>2}  {}", idx + 1, title);

            let text_color = if is_selected {
                accent_color
            } else if is_current {
                accent_color.gamma_multiply(0.7)
            } else {
                text_col
            };

            ui.horizontal(|ui| {
                ui.set_max_width(max_width);

                // Selection indicator
                if is_selected {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(3.0, typography::MD + 4.0),
                        ),
                        1.0,
                        accent_color,
                    );
                    ui.add_space(8.0);

                    // Background highlight for selected item
                    let text_rect = egui::Rect::from_min_size(
                        ui.cursor().min - egui::vec2(4.0, 2.0),
                        egui::vec2(max_width - 8.0, typography::MD + 8.0),
                    );
                    ui.painter().rect_filled(text_rect, 4.0, hover_bg);
                } else {
                    ui.add_space(11.0);
                }

                // Current step marker
                let marker = if is_current { " ←" } else { "" };

                ui.label(
                    RichText::new(format!("{step_text}{marker}"))
                        .color(text_color)
                        .size(typography::MD),
                );
            });
            ui.add_space(2.0);
        }
    }
}
