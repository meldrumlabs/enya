//! Interactive tutorial overlay component for guiding new users.
//!
//! Inspired by vim's `:tutor` command, this component provides a step-by-step
//! walkthrough of the editor's features in a modal overlay.

use egui::{Key, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, render_key_badge_large};

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
}

/// An interactive tutorial overlay that guides users through editor features
pub struct TutorialOverlay {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip input on the first frame after opening
    just_opened: bool,
    /// Current theme
    theme: AppTheme,
    /// Current step index
    current_step: usize,
    /// Tutorial steps
    steps: Vec<TutorialStep>,
}

impl Default for TutorialOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl TutorialOverlay {
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            current_step: 0,
            steps: Self::build_tutorial_steps(),
        }
    }

    /// Build the default tutorial steps
    fn build_tutorial_steps() -> Vec<TutorialStep> {
        vec![
            TutorialStep {
                title: "Welcome to Enya",
                instruction: "This tutorial will guide you through the core features of the editor. Navigate through the steps to learn vim-style keybindings and commands.",
                key_hint: "→ / Enter",
                tip: Some("Press → or Enter to continue, ← to go back"),
                icon: semantic_icons::status::INFO,
            },
            TutorialStep {
                title: "Navigate Between Panes",
                instruction: "Use vim-style navigation keys to move focus between panes in your workspace.",
                key_hint: "h j k l",
                tip: Some("h=left, j=down, k=up, l=right (or use arrow keys)"),
                icon: semantic_icons::nav::COMPASS,
            },
            TutorialStep {
                title: "Edit a Query",
                instruction: "Press 'e' to open the query editor for the focused pane. Write PromQL queries with syntax highlighting and autocompletion.",
                key_hint: "e",
                tip: Some("Press Enter to save, Esc to cancel"),
                icon: semantic_icons::action::EDIT,
            },
            TutorialStep {
                title: "Command Palette",
                instruction: "Access all commands through the command palette. Type commands like :split, :theme, :connect, and more.",
                key_hint: ":",
                tip: Some("Start typing to filter commands"),
                icon: semantic_icons::action::SEARCH,
            },
            TutorialStep {
                title: "Split Panes",
                instruction: "Create new panes by splitting horizontally or vertically. Great for comparing metrics side by side.",
                key_hint: ":split / :vsplit",
                tip: Some("Shortcuts: :sp, :vs, :hsplit"),
                icon: semantic_icons::nav::PANES,
            },
            TutorialStep {
                title: "Time Range Controls",
                instruction: "Navigate through time with vim-style motions. Jump to start/end of data, zoom in/out, or reset to default range.",
                key_hint: "gg / gG / , / . / 0",
                tip: Some("gg=jump to start, gG=jump to end, ,=zoom out, .=zoom in, 0=reset"),
                icon: semantic_icons::time::CLOCK,
            },
            TutorialStep {
                title: "Visual Multi-Select",
                instruction: "Select multiple panes at once to perform batch operations. Press 'e' to multi-edit queries across all selected panes (e.g., change env=\"prod\" to env=\"staging\").",
                key_hint: "Ctrl+V, then e",
                tip: Some("h/j/k/l to extend selection, e to multi-edit, x to close"),
                icon: semantic_icons::mode::VISUAL,
            },
            TutorialStep {
                title: "Filter Panes",
                instruction: "Quickly filter visible panes by query content. Non-matching panes are dimmed so you can focus on what matters.",
                key_hint: "/",
                tip: Some("Type to filter, Enter to apply, Esc twice to clear"),
                icon: semantic_icons::action::SEARCH,
            },
            TutorialStep {
                title: "Metrics Finder",
                instruction: "Quickly search and insert metrics using the fuzzy finder. Browse available metrics with live preview.",
                key_hint: "Space+m",
                tip: Some("Type to filter, Enter to select"),
                icon: semantic_icons::action::CHART,
            },
            TutorialStep {
                title: "Workspace Management",
                instruction: "Work with multiple workspaces using tabs. Save and load workspace configurations.",
                key_hint: "Shift+T/N/P/X",
                tip: Some("T=new tab, N=next, P=previous, X=close"),
                icon: semantic_icons::nav::TABS,
            },
            TutorialStep {
                title: "Fullscreen & Zen Mode",
                instruction: "Focus on a single pane in fullscreen, or hide all UI elements with zen mode for distraction-free viewing.",
                key_hint: "f / z",
                tip: Some("f=fullscreen pane, z=zen mode"),
                icon: semantic_icons::mode::VIEW,
            },
            TutorialStep {
                title: "Share Your Dashboard",
                instruction: "Copy a shareable URL for any pane or the entire workspace. Recipients can view your exact configuration.",
                key_hint: "yy",
                tip: Some("Yanks a URL to clipboard (vim-style yank)"),
                icon: semantic_icons::action::COPY,
            },
            TutorialStep {
                title: "Get Help Anytime",
                instruction: "Press ? to see all available keybindings at a glance. You can also run :tutorial to restart this guide.",
                key_hint: "?",
                tip: Some("You're ready to explore! Happy monitoring."),
                icon: semantic_icons::keyboard::KEYBOARD,
            },
        ]
    }

    /// Set the theme
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

    /// Show the overlay. Returns true if it was closed this frame.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        if !self.is_open {
            return false;
        }

        let mut should_close = false;

        // Skip input handling on the first frame after opening
        if self.just_opened {
            self.just_opened = false;
        } else {
            // Handle keyboard input
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    should_close = true;
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
                    should_close = true;
                }
            });
        }

        // Calculate popup dimensions - narrower but taller for readability
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.5).clamp(500.0, 650.0);

        egui::Area::new(egui::Id::new("tutorial_overlay_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = match self.theme {
                    AppTheme::Light => palette::light_border::SUBTLE,
                    AppTheme::Dark => palette::border::SUBTLE,
                };
                let muted_text = text_color(self.theme).gamma_multiply(0.6);
                let accent_color = match self.theme {
                    AppTheme::Light => palette::accent::LIGHT,
                    AppTheme::Dark => palette::accent::PRIMARY,
                };
                let key_bg = match self.theme {
                    AppTheme::Light => palette::light_bg::ELEVATED,
                    AppTheme::Dark => palette::bg::ELEVATED,
                };
                let tip_color = match self.theme {
                    AppTheme::Light => palette::accent::LIGHT,
                    AppTheme::Dark => palette::accent::HOVER,
                };

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
                                    .color(text_color(self.theme))
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
                                    .color(text_color(self.theme))
                                    .size(typography::LG),
                            );
                        });
                    });
                    ui.add_space(28.0);

                    // Key hint badge (centered)
                    ui.horizontal(|ui| {
                        let available_width = ui.available_width();
                        // Estimate badge width based on text
                        let badge_width = step.key_hint.len() as f32 * 10.0 + 24.0;
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

                    // Progress dots
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let dot_spacing = 8.0;
                        let dot_size = 6.0;
                        let total_width = self.steps.len() as f32 * dot_size
                            + (self.steps.len() - 1) as f32 * dot_spacing;
                        ui.add_space((ui.available_width() - total_width) / 2.0);

                        for i in 0..self.steps.len() {
                            let color = if i == self.current_step {
                                accent_color
                            } else if i < self.current_step {
                                muted_text
                            } else {
                                muted_text.gamma_multiply(0.4)
                            };

                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(dot_size, dot_size),
                                egui::Sense::hover(),
                            );
                            ui.painter()
                                .circle_filled(rect.center(), dot_size / 2.0, color);

                            if i < self.steps.len() - 1 {
                                ui.add_space(dot_spacing);
                            }
                        }
                    });
                    ui.add_space(16.0);

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
                            render_key_badge_large(ui, "← h", key_bg, text_color(self.theme));
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
                        render_key_badge_large(ui, "→ l", key_bg, text_color(self.theme));
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
                            render_key_badge_large(ui, "Esc", key_bg, text_color(self.theme));

                            ui.add_space(24.0);

                            // Try it hint
                            ui.label(
                                RichText::new("practice ")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            render_key_badge_large(ui, "t", key_bg, text_color(self.theme));
                        });
                    });

                    // Resume hint at the bottom
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let hint_width = ui.available_width();
                        ui.add_space((hint_width - 240.0) / 2.0); // Center the hint
                        ui.label(
                            RichText::new("Press t to practice, then :tutorial to resume")
                                .color(muted_text.gamma_multiply(0.7))
                                .size(typography::XS)
                                .italics(),
                        );
                    });
                    ui.add_space(12.0);
                });
            });

        if should_close {
            self.close();
        }

        should_close
    }
}
