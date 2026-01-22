//! Workspace creation overlay component.
//!
//! A wizard for creating new workspaces, guiding users through configuration.
//! On native: three steps (name, endpoint, git repo).
//! On WASM: single step (endpoint only, since no filesystem/git access).
//! Styled similarly to the Tutorial overlay with a frosted glass appearance.

use egui::{Key, RichText};

use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, render_key_badge_large};

/// Default workspace name
const DEFAULT_WORKSPACE_NAME: &str = "my-workspace";

/// Default connection endpoint
const DEFAULT_ENDPOINT: &str = "http://localhost:9090";

/// Total number of steps in the wizard (native: 3, WASM: 2)
#[cfg(not(target_arch = "wasm32"))]
const TOTAL_STEPS: usize = 3;

#[cfg(target_arch = "wasm32")]
const TOTAL_STEPS: usize = 2;

/// The current step in the workspace creation wizard
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum WorkspaceCreatorStep {
    #[default]
    Name,
    Endpoint,
    #[cfg(not(target_arch = "wasm32"))]
    GitRepo,
}

/// Result from the workspace creator overlay
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceCreatorResult {
    /// No action taken
    None,
    /// User cancelled the creation
    Cancelled,
    /// User completed the creation with these values
    Created {
        name: String,
        endpoint: String,
        git_repo: Option<String>,
    },
}

/// Wizard overlay for creating new workspaces.
/// On native: three steps (name, endpoint, git repo).
/// On WASM: two steps (name, endpoint).
pub struct WorkspaceCreator {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip input on the first frame after opening
    just_opened: bool,
    /// Current theme
    theme: AppTheme,
    /// Current step in the wizard
    step: WorkspaceCreatorStep,
    /// Workspace name input
    name: String,
    /// Connection endpoint input
    endpoint: String,
    /// Git repository path input (native only)
    #[cfg(not(target_arch = "wasm32"))]
    git_repo: String,
}

impl Default for WorkspaceCreator {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceCreator {
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            step: WorkspaceCreatorStep::Name,
            name: DEFAULT_WORKSPACE_NAME.to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            #[cfg(not(target_arch = "wasm32"))]
            git_repo: String::new(),
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay and reset to first step
    pub fn open(&mut self) {
        self.is_open = true;
        self.just_opened = true;
        self.step = WorkspaceCreatorStep::Name;
        self.name = DEFAULT_WORKSPACE_NAME.to_string();
        self.endpoint = DEFAULT_ENDPOINT.to_string();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.git_repo = String::new();
        }
    }

    /// Close the overlay
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if the overlay is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Go to the next step, or complete if on the last step
    fn next_step(&mut self) -> Option<WorkspaceCreatorResult> {
        match self.step {
            WorkspaceCreatorStep::Name => {
                self.step = WorkspaceCreatorStep::Endpoint;
                None
            }
            #[cfg(not(target_arch = "wasm32"))]
            WorkspaceCreatorStep::Endpoint => {
                self.step = WorkspaceCreatorStep::GitRepo;
                None
            }
            #[cfg(target_arch = "wasm32")]
            WorkspaceCreatorStep::Endpoint => {
                // On WASM, endpoint is the last step
                self.close();
                Some(WorkspaceCreatorResult::Created {
                    name: self.name.clone(),
                    endpoint: self.endpoint.clone(),
                    git_repo: None,
                })
            }
            #[cfg(not(target_arch = "wasm32"))]
            WorkspaceCreatorStep::GitRepo => {
                self.close();
                let git_repo = if self.git_repo.trim().is_empty() {
                    None
                } else {
                    Some(self.git_repo.clone())
                };
                Some(WorkspaceCreatorResult::Created {
                    name: self.name.clone(),
                    endpoint: self.endpoint.clone(),
                    git_repo,
                })
            }
        }
    }

    /// Show the overlay. Returns the result of the interaction.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> WorkspaceCreatorResult {
        if !self.is_open {
            return WorkspaceCreatorResult::None;
        }

        let mut result = WorkspaceCreatorResult::None;
        let mut should_close = false;
        let mut should_next = false;

        // Skip input handling on the first frame after opening
        if self.just_opened {
            self.just_opened = false;
        } else {
            // Handle keyboard input
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    should_close = true;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    should_next = true;
                }
            });
        }

        // Calculate popup dimensions - match Tutorial overlay sizing
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.5).clamp(500.0, 650.0);

        egui::Area::new(egui::Id::new("workspace_creator_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = self.theme.border_subtle();
                let muted_text = text_color(self.theme).gamma_multiply(0.6);
                let accent_color = self.theme.accent_primary();
                let key_bg = self.theme.bg_elevated();
                let tip_color = self.theme.accent_hover();
                let input_bg = self.theme.bg_elevated();

                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Determine step number and content based on current step
                    let step_number = match self.step {
                        WorkspaceCreatorStep::Name => 1,
                        WorkspaceCreatorStep::Endpoint => 2,
                        #[cfg(not(target_arch = "wasm32"))]
                        WorkspaceCreatorStep::GitRepo => 3,
                    };

                    let (title, label, hint, current_value) = match self.step {
                        WorkspaceCreatorStep::Name => (
                            "Workspace Name",
                            "Name",
                            "Choose a descriptive name for your workspace",
                            &mut self.name,
                        ),
                        WorkspaceCreatorStep::Endpoint => (
                            "Connection Endpoint",
                            "Endpoint",
                            "Prometheus compatible endpoint URL",
                            &mut self.endpoint,
                        ),
                        #[cfg(not(target_arch = "wasm32"))]
                        WorkspaceCreatorStep::GitRepo => (
                            "Git Repository",
                            "Path",
                            "Optional: path to git repo for commit annotations",
                            &mut self.git_repo,
                        ),
                    };

                    // Header section with step indicator
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new(semantic_icons::action::ADD)
                                .color(accent_color)
                                .size(28.0),
                        );
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("Step {step_number} of {TOTAL_STEPS}"))
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Create Workspace")
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

                    // Field label
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new(title)
                                .color(text_color(self.theme))
                                .size(typography::LG)
                                .strong(),
                        );
                    });
                    ui.add_space(12.0);

                    // Input field
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);

                        egui::Frame::new()
                            .fill(input_bg)
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::symmetric(12, 10))
                            .stroke(egui::Stroke::new(1.0, separator_color))
                            .show(ui, |ui| {
                                ui.set_width(popup_width - 72.0);

                                let response = ui.add(
                                    egui::TextEdit::singleline(current_value)
                                        .hint_text(label)
                                        .frame(false)
                                        .font(typography::proportional(typography::LG))
                                        .text_color(text_color(self.theme))
                                        .desired_width(popup_width - 96.0),
                                );

                                // Request focus on the text field
                                response.request_focus();
                            });
                    });
                    ui.add_space(20.0);

                    // Tip section
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new(semantic_icons::diagnostic::HINT)
                                .color(tip_color)
                                .size(typography::MD),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(hint)
                                .color(tip_color)
                                .size(typography::MD)
                                .italics(),
                        );
                    });
                    ui.add_space(24.0);

                    // Progress dots
                    ui.horizontal(|ui| {
                        let dot_spacing = 8.0;
                        let dot_size = 6.0;
                        let num_dots = TOTAL_STEPS;
                        let total_width =
                            (num_dots as f32) * dot_size + ((num_dots - 1) as f32) * dot_spacing;
                        ui.add_space((ui.available_width() - total_width) / 2.0);

                        for i in 0..num_dots {
                            let is_current = i == step_number - 1;
                            let color = if is_current {
                                accent_color
                            } else {
                                muted_text.gamma_multiply(0.4)
                            };

                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(dot_size, dot_size),
                                egui::Sense::hover(),
                            );
                            ui.painter()
                                .circle_filled(rect.center(), dot_size / 2.0, color);

                            if i < num_dots - 1 {
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

                        // Next/Create/Connect - depends on step and platform
                        let action_label = match self.step {
                            WorkspaceCreatorStep::Name => "next",
                            #[cfg(not(target_arch = "wasm32"))]
                            WorkspaceCreatorStep::Endpoint => "next",
                            #[cfg(target_arch = "wasm32")]
                            WorkspaceCreatorStep::Endpoint => "connect",
                            #[cfg(not(target_arch = "wasm32"))]
                            WorkspaceCreatorStep::GitRepo => "create",
                        };
                        render_key_badge_large(ui, "Enter", key_bg, text_color(self.theme));
                        ui.label(
                            RichText::new(format!(" {action_label}"))
                                .color(muted_text)
                                .size(typography::SM),
                        );

                        // Push cancel to the right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(20.0);

                            ui.label(
                                RichText::new("cancel ")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                            render_key_badge_large(ui, "Esc", key_bg, text_color(self.theme));
                        });
                    });
                    ui.add_space(12.0);
                });
            });

        // Handle state changes after rendering
        if should_close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
            result = WorkspaceCreatorResult::Cancelled;
        } else if should_next {
            if let Some(r) = self.next_step() {
                result = r;
            }
        }

        result
    }
}
